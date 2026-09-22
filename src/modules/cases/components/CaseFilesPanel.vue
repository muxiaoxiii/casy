<script setup lang="ts">
/**
 * CaseFilesPanel —— 案卷管理（index-v2 精装版规格实现）
 *
 * 结构：左目录树（阶段分组/计数/状态点） + 右文件列表（类型色图标/源→归档映射/hover 操作）
 * 能力：新建子文件夹 · 系统拖入文件落盘登记 · 既有卷宗扫描入库 · Finder 定位/打开
 * 数据全部经 casyContext.services 单一通路。
 */
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { Folder, Document, Picture, Box, Setting, FolderOpened, Plus, Search } from '../../../shared/icons'
import { casyContext } from '../../../core/plugin/context'
import EmptyState from '../../../shared/components/EmptyState.vue'
import FilePreviewPanel from './FilePreviewPanel.vue'
import RenameWorkbench from './RenameWorkbench.vue'
import { safeListen } from '../../../core/tauriEvents'

const props = defineProps<{ caseId: string; caseNo?: string }>()

const dirs = ref<any[]>([])
const files = ref<any[]>([]) // 已登记（DB）
const selectedRel = ref('') // 当前选中子目录相对名；'' = 根/全部
const loading = ref(false)
const dragging = ref(false)
let unlistenDrop: (() => void) | null = null
let unlistenWorkspace: (() => void) | null = null
let disposed = false
onMounted(async () => {
  const stop = safeListen('workspace:updated', () => { if (!loading.value) void load() })
  if (disposed) stop()
  else unlistenWorkspace = stop
})
onUnmounted(() => { disposed = true; unlistenWorkspace?.() })

const rootName = computed(() => props.caseNo || '案件卷宗')
const selectedDir = computed(() => dirs.value.find(d => d.relPath === selectedRel.value) || null)

/** 已登记文件按当前选中目录过滤 */
const visibleFiles = computed(() =>
  files.value.filter(f => {
    if (!selectedRel.value) return true
    return (f.filePath || '').includes(`/${selectedRel.value}/`)
  }),
)

/** 磁盘上未入库文件（针对当前目录） */
const unregistered = ref<any[]>([])
const renameMode = ref(false)
const previewFile = ref<any>(null)
const previewOpen = ref(false)
const unregisteredForSelected = computed(() =>
  unregistered.value.filter(u => !selectedRel.value || u.path.includes(`/${selectedRel.value}/`)),
)

function fileIconType(fileName = '', ftype = '') {
  const ext = (ftype || fileName.split('.').pop() || '').toLowerCase()
  if (['pdf'].includes(ext)) return 'pdf'
  if (['png', 'jpg', 'jpeg', 'gif', 'webp', 'heic'].includes(ext)) return 'img'
  if (['zip', 'rar', '7z'].includes(ext)) return 'zip'
  return 'doc'
}

async function load() {
  loading.value = true
  const [dirsRes, filesRes] = await Promise.all([
    casyContext.files.listCaseDirs(props.caseId),
    casyContext.files.listByCase(props.caseId),
  ])
  loading.value = false
  if (dirsRes.ok) dirs.value = dirsRes.data || []
  else ElMessage.error(dirsRes.error || '加载卷宗目录失败')

  if (filesRes.ok) {
    files.value = filesRes.data || []
    if (!selectedRel.value && dirs.value.length) selectedRel.value = dirs.value[0].relPath
    await rescanUnregistered()
  } else {
    ElMessage.error(filesRes.error || '加载文件失败')
  }
}

async function rescanUnregistered() {
  const r = await casyContext.files.scanUnregistered(props.caseId)
  if (r.ok) unregistered.value = r.data || []
}

// ── 操作 ──
async function createSubdir() {
  try {
    const { value } = await ElMessageBox.prompt(
      selectedRel.value ? `在「${selectedRel.value}」内新建子文件夹` : '在卷宗根目录新建文件夹',
      '新建文件夹',
      { inputPlaceholder: '文件夹名称', inputValue: '' },
    )
    if (!value || !value.trim()) return
    const r = await casyContext.files.createSubdir(props.caseId, selectedRel.value, value.trim())
    if (r.ok) {
      ElMessage.success('已创建')
      selectedRel.value = value.trim()
      await load()
    } else ElMessage.error(r.error || '创建失败')
  } catch { /* 取消 */ }
}

