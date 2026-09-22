<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage, ElMessageBox } from 'element-plus'
import { tauriCallSafe } from '../../../core/tauriBridge'
import type { ProcedureBoard, ProcedureEvent, ProcedureItem, ProcedureAudit } from '../../../types/bindings'
const router = useRouter()
const props = defineProps<{ caseId: string }>()
const emit = defineEmits<{ openCase: [id: string]; changed: [] }>()
const board = ref<ProcedureBoard | null>(null)
const includeRelated = ref(true)
const loading = ref(false)
const error = ref('')
const filter = ref('open')
const editor = ref(false)
const saving = ref(false)
const reason = ref('')
const history = ref<ProcedureAudit[]>([])
const showHistory = ref(false)
let loadToken = 0
const kinds = [
  ['invalidation_filed', '提出无效请求 · 补充理由/证据', 'patent_invalidation', '请求人'],
  ['supplement_filed', '提交补充材料 · 等待转送', 'patent_invalidation', '请求人'],
  ['patentee_notice', '向专利权人转送 · 指定答复', 'patent_invalidation', '专利权人'],
  ['petitioner_notice', '向请求人转送 · 指定答复', 'patent_invalidation', '请求人'],
  ['invalidation_decision_served', '无效决定送达 · 起诉救济', 'patent_invalidation', '我方'],
  ['complaint_filed', '提交起诉状 · 等待送达', 'litigation', '原告'],
  ['civil_complaint_served', '民事起诉状送达被告 · 答辩', 'civil_tort', '被告'],
  ['admin_complaint_served', '行政起诉状送达被告 · 答辩及举证', 'admin_litigation', '被告'],
  ['judgment_served', '可上诉的一审判决送达', 'litigation', '我方'],
  ['ruling_served', '可上诉的一审裁定送达', 'litigation', '我方'],
  ['document_received', '收文登记 · 每份材料单独记录', 'all', '我方'],
  ['document_forwarded', '转文登记 · 待核送达', 'all', '对方'],
  ['summons', '传票 / 口审通知 · 关联排期', 'all', '我方'],
  ['hearing_change', '延期 / 改期 / 取消通知', 'all', '我方'],
  ['notice', '其他指定期限 · 举证/回执/补正等', 'all', '我方'],
  ['monitor', '查阅 / 跟进 / 统筹检查', 'all', '我方'],
]
const roles = ['请求人', '专利权人', '原告', '被告', '第三人', '上诉人', '被上诉人', '我方', '对方', '法院/国知局', '待确认']
function today() { const d = new Date(); return `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}` }
function blank(): ProcedureEvent { return { id:'',caseId:props.caseId,kind:'monitor',title:'查阅 / 跟进',actorRole:'我方',occurredOn:today(),forwardedOn:null,startOn:null,dueOn:null,periodValue:null,periodUnit:null,internalOn:null,nextCheckOn:null,parentId:null,fileId:null,hearingId:null,legacyKey:null,scope:'notice_only',basisConfirmed:false,sourceNote:'',revision:0,retracted:false } }
const form = ref<ProcedureEvent>(blank())
const currentCase = computed(() => board.value?.cases.find(c => c.id === form.value.caseId))
const availableKinds = computed(() => kinds.filter(k => k[2]==='all' || k[2]===currentCase.value?.track || (k[2]==='litigation' && ['civil_tort','admin_litigation'].includes(currentCase.value?.track || ''))))
const sourceFiles = ref<{id:string;fileName:string}[]>([])
const sourceHearings = ref<{id:string;hearingName?:string|null;hearingDate:string}[]>([])
let sourceLoad=0
watch([editor,()=>form.value.caseId], async ([opened]) => {
  const token=++sourceLoad;sourceFiles.value=[];sourceHearings.value=[]
  if(!opened)return
  const id=form.value.caseId
  const [files,hearings]=await Promise.all([tauriCallSafe('list_case_files',{caseId:id,category:null}),tauriCallSafe('list_case_hearings',{caseId:id})])
  if(token!==sourceLoad)return
  if(files.ok)sourceFiles.value=files.data || []
  if(hearings.ok)sourceHearings.value=hearings.data || []
})
const parentOptions = computed(() => board.value?.events.filter(e=>e.caseId===form.value.caseId && !e.retracted && !e.id.startsWith('legacy:') && e.id!==form.value.id) || [])
const visibleItems = computed(() => (board.value?.items || []).filter(i => filter.value==='all' || (filter.value==='review' ? i.needsReview && i.status==='open' : filter.value==='ours' ? i.owner==='ours' && i.status==='open' : i.status==='open')))
const counts = computed(() => {
  const open=board.value?.items.filter(i=>i.status==='open') || []
  return {open:open.length,review:open.filter(i=>i.needsReview).length,overdue:open.filter(i=>i.daysLeft!==null && i.daysLeft<0).length}
})
const ownerLabel: Record<string,string> = {ours:'我方办理',opponent:'对方期限',other:'其他 / 身份待核对'}
const sourceLabel: Record<string,string> = {statutory:'法定计算',specified:'通知指定',internal:'内部安排',unconfirmed:'待确认计算',legacy:'旧字段待核对',recorded:'已有记录'}
const statusLabel: Record<string,string> = {open:'待处理',done:'已处理',not_applicable:'不适用',case_closed:'案件已结'}
async function load() {
  const token=++loadToken; const id=props.caseId; loading.value=true; error.value='';board.value=null
  const r=await tauriCallSafe('get_procedure_board',{caseId:id,includeRelated:includeRelated.value})
  if(token!==loadToken)return
  loading.value=false
  if(r.ok && r.data)board.value=r.data; else {board.value=null;error.value=r.error || '读取程序事项失败'}
}
watch([()=>props.caseId,includeRelated],()=>{editor.value=false;showHistory.value=false;void load()},{immediate:true})
function chooseKind() {
  const k=kinds.find(k=>k[0]===form.value.kind)
  if(k){form.value.title=k[1];form.value.actorRole=k[3]}
  form.value.basisConfirmed=false
}
function create() { form.value=blank(); reason.value='首次登记';editor.value=true }
function edit(e:ProcedureEvent) {
  form.value={...e}
  if(e.id.startsWith('legacy:')) {form.value.id='';form.value.revision=0;form.value.basisConfirmed=false;form.value.title=e.title.replace('旧字段核对 · ','')}
  reason.value=e.id.startsWith('legacy:')?'核对旧字段并接管程序事项':''
  editor.value=true
}
function editItem(i:ProcedureItem) { const e=board.value?.events.find(e=>e.id===i.eventId);if(e)edit(e) }
function follow(e:ProcedureEvent) {
  form.value=blank();form.value.caseId=e.caseId;form.value.parentId=e.id
  form.value.kind=e.kind==='complaint_filed'?(board.value?.cases.find(c=>c.id===e.caseId)?.track==='admin_litigation'?'admin_complaint_served':'civil_complaint_served'):['invalidation_filed','supplement_filed'].includes(e.kind)?'patentee_notice':'document_received'
  chooseKind();reason.value='登记后续转送/送达';editor.value=true
}
async function save() {
  if(saving.value)return
  saving.value=true
  const target=props.caseId
  const cleaned={...form.value}
  for(const key of ['forwardedOn','startOn','dueOn','internalOn','nextCheckOn','parentId','legacyKey','fileId','hearingId'] as const)cleaned[key]=cleaned[key] || null
  if(!cleaned.periodValue){cleaned.periodValue=null;cleaned.periodUnit=null}
  const r=await tauriCallSafe('save_procedure_event',{event:cleaned,reason:reason.value})
  saving.value=false
  if(target!==props.caseId)return
  if(r.ok){editor.value=false;ElMessage.success('程序事件已保存，期限与处理状态已重算');await load();emit('changed')}else ElMessage.error(r.error)
}
async function setState(i:ProcedureItem,status:string) {
  try {
    const {value}=await ElMessageBox.prompt(status==='done'?'请记录提交凭证或本次查阅结果。查不到补充材料不等于不存在，必要时另定复查日。':'请填写重新办理或不适用的原因。','处理程序事项',{inputType:'textarea',inputValidator:v=>!!v?.trim()||'请填写处理依据',confirmButtonText:'保存',cancelButtonText:'取消'})
    const r=await tauriCallSafe('set_procedure_item_state',{caseId:i.caseId,itemId:i.id,fingerprint:i.fingerprint,status,note:value})
    if(r.ok)await load();else ElMessage.error(r.error)
  } catch { /* user cancelled */ }
}
async function openHistory() {
  const id=props.caseId
  const r=await tauriCallSafe('get_procedure_history',{caseId:id})
  if(id!==props.caseId)return
  if(r.ok && r.data){history.value=r.data;showHistory.value=true}else ElMessage.error(r.error)
}
function readableJson(s:string|null) {if(!s)return '无';try{return JSON.stringify(JSON.parse(s),null,2)}catch{return s}}
</script>

