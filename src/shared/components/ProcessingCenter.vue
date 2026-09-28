<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { sessionOperations, retrySessionOperation } from '../../stores/sessionOperations'
import { formatTimestamp } from '../utils/date'
import { Monitor } from '../icons'
import { useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import { tauriCallSafe } from '../../core/tauriBridge'
import { isTauriRuntime } from '../../core/mockData'
import { casyContext } from '../../core/plugin/context'
import type { ProcessingCenter, ProcessingJob } from '../../types/bindings'
const emit = defineEmits<{ conversion: [] }>()
const router = useRouter()
const open = ref(false), filter = ref('all'), page = ref(1), error = ref(''), loading = ref(false)
const state = ref<ProcessingCenter>({ jobs: [], services: [], total: 0, active: 0, failed: 0 })
const localActive = computed(() => sessionOperations.filter(job => job.status === 'running').length)
const localFailed = computed(() => sessionOperations.filter(job => job.status === 'failed').length)
const localJobs = computed(() => sessionOperations.filter(job => filter.value === 'all' || (filter.value === 'active' ? job.status === 'running' : filter.value === 'failed' ? job.status === 'failed' : filter.value === 'completed' ? ['completed','cancelled'].includes(job.status) : false)))
async function revealLocal(path: string) {
  try { const result = await casyContext.files.reveal(path); if (!result.ok) throw new Error(result.error || '无法显示文件') }
  catch (cause) { ElMessage.error(String(cause)) }
}
const pageSize = 30
let timer: ReturnType<typeof setTimeout> | undefined
let disposed = false, busy = false, revision = 0
const pages = computed(() => Math.max(1, Math.ceil(state.value.total / pageSize)))
const kinds: Record<string,string> = { ai_detail: 'AI 调用记录', backup: '备份', ai: 'AI 处理', inbox: '收件箱导入', document: '文档识别 / PDF', knowledge: '知识索引', conversion: '文件转换', reminder: '提醒 / 日历', sync: '同步', scheduled: '定时任务', email: '邮件收取', calendar: '日历同步' }
const statuses: Record<string,string> = { queued:'已排队',running:'处理中',completed:'已完成',failed:'失败',cancelled:'已取消',stale:'需重建',waiting:'等待触发',disabled:'未启用' }
const stages: Record<string,string> = { preparing:'准备文件',rendering:'渲染页面',recognizing:'识别文字',analyzing_layout:'分析复杂版面',finalizing:'整理结果 / 生成 PDF',completed:'识别结果已保存',indexing:'建立文档索引',embedding:'建立知识向量索引' }
function duration(seconds: number) { const value=Math.max(0,Math.round(seconds)); return value<60?`${value} 秒`:`${Math.floor(value/60)} 分 ${value%60} 秒` }
function title(job: ProcessingJob) {
  if (job.kind !== 'ai_detail') return job.title
  return ({ai_chat:'AI 对话',daily_brief_narrative:'早报文字整理',weekly_brief_narrative:'周报文字整理',relation_insights:'关联分析'} as Record<string,string>)[job.title] || 'AI 内容处理'
}
function stage(job: ProcessingJob) {
  if (job.status === 'completed') return ({document:'识别结果已保存',conversion:'Markdown 已保存',knowledge:'知识索引已完成'} as Record<string,string>)[job.kind] || '本次处理已结束'
  if (job.kind === 'reminder') {
    const [channel,...rest] = job.stage.split(' · ')
    const label = ({local:'本机提醒',system:'系统通知',calendar:'日历同步',email_ics:'邮件邀请',feishu_message:'飞书消息',feishu_task:'飞书任务'} as Record<string,string>)[channel || ''] || '提醒'
    return [label,...rest].join(' · ')
  }
  return stages[job.stage] || job.stage
}
async function refresh() {
  if (disposed || busy || !isTauriRuntime()) return
  clearTimeout(timer); busy = true; loading.value = true
  const current = revision
  try {
    const result = await tauriCallSafe('get_processing_center', { filter: filter.value, offset: (page.value-1)*pageSize, limit: pageSize })
    if (disposed || current !== revision) return
    if (result.ok && result.data) {
      state.value = result.data; error.value = ''
      if (page.value > pages.value) { page.value = pages.value; revision++ }
    } else error.value = result.error || '后台状态读取失败'
  } catch (e) {
    if (!disposed && current === revision) error.value = String(e)
  } finally {
    busy = false
    if (!disposed) { loading.value = false; timer = setTimeout(refresh, current !== revision ? 0 : document.hidden ? 60000 : open.value ? 2000 : state.value.active > 0 ? 5000 : 60000) }
  }
}
function changeFilter(value: string) { filter.value=value; page.value=1; revision++; void refresh() }
function changePage(value: number) { page.value=value; revision++; void refresh() }
function show() { open.value=true; void refresh() }
async function locate(job: ProcessingJob) {
  open.value=false
  if (job.fileId && job.caseId) await router.push({ path:`/files/${job.caseId}`, query:{select:job.fileId} })
  else if (job.knowledgeId) await router.push({ name:'knowledge', query:{select:job.knowledgeId} })
  else if (job.kind === 'conversion') emit('conversion')
  else if (job.caseId) await router.push(`/cases/${job.caseId}`)
}
const pendingJobs = ref(new Set<string>())
function jobKey(job: ProcessingJob) { return `${job.kind}:${job.id}` }
async function act(job: ProcessingJob, retry = false) {
  const key = jobKey(job)
  if (pendingJobs.value.has(key)) return
  pendingJobs.value.add(key)
  try {
    const result = retry ? await tauriCallSafe('retry_document_job', { jobId: job.id })
      : job.kind === 'conversion' ? await tauriCallSafe('cancel_conversion', { jobId: job.id })
      : job.kind === 'document' ? await tauriCallSafe('cancel_document_job', { jobId: job.id })
      : await tauriCallSafe('cancel_knowledge_index_job', { jobId: job.id })
    if (!result.ok) throw new Error(result.error || (retry ? '重试失败' : '取消失败'))
    ElMessage.success(retry ? '已重新提交处理' : '已请求取消，请等待任务结束')
    await refresh()
  } catch (cause) { ElMessage.error(String(cause)) }
  finally { pendingJobs.value.delete(key) }
}
async function reveal(job: ProcessingJob) {
  if (!job.outputPath) return
  try {
    const result = await casyContext.files.reveal(job.outputPath)
    if (!result.ok) throw new Error(result.error || '无法显示结果文件')
  } catch(e) { ElMessage.error(String(e)) }
}
onMounted(() => { void refresh() })
onUnmounted(() => { disposed=true; revision++; clearTimeout(timer) })
</script>
<template>
  <button class="processing-trigger btn-secondary" type="button" aria-label="处理中心" @click="show">
    <el-icon :size="16"><Monitor /></el-icon>
    <span class="processing-label">处理中心</span>
    <span v-if="state.active + localActive > 0 || error || state.failed + localFailed > 0" :class="['processing-dot', { active:state.active + localActive > 0, problem:error || state.failed + localFailed > 0 }]" /> <strong v-if="state.active + localActive">{{ state.active + localActive }}</strong><span v-else-if="error">!</span>
  </button>
  <el-drawer v-model="open" title="处理中心" size="min(680px, 100vw)" append-to-body>
    <div class="processing-head">
      <p>全部案件与文件 · {{ state.active + localActive }} 项正在处理或排队 · {{ state.failed + localFailed }} 项失败或需重建</p>
      <el-button :loading="loading" @click="refresh">刷新</el-button>
    </div>
    <p class="processing-note">添加文件不等于已开始 OCR。请在文件详情点击“开始识别并生成可搜索 PDF”，或在设置中启用目录自动识别。识别保留原件。</p>
    <el-alert v-if="error" :title="error" type="error" :closable="false" description="暂时无法更新，以下可能是上一次读取的状态。" />
    <nav class="processing-filters" aria-label="任务筛选">
      <el-button v-for="item in [{id:'all',label:'全部'},{id:'active',label:'处理中'},{id:'failed',label:'失败 / 需重建'},{id:'waiting',label:'等待触发'},{id:'completed',label:'已结束'}]" :key="item.id" :type="filter === item.id ? 'primary' : 'default'" @click="changeFilter(item.id)">{{ item.label }}</el-button>
    </nav>
    <section v-if="localJobs.length" aria-label="本次会话操作">
      <h3>本次会话</h3>
      <article v-for="job in localJobs" :key="job.id" class="processing-job">
        <div class="job-heading"><strong>{{ job.title }}</strong><span :class="['job-status',job.status]">{{ statuses[job.status] }}</span></div>
        <p v-if="job.status === 'running'">正在处理，完成后会在此显示结果。此操作不支持中途取消。</p>
        <p v-if="job.error" class="job-error">{{ job.error }}</p>
        <p class="job-time">开始 {{ formatTimestamp(job.startedAt) }} · 更新 {{ formatTimestamp(job.updatedAt) }}</p>
        <div class="job-actions">
          <el-button v-if="job.outputPath" size="small" @click="revealLocal(job.outputPath)">显示结果文件</el-button>
          <el-button v-if="job.retry" size="small" @click="retrySessionOperation(job)">重新导出…</el-button>
          <el-button v-if="job.status === 'failed' && !job.retry" size="small" @click="router.push('/settings'); open = false">前往设置重新操作</el-button>
        </div>
      </article>
    </section>
    <p v-if="!state.jobs.length && !localJobs.length && !error" class="processing-empty">{{ loading ? '正在读取任务…' : '此筛选下暂无任务' }}</p>
    <ol class="processing-jobs" aria-label="后台任务">
      <li v-for="job in state.jobs" :key="`${job.kind}:${job.id}`" class="processing-job">
        <div class="job-heading"><strong>{{ title(job) }}</strong><span :class="['job-status',job.status]">{{ statuses[job.status] || job.status }}</span></div>
        <p>{{ kinds[job.kind] || job.kind }}<span v-if="job.caseName"> · {{ job.caseName }}</span></p>
        <p>{{ stage(job) }}<span v-if="job.total > 0"> · {{ job.current }} / {{ job.total }} {{ job.kind === 'knowledge' ? '段' : '页' }}</span></p>
        <p v-if="job.elapsedSeconds > 0" class="job-time">已用 {{ duration(job.elapsedSeconds) }}<template v-if="job.remainingSeconds != null"> · 预计剩余 {{ duration(job.remainingSeconds) }}</template><template v-if="job.pageTiming"> · 最近一页 {{ duration(job.pageTiming.totalMs/1000) }}</template></p>
        <el-progress v-if="job.status==='running' && job.total > 0 && job.stage !== 'indexing'" :percentage="Math.min(99,Math.max(0,Math.round(job.progress*100)))" :stroke-width="5" />
        <p v-if="job.error" class="job-error">{{ job.error }}</p>
        <p class="job-time">创建 {{ formatTimestamp(job.createdAt) }} · 更新 {{ formatTimestamp(job.updatedAt) }}</p>
        <div class="job-actions">
          <el-button v-if="job.fileId || job.knowledgeId || job.caseId || job.kind==='conversion'" size="small" @click="locate(job)">{{ job.kind==='conversion' ? '打开转换' : '查看来源' }}</el-button>
          <el-button v-if="job.outputPath" size="small" @click="reveal(job)">显示结果文件</el-button>
          <el-button v-if="job.canCancel && ['conversion','document','knowledge'].includes(job.kind)" size="small" :loading="pendingJobs.has(jobKey(job))" @click="act(job)">取消处理</el-button>
          <el-button v-if="job.kind === 'document' && ['failed','cancelled'].includes(job.status)" size="small" :loading="pendingJobs.has(jobKey(job))" @click="act(job, true)">重新处理</el-button>
          <span v-if="job.status === 'running' && !job.canCancel" class="job-time">此操作暂不支持中途取消</span>
        </div>
      </li>
    </ol>
    <div class="processing-pages"><el-button :disabled="page <= 1" @click="changePage(page-1)">上一页</el-button><span>{{ page }} / {{ pages }} 页 · {{ state.total }} 项</span><el-button :disabled="page >= pages" @click="changePage(page+1)">下一页</el-button></div>
    <details class="processing-services" open>
      <summary>后台服务 · {{ state.services.length }} 项</summary>
      <p class="processing-note">周期服务显示最近一次检查与等待计划；等待触发不计入处理数量。</p>
      <div v-for="service in state.services" :key="service.id" class="processing-job">
        <div class="job-heading"><strong>{{ service.title }}</strong><span :class="['job-status',service.status]">{{ statuses[service.status] || service.status }}</span></div>
        <p>{{ service.stage }}</p><p v-if="service.error" class="job-error">{{ service.error }}</p><p class="job-time">{{ formatTimestamp(service.updatedAt) }}</p>
      </div>
    </details>
  </el-drawer>
</template>
<style scoped>
.processing-trigger{white-space:nowrap;display:inline-flex;align-items:center;justify-content:center;gap:7px;height:36px;padding:0 12px;border:1px solid var(--c-border);border-radius:var(--c-radius);background:var(--c-bg-card);color:var(--c-text-regular);font:inherit;font-size:13px;cursor:pointer}
.processing-trigger:hover{background:var(--c-bg-hover);border-color:var(--c-border-strong)}
@media(max-width:1100px){.processing-label{display:none}.processing-trigger{padding:0 10px}}
.processing-dot{width:7px;height:7px;border-radius:50%;background:var(--c-text-tertiary,#999)}.processing-dot.problem{background:var(--el-color-warning)}.processing-dot.active{background:var(--el-color-primary)}.processing-head{display:flex;align-items:center;justify-content:space-between;gap:12px}.processing-head p{margin:0}.processing-note,.processing-empty{font-size:12px;color:var(--c-text-secondary);line-height:1.7}.processing-filters{display:flex;flex-wrap:wrap;gap:6px;margin:18px 0}.processing-filters .el-button,.job-actions .el-button{margin:0}.processing-jobs{list-style:none;padding:0;margin:0}.processing-job{border-bottom:1px solid var(--c-border);padding:14px 0;overflow-wrap:anywhere}.job-heading{display:flex;align-items:baseline;justify-content:space-between;gap:12px}.job-heading strong{font-size:14px}.job-status{white-space:nowrap;font-size:12px;color:var(--c-text-secondary)}.job-status.running{color:var(--el-color-primary)}.job-status.failed,.job-status.stale,.job-error{color:var(--el-color-danger)}.processing-job p{font-size:12px;margin:7px 0}.job-time{color:var(--c-text-secondary)}.job-actions{display:flex;gap:8px;margin-top:10px}.processing-pages{display:flex;align-items:center;justify-content:space-between;gap:10px;margin:20px 0;font-size:12px}.processing-services{border-top:1px solid var(--c-border);padding-top:16px}.processing-services summary{cursor:pointer;font-size:14px;font-weight:600}
</style>
