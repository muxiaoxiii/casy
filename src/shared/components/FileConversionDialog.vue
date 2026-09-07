<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { FolderOpened, Plus, Delete, RefreshRight, Document } from '@element-plus/icons-vue'
import { tauriCallSafe } from '../../core/tauriBridge'
import { casyContext } from '../../core/plugin/context'
import { isTauriRuntime } from '../../core/mockData'
import { safeListen } from '../../core/tauriEvents'
const props = defineProps<{ modelValue: boolean }>()
const emit = defineEmits<{ 'update:modelValue': [value: boolean] }>()
type Progress = { jobId: string; sourcePath: string; phase: string; currentPage: number; totalPages: number; elapsedSeconds: number; remainingSeconds: number | null }
type Entry = { path: string; status: 'pending' | 'running' | 'done' | 'failed'; output?: string; error?: string; progress?: Progress; startedAt?: number }
const entries = ref<Entry[]>([]), outputDir = ref(''), running = ref(false), stop = ref(false)
const now = ref(Date.now())
let unlisten: (() => void) | undefined
let timer: ReturnType<typeof setInterval> | undefined
let disposed = false
function duration(seconds: number) {
  const value = Math.max(0, Math.round(seconds))
  return value < 60 ? `${value} 秒` : `${Math.floor(value / 60)} 分 ${value % 60} 秒`
}
function elapsed(entry: Entry) { return duration((now.value - (entry.startedAt ?? now.value)) / 1000) }
const pending = computed(() => entries.value.filter(e => e.status === 'pending'))
const formats = ['pdf','png','jpg','jpeg','webp','bmp','tif','tiff','md','markdown','txt','doc','docx','docm','rtf','odt']
function add(paths: string[]) {
  for (const path of paths) {
    if (!formats.includes(path.split('.').pop()?.toLowerCase() || '')) { ElMessage.warning(`不支持的文件格式：${path.split(/[\\/]/).pop()}`); continue }
    if (!entries.value.some(e => e.path === path)) entries.value.push({ path, status: 'pending' })
  }
}
async function chooseFiles() {
  if (!isTauriRuntime()) return ElMessage.info('文件转换需要桌面版 Casy')
  const { open } = await import('@tauri-apps/plugin-dialog')
  const paths = await open({ multiple: true, filters: [{ name: '文档与扫描件', extensions: formats }] })
  if (paths) add(Array.isArray(paths) ? paths : [paths])
}
async function chooseDirectory() {
  if (!isTauriRuntime()) return ElMessage.info('文件转换需要桌面版 Casy')
  const { open } = await import('@tauri-apps/plugin-dialog')
  const path = await open({ directory: true, multiple: false })
  if (typeof path === 'string') outputDir.value = path
}
async function convert() {
  if (!outputDir.value || running.value) return
  running.value = true; stop.value = false
  try {
    unlisten = await safeListen<Progress>('document-conversion-progress', ({ payload }) => {
      const entry = entries.value.find(e => e.status === 'running' && (e.progress?.jobId === payload.jobId || (!e.progress && payload.phase === 'preparing' && e.path === payload.sourcePath)))
      if (entry) entry.progress = payload
    })
    if (disposed) return
    timer = setInterval(() => { now.value = Date.now() }, 1000)
    for (const entry of pending.value) {
      if (stop.value) break
      entry.status = 'running'; entry.error = undefined
      entry.progress = undefined; entry.startedAt = Date.now(); now.value = Date.now()
      const result = await tauriCallSafe('convert_file_to_markdown', { sourcePath: entry.path, outputDir: outputDir.value })
      entry.status = result.ok ? 'done' : 'failed'
      entry.output = result.data?.outputPath
      entry.error = result.error || undefined
    }
  } finally { unlisten?.(); unlisten = undefined; clearInterval(timer); timer = undefined; running.value = false }
}
function drop(event: Event) {
  if (props.modelValue && !running.value) add((event as CustomEvent<{ paths?: string[] }>).detail?.paths || [])
}
onMounted(() => window.addEventListener('casy:file-drop', drop))
onUnmounted(() => { disposed = true; stop.value = true; window.removeEventListener('casy:file-drop', drop); unlisten?.(); clearInterval(timer) })
</script>
<template>
  <el-dialog :model-value="modelValue" title="文件转换" width="760px" class="conversion-dialog" :close-on-click-modal="!running" :close-on-press-escape="!running" :show-close="!running" @update:model-value="emit('update:modelValue', $event)">
    <div class="conversion-toolbar"><el-button :icon="Plus" :disabled="running" @click="chooseFiles">选择文件</el-button><el-button :icon="FolderOpened" :disabled="running" @click="chooseDirectory">输出目录</el-button><span class="destination" :title="outputDir">{{ outputDir || '尚未选择输出目录' }}</span></div>
    <div class="conversion-list">
      <div v-if="!entries.length" class="conversion-empty"><el-icon :size="28"><Document /></el-icon><span>没有待转换文件</span></div>
      <div v-for="entry in entries" :key="entry.path" class="conversion-row">
        <div class="file-detail"><strong :title="entry.path">{{ entry.path.split(/[\\/]/).pop() }}</strong><span v-if="entry.error" class="conversion-error">{{ entry.error }}</span><span v-else-if="entry.status !== 'running'">{{ ({pending:'待转换',done:'已完成',failed:'失败'})[entry.status] }}</span>
          <template v-if="entry.status === 'running'">
            <span aria-live="polite">{{ entry.progress?.phase === 'finalizing' || entry.progress?.phase === 'completed' ? '正在整理结果' : entry.progress?.totalPages ? `已识别 ${entry.progress.currentPage} / ${entry.progress.totalPages} 页` : '正在准备转换' }}</span>
            <el-progress v-if="entry.progress?.totalPages" :percentage="Math.min(99, Math.round(entry.progress.currentPage / entry.progress.totalPages * 100))" :show-text="false" :stroke-width="4" />
            <span class="conversion-timing">已用 {{ elapsed(entry) }}<template v-if="entry.progress?.remainingSeconds != null"> · 预计剩余 {{ duration(entry.progress.remainingSeconds) }}</template></span>
          </template>
        </div>
        <el-button v-if="entry.output" :icon="FolderOpened" title="定位 Markdown" aria-label="定位 Markdown" @click="casyContext.files.reveal(entry.output)" />
        <el-button v-if="entry.status === 'failed'" :icon="RefreshRight" :disabled="running" title="重试" aria-label="重试" @click="entry.status = 'pending'; entry.error = undefined" />
        <el-button :icon="Delete" :disabled="running" title="移出队列" aria-label="移出队列" @click="entries = entries.filter(e => e !== entry)" />
      </div>
    </div>
    <template #footer><span class="conversion-count">{{ entries.filter(e => e.status === 'done').length }} / {{ entries.length }}</span><el-button v-if="running" :disabled="stop" @click="stop = true">{{ stop ? '当前文件完成后停止' : '停止后续转换' }}</el-button><el-button v-else @click="emit('update:modelValue', false)">关闭</el-button><el-button type="primary" :loading="running" :disabled="!outputDir || !pending.length" @click="convert">转换为 Markdown</el-button></template>
  </el-dialog>
