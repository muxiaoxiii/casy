<script setup>
import { ref, onMounted, onUnmounted, computed, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useCasesStore } from '../../../stores/cases'
import { casyContext } from '../../../core/plugin/context'
import { tauriCall, tauriCallSafe } from '../../../core/tauriBridge'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  ArrowLeft, Folder, Message, Paperclip, Upload, Download, Document,
  ChatLineRound, Files, Refresh, Search, FolderOpened, MagicStick, Close, EditPen, FolderAdd, RefreshLeft
} from '@element-plus/icons-vue'
import ReasoningSearchPanel from '../components/ReasoningSearchPanel.vue'
import BacklinksPanel from '../../knowledge/components/BacklinksPanel.vue'

const route = useRoute()
const router = useRouter()
const casesStore = useCasesStore()

const caseId = computed(() => String(route.params.caseId || ''))
const caseData = ref(null)
const loading = ref(false)
const activeCategory = ref('all')
const fileSearch = ref('')
const selectedFile = ref(null)
const fileInspector = ref(null)
const files = ref([])
const filesLoading = ref(false)
const uploading = ref(false)
const mutating = ref(false)
const loadError = ref('')
const directories = ref([])
const selectedDir = ref('__all__')
const removedFiles = ref([])
const showRemoved = ref(false)
const selectedIds = ref([])
const moveOpen = ref(false)
const moveIds = ref([])
const moveDir = ref('')
let loadRevision = 0
let disposed = false

function selectFile(file) {
  selectedFile.value = file
  if (window.matchMedia('(max-width: 1120px)').matches) {
    requestAnimationFrame(() => fileInspector.value?.scrollIntoView({ behavior: 'smooth', block: 'start' }))
  }
}

const showReasoningPanel = ref(false)

// W5 · OCR 状态（徽标 / 立即识别 / 查看文本）
const ocrStates = ref({}) // fileId -> { status, hasText }
const ocrBusy = ref({}) // fileId -> boolean
const ocrTextDialog = ref(false)
const ocrTextContent = ref('')
const ocrTextTitle = ref('')
const documentJobs = ref({})
const documentEngine = ref(null)
let documentPollTimer = null

const OCR_BADGES = {
  pending: { label: '待识别', cls: 'pending', tip: '等待 OCR 识别' },
  processing: { label: '识别中', cls: 'processing', tip: 'OCR 识别进行中' },
  completed: { label: '已完成', cls: 'completed', tip: 'OCR 已完成，可在详情中查看文本' },
  failed: {
    label: '失败',
    cls: 'failed',
    tip: '文档识别失败，可在详情中查看原因并重试',
  },
}

function isOcrCandidateFile(file) {
  const ext = String(file?.fileType || file?.fileName?.split('.').pop() || '')
    .replace('.', '').toLowerCase()
  return ['pdf', 'png', 'jpg', 'jpeg', 'tif', 'tiff', 'bmp', 'webp', 'gif', 'md', 'markdown', 'txt', 'doc', 'docx', 'docm', 'rtf', 'odt'].includes(ext)
}

function isTextDocument(file) {
  return /\.(md|markdown|txt|doc|docx|docm|rtf|odt)$/i.test(file?.fileName || '')
}

function ocrBadge(file) {
  if (!isOcrCandidateFile(file)) return null
  const job = documentJobs.value[file.id]
  if (job) {
    if (job.status === 'queued') return { ...OCR_BADGES.pending, label: '已排队', tip: '等待本地文档引擎处理' }
    if (job.status === 'running') return { ...OCR_BADGES.processing, label: `${Math.round((job.progress || 0) * 100)}%`, tip: job.totalPages ? `已识别 ${job.currentPage} / ${job.totalPages} 页` : '正在准备文档识别' }
    if (job.status === 'completed') return { ...OCR_BADGES.completed, label: '可搜索', tip: job.searchablePdfPath ? '已生成可搜索 PDF 和文字备份' : '正文索引及 Markdown 备份已就绪' }
    if (job.status === 'failed') return { ...OCR_BADGES.failed, tip: job.errorMessage || '文档处理失败' }
    if (job.status === 'cancelled') return { ...OCR_BADGES.pending, label: '已取消', tip: '任务已取消，可重新处理' }
  }
  const state = ocrStates.value[file.id]
  return OCR_BADGES[state?.status || 'pending']
}

async function loadDocumentJobs() {
  const target = caseId.value
  const jobs = await tauriCall('list_case_document_jobs', { caseId: target }, { silent: true })
  if (disposed || target !== caseId.value || !Array.isArray(jobs)) return
  documentJobs.value = Object.fromEntries(jobs.map(job => [job.fileId, job]))
}

async function loadDocumentEngine() {
  documentEngine.value = await tauriCall('get_document_engine_status', {}, { silent: true })
}

async function loadOcrStates() {
  if (!caseId.value) return
  const target = caseId.value
  const data = await tauriCall('list_case_ocr_states', { caseId: target }, { silent: true })
  if (disposed || target !== caseId.value || !Array.isArray(data)) return
  const map = {}
  for (const item of data) map[item.fileId] = { status: item.ocrStatus, hasText: item.hasText }
  ocrStates.value = map
}

