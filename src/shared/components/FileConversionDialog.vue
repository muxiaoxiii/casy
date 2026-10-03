<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { FolderOpened, Plus, Delete, RefreshRight, Document } from '../icons'
import { tauriCallSafe } from '../../core/tauriBridge'
import { casyContext } from '../../core/plugin/context'
import { isTauriRuntime } from '../../core/mockData'
import { safeListen } from '../../core/tauriEvents'
const props = defineProps<{ modelValue: boolean }>()
const emit = defineEmits<{ 'update:modelValue': [value: boolean] }>()
type Progress = { jobId: string; sourcePath: string; phase: string; currentPage: number; totalPages: number; elapsedSeconds: number; remainingSeconds: number | null; pageTiming?: {renderMs:number;ocrMs:number;layoutMs:number;totalMs:number} }
type Entry = { jobId?: string; path: string; status: 'pending' | 'running' | 'done' | 'failed' | 'cancelled'; output?: string; pdfOutput?: string; error?: string; progress?: Progress; startedAt?: number }
const entries = ref<Entry[]>([]), outputDir = ref(''), running = ref(false), stop = ref(false), targetFormat = ref<'markdown' | 'pdf' | 'both'>('markdown')
const now = ref(Date.now())
let unlisten: (() => void) | undefined
let timer: ReturnType<typeof setInterval> | undefined
let disposed = false
function duration(seconds: number) {
  const value = Math.max(0, Math.round(seconds))
  return value < 60 ? `${value} 秒` : `${Math.floor(value / 60)} 分 ${value % 60} 秒`
}
function elapsed(entry: Entry) { return duration(entry.progress?.elapsedSeconds ?? (now.value - (entry.startedAt ?? now.value)) / 1000) }
const pending = computed(() => entries.value.filter(e => e.status === 'pending'))
const hasTextInput = computed(() => pending.value.some(e => /\.(md|markdown|txt|doc|docx|docm|rtf|odt)$/i.test(e.path)))
watch(hasTextInput, value => { if (value && !running.value) targetFormat.value = 'markdown' })
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
  if (hasTextInput.value && targetFormat.value !== 'markdown') { ElMessage.warning('文字文档请选择 Markdown 输出'); return }
  running.value = true; stop.value = false
  const batch = [...pending.value]
  try {
    const registered = await tauriCallSafe('register_conversion_batch', { sourcePaths: batch.map(e => e.path) })
    if (!registered.ok || !registered.data || registered.data.length !== batch.length) {
      ElMessage.warning(registered.error || '无法登记转换任务'); return
    }
    batch.forEach((entry,index) => { entry.jobId = registered.data![index] })
    const subscription = safeListen<Progress>('document-conversion-progress', ({ payload }) => {
      const entry = entries.value.find(e => e.status === 'running' && (e.progress?.jobId === payload.jobId || (!e.progress && payload.phase === 'preparing' && e.path === payload.sourcePath)))
      if (entry) entry.progress = payload
    })
    unlisten = subscription
    await subscription.ready
    if (disposed) return
    timer = setInterval(() => { now.value = Date.now() }, 1000)
    for (const entry of batch) {
      if (stop.value || disposed) break
      entry.status = 'running'; entry.error = undefined
      entry.progress = undefined; entry.startedAt = Date.now(); now.value = Date.now()
      const result = await tauriCallSafe('convert_file_to_markdown', { sourcePath: entry.path, outputDir: outputDir.value, jobId: entry.jobId, targetFormat: targetFormat.value })
      entry.status = result.ok ? 'done' : result.error?.includes('CONVERSION_CANCELLED') ? 'cancelled' : 'failed'
      entry.output = result.data?.markdownPath || (targetFormat.value === 'markdown' ? result.data?.outputPath : undefined)
      entry.pdfOutput = result.data?.pdfPath || (targetFormat.value === 'pdf' ? result.data?.outputPath : undefined)
      entry.error = result.error || undefined
    }
  } finally {
    const waiting = batch.filter(e => e.status === 'pending' && e.jobId).map(e => e.jobId!)
    if (waiting.length) {
      const cancelled = await tauriCallSafe('cancel_queued_conversions', { jobIds: waiting })
      if (!cancelled.ok && !disposed) ElMessage.warning(cancelled.error || '取消后续任务失败，请查看处理中心')
    }
    unlisten?.(); unlisten = undefined; clearInterval(timer); timer = undefined; running.value = false }
}
async function cancelCurrent() {
  stop.value = true
  const entry = entries.value.find(item => item.status === 'running')
  if (!entry?.jobId) return
  const result = await tauriCallSafe('cancel_conversion', { jobId: entry.jobId })
  if (!result.ok) ElMessage.warning(result.error || '无法取消，请查看处理中心')
}
function drop(event: Event) {
  if (props.modelValue && !running.value) add((event as CustomEvent<{ paths?: string[] }>).detail?.paths || [])
}
onMounted(() => window.addEventListener('casy:file-drop', drop))
onUnmounted(() => { disposed = true; stop.value = true; window.removeEventListener('casy:file-drop', drop); unlisten?.(); clearInterval(timer) })
</script>
<template>
  <el-dialog :model-value="modelValue" title="文件转换" width="760px" class="conversion-dialog" :close-on-click-modal="!running" :close-on-press-escape="!running" :show-close="!running" @update:model-value="emit('update:modelValue', $event)">
    <div class="conversion-toolbar">
      <el-button :icon="Plus" :disabled="running" @click="chooseFiles">选择文件</el-button>
      <el-button :icon="FolderOpened" :disabled="running" @click="chooseDirectory">输出目录</el-button>
      <el-radio-group v-model="targetFormat" :disabled="running" size="small">
        <el-radio-button value="markdown">Markdown</el-radio-button>
        <el-radio-button value="pdf" :disabled="hasTextInput">双层 PDF</el-radio-button>
        <el-radio-button value="both" :disabled="hasTextInput">两者都要</el-radio-button>
      </el-radio-group>
      <span class="destination" :title="outputDir">{{ outputDir || '尚未选择输出目录' }}</span>
    </div>
    <p v-if="hasTextInput" class="conversion-image-note">文字文档输出 Markdown；需要排版 PDF 时，请使用文书编辑器的导出功能。</p>
    <p v-if="targetFormat !== 'pdf'" class="conversion-image-note">配图会另存到同目录的 casy-images-… 文件夹；移动或分享 Markdown 时，请一并携带该文件夹。</p>
    <div class="conversion-list">
      <div v-if="!entries.length" class="conversion-empty"><el-icon :size="28"><Document /></el-icon><span>没有待转换文件</span></div>
      <div v-for="entry in entries" :key="entry.path" class="conversion-row">
        <div class="file-detail"><strong :title="entry.path">{{ entry.path.split(/[\\/]/).pop() }}</strong><span v-if="entry.error" class="conversion-error">{{ entry.error }}</span><span v-else-if="entry.status !== 'running'">{{ ({pending:'待转换',done:'已完成',failed:'失败',cancelled:'已取消'})[entry.status] }}</span>
          <template v-if="entry.status === 'running'">
            <span aria-live="polite">{{ entry.progress?.phase?.startsWith('finalizing') || entry.progress?.phase === 'completed' ? '正在整理结果' : entry.progress?.phase === 'analyzing_layout' ? '正在分析复杂版面' : entry.progress?.phase === 'rendering' ? '正在渲染页面' : entry.progress?.totalPages ? `已识别 ${entry.progress.currentPage} / ${entry.progress.totalPages} 页` : '正在准备转换' }}</span>
            <el-progress v-if="entry.progress?.totalPages" :percentage="Math.min(99, Math.round(entry.progress.currentPage / entry.progress.totalPages * 100))" :show-text="false" :stroke-width="4" />
            <span class="conversion-timing">已用 {{ elapsed(entry) }}<template v-if="entry.progress?.remainingSeconds != null"> · 预计剩余 {{ duration(entry.progress.remainingSeconds) }}</template></span>
            <span v-if="entry.progress?.pageTiming" class="conversion-timing">最近一页：渲染 {{ duration(entry.progress.pageTiming.renderMs/1000) }} · 文字 {{ duration(entry.progress.pageTiming.ocrMs/1000) }} · 版面 {{ duration(entry.progress.pageTiming.layoutMs/1000) }}</span>
          </template>
        </div>
        <el-button v-if="entry.output" :icon="Document" title="定位 Markdown" aria-label="定位 Markdown" @click="casyContext.files.reveal(entry.output)">MD</el-button>
        <el-button v-if="entry.pdfOutput" :icon="FolderOpened" title="定位双层 PDF" aria-label="定位双层 PDF" @click="casyContext.files.reveal(entry.pdfOutput)">PDF</el-button>
        <el-button v-if="(entry.status === 'failed' || entry.status === 'cancelled')" :icon="RefreshRight" :disabled="running" title="重试" aria-label="重试" @click="entry.status = 'pending'; entry.error = undefined" />
        <el-button :icon="Delete" :disabled="running" title="移出队列" aria-label="移出队列" @click="entries = entries.filter(e => e !== entry)" />
      </div>
    </div>
    <template #footer>
      <el-button v-if="running" @click="emit('update:modelValue', false)">收起到后台</el-button>
      <span class="conversion-count">{{ entries.filter(e => e.status === 'done').length }} / {{ entries.length }}</span>
      <el-button v-if="running" :disabled="stop" @click="cancelCurrent">{{ stop ? '正在停止' : '取消当前并停止' }}</el-button>
      <el-button v-else @click="emit('update:modelValue', false)">关闭</el-button>
      <el-button type="primary" :loading="running" :disabled="!outputDir || !pending.length" @click="convert">
        {{ targetFormat === 'pdf' ? '转换为双层 PDF' : targetFormat === 'both' ? '转换为 Markdown 与双层 PDF' : '转换为 Markdown' }}
      </el-button>
    </template>
  </el-dialog>