<template>
  <section class="procedure-board" v-loading="loading">
    <header class="board-head">
      <div><h3>程序期限与关联案件统筹</h3><p>按实际事件、我方身份和通知依据推进；各案分别起算，共享工作集中查看。</p></div>
      <div class="actions"><el-button @click="router.push({name:'case-detail',params:{id:props.caseId},query:{tab:'hearings'}})">历次庭审 / 改期</el-button><el-button @click="openHistory">修订记录</el-button><el-button @click="load">刷新</el-button><el-button type="primary" @click="create">登记程序事件</el-button></div>
    </header>
    <el-alert v-if="error" :title="error" type="error" :closable="false" />
    <div class="board-controls"><el-checkbox v-model="includeRelated">包括直接和间接关联案件</el-checkbox><span>{{ counts.open }} 项待处理 · {{ counts.review }} 项需核对 · {{ counts.overdue }} 项日期已过</span></div>
    <div class="case-strip"><button v-for="c in board?.cases" :key="c.id" class="case-chip" @click="emit('openCase',c.id)"><strong>{{ c.name }}</strong><span>我方：{{ c.ourRole }} · {{ c.track==='patent_invalidation'?'专利无效':c.track==='admin_litigation'?'行政诉讼':c.track==='civil_tort'?'民事诉讼':c.track }}</span></button></div>
    <el-alert v-for="tip in board?.coordination" :key="tip" :title="tip" type="warning" :closable="false" show-icon />
    <el-radio-group v-model="filter" class="filters"><el-radio-button value="open">全部待处理</el-radio-button><el-radio-button value="ours">我方办理</el-radio-button><el-radio-button value="review">需核对</el-radio-button><el-radio-button value="all">含已处理</el-radio-button></el-radio-group>
    <el-empty v-if="!loading&&!error&&!visibleItems.length" description="暂无程序事项。登记请求、送达通知或查阅计划后开始推进。" />
    <article v-for="i in visibleItems" :key="i.id" class="procedure-item" :class="{overdue:i.status==='open'&&i.daysLeft!==null&&i.daysLeft<0}">
      <div class="item-date"><strong>{{ i.dueOn || '尚无确定日期' }}</strong><small v-if="i.daysLeft!==null">{{ i.daysLeft<0?`已过 ${-i.daysLeft} 天`:i.daysLeft===0?'今天':`还有 ${i.daysLeft} 天` }}</small><small v-if="i.rawDueOn&&i.rawDueOn!==i.dueOn">原届满 {{ i.rawDueOn }}，已顺延</small></div>
      <div class="item-main"><div class="item-tags"><el-tag size="small" :type="i.owner==='ours'?'primary':'info'">{{ ownerLabel[i.owner] }} · {{ i.actorRole }}</el-tag><el-tag size="small" :type="i.needsReview?'warning':'info'">{{ sourceLabel[i.source] || i.source }}</el-tag><el-tag v-if="i.status!=='open'" size="small" type="success">{{ statusLabel[i.status] }}</el-tag></div>
        <h4>{{ i.title }}</h4><button class="case-link" @click="emit('openCase',i.caseId)">{{ i.caseName }} · 我方 {{ i.ourRole }}</button>
        <p>{{ i.explanation }}</p><p v-if="i.stateNote">处理记录：{{ i.stateNote }}</p>
        <details v-if="i.legalBasis"><summary>起算规则与依据</summary><p>{{ i.legalBasis }}</p><a v-if="i.legalUrl" :href="i.legalUrl" target="_blank" rel="noopener noreferrer">查看法规原文</a></details>
      </div>
      <div v-if="!i.eventId" class="item-actions"><el-button v-if="i.kind==='hearing'" size="small" @click="router.push({name:'case-detail',params:{id:i.caseId},query:{tab:'hearings'}})">维护本次排期</el-button><el-button v-if="i.kind==='task'" size="small" @click="router.push({name:'tasks',query:{edit:i.id.replace(/^task:/,'')}})">打开任务</el-button></div>
      <div v-if="i.eventId&&i.status!=='case_closed'" class="item-actions"><el-button size="small" @click="editItem(i)">{{ i.editable?'核对 / 修订':'核对旧字段' }}</el-button><el-button v-if="i.status==='open'" size="small" @click="setState(i,'done')">记录处理结果</el-button><el-button v-if="i.status==='open'" size="small" @click="setState(i,'not_applicable')">不适用</el-button><el-button v-else size="small" @click="setState(i,'open')">重新办理</el-button></div>
    </article>
    <details class="event-register" open><summary>程序事件登记簿（含撤销事件） · {{ board?.events.length || 0 }}</summary><div v-for="e in board?.events" :key="e.id" class="event-row"><span>{{ board?.cases.find(c=>c.id===e.caseId)?.name }} · {{ e.occurredOn }} · {{ e.title }} {{ e.retracted?'（已撤销）':'' }}</span><el-button size="small" @click="edit(e)">查看 / 修订</el-button><el-button v-if="!e.retracted&&!e.id.startsWith('legacy:')" size="small" @click="follow(e)">登记后续通知</el-button></div></details>
    <p class="footnote">规则版本 {{ board?.ruleVersion }}。旧字段与导入公式保留供核对；未知程序、涉外或特殊送达先按通知登记。申请延期、中止或关联案件变化均不会自动停止其他期限。</p>

    <el-dialog append-to-body destroy-on-close v-model="editor" :title="form.id?'核对 / 修订程序事件':'登记程序事件'" width="min(780px, 94vw)" :close-on-click-modal="false" :close-on-press-escape="!saving" :show-close="!saving">
      <el-form label-position="top" class="event-form">
        <el-form-item label="所属案件与我方身份"><strong>{{ currentCase?.name }} · 我方 {{ currentCase?.ourRole }}</strong></el-form-item>
        <div class="form-grid"><el-form-item label="程序事件"><el-select v-model="form.kind" @change="chooseKind"><el-option v-for="k in availableKinds" :key="k[0]" :value="k[0]" :label="k[1]" /></el-select></el-form-item><el-form-item label="期限责任方 / 收文方身份"><el-select v-model="form.actorRole"><el-option v-for="r in roles" :key="r" :value="r" :label="r" /></el-select></el-form-item></div>
        <el-form-item label="事项名称 / 材料轮次"><el-input v-model="form.title" maxlength="200" placeholder="例如：第二轮补充证据转送后的答复" /></el-form-item>
        <el-form-item label="关联前序提交或通知（用于区分材料轮次）"><el-select v-model="form.parentId" clearable placeholder="选择前序事件"><el-option v-for="e in parentOptions" :key="e.id" :value="e.id" :label="`${e.occurredOn} · ${e.title}`" /></el-select></el-form-item>
        <div class="form-grid"><el-form-item label="事件日期 / 提交或发文日"><el-date-picker v-model="form.occurredOn" type="date" value-format="YYYY-MM-DD" /></el-form-item><el-form-item label="转文日期（仅记录，不自动起算）"><el-date-picker v-model="form.forwardedOn" type="date" value-format="YYYY-MM-DD" clearable /></el-form-item><el-form-item label="经核实的期限起算日 / 有效送达日"><el-date-picker v-model="form.startOn" type="date" value-format="YYYY-MM-DD" clearable /></el-form-item><el-form-item label="文书明确截止日（优先采用）"><el-date-picker v-model="form.dueOn" type="date" value-format="YYYY-MM-DD" clearable /></el-form-item></div>
        <el-alert title="转文日与送达日可能不同。电子送达、邮寄送达按对应规则核实；未取得有效起算依据时保留待核对，不用我方提交日代替。" type="info" :closable="false" />
        <div class="form-grid"><el-form-item label="通知指定期间（仅用于指定期限类事件）"><el-input-number v-model="form.periodValue" :min="1" :max="3650" :value-on-clear="null" /><el-select v-model="form.periodUnit" clearable placeholder="单位"><el-option value="day" label="自然日"/><el-option value="calendar_month" label="日历月"/></el-select></el-form-item><el-form-item label="法定规则适用范围"><el-select v-model="form.scope"><el-option value="notice_only" label="待核对 / 仅采用明确通知期限"/><el-option value="cn_current" label="现行中国专利 / 行政程序"/><el-option value="domestic_ordinary" label="境内普通民事 / 可上诉的一审裁判"/><el-option value="foreign_no_domicile" label="境内无住所 / 涉外，另核通知与法律"/></el-select></el-form-item><el-form-item label="内部交稿目标（不改变程序期限）"><el-date-picker v-model="form.internalOn" type="date" value-format="YYYY-MM-DD" clearable /></el-form-item><el-form-item label="下一次查阅 / 跟进日"><el-date-picker v-model="form.nextCheckOn" type="date" value-format="YYYY-MM-DD" clearable /></el-form-item></div>
        <div class="form-grid"><el-form-item label="来源卷宗 / 传票 / 收文原件"><el-select v-model="form.fileId" clearable filterable placeholder="从本案卷宗选择"><el-option v-for="f in sourceFiles" :key="f.id" :value="f.id" :label="f.fileName"/></el-select><el-button v-if="form.fileId" text @click="router.push({name:'files',params:{caseId:form.caseId},query:{select:form.fileId}})">打开原件</el-button></el-form-item><el-form-item label="对应哪一次开庭 / 口审"><el-select v-model="form.hearingId" clearable placeholder="关联已有庭审；支持多份通知对应同一次排期"><el-option v-for="h in sourceHearings" :key="h.id" :value="h.id" :label="`${h.hearingDate} · ${h.hearingName || '开庭 / 口审'}`"/></el-select><el-button text @click="router.push({name:'case-detail',params:{id:form.caseId},query:{tab:'hearings'}})">管理历次庭审</el-button></el-form-item></div>
        <el-alert v-if="['summons','hearing_change'].includes(form.kind)" title="通知登记不会直接改动庭审时间。核实决定后，在「历次庭审 / 改期」中维护对应排期；仅提出延期申请时保留原排期。" type="info" :closable="false"/>
        <el-form-item label="文书、送达凭证、收件人及适用条件"><el-input v-model="form.sourceNote" type="textarea" :rows="3" placeholder="记录通知书名称/文号、实际收文人、起算依据及特殊条件；无效请求须核实受理，裁判须核实可上诉性和我方资格。" /></el-form-item>
        <el-checkbox v-model="form.basisConfirmed">已核实起算依据、责任方及所选规则适用条件</el-checkbox>
        <el-alert v-if="currentCase?.track==='patent_invalidation'" title="无效请求补充理由/证据按一个日历月计算。无效程序指定期限不得延长；修订截止日仅用于纠错或有依据的变更，不能把延期申请当成获准。" type="warning" :closable="false" />
        <el-form-item label="本次登记 / 修订原因"><el-input v-model="reason" placeholder="例如核对电子送达回执、更正录入日期、收到更正通知" /></el-form-item>
        <el-checkbox v-if="form.id" v-model="form.retracted">撤销此事件（保留修订记录，撤销后不再产生事项）</el-checkbox>
      </el-form>
      <template #footer><el-button :disabled="saving" @click="editor=false">取消</el-button><el-button type="primary" :loading="saving" @click="save">保存并重算</el-button></template>
    </el-dialog>
    <el-dialog append-to-body v-model="showHistory" title="本案程序修订记录（最近 200 条）" width="min(850px,94vw)"><el-empty v-if="!history.length" description="暂无修订记录"/><article v-for="h in history" :key="h.id" class="history-row"><strong>{{ h.createdAt }} · {{ h.reason }}</strong><details><summary>查看修改前后 / 处理依据</summary><div class="history-grid"><pre>{{ readableJson(h.beforeJson) }}</pre><pre>{{ readableJson(h.afterJson) }}</pre></div></details></article></el-dialog>
  </section>