async function importPaths(paths: string[]) {
  if (!paths.length) return
  const r = await casyContext.files.importToCase(
    props.caseId,
    selectedRel.value,
    paths,
  )
  if (r.ok) {
    ElMessage.success(`已归档 ${r.data?.length ?? 0} 个文件`)
    await load()
  } else {
    ElMessage.error(r.error || '导入失败')
  }
}

function onDropFiles(e: Event) {
  // 浏览器 File 对象路径不可得 → 提示走 Tauri 原生拖拽（下方 onDragDropEvent）
  void e
  ElMessage.info('请从访达直接拖入文件窗口（支持原生拖拽落盘）；或使用「扫描未入库」导入既有文件')
}

async function registerOne(u: any) {
  const r = await casyContext.files.registerExisting(props.caseId, [u.path])
  if (r.ok) {
    ElMessage.success(`已登记 ${u.fileName}`)
    await load()
  } else ElMessage.error(r.error || '登记失败')
}

async function registerAll() {
  const paths = unregisteredForSelected.value.map(u => u.path)
  if (!paths.length) return
  const r = await casyContext.files.registerExisting(props.caseId, paths)
  if (r.ok) {
    ElMessage.success(`已批量登记 ${r.data ?? 0} 个文件`)
    await load()
  } else ElMessage.error(r.error || '批量登记失败')
}

async function revealFile(f: any) {
  const r = await casyContext.files.reveal(f.filePath)
  if (!r.ok) ElMessage.error(r.error || '定位失败')
}
async function openFile(f: any) {
  const r = await casyContext.files.openDefault(f.filePath)
  if (!r.ok) ElMessage.error(r.error || '打开失败')
}
async function openRoot() {
  const dirs0 = await casyContext.files.listCaseDirs(props.caseId)
  void dirs0
  // reveal 根目录：用第一个已知文件或 case 目录推断——后端提供根路径更直接，这里复用 reveal 第一个文件
  const first = files.value[0]
  if (first) return revealFile(first)
  ElMessage.info('卷宗为空，先拖入或创建文件')
}

async function removeFile(f: any) {
  try {
    await ElMessageBox.confirm(`删除登记「${f.fileName}」？（磁盘文件保留）`, '删除登记', {
      type: 'warning',
      confirmButtonText: '仅移除登记',
    })
  } catch { /* 用户取消：属预期 */ return }
  const r = await casyContext.files.remove(f.id)
  if (r.ok) await load()
}

onMounted(async () => {
  await load()
  // Tauri 原生拖拽：拿到真实文件系统路径 → 落盘复制登记
  unlistenDrop = await getCurrentWebview().onDragDropEvent(ev => {
    if (ev.payload.type === 'enter') dragging.value = true
    else if (ev.payload.type === 'leave') dragging.value = false
    else if (ev.payload.type === 'drop') {
      dragging.value = false
      const paths = (ev.payload.paths as string[]) || []
      if (paths.length) void importPaths(paths)
    }
  })
})

onUnmounted(() => {
  if (unlistenDrop) unlistenDrop()
})
</script>

