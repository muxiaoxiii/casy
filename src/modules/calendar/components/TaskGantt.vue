<script setup lang="ts">
import { computed, ref, watch, onBeforeUnmount } from 'vue'
import HolidayBadges from './HolidayBadges.vue'
import { casyContext } from '../../../core/plugin/context'
import type { TaskPlan } from '../../../types/bindings'
import type { HolidayCalendarEntry } from '../../../types/ipc'
import { addDaysLocalISO, todayLocalISO } from '../../../shared/utils/date'
import { clipPlan, dayDistance, movePlan, planRange, planningWarnings, rangeError, validPlanDate, type PlanningTask, type PlanRange } from '../taskPlanning'
import { isPlanningWorkday, planningRestIntervals, availabilityTimeLabel } from '../calendarDates'
import { parseLocalDate } from '../../../shared/utils/date'
const props = defineProps<{
  date: string; tasks: PlanningTask[]; plans: TaskPlan[]; cases: Array<{id:string;caseName?:string;caseNo?:string}>
  holidays: HolidayCalendarEntry[]; events: Array<{id:string;date:string;title:string;caseId?:string|null;type?:string}>
  loading?: boolean; error?: string
}>()
const emit=defineEmits<{saved:[plan:TaskPlan];refresh:[];navigate:[date:string];open:[task:PlanningTask];event:[event:typeof props.events[number]]}>()
const scale=ref<'day'|'week'>('day'), search=ref(''), includeCompleted=ref(false), page=ref(0)
const daysCount=computed(()=>scale.value==='day'?28:84), cellWidth=computed(()=>scale.value==='day'?44:18)
const endDate=computed(()=>addDaysLocalISO(props.date,daysCount.value-1))
const planMap=computed(()=>new Map(props.plans.map(p=>[p.taskId,p])))
const caseMap=computed(()=>new Map(props.cases.map(c=>[c.id,c.caseName||c.caseNo||'未命名案件'])))
const visibleTasks=computed(()=>props.tasks.filter(t=>(includeCompleted.value||!t.completed)&&`${t.taskName} ${caseMap.value.get(t.caseId||'')||t.caseName||''}`.toLowerCase().includes(search.value.trim().toLowerCase())))
watch([search,includeCompleted],()=>page.value=0)
watch(()=>visibleTasks.value.length,()=>page.value=Math.min(page.value,Math.max(0,Math.ceil(visibleTasks.value.length/40)-1)))
const groups=computed(()=>{
  const map=new Map<string,{id:string;name:string;tasks:PlanningTask[];fixed:typeof props.events}>()
  for(const task of visibleTasks.value.slice(page.value*40,page.value*40+40)) {
    const id=task.caseId||''
    if(!map.has(id))map.set(id,{id,name:caseMap.value.get(id)||task.caseName||'未关联案件',tasks:[],fixed:props.events.filter(e=>e.caseId===id && id && (e.type==='hearing'||e.type==='court'||e.type==='appeal'||e.type?.startsWith('deadline')) && e.date>=props.date && e.date<=endDate.value)})
    map.get(id)!.tasks.push(task)
  }
  return [...map.values()]
})
const days=computed(()=>Array.from({length:daysCount.value},(_,index)=>{
  const date=addDaysLocalISO(props.date,index), entries=props.holidays.filter(h=>h.date===date), local=parseLocalDate(date)!
  return {date,index,entries,rest:!isPlanningWorkday(local,entries),partial:isPlanningWorkday(local,entries)&&planningRestIntervals(local,entries).length>0,today:date===todayLocalISO(),label:`${date}${entries.map(e=>` · ${e.source==='personal'?'个人':'法定'}${e.kind==='holiday'?'休':'班'} ${e.name} ${availabilityTimeLabel(e)}`).join('')}`}
}))
const canEdit=computed(()=>!props.loading&&!props.error)
const drag=ref<{id:string;originX:number;delta:number;edge:'move'|'start'|'end';range:PlanRange;revision:number}|null>(null)
let suppressClick=false
function displayedRange(task:PlanningTask) { return drag.value?.id===task.id ? movePlan(drag.value.range,drag.value.delta,drag.value.edge) : planRange(planMap.value.get(task.id)) }
function bar(task:PlanningTask) { const range=displayedRange(task);return range?clipPlan(range,props.date,daysCount.value):null }
function barStyle(task:PlanningTask) { const b=bar(task)!;return {left:`${b.offset*cellWidth.value+2}px`,width:`${b.span*cellWidth.value-4}px`} }
function deadline(task:PlanningTask) { const date=task.dueDate||task.deadline;if(!date)return null;const offset=dayDistance(props.date,date);return offset>=0&&offset<daysCount.value?{date,offset}:null }
function beginDrag(e:PointerEvent,task:PlanningTask,edge:'move'|'start'|'end') {
  if(e.button!==0||!canEdit.value||task.completed)return
  const range=planRange(planMap.value.get(task.id));if(!range)return
  suppressClick=false
  drag.value={id:task.id,originX:e.clientX,delta:0,range,edge,revision:planMap.value.get(task.id)!.revision}
  ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
}
function moveDrag(e:PointerEvent) { if(drag.value)drag.value.delta=Math.round((e.clientX-drag.value.originX)/cellWidth.value) }
function finishDrag(task:PlanningTask) {
  if(!drag.value)return
  const current=drag.value;drag.value=null
  if(current.delta) {suppressClick=true;edit(task,movePlan(current.range,current.delta,current.edge),current.revision)}
}
const draft=ref<{task:PlanningTask;start:string;end:string;revision:number;clear:boolean}|null>(null)
const busy=ref(false),saveError=ref('')
const checking=ref(false), reviewError=ref('')
const reviewedHolidays=ref<HolidayCalendarEntry[]>([]), reviewedEvents=ref<typeof props.events>([])
const reviewRetry=ref(0)
let reviewRequest=0
onBeforeUnmount(()=>reviewRequest++)
watch(()=>draft.value && !draft.value.clear && !rangeError(draft.value) ? `${draft.value.start}/${draft.value.end}/${reviewRetry.value}` : '',async key=>{
  const request=++reviewRequest
  checking.value=false;reviewError.value='';reviewedHolidays.value=[];reviewedEvents.value=[]
  if(!key)return
  checking.value=true
  const [start,end]=key.split('/'), from=parseLocalDate(start)!, through=parseLocalDate(end)!
  const years=Array.from({length:through.getFullYear()-from.getFullYear()+1},(_,i)=>from.getFullYear()+i)
  const months=(through.getFullYear()-from.getFullYear())*12+through.getMonth()-from.getMonth()+1
  try {
    const holidayResults=await Promise.all(years.map(year=>casyContext.calendar.holidays(year)))
    const eventResults=await Promise.all(Array.from({length:Math.ceil(months/12)},(_,index)=>{
      const month=new Date(from.getFullYear(),from.getMonth()+index*12,1)
      return casyContext.calendar.events(month.getFullYear(),month.getMonth()+1,Math.min(12,months-index*12))
    }))
    if(request!==reviewRequest)return
    if(holidayResults.some(r=>!r.ok||!Array.isArray(r.data?.entries))||eventResults.some(r=>!r.ok||!Array.isArray(r.data)))throw new Error('无法核对该日期范围的休班安排和案件期限，请重试')
    reviewedHolidays.value=holidayResults.flatMap(r=>r.data!.entries)
    reviewedEvents.value=eventResults.flatMap(r=>r.data!)
  } catch(error) {if(request===reviewRequest)reviewError.value=error instanceof Error?error.message:'排期核对失败'}
  finally {if(request===reviewRequest)checking.value=false}
})
function retryReview() { reviewRetry.value++ }

