import { addDaysLocalISO, parseLocalDate, toLocalISODate } from '../../shared/utils/date'
import { planningRestIntervals } from './calendarDates'
import type { TaskPlan } from '../../types/bindings'
import type { HolidayCalendarEntry } from '../../types/ipc'

export interface PlanningTask {
  id: string; taskName: string; caseId?: string | null; caseName?: string | null
  startDate?: string | null; dueDate?: string | null; deadline?: string | null; completed?: number | boolean
}
export interface PlanRange { start: string; end: string }
export function validPlanDate(value: string) {
  const date = parseLocalDate(value)
  return /^\d{4}-\d{2}-\d{2}$/.test(value) && value >= '1900-01-01' && value <= '9999-12-31' && date !== null && toLocalISODate(date) === value
}
/** Calendar arithmetic independent of local DST transitions. */
export function dayDistance(from: string, to: string) {
  const ordinal = (value: string) => { const [y,m,d] = value.split('-').map(Number); return Date.UTC(y,m-1,d) / 86400000 }
  return ordinal(to) - ordinal(from)
}
export function rangeError(range: PlanRange) {
  if (!validPlanDate(range.start) || !validPlanDate(range.end)) return '请填写有效的计划开始和结束日期'
  const days = dayDistance(range.start,range.end)
  return days < 0 ? '计划结束不能早于开始' : days > 3659 ? '计划跨度不能超过 3660 天' : ''
}
export function movePlan(range: PlanRange, delta: number, edge: 'move' | 'start' | 'end'): PlanRange {
  const start = edge === 'end' ? range.start : addDaysLocalISO(range.start,delta)
  const end = edge === 'start' ? range.end : addDaysLocalISO(range.end,delta)
  return {start: edge === 'start' && start > end ? end : start, end: edge === 'end' && end < start ? start : end}
}
export function planRange(plan?: TaskPlan): PlanRange | null {
  return plan?.startDate && plan.endDate ? {start:plan.startDate,end:plan.endDate} : null
}
export function clipPlan(range: PlanRange, windowStart: string, days: number) {
  const start=dayDistance(windowStart,range.start), end=dayDistance(windowStart,range.end)
  if (end < 0 || start >= days) return null
  return { offset:Math.max(0,start), span:Math.min(days-1,end)-Math.max(0,start)+1, clippedStart:start<0, clippedEnd:end>=days }
}
export function planningWarnings(task: PlanningTask, range: PlanRange, tasks: PlanningTask[], plans: TaskPlan[], holidays: HolidayCalendarEntry[], deadlines: Array<{date: string; title: string; caseId?: string | null; type?: string}> = []) {
  if(rangeError(range)) return []
  const warnings: string[] = []
  const due = task.dueDate || task.deadline
  if(due && range.end > due) warnings.push(`计划结束晚于任务截止日 ${due}`)
  const byDate = new Map<string, HolidayCalendarEntry[]>()
  holidays.forEach(e=>byDate.set(e.date,[...(byDate.get(e.date)||[]),e]))
  let rest=0
  const partial: string[]=[]
  for(let date=range.start;date<=range.end;date=addDaysLocalISO(date,1)) {
    const entries=byDate.get(date)||[], intervals=planningRestIntervals(parseLocalDate(date)!,entries)
    if(intervals.reduce((n,[s,e])=>n+e-s,0)===1440)rest++
    else if(intervals.length) {
      const clock=(minutes:number)=>`${String(Math.floor(minutes/60)).padStart(2,'0')}:${String(minutes%60).padStart(2,'0')}`
      partial.push(`${date} ${intervals.map(([start,end])=>`${clock(start)}–${clock(end)}`).join('、')}`)
    }
    if(date==='9999-12-31')break
  }
  if(rest)warnings.push(`计划包含 ${rest} 个休息日（含周末和个人安排）`)
  if(partial.length)warnings.push(`计划包含 ${partial.length} 天的部分休息时段（${partial.slice(0,3).join('；')}${partial.length>3?'等':''}），日级计划未指定具体时间，请核对`)
  const active = new Set(tasks.filter(t=>!t.completed && t.id!==task.id).map(t=>t.id))
  const overlaps = plans.filter(p=>active.has(p.taskId) && p.startDate && p.endDate && p.startDate<=range.end && p.endDate>=range.start)
  if(overlaps.length) warnings.push(`与 ${overlaps.length} 项任务的计划日期重叠，请核对工作量`)
  const fixed = deadlines.filter(d=>task.caseId && d.caseId===task.caseId && (d.type?.startsWith('deadline') || d.type==='appeal') && d.date>=range.start && d.date<range.end)
  if(fixed.length)warnings.push(`计划跨过案件期限：${fixed.map(d=>`${d.title}（${d.date}）`).join('、')}`)
  return warnings
}
