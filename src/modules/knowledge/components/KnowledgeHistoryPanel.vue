<script setup>
import { ref, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Clock, RefreshLeft } from '@element-plus/icons-vue'
import { casyContext } from '../../../core/plugin/context'

const props = defineProps({
  note: { type: Object, default: null },
  beforeRestore: { type: Function, default: null },
})
const emit = defineEmits(['restored'])
const versions = ref([])
const loading = ref(false)
const selected = ref(null)
const diff = ref([])
const restoring = ref(false)

function normalize(value) { return Array.isArray(value) ? value : [] }
function formatTime(value) { return value ? String(value).replace('T', ' ').slice(0, 19) : '未知时间' }
function reasonLabel(reason) { return reason === 'before_restore' ? '恢复前快照' : reason === 'edit_session' ? '编辑会话' : reason || '编辑' }

async function load() {
  versions.value = []
  selected.value = null
  diff.value = []
  if (!props.note?.id) return
  loading.value = true
  const result = await casyContext.knowledge.versions(props.note.id)
  versions.value = result.ok ? normalize(result.data) : []
  loading.value = false
}

async function inspect(version) {
  selected.value = version
  const result = await casyContext.knowledge.diffWithCurrent(version.id, props.note.id)
  diff.value = result.ok && Array.isArray(result.data?.diffs) ? result.data.diffs : []
}

async function restore() {
  if (!selected.value) return
  try {
    await ElMessageBox.confirm('恢复后，当前正文会先保存为一个快照，可以再次找回。', '恢复历史版本', { type: 'warning', confirmButtonText: '恢复' })
  } catch { /* 用户取消：属预期 */ return }
  if (props.beforeRestore && !(await props.beforeRestore())) return
  restoring.value = true
  const result = await casyContext.knowledge.restoreVersion(props.note.id, selected.value.id)
  restoring.value = false
  if (!result.ok) return ElMessage.error(result.error || '恢复失败')
  ElMessage.success('历史版本已恢复')
  emit('restored')
  await load()
}

watch(() => props.note?.id, load, { immediate: true })
defineExpose({ reload: load })
</script>

<template>
  <div class="history-panel">
    <div class="history-title"><Clock /> 版本历史 <span>{{ versions.length }}</span></div>
    <div v-loading="loading" class="version-list">
      <button v-for="version in versions" :key="version.id" :class="['version-item',{active:selected?.id===version.id}]" @click="inspect(version)">
        <strong>{{ formatTime(version.changedAt) }}</strong>
        <span>{{ reasonLabel(version.changeReason) }} · {{ String(version.content || '').length }} 字</span>
      </button>
      <div v-if="!loading && !versions.length" class="empty">正文发生修改后，这里会按编辑会话保存历史快照。</div>
    </div>
    <template v-if="selected">
      <div class="history-actions"><el-button :icon="RefreshLeft" type="primary" plain size="small" :loading="restoring" @click="restore">恢复此版本</el-button></div>
      <div class="diff-list">
        <div v-for="(line,index) in diff.slice(0,200)" :key="index" :class="['diff-line',line.type]">
          <span>{{ line.type === 'added' ? '+' : line.type === 'removed' ? '−' : ' ' }}</span><code>{{ line.text }}</code>
        </div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.history-panel{height:100%;display:flex;flex-direction:column;padding:14px;box-sizing:border-box}.history-title{display:flex;align-items:center;gap:6px;font-size:12px;font-weight:700;margin-bottom:10px}.history-title svg{width:14px}.history-title span{margin-left:auto;color:var(--c-text-secondary)}.version-list{max-height:260px;overflow:auto;border:1px solid var(--c-border);border-radius:8px}.version-item{display:flex;flex-direction:column;gap:3px;width:100%;padding:9px 10px;border:0;border-bottom:1px solid var(--c-border);background:transparent;color:inherit;text-align:left;cursor:pointer}.version-item:hover,.version-item.active{background:var(--c-bg-subtle)}.version-item strong{font-size:11px}.version-item span,.empty{font-size:10px;color:var(--c-text-secondary)}.empty{padding:14px;line-height:1.5}.history-actions{padding:10px 0}.diff-list{flex:1;overflow:auto;border:1px solid var(--c-border);border-radius:8px;background:var(--c-bg-page)}.diff-line{display:grid;grid-template-columns:18px 1fr;padding:2px 6px;font-size:10px;line-height:1.5}.diff-line.added{background:rgba(34,197,94,.1);color:#3f8b59}.diff-line.removed{background:rgba(239,68,68,.1);color:#b4554f}.diff-line code{white-space:pre-wrap;word-break:break-word}
</style>