async function ocrNow(file) {
  ocrBusy.value = { ...ocrBusy.value, [file.id]: true }
  try {
    const result = await tauriCallSafe('queue_document_processing', { fileId: file.id })
    if (!result.ok) ElMessage.error(result.error || '无法创建文档处理任务')
    else ElMessage.success(result.data.status === 'completed' ? '已复用识别结果' : '已加入本地文档处理队列')
    await loadDocumentJobs()
    await loadOcrStates()
  } finally {
    ocrBusy.value = { ...ocrBusy.value, [file.id]: false }
  }
}

async function openSearchablePdf(file) {
  const path = documentJobs.value[file.id]?.searchablePdfPath
  if (!path) return
  const result = await casyContext.files.openDefault(path)
  if (!result.ok) ElMessage.error(result.error || '无法打开可搜索 PDF')
}

async function retryDocument(file) {
  const job = documentJobs.value[file.id]
  if (!job) return ocrNow(file)
  const result = await tauriCallSafe('retry_document_job', { jobId: job.id })
  if (!result.ok) ElMessage.error(result.error || '重试失败')
  else ElMessage.success('已重新加入处理队列')
  await loadDocumentJobs()
  await loadOcrStates()
}

async function cancelDocument(file) {
  const job = documentJobs.value[file.id]
  if (!job) return
  const result = await tauriCallSafe('cancel_document_job', { jobId: job.id })
  if (!result.ok) ElMessage.error(result.error || '取消失败')
  await loadDocumentJobs()
  await loadOcrStates()
}

async function viewOcrText(file) {
  const text = await tauriCall('get_file_ocr_text', { fileId: file.id })
  if (text === null) return
  ocrTextContent.value = text || '（该文件暂无 OCR 文本）'
  ocrTextTitle.value = file.fileName
  ocrTextDialog.value = true
}

const categories = [
  { key: 'all', label: '全部文件', icon: Folder },
  { key: 'summons', label: '传票 / 通知书', icon: Message },
  { key: 'evidence', label: '证据材料', icon: Paperclip },
  { key: 'submitted', label: '提交文件', icon: Upload },
  { key: 'received', label: '接收文件', icon: Download },
  { key: 'internal', label: '内部文件', icon: Document },
  { key: 'correspondence', label: '往来函件', icon: ChatLineRound },
  { key: 'other', label: '其他', icon: Files },
]

const currentCategory = computed(() =>
  showRemoved.value ? { label: '已移除登记' } : categories.find(category => category.key === activeCategory.value) || categories[0]
)

async function loadCase() {
  if (!caseId.value) return
  loading.value = true
  const target = caseId.value
  const result = await casesStore.loadCase(target)
  if (disposed || target !== caseId.value) return
  if (result.ok) caseData.value = result.data
  loading.value = false
}

async function loadFiles() {
  if (!caseId.value) return
  const target = caseId.value
  const revision = ++loadRevision
  filesLoading.value = true
  const [result, removed, dirs] = await Promise.all([
    casyContext.files.list(target), casyContext.files.removed(target), casyContext.files.listCaseDirs(target),
  ])
  if (disposed || target !== caseId.value || revision !== loadRevision) return
  loadError.value = [result, removed, dirs].filter(item => !item.ok).map(item => item.error || '读取卷宗失败').join('；')
  if (removed.ok) removedFiles.value = removed.data || []
  if (dirs.ok) directories.value = dirs.data || []
  if (result.ok) {
    files.value = Array.isArray(result.data) ? result.data : []
    selectedIds.value = selectedIds.value.filter(id => files.value.some(file => file.id === id))
    // W4: 消费证据链接跳转约定 ?select=<fileId>&anchor=page:N（消费后即清除，避免粘性污染后续刷新）
    const wantId = route.query.select
    if (wantId) {
      const fromQuery = files.value.find(file => file.id === wantId)
      if (fromQuery) {
        showRemoved.value = false
        activeCategory.value = 'all'
        selectedDir.value = '__all__'
        selectedFile.value = fromQuery
        if (route.query.anchor) {
          ElMessage.info(`已定位到证据链接出处（${route.query.anchor}）`)
        }
      } else {
        ElMessage.warning('证据链接指向的文件不在本案卷宗中（可能已移除）')
      }
      router.replace({ query: {} })
    } else {
      const retained = filteredFiles.value.find(file => file.id === selectedFile.value?.id)
      selectedFile.value = retained || filteredFiles.value[0] || null
    }
    loadOcrStates()
    loadDocumentJobs()
  }
  filesLoading.value = false
}

async function uploadFile() {
  if (uploading.value || mutating.value) return
  const target = caseId.value
  const dir = selectedDir.value === '__all__' ? null : selectedDir.value
  const category = activeCategory.value === 'all' ? 'other' : activeCategory.value
  const { open } = await import('@tauri-apps/plugin-dialog')
  const selected = await open({ multiple: true })
  if (!selected) return

  uploading.value = true
  const paths = Array.isArray(selected) ? selected : [selected]
  const result = await casyContext.files.importToCase(target, dir, paths, category)
  uploading.value = false
  if (!result.ok) ElMessage.error(result.error || '文件添加失败')
  else ElMessage.success(`已登记 ${result.data?.length ?? 0} 个文件`)
  await loadFiles()
}