const draftError=computed(()=>!draft.value||draft.value.clear?'':rangeError(draft.value))
const warnings=computed(()=>!draft.value||draft.value.clear?[]:planningWarnings(draft.value.task,draft.value,props.tasks,props.plans,reviewedHolidays.value,reviewedEvents.value))
function edit(task:PlanningTask,range?:PlanRange,revision?:number) {
  if(!canEdit.value||task.completed)return
  const plan=planMap.value.get(task.id), initial=task.startDate&&validPlanDate(task.startDate)?task.startDate:props.date
  const current=range||planRange(plan)||{start:initial,end:initial}
  draft.value={task,...current,revision:revision??plan?.revision??0,clear:false};saveError.value=''
}
function activate(task:PlanningTask) {if(suppressClick){suppressClick=false;return}edit(task)}
function keyboard(e:KeyboardEvent,task:PlanningTask,edge:'move'|'start'|'end') {
  if(e.key==='Escape'){drag.value=null;return}
  if(!['ArrowLeft','ArrowRight'].includes(e.key))return
  e.preventDefault();const range=planRange(planMap.value.get(task.id));if(!range)return
  edit(task,movePlan(range,(e.key==='ArrowRight'?1:-1)*(e.shiftKey?7:1),edge))
}
async function save() {
  if(!draft.value||busy.value||checking.value||reviewError.value||draftError.value||!canEdit.value)return
  busy.value=true;saveError.value=''
  try {
    const current=draft.value
    const result=await casyContext.calendar.saveTaskPlan({taskId:current.task.id,startDate:current.clear?null:current.start,endDate:current.clear?null:current.end,expectedRevision:current.revision})
    if(!result.ok||!result.data){saveError.value=result.error||'保存失败，原计划保持不变';emit('refresh');return}
    emit('saved',result.data);draft.value=null
  } catch(error) {saveError.value=error instanceof Error?error.message:'保存失败，原计划保持不变';emit('refresh')}
  finally {busy.value=false}
}
function reloadDraft() { if(draft.value)edit(draft.value.task) }
</script>

