<script setup lang="ts">
import { formatTimestamp, relativeTimestamp } from '../../../shared/utils/date'
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { casyContext } from '../../../core/plugin/context'
import { sanitizePreviewHtml } from '../../../shared/markdown/mdBridge'
import type { Draft, DraftVersion } from '../../../types/bindings'
const props = defineProps<{ modelValue: boolean; draft: Draft | null; beforeRestore: () => Promise<boolean> }>()
const emit = defineEmits<{ 'update:modelValue': [value: boolean]; restored: [draft: Draft] }>()
const versions = ref<DraftVersion[]>([])
const selected = ref<DraftVersion | null>(null)
const loading = ref(false)
const restoring = ref(false)
const error = ref('')
const more = ref(false)
let revision = 0
const preview = computed(() => sanitizePreviewHtml(selected.value?.content || ''))
async function load(append = false) {
  if (!props.draft) return
  const request = ++revision
  loading.value = true; error.value = ''
  if (!append) { versions.value = []; selected.value = null }
  try {
    const result = await casyContext.docs.draftVersions(props.draft.id, append ? versions.value.length : 0)
    if (request !== revision) return
    if (!result.ok) throw new Error(result.error || '历史版本加载失败')
    const rows = result.data || []
    versions.value = append ? [...versions.value, ...rows] : rows
    if (!append) selected.value = rows[0] || null
    more.value = rows.length === 50
  } catch (e) { if (request === revision) error.value = String(e) }
  finally { if (request === revision) loading.value = false }
}
async function restore() {
  if (!selected.value || !props.draft || restoring.value) return
  const id = props.draft.id, version = selected.value.version
  try { await ElMessageBox.confirm('将恢复此版本的标题和正文。当前稿会自动留存，关联案件和文书状态保持不变。', '恢复历史版本', { type: 'warning', confirmButtonText: '恢复此版本', cancelButtonText: '保留当前稿' }) } catch { return }
  if (!props.modelValue || props.draft?.id !== id || selected.value?.version !== version) return
  restoring.value = true
  try {
    if (!await props.beforeRestore() || props.draft?.id !== id) return
    const result = await casyContext.docs.restoreDraftVersion(id, version, props.draft.version)
    if (!result.ok || !result.data) throw new Error(result.error || '恢复失败')
    emit('restored', result.data)
    ElMessage.success('历史版本已恢复，恢复前的稿件已留存')
    emit('update:modelValue', false)
  } catch (e) { error.value = String(e) }
  finally { restoring.value = false }
}
watch(() => [props.modelValue, props.draft?.id], () => { if (props.modelValue) void load(); else revision++ }, { immediate: true })
onBeforeUnmount(() => { revision++ })
</script>
<template>
  <el-dialog :model-value="modelValue" title="文书历史版本" width="960px" top="5vh" :close-on-click-modal="false" :close-on-press-escape="!restoring" :show-close="!restoring" @update:model-value="!restoring && emit('update:modelValue', $event)">
    <p class="history-help">每次标题或正文保存时，自动留存修改前的稿件。历史从本次升级后开始记录。</p>
    <el-alert v-if="error" :title="error" type="error" :closable="false"><el-button :disabled="restoring" text @click="load()">重新加载</el-button></el-alert>
    <div v-loading="loading" class="draft-history-layout">
      <nav aria-label="文书历史版本" class="version-list">
        <button v-for="item in versions" :key="item.version" :aria-pressed="selected?.version === item.version" :disabled="restoring" @click="selected = item"><strong>版本 {{ item.version }}</strong><span>{{ formatTimestamp(item.savedAt) }}</span><small>{{ item.title }}</small></button>
        <el-button v-if="more" :loading="loading" :disabled="restoring" @click="load(true)">加载更早版本</el-button>
      </nav>
      <article v-if="selected" class="version-preview legal-document" aria-label="历史文书完整正文"><h3>{{ selected.title }}</h3><div v-if="selected.content" v-html="preview" /><p v-else>此版本正文为空</p></article>
      <el-empty v-else-if="!loading && !error" description="暂无历史版本；下次修改并保存后，可在这里找回当前稿。" />
    </div>
    <template #footer><el-button :disabled="restoring" @click="emit('update:modelValue', false)">关闭</el-button><el-button type="primary" :disabled="!selected || loading" :loading="restoring" @click="restore">恢复此版本</el-button></template>
  </el-dialog>
</template>
<style scoped>
.history-help{font-size:13px;color:var(--c-text-secondary);line-height:1.6;margin-top:0}.draft-history-layout{display:grid;grid-template-columns:200px minmax(0,1fr);gap:20px;min-height:240px;height:min(55vh,540px)}.version-list{overflow:auto}.version-list>button:not(.el-button){display:flex;flex-direction:column;gap:6px;padding:12px;width:100%;border:0;border-bottom:1px solid var(--c-border);background:transparent;color:var(--c-text);text-align:left;cursor:pointer}.version-list>button[aria-pressed=true]{background:var(--c-bg-selected);color:var(--c-primary)}.version-list span,.version-list small{font-size:12px;overflow-wrap:anywhere}.version-preview{padding:20px;border:1px solid var(--c-border);overflow:auto;overflow-wrap:anywhere;background:var(--c-bg-card);font-size:14px;line-height:1.8}.version-preview h3{margin-top:0}.version-preview :deep(img){max-width:100%}@media(max-width:650px){.draft-history-layout{grid-template-columns:1fr;height:auto}.version-list{max-height:150px}.version-preview{max-height:45vh}}
</style>
