<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { ElMessage } from 'element-plus'
import { Close, Refresh, Search, Setting } from '@element-plus/icons-vue'
import { tauriCallSafe } from '../../../core/tauriBridge'
import type { KnowledgeIndexJob, KnowledgeIndexStatus, KnowledgeSearchResponse } from '../../../types/knowledgeIndex'

const emit = defineEmits<{ navigate: [id: string]; settings: [] }>()
const tab = ref('search')
const query = ref('')
const mode = ref('keyword')
const response = ref<KnowledgeSearchResponse | null>(null)
const status = ref<KnowledgeIndexStatus | null>(null)
const error = ref('')
const statusError = ref('')
const searching = ref(false)
const mutating = ref(false)
const refreshing = ref(false)
let disposed = false
let sequence = 0
let timer: ReturnType<typeof setTimeout> | undefined
const stateLabels: Record<string, string> = {
  queued: '排队中', running: '正在处理', completed: '已完成', failed: '失败', cancelled: '已取消', stale: '需更新',
}
const semanticLabel = computed(() => ({
  ready: '混合检索', disabled: '关键词检索', not_configured: '关键词结果 · 向量接口未配置',
  not_indexed: '关键词结果 · 尚无当前模型的索引', unavailable: '关键词结果 · 向量接口不可用',
}[response.value?.semanticStatus || ''] || ''))

async function refresh() {
  if (refreshing.value || disposed) return
  clearTimeout(timer)
  refreshing.value = true
  const result = await tauriCallSafe('get_knowledge_index_status', {})
  refreshing.value = false
  if (disposed) return
  statusError.value = result.ok ? '' : result.error || '读取索引状态失败'
  if (result.ok) status.value = result.data!
  timer = setTimeout(refresh, 3000)
}

async function search() {
  const text = query.value.trim()
  if (!text) return
  const request = ++sequence
  searching.value = true
  error.value = ''
  response.value = null
  const result = await tauriCallSafe('search_knowledge_index', { query: text, useSemantic: mode.value === 'hybrid' })
  if (disposed || request !== sequence) return
  searching.value = false
  if (result.ok) response.value = result.data!
  else error.value = result.error || '检索失败'
}

async function queueAll() {
  if (mutating.value) return
  mutating.value = true
  const result = await tauriCallSafe('embed_all_knowledge', {})
  mutating.value = false
  if (disposed) return
  if (!result.ok) error.value = result.error || '提交索引任务失败'
  else {
    error.value = ''
    ElMessage.success(`新增 ${result.data!.queued} 项，已有 ${result.data!.upToDate} 项最新、${result.data!.alreadyQueued} 项排队或处理中`)
  }
  await refresh()
}

async function act(job: KnowledgeIndexJob, cancel: boolean) {
  if (mutating.value) return
  mutating.value = true
  const result = cancel
    ? await tauriCallSafe('cancel_knowledge_index_job', { jobId: job.id })
    : await tauriCallSafe('embed_knowledge', { itemId: job.itemId, force: job.status === 'completed' })
  mutating.value = false
  if (disposed) return
  error.value = result.ok ? '' : result.error || '操作失败'
  await refresh()
}

onMounted(refresh)
onBeforeUnmount(() => { disposed = true; ++sequence; clearTimeout(timer) })
</script>