</template>
<style scoped>
.conversion-image-note{font-size:12px;line-height:1.6;color:var(--c-text-secondary);margin:10px 0 0}
.conversion-toolbar{display:flex;flex-wrap:wrap;gap:8px;align-items:center}.conversion-toolbar .el-button{margin:0}.destination{font-size:12px;overflow-wrap:anywhere;min-width:0;flex:1}.conversion-list{margin-top:16px;max-height:50dvh;overflow:auto;border-top:1px solid var(--c-border)}.conversion-row{display:flex;align-items:center;gap:8px;padding:12px 0;border-bottom:1px solid var(--c-border)}.file-detail{flex:1;min-width:0;display:flex;flex-direction:column;gap:4px;overflow-wrap:anywhere}.file-detail span{font-size:12px;color:var(--c-text-secondary)}.file-detail .conversion-error{color:var(--el-color-danger)}.conversion-row .el-button{margin:0}.conversion-empty{display:flex;flex-direction:column;align-items:center;gap:12px;padding:40px}.conversion-count{margin-right:12px}
</style>
<style>.el-dialog.conversion-dialog{max-width:calc(100vw - 24px)}.conversion-dialog .el-dialog__footer{display:flex;justify-content:flex-end;flex-wrap:wrap;gap:8px;align-items:center}.conversion-dialog .el-dialog__footer .el-button{margin:0}@media(max-width:600px){.conversion-dialog .destination{flex-basis:100%;margin-top:4px}}</style>