<template>
  <div class="cfiles" :class="{ dragging }">
    <!-- 案卷状态条 -->
    <div class="file-status">
      <div class="fs-item">
        <span class="v">{{ files.length }}<small>件</small></span>
        <span class="k">已登记</span>
      </div>
      <div class="fs-sep" />
      <div class="fs-item">
        <span class="v">{{ dirs.length }}</span>
        <span class="k">目录</span>
      </div>
      <div class="fs-sep" />
      <div class="fs-item">
        <span class="v" :class="{ warn: unregistered.length > 0 }">{{ unregistered.length }}</span>
        <span class="k">未入库</span>
      </div>
      <div class="fs-actions">
        <el-button size="small" :icon="Plus" @click="createSubdir">新建文件夹</el-button>
        <el-button
          v-if="selectedRel"
          size="small"
          type="primary"
          plain
          @click="renameMode = !renameMode"
        >
          {{ renameMode ? '退出重命名' : '智能重命名' }}
        </el-button>
        <el-button size="small" :icon="Search" @click="rescanUnregistered(); load()">扫描未入库</el-button>
      </div>
    </div>

    <div class="files-layout">
      <!-- 左：目录树 -->
      <div class="dir-tree">
        <div class="dir-stage"><span class="st-dot" style="background: var(--green)" />{{ rootName }}</div>
        <div
          v-for="d in dirs"
          :key="d.relPath"
          class="dir-item"
          :class="{ active: selectedRel === d.relPath }"
          @click="selectedRel = d.relPath"
        >
          <el-icon class="di-ico"><Folder /></el-icon>
          <span class="di-name">{{ d.relPath }}</span>
          <span class="di-count">{{ d.fileCount }}</span>
          <span class="di-state" :class="d.state" />
        </div>

        <div class="files-toolbar">
          <el-tooltip content="文件夹模板设置">
            <el-button size="small" text :icon="Setting" @click="$router.push('/settings?tab=folder-template')" />
          </el-tooltip>
          <el-tooltip content="在 Finder 中显示">
            <el-button size="small" text :icon="FolderOpened" @click="openRoot" />
          </el-tooltip>
          <span class="ft-hint">
            <span class="circle-dot" />监听中 · 新文件自动捕获
          </span>
        </div>
      </div>

      <!-- 右：重命名工作台 / 文件列表 -->
      <div v-if="renameMode" class="dir-files">
        <RenameWorkbench
          :case-id="caseId"
          :dir-name="selectedRel || rootName"
          :files="visibleFiles"
          @close="renameMode = false"
          @applied="load"
        />
      </div>
      <div v-else class="dir-files" @drop.prevent="onDropFiles" @dragover.prevent>
        <div class="df-head">
          <span class="df-title">{{ selectedRel || rootName }}</span>
          <span class="df-tag">{{ visibleFiles.length }} 项</span>
          <span v-if="unregisteredForSelected.length" class="df-tag warn">
            {{ unregisteredForSelected.length }} 个未入库
            <el-button size="small" text type="primary" @click="registerAll">一键入库</el-button>
          </span>
        </div>

        <div v-if="visibleFiles.length" class="fg-body">
          <div v-for="f in visibleFiles" :key="f.id" class="file-card">
            <div class="f-ico" :class="fileIconType(f.fileName, f.fileType)">
              <el-icon><Document /></el-icon>
            </div>
            <div class="f-main" style="cursor: pointer" title="点击预览" @click.stop="previewFile = f; previewOpen = true">
              <div class="fn">{{ f.fileName }}</div>
              <div class="fm">
                <span>{{ f.fileType || '—' }}</span>
                <span>{{ (f.createdAt || '').slice(5, 10) }}</span>
                <span v-if="f.fileSize">{{ (f.fileSize / 1024).toFixed(0) }} KB</span>
                <span class="tag-ok">已登记</span>
              </div>
            </div>
            <div class="f-ops">
              <el-button size="small" text :icon="FolderOpened" title="在 Finder 中显示" @click.stop="revealFile(f)" />
              <el-button size="small" text title="打开" @click.stop="openFile(f)">开</el-button>
              <el-button size="small" text type="danger" title="移除登记" @click.stop="removeFile(f)">删</el-button>
            </div>
          </div>
        </div>

        <EmptyState
          v-if="!loading && !visibleFiles.length && !dragging"
          type="custom"
          icon="📄"
          title="此目录暂无已登记文件"
          description="把文件从访达拖进本窗口即可自动复制归档并登记；磁盘上已有但未登记的文件可用上方「一键入库」。"
          compact
        />
        <div v-if="dragging" class="drop-hint">松手即归档到「{{ selectedRel || rootName }}」</div>
      </div>
    </div>
  </div>

  <!-- 文件预览侧板 -->
  <FilePreviewPanel v-model="previewOpen" :file="previewFile" />
</template>

<style scoped>
.cfiles { display: flex; flex-direction: column; gap: 14px; }
.file-status {
  display: flex; align-items: center; gap: 22px;
  padding: 12px 16px;
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  background: var(--c-surface);
}
.fs-item { display: flex; flex-direction: column; gap: 3px; }
.fs-item .v { font-size: 17px; font-weight: 700; color: var(--c-text); line-height: 1.1; }
.fs-item .v.warn { color: var(--c-warning); }
.fs-item .v small { font-size: 10.5px; font-weight: 500; color: var(--c-text-secondary); margin-left: 2px; }
.fs-item .k { font-size: 10.5px; color: var(--c-text-secondary); }
.fs-sep { width: 1px; height: 34px; background: var(--c-border); }
.fs-actions { margin-left: auto; display: flex; gap: 8px; }

