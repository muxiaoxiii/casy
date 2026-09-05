<script setup>
import { computed, onMounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { Document, Download, FolderOpened, Search, Refresh } from '@element-plus/icons-vue'
import { casyContext } from '../../../core/plugin/context'

const emit = defineEmits(['imported', 'navigate'])
const sources = ref([])
const loading = ref(false)
const loadError = ref('')
const importingId = ref('')
const query = ref('')
const filtered = computed(() => {
  const q = query.value.trim().toLowerCase()
  return sources.value.filter(item => !q || `${item.fileName} ${item.caseName}`.toLowerCase().includes(q))
})

async function load() {
  loading.value = true
  const result = await casyContext.knowledge.documentSources()
  loadError.value = result.ok ? '' : (result.error || '无法加载文档资料')
  sources.value = result.ok && Array.isArray(result.data) ? result.data : []
  loading.value = false
}

async function importSource(source) {
  if (source.importedKnowledgeId) return emit('navigate', source.importedKnowledgeId)
  importingId.value = source.fileId
  const result = await casyContext.knowledge.importPageIndex(source.fileId)
  importingId.value = ''
  if (!result.ok) return ElMessage.error(result.error || '沉淀失败')
  ElMessage.success(result.data?.reused ? '已打开既有知识树' : `已沉淀全文和 ${result.data?.childCount || 0} 个结构节点`)
  await load()
  emit('imported', result.data?.knowledgeId)
}

onMounted(load)
defineExpose({ reload: load })
</script>

<template>
  <div class="import-panel">
    <div class="import-title"><Download /> 文档资料<el-button :icon="Refresh" text :loading="loading" title="刷新文档资料" aria-label="刷新文档资料" @click="load" /></div>
    <div class="source-search"><Search /><input v-model="query" placeholder="搜索文件或案件" /></div>
    <div v-loading="loading" class="source-list">
      <el-alert v-if="loadError" :title="loadError" type="error" :closable="false" />
      <div v-for="source in filtered" :key="source.fileId" class="source-item">
        <div class="source-icon"><Document /></div>
        <div class="source-main"><strong>{{ source.fileName }}</strong><span><FolderOpened /> {{ source.caseName }}</span><small>{{ source.totalPages }} {{ source.searchablePdfPath ? '页' : '段' }} · {{ source.markdownPath ? 'Markdown 已就绪' : '正文已就绪' }}</small></div>
        <el-button size="small" :type="source.importedKnowledgeId ? 'success' : 'primary'" :plain="!!source.importedKnowledgeId" :loading="importingId === source.fileId" @click="importSource(source)">{{ source.importedKnowledgeId ? '打开' : '沉淀' }}</el-button>
      </div>
      <div v-if="!loading && !loadError && !filtered.length" class="empty">暂无已处理的文档</div>
    </div>
  </div>
</template>

<style scoped>
.import-panel{height:100%;display:flex;flex-direction:column;padding:14px;box-sizing:border-box}.import-title{display:flex;align-items:center;gap:6px;font-size:12px;font-weight:700}.import-title svg{width:14px}.import-help{font-size:10px;line-height:1.55;color:var(--c-text-secondary);margin:8px 0 10px}.source-search{display:flex;align-items:center;gap:6px;border:1px solid var(--c-border);border-radius:7px;padding:6px 8px}.source-search svg{width:12px;color:var(--c-text-secondary)}.source-search input{width:100%;border:0;outline:0;background:transparent;color:inherit;font-size:11px}.source-list{flex:1;overflow:auto;margin-top:9px}.source-item{display:grid;grid-template-columns:28px 1fr auto;align-items:center;gap:8px;padding:9px 3px;border-bottom:1px solid var(--c-border)}.source-icon{width:26px;height:30px;border-radius:5px;background:rgba(180,85,79,.1);color:#b4554f;display:grid;place-items:center}.source-icon svg{width:14px}.source-main{min-width:0;display:flex;flex-direction:column;gap:2px}.source-main strong{font-size:11px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.source-main span,.source-main small{display:flex;align-items:center;gap:3px;font-size:9px;color:var(--c-text-secondary)}.source-main span svg{width:10px}.empty{padding:18px 4px;text-align:center;font-size:10px;line-height:1.55;color:var(--c-text-secondary)}
</style>