async function deleteFile(file) {
  if (mutating.value) return
  try {
    await ElMessageBox.confirm(
      `移除「${file.fileName}」的登记？磁盘文件保留，可从已移除列表恢复。`,
      '移除文件',
      { type: 'warning', confirmButtonText: '移除登记', cancelButtonText: '取消' }
    )
  } catch { /* 用户取消：属预期 */ return }
  mutating.value = true
  const result = await casyContext.files.remove(file.id)
  mutating.value = false
  if (result.ok) {
    selectedFile.value = null
    ElMessage.success('已移除登记')
    await loadFiles()
  } else ElMessage.error(result.error || '移除失败')
}

async function restoreFile(file) {
  if (mutating.value) return
  mutating.value = true
  const result = await casyContext.files.restore(file.id)
  mutating.value = false
  if (!result.ok) return ElMessage.error(result.error || '恢复失败')
  showRemoved.value = false
  activeCategory.value = 'all'
  selectedDir.value = '__all__'
  selectedFile.value = file
  await loadFiles()
  ElMessage.success('文件登记已恢复')
}

async function renameFile(file) {
  if (mutating.value) return
  const target = caseId.value
  let value
  try { ({ value } = await ElMessageBox.prompt('文件名', '重命名文件', { inputValue: file.fileName, confirmButtonText: '重命名', cancelButtonText: '取消', inputValidator: value => !!value?.trim() || '请输入文件名' })) }
  catch { return }
  if (disposed || target !== caseId.value) return
  mutating.value = true
  const result = await casyContext.files.applyRenames(target, [{ id: file.id, newName: value }])
  mutating.value = false
  if (!result.ok) return ElMessage.error(result.error || '重命名失败')
  const warning = result.data?.find(item => item.warning)?.warning
  if (warning) ElMessage.warning(warning)
  else ElMessage.success('文件已重命名')
  await loadFiles()
}

function openMove(ids) {
  moveIds.value = [...ids]
  moveDir.value = selectedDir.value === '__all__' ? '' : selectedDir.value
  moveOpen.value = true
}

async function moveFiles() {
  if (mutating.value) return
  mutating.value = true
  const result = await casyContext.files.move(caseId.value, moveIds.value, moveDir.value || null)
  mutating.value = false
  if (!result.ok) return ElMessage.error(result.error || '移动失败')
  const warning = result.data?.find(item => item.warning)?.warning
  if (warning) ElMessage.warning(warning)
  else ElMessage.success(`已移动 ${result.data?.length || 0} 个文件`)
  selectedDir.value = moveDir.value
  selectedIds.value = []
  moveOpen.value = false
  await loadFiles()
}

async function createDirectory() {
  if (mutating.value) return
  const target = caseId.value
  const parent = selectedDir.value === '__all__' ? '' : selectedDir.value
  let value
  try { ({ value } = await ElMessageBox.prompt('文件夹名称', '新建文件夹', { confirmButtonText: '新建', cancelButtonText: '取消' })) }
  catch { return }
  if (disposed || target !== caseId.value) return
  mutating.value = true
  const result = await casyContext.files.createSubdir(target, parent || null, value)
  mutating.value = false
  if (!result.ok) return ElMessage.error(result.error || '新建文件夹失败')
  await loadFiles()
  if (!disposed && target === caseId.value) selectedDir.value = [parent, value.trim()].filter(Boolean).join('/')
}

async function changeCategory(file, value) {
  if (mutating.value) return
  mutating.value = true
  const result = await casyContext.files.setCategory(file.id, value)
  mutating.value = false
  if (!result.ok) return ElMessage.error(result.error || '修改分类失败')
  await loadFiles()
}

function toggleSelection(id, checked) {
  selectedIds.value = checked ? [...new Set([...selectedIds.value, id])] : selectedIds.value.filter(value => value !== id)
}

function showRemovedFiles() {
  showRemoved.value = true
  selectedIds.value = []
  selectedFile.value = filteredFiles.value[0] || null
}

async function openFile(file) {
  const result = await casyContext.files.openDefault(file.filePath)
  if (!result.ok) ElMessage.error(result.error || '打开失败')
}

async function revealFile(file) {
  const result = await casyContext.files.reveal(file.filePath)
  if (!result.ok) ElMessage.error(result.error || '定位失败')
}

function formatSize(bytes) {
  if (bytes === null || bytes === undefined) return '未知大小'
  if (bytes < 1024) return bytes + ' B'
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB'
  return (bytes / (1024 * 1024)).toFixed(1) + ' MB'
}

function formatDate(value) {
  if (!value) return '未记录'
  return String(value).replace('T', ' ').slice(0, 16)
}

function fileExtension(file) {
  const source = file?.fileType || file?.fileName?.split('.').pop() || ''
  return String(source).replace('.', '').toUpperCase() || 'FILE'
}

