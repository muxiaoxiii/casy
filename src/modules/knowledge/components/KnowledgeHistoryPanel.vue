<script setup>
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Clock, RefreshLeft } from '../../../shared/icons'
import { casyContext } from '../../../core/plugin/context'

const props = defineProps({
  note: { type: Object, default: null },
  beforeRestore: { type: Function, default: null },
  restoreVersion: { type: Function, default: null },
})
const emit = defineEmits(['restored'])
const versions = ref([])
const loading = ref(false)
const selected = ref(null)
const diff = ref([])
const showDiff = ref(false)
const diffLoading = ref(false)
const error = ref('')
const diffError = ref('')
const preview = computed(() => selected.value?.content ?? '')
const restoring = ref(false)
let loadRevision = 0
let diffRevision = 0

function normalize(value) { return Array.isArray(value) ? value : [] }
function formatTime(value) { return value ? String(value).replace('T', ' ').slice(0, 19) : '未知时间' }
function reasonLabel(reason) { return reason === 'before_restore' ? '恢复前快照' : reason === 'edit_session' ? '编辑会话' : reason || '编辑' }

async function load() {
  const request = ++loadRevision
  ++diffRevision
  error.value = ''
  showDiff.value = false
  diffLoading.value = false
  versions.value = []
  selected.value = null
  diff.value = []
  loading.value = false
  if (!props.note?.id) return
  loading.value = true
  try {
    const result = await casyContext.knowledge.versions(props.note.id)
    if (request !== loadRevision) return
    if (!result.ok) throw new Error(result.error || '历史版本加载失败')
    versions.value = normalize(result.data)
    selected.value = versions.value[0] || null
  } catch (e) { if (request === loadRevision) error.value = String(e) }
  finally { if (request === loadRevision) loading.value = false }
}

function inspect(version) {
  ++diffRevision
  selected.value = version
  showDiff.value = false
  diffLoading.value = false
  diffError.value = ''
  diff.value = []
}
async function compare() {
  if (!selected.value) return
  const request = ++diffRevision
  const noteId = props.note?.id
  const versionId = selected.value.id
  showDiff.value = true
  diffLoading.value = true
  diffError.value = ''
  try {
    const result = await casyContext.knowledge.diffWithCurrent(versionId, noteId)
    if (request !== diffRevision || noteId !== props.note?.id) return
    if (!result.ok) throw new Error(result.error || '对比加载失败')
    diff.value = Array.isArray(result.data?.diffs) ? result.data.diffs.map(line => ({ ...line, type: line.diffType || line.type })) : []
  } catch (e) { if (request === diffRevision) diffError.value = String(e) }
  finally { if (request === diffRevision) diffLoading.value = false }
}

async function restore() {
  if (!selected.value || restoring.value) return
  const noteId = props.note?.id
  const versionId = selected.value.id
  try {
    await ElMessageBox.confirm('恢复后，当前正文会先保存为一个快照，可以再次找回。', '恢复历史版本', { type: 'warning', confirmButtonText: '恢复此版本', cancelButtonText: '保留当前正文' })
  } catch { /* 用户取消：属预期 */ return }
  if (noteId !== props.note?.id || versionId !== selected.value?.id) return
  restoring.value = true
  try {
    if (props.beforeRestore && !(await props.beforeRestore())) return
    if (noteId !== props.note?.id || versionId !== selected.value?.id) return
    const result = props.restoreVersion
      ? await props.restoreVersion(noteId, versionId)
      : await casyContext.knowledge.restoreVersion(noteId, versionId)
    if (!result.ok) return ElMessage.error(result.error || '恢复失败')
    ElMessage.success('历史版本已恢复')
    emit('restored')
    await load()
  } catch (e) { ElMessage.error(String(e)) } finally { restoring.value = false }
}

watch(() => props.note?.id, load, { immediate: true })
onBeforeUnmount(() => { loadRevision++; diffRevision++ })
defineExpose({ reload: load })
</script>