.files-layout { display: flex; gap: 14px; align-items: stretch; min-height: 320px; }
.dir-tree {
  width: 300px; flex-shrink: 0; display: flex; flex-direction: column; gap: 2px;
  overflow-y: auto; border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg); background: var(--c-surface); padding: 8px;
}
.dir-stage {
  font-size: 10px; font-weight: 600; color: var(--c-text-secondary);
  letter-spacing: .5px; padding: 10px 10px 4px;
}
.st-dot { display: inline-block; width: 6px; height: 6px; border-radius: 50%; margin-right: 6px; vertical-align: 1px; }
.dir-item {
  display: flex; align-items: center; gap: 8px; height: 34px; padding: 0 10px;
  border-radius: var(--c-radius-btn); cursor: pointer; border: 1px solid transparent;
  transition: background var(--motion-fast) var(--ease-out);
}
.dir-item:hover { background: var(--c-bg-hover); }
.dir-item.active { background: var(--c-primary-light); border-color: var(--c-primary-lighter, #C3CFE3); }
.di-ico { width: 15px; flex-shrink: 0; color: var(--c-warning); }
.di-name {
  flex: 1; font-size: 12.5px; color: var(--c-text);
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
}
.di-count { font-size: 10.5px; color: var(--c-text-secondary); font-family: var(--font-mono); }
.di-state { width: 7px; height: 7px; border-radius: 50%; flex-shrink: 0; }
.di-state.ok { background: var(--c-success); }
.di-state.empty { background: #D8DBE0; }
.files-toolbar {
  display: flex; align-items: center; gap: 8px;
  padding: 8px 2px 0; margin-top: auto; border-top: 1px solid var(--c-border);
}
.ft-hint {
  font-size: 10.5px; color: var(--c-text-secondary); margin-left: auto;
  display: flex; align-items: center; gap: 5px;
}
.circle-dot { width: 6px; height: 6px; border-radius: 50%; background: var(--c-success); }

.dir-files {
  flex: 1; min-width: 0; overflow-y: auto; position: relative;
  border: 1px dashed transparent; border-radius: var(--c-radius-lg);
}
.cfiles.dragging .dir-files {
  border-color: var(--c-primary);
  background: var(--c-primary-light);
}
.df-head { display: flex; align-items: center; gap: 8px; padding: 2px 2px 10px; }
.df-title { font-size: 13px; font-weight: 600; color: var(--c-text); }
.df-tag {
  font-size: 10.5px; color: var(--c-text-secondary); font-family: var(--font-mono);
  background: var(--gray-50); border-radius: 999px; padding: 2px 9px;
}
.df-tag.warn { background: var(--c-warning-light); color: var(--c-warning); }

.fg-body {
  display: flex; flex-direction: column; gap: 4px;
  border: 1px solid var(--c-border); border-radius: var(--c-radius-lg);
  padding: 6px; background: var(--c-surface);
}
.file-card {
  display: flex; align-items: center; gap: 12px;
  padding: 8px 10px; border-radius: var(--c-radius-btn);
  border: 1px solid transparent;
  transition: background var(--motion-fast) var(--ease-out), border-color var(--motion-fast) var(--ease-out);
}
.file-card:hover { background: var(--c-bg-hover); border-color: var(--c-border); }
.f-ico {
  width: 32px; height: 32px; border-radius: var(--c-radius-btn);
  display: flex; align-items: center; justify-content: center; flex-shrink: 0;
}
.f-ico.pdf { background: var(--c-danger-light); color: var(--c-danger); }
.f-ico.doc { background: var(--c-primary-light); color: var(--c-primary); }
.f-ico.img { background: var(--c-success-light); color: var(--c-success); }
.f-ico.zip { background: var(--c-warning-light); color: var(--c-warning); }
.f-main { flex: 1; min-width: 0; }
.fn {
  font-size: 12.5px; font-weight: 500; color: var(--c-text);
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
}
.fm {
  font-size: 10.5px; color: var(--c-text-secondary); margin-top: 2px;
  display: flex; gap: 8px; align-items: center;
}
.tag-ok {
  font-size: 9.5px; padding: 0 6px; border-radius: 999px;
  background: var(--c-success-light); color: var(--c-success);
}
.f-ops {
  display: flex; gap: 2px; opacity: 0;
  transition: opacity var(--motion-fast) var(--ease-out);
}
.file-card:hover .f-ops { opacity: 1; }
.drop-hint {
  position: absolute; inset: 0; z-index: 5;
  display: flex; align-items: center; justify-content: center;
  font-size: 14px; font-weight: 600; color: var(--c-primary);
  background: rgba(237, 241, 248, 0.85);
  border-radius: var(--c-radius-lg);
  pointer-events: none;
}
</style>