function fileTone(file) {
  const extension = fileExtension(file).toLowerCase()
  if (extension === 'pdf') return 'pdf'
  if (['png', 'jpg', 'jpeg', 'gif', 'webp'].includes(extension)) return 'image'
  if (['doc', 'docx', 'wps', 'rtf'].includes(extension)) return 'document'
  if (['zip', 'rar', '7z'].includes(extension)) return 'archive'
  return 'other'
}

const categoryCounts = computed(() => {
  const counts = { all: files.value.length }
  for (const category of categories.slice(1)) {
    counts[category.key] = files.value.filter(file => file.category === category.key).length
  }
  return counts
})

const filteredFiles = computed(() => {
  const query = fileSearch.value.trim().toLowerCase()
  const directory = directories.value.find(dir => dir.relPath === selectedDir.value)?.absolutePath?.replaceAll('\\', '/')
  return (showRemoved.value ? removedFiles.value : files.value).filter(file => {
    const matchesCategory = showRemoved.value || activeCategory.value === 'all' || file.category === activeCategory.value
    const normalized = file.filePath.replaceAll('\\', '/')
    const matchesDir = showRemoved.value || selectedDir.value === '__all__' || normalized.slice(0, normalized.lastIndexOf('/')) === directory
    const matchesSearch = !query || file.fileName?.toLowerCase().includes(query)
    return matchesCategory && matchesSearch && matchesDir
  })
})

function categoryLabel(key) {
  return categories.find(category => category.key === key)?.label || key || '未分类'
}

function onCategoryChange(key) {
  showRemoved.value = false
  selectedIds.value = []
  activeCategory.value = key
  selectedFile.value = filteredFiles.value[0] || null
}

watch(selectedDir, () => { selectedIds.value = []; selectedFile.value = filteredFiles.value[0] || null })
watch(() => route.query.select, id => { if (id) loadFiles() })
watch(caseId, () => {
  files.value = []; removedFiles.value = []; directories.value = []; selectedIds.value = []
  selectedFile.value = null; caseData.value = null; documentJobs.value = {}; ocrStates.value = {}
  selectedDir.value = '__all__'; activeCategory.value = 'all'; showRemoved.value = false; moveOpen.value = false
  loadCase(); loadFiles()
}, { immediate: true })

onMounted(() => {
  loadDocumentEngine()
  let polling = false
  documentPollTimer = window.setInterval(async () => {
    if (polling) return
    polling = true
    try {
      const before = JSON.stringify(documentJobs.value)
      await loadDocumentJobs()
      await loadOcrStates()
      if (before !== JSON.stringify(documentJobs.value)) await loadFiles()
    } finally {
      polling = false
    }
  }, 3000)
})
onUnmounted(() => { disposed = true; ++loadRevision; if (documentPollTimer) window.clearInterval(documentPollTimer) })
</script>

