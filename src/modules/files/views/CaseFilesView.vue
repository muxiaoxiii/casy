<script setup>
import { ref, onMounted, computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useCasesStore } from '../../../stores/cases'
import { casyContext } from '../../../core/plugin/context'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  ArrowLeft, Folder, Message, Paperclip, Upload, Download, Document,
  ChatLineRound, Files, Refresh, Search, FolderOpened, MagicStick
} from '@element-plus/icons-vue'
import ReasoningSearchPanel from '../components/ReasoningSearchPanel.vue'

const route = useRoute()
const router = useRouter()
const casesStore = useCasesStore()

const caseId = ref(route.params.caseId)
const caseData = ref(null)
const loading = ref(false)
const activeCategory = ref('all')
const fileSearch = ref('')
const selectedFile = ref(null)
const files = ref([])
const filesLoading = ref(false)
const uploading = ref(false)

const showReasoningPanel = ref(false)
const allFileIds = computed(() => files.value.map(f => f.id))

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
  categories.find(category => category.key === activeCategory.value) || categories[0]
)

async function loadCase() {
  if (!caseId.value) return
  loading.value = true
  const result = await casesStore.loadCase(caseId.value)
  if (result.ok) caseData.value = result.data
  loading.value = false
}

async function loadFiles() {
  if (!caseId.value) return
  filesLoading.value = true
  const result = await casyContext.files.list(caseId.value)
  if (result.ok) {
    files.value = Array.isArray(result.data) ? result.data : []
    const retained = files.value.find(file => file.id === selectedFile.value?.id)
    selectedFile.value = retained || filteredFiles.value[0] || null
  }
  filesLoading.value = false
}

async function uploadFile() {
  const { open } = await import('@tauri-apps/plugin-dialog')
  const selected = await open({ multiple: true })
  if (!selected) return

  uploading.value = true
  const paths = Array.isArray(selected) ? selected : [selected]
  let failed = 0
  for (const filePath of paths) {
    const result = await casyContext.files.add(
      caseId.value,
      filePath,
      activeCategory.value === 'all' ? 'other' : activeCategory.value
    )
    if (!result.ok) failed += 1
  }
  uploading.value = false
  if (failed) ElMessage.error(`${failed} 个文件添加失败`)
  else ElMessage.success(paths.length > 1 ? `已添加 ${paths.length} 个文件` : '文件已添加')
  await loadFiles()
}

async function deleteFile(file) {
  try {
    await ElMessageBox.confirm(
      `确定移除文件「${file.fileName}」的登记？`,
      '移除文件',
      { type: 'warning', confirmButtonText: '移除登记', cancelButtonText: '取消' }
    )
  } catch {
    return
  }
  const result = await casyContext.files.remove(file.id)
  if (result.ok) {
    selectedFile.value = null
    ElMessage.success('已移除登记')
    await loadFiles()
  } else ElMessage.error(result.error || '移除失败')
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
  if (!bytes) return '未知大小'
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
  return files.value.filter(file => {
    const matchesCategory = activeCategory.value === 'all' || file.category === activeCategory.value
    const matchesSearch = !query || file.fileName?.toLowerCase().includes(query)
    return matchesCategory && matchesSearch
  })
})

function categoryLabel(key) {
  return categories.find(category => category.key === key)?.label || key || '未分类'
}

function onCategoryChange(key) {
  activeCategory.value = key
  selectedFile.value = filteredFiles.value[0] || null
}