<template>
  <section class="task-gantt" aria-label="任务甘特排期" :aria-busy="loading">
    <header class="gantt-toolbar">
      <div><h2>任务计划</h2><p>计划条表示工作安排；◆ 为任务截止日，案件期限与庭审单独列示。</p></div>
      <div class="gantt-controls"><button type="button" :aria-pressed="scale==='day'" @click="scale='day'">按日 · 4 周</button><button type="button" :aria-pressed="scale==='week'" @click="scale='week'">按周 · 12 周</button></div>
    </header>
    <div class="gantt-controls gantt-filters">
      <button type="button" @click="emit('navigate',todayLocalISO())">今天</button><button type="button" aria-label="甘特图上一时段" @click="emit('navigate',addDaysLocalISO(date,-daysCount))">←</button><span>{{ date }} — {{ endDate }}</span><button type="button" aria-label="甘特图下一时段" @click="emit('navigate',addDaysLocalISO(date,daysCount))">→</button>
      <input v-model="search" aria-label="筛选甘特任务或案件" placeholder="搜索任务 / 案件" /><label><input v-model="includeCompleted" type="checkbox" />含已完成</label>
    </div>
    <div v-if="error" class="gantt-error" role="alert">{{ error }} <button type="button" @click="emit('refresh')">重新加载</button></div>
    <p class="gantt-help">拖动计划条改期，拖动两端调整跨度；点击任务行填写日期。聚焦计划条后，← / → 移动一天，Shift 移动七天；所有修改均在确认后保存。</p>
    <div class="gantt-scroll" tabindex="0" aria-label="横向滚动查看计划日期">
      <div class="gantt-sheet" :style="{'--day-width':`${cellWidth}px`,'--lane-width':`${cellWidth*daysCount}px`}">
        <div class="gantt-axis"><div class="gantt-label">案件 / 任务</div><div class="gantt-lane axis-lane">
          <div v-for="day in days" :key="day.date" class="gantt-day" :class="{rest:day.rest,partial:day.partial,today:day.today,weekly:scale==='week'}" :title="day.label" :aria-label="day.label">
            <template v-if="scale==='day'"><span>{{ day.date.slice(5) }}</span><HolidayBadges :entries="day.entries" compact /></template>
            <template v-else><span v-if="day.index%7===0" class="week-tick">{{ day.date.slice(5) }}</span><span v-for="entry in day.entries" :key="entry.source" class="week-holiday" :class="{personal:entry.source==='personal',workday:entry.kind==='workday'}" /></template>
          </div>
        </div></div>
        <section v-for="group in groups" :key="group.id" class="gantt-group">
          <h3 class="gantt-group-heading">{{ group.name }} <small>{{ group.tasks.length }} 项</small></h3>
          <div v-for="fixed in group.fixed" :key="fixed.type+fixed.id" class="gantt-row fixed-row">
            <button type="button" class="gantt-label" :title="fixed.title" @click="emit('event',fixed)">{{ fixed.type==='hearing'||fixed.type==='court'?'庭审':'案件期限' }} · {{ fixed.title }}</button>
            <div class="gantt-lane"><span class="fixed-marker" :style="{left:`${(dayDistance(date,fixed.date)+.5)*cellWidth}px`}" :title="`${fixed.title} · ${fixed.date}（固定节点）`">◆</span></div>
          </div>
          <div v-for="task in group.tasks" :key="task.id" class="gantt-row" :class="{completed:task.completed}">
            <button type="button" class="gantt-label" :title="task.taskName" :disabled="!canEdit && !task.completed" @click="task.completed?emit('open',task):edit(task)"><strong>{{ task.taskName }}</strong><small>{{ task.completed?'已完成':planRange(planMap.get(task.id))?'编辑计划':'未排计划 · 点击安排' }}</small><small v-if="!planMap.has(task.id)&&task.startDate">已有开始日 {{ task.startDate }}</small><small v-if="task.dueDate||task.deadline">截止 {{ task.dueDate||task.deadline }}</small></button>
            <div class="gantt-lane task-lane">
              <span v-for="day in days.filter(d=>d.rest||d.partial||d.today)" :key="day.date" class="gantt-day-shade" :class="{rest:day.rest,partial:day.partial,today:day.today}" :style="{left:`${day.index*cellWidth}px`}" aria-hidden="true" />
              <div v-if="bar(task)" class="gantt-bar" :style="barStyle(task)" :class="{dragging:drag?.id===task.id}">
                <button v-if="!bar(task)!.clippedStart && bar(task)!.span * cellWidth >= 36" type="button" class="resize-handle start" :disabled="!canEdit||!!task.completed" :aria-label="`调整开始：${task.taskName}`" @pointerdown.stop="beginDrag($event,task,'start')" @pointermove="moveDrag" @pointerup="finishDrag(task)" @pointercancel="drag=null" @lostpointercapture="drag=null" @click="activate(task)" @keydown="keyboard($event,task,'start')">‹</button>
                <button type="button" class="bar-body" :disabled="!canEdit||!!task.completed" :aria-label="`移动计划：${task.taskName}`" :title="`${task.taskName} · ${displayedRange(task)!.start} — ${displayedRange(task)!.end}`" @pointerdown="beginDrag($event,task,'move')" @pointermove="moveDrag" @pointerup="finishDrag(task)" @pointercancel="drag=null" @lostpointercapture="drag=null" @click="activate(task)" @keydown="keyboard($event,task,'move')">{{ bar(task)!.clippedStart?'← ':'' }}{{ task.taskName }}{{ bar(task)!.clippedEnd?' →':'' }}</button>
                <button v-if="!bar(task)!.clippedEnd && bar(task)!.span * cellWidth >= 36" type="button" class="resize-handle end" :disabled="!canEdit||!!task.completed" :aria-label="`调整结束：${task.taskName}`" @pointerdown.stop="beginDrag($event,task,'end')" @pointermove="moveDrag" @pointerup="finishDrag(task)" @pointercancel="drag=null" @lostpointercapture="drag=null" @click="activate(task)" @keydown="keyboard($event,task,'end')">›</button>
              </div>
              <span v-else-if="planRange(planMap.get(task.id))" class="outside-plan">计划在当前范围外：{{ planMap.get(task.id)?.startDate }} — {{ planMap.get(task.id)?.endDate }}</span>
              <span v-if="deadline(task)" class="fixed-marker task-deadline" :style="{left:`${(deadline(task)!.offset+.5)*cellWidth}px`}" :title="`任务截止 ${deadline(task)!.date}（不随计划移动）`">◆</span>
            </div>
          </div>
        </section>
        <p v-if="!groups.length" class="gantt-empty">{{ loading?'正在加载任务计划…':'没有符合条件的任务' }}</p>
      </div>
    </div>
    <footer class="gantt-controls"><span>共 {{ visibleTasks.length }} 项 · 休息日底纹随主题显示 · 实色为法定，虚线为个人</span><template v-if="visibleTasks.length>40"><button :disabled="!page" @click="page--">上一页</button><span>{{ page+1 }} / {{ Math.ceil(visibleTasks.length/40) }}</span><button :disabled="(page+1)*40>=visibleTasks.length" @click="page++">下一页</button></template></footer>
    <el-dialog :model-value="!!draft" title="调整任务计划" width="min(520px, 94vw)" :close-on-click-modal="!busy" :close-on-press-escape="!busy" :show-close="!busy" @update:model-value="!busy&&(draft=null)">
      <form v-if="draft" class="plan-form" @submit.prevent="save">
        <strong>{{ draft.task.taskName }}</strong>
        <p>任务截止：{{ draft.task.dueDate||draft.task.deadline||'未设置' }}。修改计划不会更改截止日期或案件期限。</p>
        <label>计划开始<input v-model="draft.start" type="date" min="1900-01-01" max="9999-12-31" :disabled="busy||draft.clear" /></label>
        <label>计划结束<input v-model="draft.end" type="date" min="1900-01-01" max="9999-12-31" :disabled="busy||draft.clear" /></label>
        <label v-if="planRange(planMap.get(draft.task.id))" class="clear-plan"><input v-model="draft.clear" type="checkbox" :disabled="busy" />取消这个任务的计划安排</label>
        <p v-if="draftError" class="gantt-error" role="alert">{{ draftError }}</p>
        <p v-if="checking" role="status">正在核对休班安排和案件期限…</p>
        <p v-if="reviewError" class="gantt-error" role="alert">{{ reviewError }} <button type="button" @click="retryReview">重试核对</button></p>
        <ul v-if="!checking && !reviewError && warnings.length" class="plan-warnings" aria-label="排期提醒"><li v-for="warning in warnings" :key="warning">{{ warning }}</li></ul>
        <p v-if="saveError" class="gantt-error" role="alert">{{ saveError }}<button type="button" :disabled="busy||loading" @click="reloadDraft">载入最新计划</button></p>
        <div class="plan-actions"><button type="button" :disabled="busy" @click="draft=null">取消</button><button type="submit" class="plan-save" :disabled="busy||checking||!!reviewError||!!draftError||!canEdit">{{ busy?'正在保存…':draft.clear?'确认取消计划':warnings.length?'确认仍然安排':'保存计划' }}</button></div>
      </form>
    </el-dialog>
  </section>
