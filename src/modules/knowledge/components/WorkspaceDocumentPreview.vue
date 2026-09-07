<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { Document, Refresh, EditPen, FolderOpened } from '@element-plus/icons-vue'
import { tauriCallSafe } from '../../../core/tauriBridge'
import { casyContext } from '../../../core/plugin/context'
import { mdToSafeHtml } from '../../../shared/markdown/mdBridge'
import DocumentSourceViewer from '../../files/components/DocumentSourceViewer.vue'
import type { CommandMap } from '../../../types/commandMap'
type Source = CommandMap['list_workspace_sources']['result'][number]
const props = defineProps<{ source: Source }>()
const emit = defineEmits<{ refreshed: []; note: [id: string] }>()
const document = ref<CommandMap['get_workspace_document']['result'] | null>(null)
const error = ref(''), loading = ref(false), processing = ref(false), viewer = ref(false)
const initialPage = ref(1)
const locations = ref<import('../../../types/documentRetrieval').SourceLocation[]>([])
let revision = 0
const html = computed(() => mdToSafeHtml(document.value?.markdown || ''))
watch(() => [props.source.fileId, props.source.jobId, props.source.status], load, { immediate: true })
async function load() {
  const request = ++revision
  document.value = null; error.value = ''; loading.value = false
  if (props.source.status !== 'completed') return
  loading.value = true
  const result = await tauriCallSafe('get_workspace_document', { fileId: props.source.fileId })
  if (request !== revision) return
  loading.value = false
  if (result.ok) document.value = result.data!
  else error.value = result.error || '读取正文失败'
}
async function process() {
  processing.value = true
  const result = await tauriCallSafe('queue_document_processing', { fileId: props.source.fileId })
  processing.value = false
  if (!result.ok) ElMessage.error(result.error || '提交失败')
  else emit('refreshed')
}
async function editCopy() {
  processing.value = true
  const result = await casyContext.knowledge.importPageIndex(props.source.fileId)
  processing.value = false
  if (result.ok && result.data) emit('note', result.data.knowledgeId)
  else ElMessage.error(result.error || '创建知识快照失败')
}
</script>
<template>
  <section class="workspace-document" v-loading="loading">
    <header><h2>{{ source.fileName }}</h2><span>{{ source.totalPages || 0 }} 页 / 段</span></header>
    <div class="document-actions">
      <el-button :icon="Document" :disabled="!source.jobId || source.status !== 'completed'" @click="initialPage=1;locations=[];viewer=true">原文对照</el-button>
      <el-button :icon="EditPen" :disabled="!document" :loading="processing" @click="editCopy">编辑知识快照</el-button>
      <el-button :icon="FolderOpened" title="定位原文件" aria-label="定位原文件" @click="casyContext.files.reveal(source.filePath)" />
      <el-button :icon="Refresh" title="刷新正文" aria-label="刷新正文" @click="load" />
    </div>
    <el-alert v-if="error || source.error || source.missing" :title="error || source.error || '原文件缺失'" type="warning" :closable="false" />
    <div v-if="document?.continuations?.length" class="continuations"><el-button v-for="link in document.continuations" :key="link.fromPage" link @click="initialPage=link.fromPage;locations=link.locations;viewer=true">可能续接：第 {{ link.fromPage }} / {{ link.toPage }} 页</el-button></div>
    <article v-if="document" class="source-markdown-preview" v-html="html" />
    <div v-else class="document-pending"><span>{{ ({ queued: '等待提取正文', running: '正在提取正文', failed: '正文提取失败', cancelled: '已取消' } as Record<string,string>)[source.status || ''] || '尚未提取正文' }}</span><el-button v-if="!['queued','running'].includes(source.status || '')" :loading="processing" @click="process">提取正文</el-button></div>
    <DocumentSourceViewer v-if="source.jobId" v-model="viewer" :file-id="source.fileId" :job-id="source.jobId" :initial-page="initialPage" :locations="locations" />
  </section>
</template>
<style scoped>
.workspace-document{height:100%;min-height:0;display:flex;flex-direction:column;overflow:hidden}.workspace-document header{padding:16px 20px 8px;display:flex;align-items:center;gap:12px}.workspace-document h2{font-size:18px;line-height:1.5;margin:0;flex:1;overflow-wrap:anywhere}.workspace-document header span{font-size:11px;color:var(--c-text-secondary);white-space:nowrap}.document-actions{display:flex;flex-wrap:wrap;gap:6px;padding:8px 20px 14px;border-bottom:1px solid var(--c-border)}.document-actions .el-button{margin:0}.source-markdown-preview{flex:1;min-height:0;overflow:auto;padding:24px 32px;overflow-wrap:anywhere}.source-markdown-preview :deep(table){border-collapse:collapse;width:100%}.source-markdown-preview :deep(img){max-width:100%}.source-markdown-preview :deep(pre){overflow:auto}.document-pending{padding:24px;display:flex;align-items:center;gap:16px}
@media(max-width:600px){.source-markdown-preview{padding:16px}.workspace-document header{flex-wrap:wrap}}
</style>