onMounted(() => {
  loadCase()
  loadFiles()
})
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
        <el-button @click="loadFiles" circle>
          <el-icon><Refresh /></el-icon>
        </el-button>
        
        <el-button type="primary" plain @click="showReasoningPanel = true" class="deep-search-btn">
          <el-icon><MagicStick /></el-icon> 深度推理检索
        </el-button>

        <el-button type="primary" @click="uploadFile" :loading="uploading">
          <el-icon><Upload /></el-icon>
          上传文件
        </el-button>
      </div>
    </header>

    <div class="files-workbench">
      <aside class="folder-panel">
        <div class="panel-heading">
          <div>
            <span class="panel-kicker">卷宗目录</span>
            <strong>{{ files.length }} 个文件</strong>
          </div>
        </div>

        <nav class="folder-list" aria-label="文件分类">
          <button
            v-for="category in categories"
            :key="category.key"
            type="button"
            :class="['folder-item', { active: activeCategory === category.key }]"
            @click="onCategoryChange(category.key)"
          >
            <el-icon><component :is="category.icon" /></el-icon>
            <span>{{ category.label }}</span>
            <small>{{ categoryCounts[category.key] || 0 }}</small>
          </button>
        </nav>

        <div class="folder-note">
          <span class="status-dot" />
          文件登记与案件保持关联
        </div>
      </aside>

      <main class="file-index">
        <div class="index-heading">
          <div>
            <span class="panel-kicker">{{ currentCategory.label }}</span>
            <strong>{{ filteredFiles.length }} 项</strong>
          </div>
          <el-button text :icon="Refresh" :loading="filesLoading" title="刷新" @click="loadFiles" />
        </div>

        <div v-if="filteredFiles.length" class="file-list" v-loading="filesLoading">
          <button
            v-for="file in filteredFiles"
            :key="file.id"
            type="button"
            :class="['file-row', { selected: selectedFile?.id === file.id }]"
            @click="selectedFile = file"
            @dblclick="openFile(file)"
          >
            <span :class="['file-mark', fileTone(file)]">{{ fileExtension(file).slice(0, 4) }}</span>
            <span class="file-copy">
              <strong>{{ file.fileName }}</strong>
              <small>{{ categoryLabel(file.category) }} · {{ formatDate(file.createdAt) }}</small>
            </span>
            <span class="file-size">{{ formatSize(file.fileSize) }}</span>
          </button>
        </div>

        <div v-else class="file-empty">
          <div class="empty-icon"><el-icon><Document /></el-icon></div>
          <strong>{{ fileSearch ? '没有匹配的文件' : '这个目录还是空的' }}</strong>
          <span>{{ fileSearch ? '换一个关键词试试' : '添加文件后会在这里形成可追溯的卷宗索引' }}</span>
        </div>
      </main>

      <aside class="file-inspector">
        <template v-if="selectedFile">
          <div class="inspector-preview">
            <span :class="['preview-mark', fileTone(selectedFile)]">{{ fileExtension(selectedFile).slice(0, 4) }}</span>
            <strong>{{ selectedFile.fileName }}</strong>
            <span>{{ formatSize(selectedFile.fileSize) }}</span>
          </div>

          <div class="inspector-section">
            <span class="panel-kicker">文件信息</span>
            <dl>
              <div><dt>分类</dt><dd>{{ categoryLabel(selectedFile.category) }}</dd></div>
              <div><dt>类型</dt><dd>{{ fileExtension(selectedFile) }}</dd></div>
              <div><dt>登记时间</dt><dd>{{ formatDate(selectedFile.createdAt) }}</dd></div>
            </dl>
          </div>

          <div class="inspector-section source-section">
            <span class="panel-kicker">来源路径</span>
            <p>{{ selectedFile.filePath }}</p>
          </div>

          <div class="inspector-actions">
            <el-button type="primary" @click="openFile(selectedFile)">打开文件</el-button>
            <el-button text :icon="FolderOpened" @click="revealFile(selectedFile)">在访达中显示</el-button>
            <el-button text type="danger" @click="deleteFile(selectedFile)">移除登记</el-button>
          </div>
        </template>

        <div v-else class="inspector-empty">
          <div class="empty-icon"><el-icon><Files /></el-icon></div>
          <strong>选择一个文件</strong>
          <span>在右侧查看来源、类型与登记信息</span>
        </div>
      </aside>
    </div>
    
    <ReasoningSearchPanel 
      v-model="showReasoningPanel" 
      :caseId="caseId" 
      :fileIds="allFileIds" 
    />
  </div>
</template>

<style scoped>
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
  font-weight: 650; letter-spacing: -0.02em; text-overflow: ellipsis; white-space: nowrap;
}
.workspace-eyebrow, .panel-kicker {
  display: block; color: var(--c-text-secondary); font-size: 10px; font-weight: 650;
  letter-spacing: 0.08em; text-transform: uppercase;
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
  display: grid; grid-template-columns: 38px minmax(0, 1fr) auto; width: 100%;
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
.inspector-section dl div { display: grid; grid-template-columns: 76px 1fr; gap: 8px; padding: 6px 0; }
.inspector-section dt { color: var(--c-text-secondary); font-size: 10.5px; }
.inspector-section dd { margin: 0; color: var(--c-text-regular); font-size: 11px; text-align: right; }
.source-section p {
  margin: 9px 0 0; color: var(--c-text-secondary); font-family: var(--font-mono);
  font-size: 10px; line-height: 1.55; overflow-wrap: anywhere;
}
.inspector-actions { display: flex; flex-direction: column; align-items: stretch; gap: 3px; padding: 16px; }

@media (max-width: 1120px) {
  .files-workbench { grid-template-columns: 190px minmax(330px, 1fr); }
  .file-inspector { display: none; }
}
@media (max-width: 760px) {
  .case-files-view { padding: 18px 14px 24px; }
  .workspace-header { align-items: flex-start; flex-direction: column; }
  .workspace-actions { width: 100%; }
  .file-search { flex: 1; width: auto; }
  .files-workbench { display: block; min-height: auto; }
  .folder-panel { border-right: 0; border-bottom: 1px solid var(--c-border); }
  .folder-list { display: flex; overflow-x: auto; }
  .folder-item { flex: 0 0 auto; width: auto; }
  .folder-note { display: none; }
  .file-index { min-height: 460px; }
}
</style>