<template>
  <div class="case-files-view" v-loading="loading">
    <header class="workspace-header">
      <div class="workspace-identity">
        <button class="back-button" type="button" aria-label="返回案件" @click="router.back()">
          <el-icon><ArrowLeft /></el-icon>
        </button>
        <div>
          <div class="workspace-eyebrow">案件卷宗</div>
          <div class="workspace-title-row">
            <h1>{{ caseData?.caseName || '案件文件' }}</h1>
            <span v-if="caseData?.caseNo" class="case-number">{{ caseData.caseNo }}</span>
          </div>
        </div>
      </div>

      <div class="workspace-actions">
        <el-input
          v-model="fileSearch"
          class="file-search"
          :prefix-icon="Search"
          clearable
          placeholder="搜索当前卷宗"
        />
        <el-button @click="loadFiles" circle title="刷新文件" aria-label="刷新文件">
          <el-icon><Refresh /></el-icon>
        </el-button>
        
        <el-button type="primary" plain @click="showReasoningPanel = true" class="deep-search-btn" :disabled="showRemoved">
          <el-icon><Search /></el-icon> 卷宗检索
        </el-button>

        <el-button type="primary" @click="uploadFile" :loading="uploading" :disabled="mutating || showRemoved">
          <el-icon><Upload /></el-icon>
          上传文件
        </el-button>
      </div>
    </header>
    <el-alert v-if="loadError" :title="loadError" type="error" :closable="false" class="files-load-error" />

    <div class="files-workbench">
      <aside class="folder-panel">
        <div class="panel-heading">
          <div>
            <span class="panel-kicker">卷宗目录</span>
            <strong>{{ files.length }} 个文件</strong>
          </div>
        </div>

        <div class="directory-picker">
          <el-select v-model="selectedDir" aria-label="卷宗文件夹" :disabled="showRemoved" filterable>
            <el-option label="所有文件夹" value="__all__" />
            <el-option v-for="dir in directories" :key="dir.relPath" :label="dir.name" :value="dir.relPath" />
          </el-select>
          <el-button :icon="FolderAdd" :disabled="mutating || showRemoved" title="新建文件夹" aria-label="新建文件夹" @click="createDirectory" />
        </div>

        <nav class="folder-list" aria-label="文件分类">
          <button
            v-for="category in categories"
            :key="category.key"
            type="button"
            :class="['folder-item', { active: !showRemoved && activeCategory === category.key }]"
            @click="onCategoryChange(category.key)"
          >
            <el-icon><component :is="category.icon" /></el-icon>
            <span>{{ category.label }}</span>
            <small>{{ categoryCounts[category.key] || 0 }}</small>
          </button>
          <button type="button" :class="['folder-item', { active: showRemoved }]" @click="showRemovedFiles">
            <el-icon><RefreshLeft /></el-icon><span>已移除登记</span><small>{{ removedFiles.length }}</small>
          </button>
        </nav>
      </aside>

      <main class="file-index">
        <div class="index-heading">
          <div>
            <span class="panel-kicker">{{ currentCategory.label }}</span>
            <strong>{{ filteredFiles.length }} 项</strong>
          </div>
          <el-button text :icon="Refresh" :loading="filesLoading" title="刷新" @click="loadFiles" />
        </div>
        <div v-if="!showRemoved && filteredFiles.length" class="selection-actions">
          <el-checkbox :model-value="filteredFiles.every(file => selectedIds.includes(file.id))" :indeterminate="selectedIds.length > 0 && !filteredFiles.every(file => selectedIds.includes(file.id))" aria-label="选择全部文件" @change="checked => selectedIds = checked ? filteredFiles.map(file => file.id) : []" />
          <span>{{ selectedIds.length ? `已选择 ${selectedIds.length} 项` : `${filteredFiles.length} 项` }}</span>
          <el-button v-if="selectedIds.length" :icon="FolderOpened" size="small" :disabled="mutating" @click="openMove(selectedIds)">移动</el-button>
        </div>

        <div v-if="filteredFiles.length" class="file-list" v-loading="filesLoading">
          <div
            v-for="file in filteredFiles"
            :key="file.id"
            role="button"
            tabindex="0"
            :class="['file-row', { selected: selectedFile?.id === file.id }]"
            @click="selectFile(file)"
            @dblclick="openFile(file)"
            @keydown.enter.self="selectFile(file)"
          >
            <el-checkbox v-if="!showRemoved" :model-value="selectedIds.includes(file.id)" :aria-label="`选择 ${file.fileName}`" @click.stop @change="checked => toggleSelection(file.id, checked)" />
            <span v-else class="removed-mark"><RefreshLeft /></span>
            <span :class="['file-mark', fileTone(file)]">{{ fileExtension(file).slice(0, 4) }}</span>
            <span class="file-copy">
              <strong>
                {{ file.fileName }}
                <span
                  v-if="!showRemoved && ocrBadge(file)"
                  :class="['ocr-badge', ocrBadge(file).cls]"
                  :title="ocrBadge(file).tip"
                >{{ ocrBadge(file).label }}</span>
              </strong>
              <small>{{ categoryLabel(file.category) }} · {{ formatDate(file.createdAt) }}</small>
            </span>
            <span class="file-size">{{ formatSize(file.fileSize) }}</span>
          </div>
        </div>

        <div v-else class="file-empty">
          <div class="empty-icon"><el-icon><Document /></el-icon></div>
          <strong>{{ fileSearch ? '没有匹配的文件' : '这个目录还是空的' }}</strong>
        </div>
      </main>

      <aside ref="fileInspector" class="file-inspector">
        <template v-if="selectedFile">
          <div class="inspector-preview">
            <span :class="['preview-mark', fileTone(selectedFile)]">{{ fileExtension(selectedFile).slice(0, 4) }}</span>
            <strong>{{ selectedFile.fileName }}</strong>
            <span>{{ formatSize(selectedFile.fileSize) }}</span>
          </div>

          <div class="inspector-section">
            <span class="panel-kicker">文件信息</span>
            <dl>
              <div><dt>分类</dt><dd><el-select v-if="!showRemoved" :model-value="selectedFile.category" size="small" aria-label="文件分类" :disabled="mutating" @change="value => changeCategory(selectedFile, value)"><el-option v-for="item in categories.slice(1)" :key="item.key" :label="item.label" :value="item.key" /></el-select><span v-else>{{ categoryLabel(selectedFile.category) }}</span></dd></div>
              <div><dt>类型</dt><dd>{{ fileExtension(selectedFile) }}</dd></div>
              <div><dt>登记时间</dt><dd>{{ formatDate(selectedFile.createdAt) }}</dd></div>
            </dl>
          </div>

          <div class="inspector-section source-section">
            <span class="panel-kicker">来源路径</span>
            <p>{{ selectedFile.filePath }}</p>
          </div>

          <div v-if="!showRemoved && isOcrCandidateFile(selectedFile)" class="inspector-section">
            <span class="panel-kicker">文档识别</span>
            <div class="ocr-status-row">
              <span
                v-if="ocrBadge(selectedFile)"
                :class="['ocr-badge', ocrBadge(selectedFile).cls]"
                :title="ocrBadge(selectedFile).tip"
              >{{ ocrBadge(selectedFile).label }}</span>
              <span v-if="ocrStates[selectedFile.id]?.hasText" class="ocr-has-text">已提取文本</span>
            </div>
            <p v-if="!isTextDocument(selectedFile) && documentEngine && !documentEngine.available" class="ocr-engine-warning">
              引擎未就绪：{{ documentEngine.missing?.join('、') || documentEngine.error }}
            </p>
            <p v-if="documentJobs[selectedFile.id]?.errorMessage" class="ocr-engine-error">
              {{ documentJobs[selectedFile.id].errorMessage }}
            </p>
          </div>

          <!-- W4: 跨模块双链反链（谁引用了这份文件） -->
          <div class="inspector-section">
            <BacklinksPanel target-type="file" :target-id="selectedFile.id" />
          </div>

          <div class="inspector-actions">
            <el-button v-if="showRemoved" type="primary" :icon="RefreshLeft" :loading="mutating" @click="restoreFile(selectedFile)">恢复登记</el-button>
            <template v-else>
            <el-button type="primary" @click="openFile(selectedFile)">打开文件</el-button>
            <el-button :icon="EditPen" :disabled="mutating" @click="renameFile(selectedFile)">重命名</el-button>
            <el-button :icon="FolderOpened" :disabled="mutating" @click="openMove([selectedFile.id])">移动到文件夹</el-button>
            <el-button
              v-if="isOcrCandidateFile(selectedFile)"
              :loading="!!ocrBusy[selectedFile.id]"
              :disabled="['queued', 'running'].includes(documentJobs[selectedFile.id]?.status)"
              @click="ocrNow(selectedFile)"
            >{{ isTextDocument(selectedFile) ? '提取正文并索引' : '生成可搜索 PDF' }}</el-button>
            <el-button
              v-if="['queued', 'running'].includes(documentJobs[selectedFile.id]?.status)"
              :icon="Close"
              @click="cancelDocument(selectedFile)"
            >取消处理</el-button>
            <el-button
              v-if="documentJobs[selectedFile.id]?.searchablePdfPath"
              type="success"
              plain
              @click="openSearchablePdf(selectedFile)"
            >打开可搜索 PDF</el-button>
            <el-button
              v-if="documentJobs[selectedFile.id]?.markdownPath"
              :icon="Document"
              @click="casyContext.files.openDefault(documentJobs[selectedFile.id].markdownPath)"
            >打开 Markdown 备份</el-button>
            <el-button
              v-if="['failed', 'cancelled'].includes(documentJobs[selectedFile.id]?.status)"
              type="warning"
              plain
              @click="retryDocument(selectedFile)"
            >重试</el-button>
            <el-button
              v-if="ocrStates[selectedFile.id]?.hasText"
              text
              type="primary"
              @click="viewOcrText(selectedFile)"
            >查看文本</el-button>
            <el-button text :icon="FolderOpened" @click="revealFile(selectedFile)">在访达中显示</el-button>
            <el-button text type="danger" :disabled="mutating" @click="deleteFile(selectedFile)">移除登记</el-button>
            </template>
          </div>
        </template>

        <div v-else class="inspector-empty">
          <div class="empty-icon"><el-icon><Files /></el-icon></div>
          <strong>选择一个文件</strong>
        </div>
      </aside>
    </div>
    <el-dialog v-model="moveOpen" title="移动文件" width="min(480px, calc(100vw - 32px))" :close-on-click-modal="!mutating" :show-close="!mutating">
      <el-form label-position="top"><el-form-item :label="`目标文件夹 · ${moveIds.length} 个文件`"><el-select v-model="moveDir" filterable aria-label="移动目标文件夹" style="width:100%" :disabled="mutating"><el-option v-for="dir in directories" :key="dir.relPath" :value="dir.relPath" :label="dir.name" /></el-select></el-form-item></el-form>
      <template #footer><el-button :disabled="mutating" @click="moveOpen = false">取消</el-button><el-button type="primary" :loading="mutating" @click="moveFiles">移动</el-button></template>
    </el-dialog>
    
    <el-dialog v-model="ocrTextDialog" :title="`OCR 文本 · ${ocrTextTitle}`" width="min(640px, calc(100vw - 32px))">
      <pre class="ocr-text-view">{{ ocrTextContent }}</pre>
      <template #footer>
        <el-button @click="ocrTextDialog = false">关闭</el-button>
      </template>
    </el-dialog>

    <ReasoningSearchPanel
      v-model="showReasoningPanel" 
      :caseId="caseId" 
      :files="files"
    />
  </div>