</template>
<style scoped>
.conversion-toolbar{display:flex;flex-wrap:wrap;gap:8px;align-items:center}.conversion-toolbar .el-button{margin:0}.destination{font-size:12px;overflow-wrap:anywhere;min-width:0;flex:1}.conversion-list{margin-top:16px;max-height:50dvh;overflow:auto;border-top:1px solid var(--c-border)}.conversion-row{display:flex;align-items:center;gap:8px;padding:12px 0;border-bottom:1px solid var(--c-border)}.file-detail{flex:1;min-width:0;display:flex;flex-direction:column;gap:4px;overflow-wrap:anywhere}.file-detail span{font-size:12px;color:var(--c-text-secondary)}.file-detail .conversion-error{color:var(--el-color-danger)}.conversion-row .el-button{margin:0}.conversion-empty{display:flex;flex-direction:column;align-items:center;gap:12px;padding:40px}.conversion-count{margin-right:12px}
</style>
<style>.el-dialog.conversion-dialog{max-width:calc(100vw - 24px)}.conversion-dialog .el-dialog__footer{display:flex;justify-content:flex-end;flex-wrap:wrap;gap:8px;align-items:center}.conversion-dialog .el-dialog__footer .el-button{margin:0}@media(max-width:600px){.conversion-dialog .destination{flex-basis:100%;margin-top:4px}}</style>