<template>
  <div class="history-panel">
    <div class="history-title"><Clock /> 版本历史 <span>{{ versions.length }}</span></div>
    <p class="history-hint">选择时间点查看完整正文。恢复前会保留当前版本。</p>
    <div v-if="error" class="empty" role="alert">{{ error }} <el-button text @click="load">重试</el-button></div>
    <div v-loading="loading" class="version-list">
      <button v-for="version in versions" :key="version.id" :class="['version-item',{active:selected?.id===version.id}]" :aria-pressed="selected?.id===version.id" :disabled="restoring" @click="inspect(version)">
        <strong>{{ formatTime(version.changedAt) }}</strong>
        <span>{{ reasonLabel(version.changeReason) }} · {{ String(version.content || '').length }} 字</span>
      </button>
      <div v-if="!loading && !error && !versions.length" class="empty">正文发生修改后，这里会按编辑会话保存历史快照。</div>
    </div>
    <template v-if="selected">
      <div class="history-actions"><el-button :icon="RefreshLeft" type="primary" plain size="small" :loading="restoring" @click="restore">恢复此版本</el-button><el-button size="small" text @click="showDiff ? showDiff = false : compare()">{{ showDiff ? '查看完整正文' : '对比已保存正文' }}</el-button></div>
      <div v-if="!showDiff" class="version-preview" aria-label="历史版本完整正文"><pre>{{ preview || '（此版本正文为空）' }}</pre></div>
      <div v-else v-loading="diffLoading" class="diff-list">
        <p v-if="diffError" class="empty" role="alert">{{ diffError }} <el-button text @click="compare">重试</el-button></p>
        <p v-else-if="!diffLoading && !diff.length" class="empty">与已保存正文一致</p>
        <div v-for="(line,index) in diff" :key="index" :class="['diff-line',line.type]">
          <span>{{ line.type === 'added' ? '+' : line.type === 'removed' ? '−' : ' ' }}</span><code>{{ line.text }}</code>
        </div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.history-hint{font-size:12px;line-height:1.6;color:var(--c-text-secondary);margin:0 0 12px}.version-preview{flex:1;min-height:120px;overflow:auto;padding:12px;background:var(--c-bg-page);border:1px solid var(--c-border);border-radius:8px}.version-preview pre{font:13px/1.8 var(--font-family);white-space:pre-wrap;overflow-wrap:anywhere;margin:0}.history-actions{display:flex;flex-wrap:wrap;gap:4px}.version-list{flex-shrink:0}.history-panel{height:100%;display:flex;flex-direction:column;padding:14px;box-sizing:border-box}.history-title{display:flex;align-items:center;gap:6px;font-size:12px;font-weight:700;margin-bottom:10px}.history-title svg{width:14px}.history-title span{margin-left:auto;color:var(--c-text-secondary)}.version-list{max-height:260px;overflow:auto;border:1px solid var(--c-border);border-radius:8px}.version-item{display:flex;flex-direction:column;gap:3px;width:100%;padding:9px 10px;border:0;border-bottom:1px solid var(--c-border);background:transparent;color:inherit;text-align:left;cursor:pointer}.version-item:hover,.version-item.active{background:var(--c-bg-subtle)}.version-item strong{font-size:11px}.version-item span,.empty{font-size:10px;color:var(--c-text-secondary)}.empty{padding:14px;line-height:1.5}.history-actions{padding:10px 0}.diff-list{flex:1;overflow:auto;border:1px solid var(--c-border);border-radius:8px;background:var(--c-bg-page)}.diff-line{display:grid;grid-template-columns:18px 1fr;padding:2px 6px;font-size:10px;line-height:1.5}.diff-line.added{background:rgba(34,197,94,.1);color:#3f8b59}.diff-line.removed{background:rgba(239,68,68,.1);color:#b4554f}.diff-line code{white-space:pre-wrap;word-break:break-word}
</style>