</template>

<style scoped>
.directory-picker{display:flex;gap:6px;padding:0 12px 12px;min-width:0}.directory-picker .el-select{min-width:0;flex:1}.directory-picker .el-button{margin:0;padding:8px}
.selection-actions{display:flex;gap:10px;align-items:center;padding:8px 14px;border-bottom:1px solid var(--c-border);font-size:12px;color:var(--c-text-secondary)}.selection-actions span{margin-right:auto}
.removed-mark svg{width:16px;color:var(--c-text-secondary)}.files-load-error{margin-bottom:12px}
/* Stitch v4.1.1: 目录、文件索引、详情三栏工作台 */
.case-files-view {
  width: min(1420px, 100%);
  min-height: calc(100vh - 82px);
  margin: 0 auto;
  padding: 28px 28px 32px;
  color: var(--c-text);
}

.workspace-header, .workspace-identity, .workspace-title-row, .workspace-actions,
.panel-heading, .index-heading { display: flex; align-items: center; }
.workspace-header { justify-content: space-between; gap: 24px; margin-bottom: 18px; }
.workspace-identity { gap: 12px; min-width: 0; }
.workspace-title-row { gap: 10px; min-width: 0; }
.workspace-title-row h1 {
  margin: 2px 0 0; overflow: hidden; color: var(--c-text); font-size: 20px;
  font-weight: 650; letter-spacing: 0; text-overflow: ellipsis; white-space: nowrap;
}
.workspace-eyebrow, .panel-kicker {
  display: block; color: var(--c-text-secondary); font-size: 10px; font-weight: 650;
  letter-spacing: 0; text-transform: uppercase;
}
.case-number {
  padding: 3px 7px; border: 1px solid var(--c-border); border-radius: 4px;
  color: var(--c-text-secondary); font-family: var(--font-mono); font-size: 10px;
}
.back-button {
  display: grid; width: 30px; height: 30px; padding: 0; border: 1px solid var(--c-border);
  border-radius: 50%; background: var(--c-surface); color: var(--c-text-regular); cursor: pointer; place-items: center;
}
.back-button:hover { border-color: var(--c-primary); color: var(--c-primary); }
.workspace-actions { flex-shrink: 0; gap: 8px; }
.file-search { width: 230px; }
.deep-search-btn { font-weight: 600; }
.deep-search-btn .el-icon { margin-right: 4px; font-size: 16px; }