</template>

<style scoped>
.task-gantt { min-width:0; padding:20px; border:1px solid var(--c-border); border-radius:12px; background:var(--c-bg-card); }
.gantt-toolbar { display:flex; flex-wrap:wrap; align-items:center; justify-content:space-between; gap:12px; }
h2 { margin:0; font-size:18px; } p { color:var(--c-text-secondary); font-size:12px; line-height:1.7; }
.gantt-controls { display:flex; align-items:center; gap:8px; flex-wrap:wrap; font-size:12px; }
.gantt-controls button,.plan-actions button,.gantt-error button { font:inherit; border:1px solid var(--c-border); border-radius:6px; padding:6px 10px; background:var(--c-bg-card); color:var(--c-text); cursor:pointer; }
.gantt-controls button[aria-pressed=true],.plan-actions .plan-save { background:var(--c-primary); border-color:var(--c-primary); color:var(--c-primary-contrast); }
.gantt-filters { margin-top:12px; }.gantt-filters>input { min-width:150px; flex:1; }.gantt-filters label { display:flex; align-items:center; gap:4px; }
input { font:inherit; color:var(--c-text); border:1px solid var(--c-border); border-radius:5px; background:var(--c-bg-page); padding:7px; min-width:0; }
.gantt-help { margin:12px 0; }.gantt-scroll { overflow:auto; max-height:65vh; border:1px solid var(--c-border); border-radius:8px; overscroll-behavior:contain; }
.gantt-sheet { --label-width:220px; width:calc(var(--label-width) + var(--lane-width)); min-height:140px; }
.gantt-axis,.gantt-row { display:flex; }.gantt-axis { position:sticky; top:0; z-index:5; background:var(--c-bg-card); }
.gantt-label { position:sticky; left:0; z-index:3; flex:0 0 var(--label-width); width:var(--label-width); min-width:0; border:0; border-right:1px solid var(--c-border); background:var(--c-bg-card); color:var(--c-text); text-align:left; padding:9px 12px; font:inherit; font-size:12px; cursor:pointer; }
.gantt-label strong { display:block; overflow:hidden; white-space:nowrap; text-overflow:ellipsis; font-weight:550; }.gantt-label small { display:block; color:var(--c-text-secondary); font-size:10px; margin-top:4px; }
.gantt-lane { flex:0 0 var(--lane-width); width:var(--lane-width); position:relative; background:repeating-linear-gradient(to right,transparent 0,transparent calc(var(--day-width) - 1px),var(--c-border-light) calc(var(--day-width) - 1px),var(--c-border-light) var(--day-width)); }
.axis-lane { display:flex; }.gantt-day { width:var(--day-width); flex:0 0 var(--day-width); min-height:54px; display:flex; flex-direction:column; align-items:center; padding:6px 0; gap:4px; font-size:10px; position:relative; }
.partial { background:repeating-linear-gradient(135deg,transparent 0,transparent 5px,color-mix(in srgb,var(--c-primary) 12%,transparent) 5px,color-mix(in srgb,var(--c-primary) 12%,transparent) 8px); }.rest { background:color-mix(in srgb,var(--c-primary) 8%,transparent); }.today { box-shadow:inset 1px 0 var(--c-primary); }.week-tick { position:absolute; top:6px; left:2px; white-space:nowrap; }.weekly { padding-top:24px; }.week-holiday { width:7px; height:6px; background:var(--c-warning-light); border:1px solid var(--c-warning); }.week-holiday.personal { border:1px dashed var(--c-primary); background:var(--c-bg-card); }.week-holiday.workday { border-color:var(--c-info); background:var(--c-info-light); }
.gantt-group-heading { position:sticky; left:0; max-width:calc(100cqi - 48px); padding:10px 12px; margin:0; font-size:12px; line-height:1.6; color:var(--c-text-heading); overflow-wrap:anywhere; }.gantt-group-heading small { font-weight:400; color:var(--c-text-secondary); }.gantt-group { background:var(--c-bg-page); }.gantt-row { min-height:74px; border-top:1px solid var(--c-border-light); background:var(--c-bg-card); }.fixed-row { min-height:32px; }.fixed-row .gantt-label { white-space:nowrap; overflow:hidden; text-overflow:ellipsis; font-size:11px; color:var(--c-danger); }
.gantt-day-shade { position:absolute; top:0; bottom:0; width:var(--day-width); pointer-events:none; }.gantt-bar { position:absolute; top:19px; height:30px; display:flex; border:1px solid var(--c-primary); border-radius:5px; background:var(--c-primary-light); color:var(--c-primary); z-index:1; }.gantt-bar.dragging { opacity:.7; }.gantt-bar button { background:transparent; color:inherit; border:0; font:inherit; touch-action:none; user-select:none; }.bar-body { flex:1; min-width:0; font-size:11px!important; white-space:nowrap; overflow:hidden; text-overflow:ellipsis; cursor:grab; padding:0 3px; }.resize-handle { flex:0 0 10px; width:10px; padding:0; cursor:ew-resize; }.fixed-marker { position:absolute; top:7px; transform:translateX(-50%); color:var(--c-danger); font-size:13px; z-index:2; }.task-deadline { top:51px; }.outside-plan { position:absolute; top:25px; left:12px; color:var(--c-text-secondary); font-size:11px; }.completed .gantt-bar { opacity:.5; }.completed .gantt-label strong { text-decoration:line-through; }.gantt-empty { position:sticky; left:0; width:300px; padding:20px; }
footer { margin-top:12px; color:var(--c-text-secondary); }.gantt-error { color:var(--c-danger); font-size:12px; line-height:1.6; }.plan-form { display:flex; flex-direction:column; gap:12px; }.plan-form>strong { overflow-wrap:anywhere; }.plan-form p { margin:0; }.plan-form label { display:flex; align-items:center; gap:12px; font-size:13px; }.plan-form label input[type=date] { flex:1; }.plan-warnings { padding:12px 12px 12px 28px; margin:0; background:var(--c-warning-light); color:var(--c-warning); line-height:1.7; font-size:12px; border-radius:6px; }.plan-actions { display:flex; justify-content:flex-end; gap:8px; margin-top:8px; }button:disabled { opacity:.5; cursor:default; }button:focus-visible,input:focus-visible,.gantt-scroll:focus-visible { outline:2px solid var(--c-primary); outline-offset:-2px; }
@container(max-width:600px) { .task-gantt { padding:12px; }.gantt-sheet { --label-width:150px; }.gantt-group-heading { max-width:calc(100cqi - 28px); } }
</style>