</template>
<style scoped>
.procedure-board{display:grid;gap:16px;color:var(--el-text-color-primary)}.board-head,.board-controls{display:flex;align-items:center;justify-content:space-between;gap:16px;flex-wrap:wrap}.board-head h3{margin:0}.board-head p,.footnote{color:var(--el-text-color-secondary);font-size:13px;line-height:1.6}.actions{display:flex;gap:6px;flex-wrap:wrap}.actions .el-button{margin:0}.case-strip{display:flex;gap:10px;overflow-x:auto;padding-bottom:4px}.case-chip{display:grid;gap:6px;text-align:left;min-width:190px;max-width:300px;flex-shrink:0;border:1px solid var(--el-border-color);border-radius:8px;padding:12px;background:var(--el-fill-color-light);color:inherit;cursor:pointer;white-space:normal}.case-chip span{font-size:12px;color:var(--el-text-color-secondary)}.procedure-item{display:grid;grid-template-columns:150px minmax(0,1fr) auto;gap:18px;border:1px solid var(--el-border-color);border-radius:8px;padding:18px}.procedure-item.overdue{border-left:3px solid var(--el-color-danger)}.item-date{display:flex;flex-direction:column;gap:7px;font-variant-numeric:tabular-nums}.item-date small{font-size:12px;color:var(--el-text-color-secondary)}.item-main h4{margin:10px 0 6px}.item-main p{margin:8px 0;font-size:13px;line-height:1.7;color:var(--el-text-color-secondary);overflow-wrap:anywhere}.item-tags{display:flex;gap:5px;flex-wrap:wrap}.case-link{border:0;padding:0;background:none;color:var(--el-color-primary);cursor:pointer;text-align:left}.item-actions{display:flex;flex-direction:column;gap:8px;align-items:stretch}.item-actions .el-button{margin:0}.form-grid{display:grid;grid-template-columns:1fr 1fr;gap:0 20px}.event-form .el-date-editor,.event-form .el-select{width:100%}.event-form .el-alert{margin:8px 0 18px}.event-form .el-checkbox{white-space:normal;height:auto;line-height:1.8;margin-bottom:14px}.event-row{display:flex;gap:8px;align-items:center;flex-wrap:wrap;padding:12px 0;border-bottom:1px solid var(--el-border-color-lighter)}.event-row span{flex:1;min-width:180px}.event-register summary,details summary{cursor:pointer;font-size:13px}.history-row{padding:16px 0;border-bottom:1px solid var(--el-border-color)}.history-grid{display:grid;grid-template-columns:1fr 1fr;gap:16px}.history-grid pre{white-space:pre-wrap;overflow-wrap:anywhere;font-size:12px;background:var(--el-fill-color-light);padding:10px}.footnote{margin:0}@media(max-width:760px){.procedure-item{grid-template-columns:1fr}.item-actions{flex-direction:row;flex-wrap:wrap}.form-grid,.history-grid{grid-template-columns:1fr}.filters{display:flex;flex-wrap:wrap}.item-date{flex-direction:row;flex-wrap:wrap}}
</style>