<template>
  <div class="knowledge-search-panel">
    <el-tabs v-model="tab">
      <el-tab-pane label="检索" name="search" />
      <el-tab-pane label="索引任务" name="index" />
    </el-tabs>
    <el-alert v-if="error" :title="error" type="error" :closable="false" />
    <template v-if="tab === 'search'">
      <form class="query-form" @submit.prevent="search">
        <el-radio-group v-model="mode" size="small" aria-label="检索方式">
          <el-radio-button value="keyword">关键词</el-radio-button>
          <el-radio-button value="hybrid">混合检索</el-radio-button>
        </el-radio-group>
        <div class="query-row">
          <el-input v-model="query" placeholder="关键词或案情问题" aria-label="知识库检索问题" :maxlength="500" clearable />
          <el-button native-type="submit" type="primary" :icon="Search" :loading="searching" :disabled="!query.trim()">检索</el-button>
        </div>
      </form>
      <el-alert v-if="response?.warning" :title="response.warning" type="warning" :closable="false" />
      <div v-if="response" class="result-count" role="status">{{ response.results.length }} 条结果 · {{ semanticLabel }}</div>
      <div v-if="response && !response.results.length" class="empty-state">没有找到相关笔记</div>
      <div class="knowledge-hits">
        <button v-for="hit in response?.results || []" :key="hit.id" class="knowledge-hit" @click="emit('navigate', hit.id)">
          <span class="hit-heading"><strong>{{ hit.title }}</strong><small>{{ { fts: '关键词', semantic: '语义', hybrid: '混合' }[hit.source] || hit.source }}</small></span>
          <p>{{ hit.content }}</p>
        </button>
      </div>
    </template>
    <template v-else>
      <div class="index-actions">
        <el-button type="primary" :icon="Refresh" :loading="mutating" :disabled="!status?.configured" @click="queueAll">更新全部索引</el-button>
        <el-button :icon="Setting" @click="emit('settings')">向量接口</el-button>
        <el-button :icon="Refresh" :loading="refreshing" title="刷新索引状态" aria-label="刷新索引状态" @click="refresh" />
      </div>
      <el-alert v-if="statusError" :title="statusError" type="error" :closable="false" />
      <div v-if="status" class="index-counts" role="status">
        <span>已索引 <b>{{ status.indexed }} / {{ status.total }}</b></span>
        <span>排队 <b>{{ status.queued }}</b></span>
        <span>处理 <b>{{ status.running }}</b></span>
        <span>失败 <b>{{ status.failed }}</b></span>
      </div>
      <div v-if="status && !status.configured" class="empty-state">尚未配置向量接口</div>
      <div v-else-if="status && !status.jobs.length" class="empty-state">暂无索引任务</div>
      <div v-for="job in status?.jobs || []" :key="job.id" class="index-job" :data-status="job.status">
        <div class="job-main">
          <button class="job-title" @click="emit('navigate', job.itemId)">{{ job.title }}</button>
          <span class="job-state">{{ stateLabels[job.status] }} · {{ job.completedChunks }} / {{ job.totalChunks }} 段 · {{ job.model }}</span>
          <el-progress v-if="job.status === 'running'" :percentage="job.totalChunks ? Math.floor(job.completedChunks / job.totalChunks * 100) : 0" />
          <span v-if="job.error" class="job-error">{{ job.error }}</span>
        </div>
        <el-button v-if="['queued', 'running'].includes(job.status)" :icon="Close" :disabled="mutating" title="取消索引" aria-label="取消索引" @click="act(job, true)" />
        <el-button v-else :icon="Refresh" :disabled="mutating || !status?.configured" :title="job.status === 'completed' ? '重建索引' : '重试索引'" :aria-label="job.status === 'completed' ? '重建索引' : '重试索引'" @click="act(job, false)" />
      </div>
    </template>
  </div>
</template>

<style scoped>
.knowledge-search-panel{min-width:0;color:var(--c-text)}
.query-form{display:grid;gap:12px;margin:12px 0 18px}.query-row{display:flex;gap:8px}.query-row .el-input{min-width:0}
.result-count,.job-state{color:var(--c-text-secondary);font-size:12px;line-height:1.6}.result-count{margin:14px 0 4px}
.knowledge-hit{display:block;width:100%;padding:16px 0;border:0;border-bottom:1px solid var(--c-border);text-align:left;background:transparent;color:inherit;cursor:pointer}.knowledge-hit:hover strong,.job-title:hover{color:var(--c-primary)}
.hit-heading{display:flex;gap:12px;align-items:baseline}.hit-heading strong{font-size:15px;overflow-wrap:anywhere;flex:1;min-width:0}.hit-heading small{font-size:11px;color:var(--c-text-secondary);white-space:nowrap}
.knowledge-hit p{white-space:pre-wrap;overflow-wrap:anywhere;font-size:13px;line-height:1.7;margin:8px 0 0;max-height:13em;overflow:auto}
.index-actions{display:flex;flex-wrap:wrap;gap:8px;margin:8px 0 16px}.index-actions .el-button{margin:0}
.index-counts{display:flex;flex-wrap:wrap;gap:12px;padding:12px 0;border-bottom:1px solid var(--c-border);font-size:12px}.index-counts b{color:var(--c-primary);font-variant-numeric:tabular-nums}
.index-job{display:flex;gap:12px;align-items:center;padding:14px 0;border-bottom:1px solid var(--c-border)}.job-main{display:grid;gap:5px;flex:1;min-width:0;overflow-wrap:anywhere}.job-title{border:0;padding:0;text-align:left;color:inherit;background:transparent;font:inherit;cursor:pointer;overflow-wrap:anywhere}.job-error{font-size:12px;color:var(--el-color-danger)}
.empty-state{padding:28px 0;color:var(--c-text-secondary);text-align:center;font-size:13px}
</style>