.files-workbench {
  display: grid; grid-template-columns: 224px minmax(360px, 1fr) 316px;
  min-height: min(720px, calc(100vh - 170px)); overflow: hidden;
  border: 1px solid var(--c-border); border-radius: 8px; background: var(--c-surface);
}
.folder-panel, .file-index, .file-inspector { min-width: 0; background: var(--c-surface); }
.folder-panel, .file-index { border-right: 1px solid var(--c-border); }
.folder-panel { display: flex; flex-direction: column; }
.panel-heading, .index-heading {
  min-height: 62px; justify-content: space-between; padding: 12px 16px;
  border-bottom: 1px solid var(--c-border);
}
.panel-heading strong, .index-heading strong { display: block; margin-top: 3px; font-size: 13px; font-weight: 600; }
.folder-list { padding: 8px; }
.folder-item {
  display: grid; grid-template-columns: 18px 1fr auto; width: 100%; min-height: 38px;
  align-items: center; gap: 8px; padding: 0 10px; border: 1px solid transparent;
  border-radius: 6px; background: transparent; color: var(--c-text-regular); cursor: pointer;
  font-size: 12.5px; text-align: left;
}
.folder-item:hover { background: var(--c-bg-hover); }
.folder-item.active {
  border-color: var(--c-primary-lighter); background: var(--c-primary-light);
  color: var(--c-primary); font-weight: 600;
}
.folder-item small { color: var(--c-text-secondary); font-family: var(--font-mono); }
.folder-note {
  display: flex; align-items: center; gap: 7px; margin: auto 14px 14px; padding-top: 12px;
  border-top: 1px solid var(--c-border-light); color: var(--c-text-secondary); font-size: 10.5px;
}
.status-dot { width: 6px; height: 6px; border-radius: 50%; background: var(--c-success); }

