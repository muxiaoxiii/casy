/** One read model for independent plans. Legacy start/due spans remain fallback only. */
export interface ScheduledTask {
  startDate?: string | null; dueDate?: string | null; deadline?: string | null
  planDefined?: boolean; plannedStartDate?: string | null; plannedEndDate?: string | null
}
export function taskPlanStart(task: ScheduledTask) {
  return task.planDefined ? task.plannedStartDate || null : task.startDate || null
}
export function taskPlanSpan(task: ScheduledTask) {
  const start = taskPlanStart(task)
  const end = task.planDefined ? task.plannedEndDate : task.dueDate || task.deadline || start
  return start && end && start <= end ? { start, end } : null
}
export function hasMultiDayPlan(task: ScheduledTask) {
  const range = taskPlanSpan(task)
  return !!range && range.start !== range.end
}
export function taskPlanLabel(task: ScheduledTask) {
  if (!task.planDefined) return ''
  const range = taskPlanSpan(task)
  return range ? `计划 ${range.start} — ${range.end}` : '尚未安排计划'
}