.file-index { display: flex; flex-direction: column; }
.file-list { padding: 6px 0; }
.file-row {
  display: grid; grid-template-columns: 20px 38px minmax(0, 1fr) auto; width: 100%; box-sizing: border-box;
  align-items: center; gap: 11px; padding: 10px 16px; border: 0;
  border-left: 2px solid transparent; background: transparent; color: inherit; cursor: pointer; text-align: left;
}
.file-row + .file-row { border-top: 1px solid var(--c-border-light); }
.file-row:hover { background: var(--c-bg-hover); }
.file-row.selected { border-left-color: var(--c-primary); background: var(--c-primary-light); }
.file-mark, .preview-mark {
  display: grid; border: 1px solid var(--c-border); border-radius: 6px; background: var(--c-bg-muted);
  color: var(--c-text-secondary); font-family: var(--font-mono); font-weight: 700; place-items: center;
}
.file-mark { width: 36px; height: 42px; font-size: 9px; }
.preview-mark { width: 64px; height: 78px; font-size: 12px; }
.file-mark.pdf, .preview-mark.pdf { border-color: var(--c-danger-lighter); background: var(--c-danger-light); color: var(--c-danger); }
.file-mark.document, .preview-mark.document { border-color: var(--c-primary-lighter); background: var(--c-primary-light); color: var(--c-primary); }
.file-mark.image, .preview-mark.image { border-color: var(--c-success-lighter); background: var(--c-success-light); color: var(--c-success); }
.file-mark.archive, .preview-mark.archive { border-color: var(--c-warning-lighter); background: var(--c-warning-light); color: var(--c-warning); }
.file-copy { min-width: 0; }
.file-copy strong, .file-copy small { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.file-copy strong { color: var(--c-text); font-size: 12.5px; font-weight: 600; }
.file-copy small { margin-top: 4px; color: var(--c-text-secondary); font-size: 10.5px; }
.file-size { color: var(--c-text-secondary); font-family: var(--font-mono); font-size: 10.5px; }

.file-empty, .inspector-empty {
  display: flex; flex: 1; min-height: 260px; align-items: center; justify-content: center;
  flex-direction: column; gap: 7px; padding: 32px; color: var(--c-text-secondary); text-align: center;
}
.file-empty strong, .inspector-empty strong { color: var(--c-text-regular); font-size: 13px; }
.file-empty span, .inspector-empty span { max-width: 260px; font-size: 11px; line-height: 1.6; }
.empty-icon { display: grid; width: 38px; height: 38px; border-radius: 8px; background: var(--c-bg-muted); place-items: center; }

.file-inspector { display: flex; flex-direction: column; }
.inspector-preview {
  display: flex; min-height: 230px; align-items: center; justify-content: center; flex-direction: column;
  gap: 9px; padding: 24px; border-bottom: 1px solid var(--c-border); background: var(--c-bg-muted); text-align: center;
}
.inspector-preview strong { max-width: 250px; font-size: 13px; word-break: break-word; }
.inspector-preview > span:last-child { color: var(--c-text-secondary); font-family: var(--font-mono); font-size: 10.5px; }
.inspector-section { padding: 15px 16px; border-bottom: 1px solid var(--c-border); }
.inspector-section dl { margin: 9px 0 0; }
.inspector-section dl > div { display: grid; grid-template-columns: 76px minmax(0, 1fr); gap: 8px; padding: 6px 0; }
.inspector-section dt { color: var(--c-text-secondary); font-size: 10.5px; }
.inspector-section dd { margin: 0; color: var(--c-text-regular); font-size: 11px; text-align: right; }
.source-section p {
  margin: 9px 0 0; color: var(--c-text-secondary); font-family: var(--font-mono);
  font-size: 10px; line-height: 1.55; overflow-wrap: anywhere;
}
.inspector-actions { display: flex; flex-direction: column; align-items: stretch; gap: 3px; padding: 16px; }
.inspector-actions :deep(.el-button + .el-button) { margin-left: 0; }

/* W5 · OCR 状态徽标与文本查看 */
.ocr-badge {
  display: inline-block; margin-left: 6px; padding: 1px 6px; border: 1px solid var(--c-border);
  border-radius: 4px; font-size: 9px; font-weight: 600; line-height: 1.5;
  color: var(--c-text-secondary); background: var(--c-bg-muted); vertical-align: 1px;
}
.ocr-badge.pending { border-color: var(--c-border); color: var(--c-text-secondary); }
.ocr-badge.processing {
  border-color: var(--c-warning-lighter); background: var(--c-warning-light); color: var(--c-warning);
}
.ocr-badge.completed {
  border-color: var(--c-success-lighter); background: var(--c-success-light); color: var(--c-success);
}
.ocr-badge.failed {
  border-color: var(--c-danger-lighter); background: var(--c-danger-light); color: var(--c-danger);
}
.ocr-status-row { display: flex; align-items: center; gap: 8px; margin-top: 9px; }
.ocr-status-row .ocr-badge { margin-left: 0; }
.ocr-has-text { color: var(--c-text-secondary); font-size: 10.5px; }
.ocr-engine-warning, .ocr-engine-error { margin: 8px 0 0; font-size: 11px; line-height: 1.45; }
.ocr-engine-warning { color: var(--c-warning); }
.ocr-engine-error { color: var(--c-danger); }
.ocr-text-view {
  max-height: 420px; margin: 0; padding: 12px; overflow: auto; border: 1px solid var(--c-border);
  border-radius: 6px; background: var(--c-bg-muted); color: var(--c-text-regular);
  font-family: var(--font-mono); font-size: 11.5px; line-height: 1.7; white-space: pre-wrap;
  overflow-wrap: anywhere; user-select: text;
}

@media (max-width: 1120px) {
  .files-workbench { grid-template-columns: 190px minmax(330px, 1fr); }
  .file-inspector { grid-column: 1 / -1; border-top: 1px solid var(--c-border); scroll-margin-top: 72px; }
  .inspector-preview { min-height: 110px; padding: 16px; }
}
@media (max-width: 760px) {
  .case-files-view { padding: 18px 14px 24px; }
  .workspace-header { align-items: flex-start; flex-direction: column; }
  .workspace-actions { width: 100%; flex-wrap: wrap; }
  .file-search { flex: 1 1 calc(100% - 48px); width: auto; min-width: 0; }
  .files-workbench { display: block; min-height: auto; }
  .folder-panel { border-right: 0; border-bottom: 1px solid var(--c-border); }
  .folder-list { display: flex; overflow-x: auto; }
  .folder-item { flex: 0 0 auto; width: auto; }
  .folder-note { display: none; }
  .file-index { min-height: 160px; border-right: 0; }
}
</style>
