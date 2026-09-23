import { toLocalISODate } from '../../shared/utils/date'
export interface HeatmapTask { id: string; taskName?: string; startDate?: string | null; dueDate?: string | null; deadline?: string | null; completed?: boolean | number; planDefined?: boolean; plannedStartDate?: string | null; plannedEndDate?: string | null }
export function yearTaskDays(year: number, tasks: HeatmapTask[]) {
  const days = new Map<string, HeatmapTask[]>()
  const first = `${year}-01-01`, last = `${year}-12-31`
  for (const task of tasks) {
    // A plan is work allocation; its presence/clear must not hide the task deadline.
    const due = task.dueDate || task.deadline
    if (task.planDefined && due && due >= first && due <= last) {
      const entries = days.get(due) || []
      if (!entries.some(entry => entry.id === task.id)) entries.push(task)
      days.set(due, entries)
    }
    const start = task.planDefined ? task.plannedStartDate : task.startDate || task.dueDate, end = task.planDefined ? task.plannedEndDate : task.dueDate || task.startDate
    if (!start || !end || start > end || end < first || start > last) continue
    const from = start < first ? first : start, through = end > last ? last : end
    const [y, m, d] = from.split('-').map(Number)
    const date = new Date(y, m - 1, d)
    if (toLocalISODate(date) !== from) continue
    // At most 366 iterations per task; clamp long running tasks to this year.
    for (let n = 0; n < 366 && toLocalISODate(date) <= through; n++, date.setDate(date.getDate() + 1)) {
      const key = toLocalISODate(date), entries = days.get(key) || []
      if (!entries.some(entry => entry.id === task.id)) entries.push(task)
      days.set(key, entries)
    }
  }
  return days
}
export function heatLevel(count: number): number { return count === 0 ? 0 : count <= 1 ? 1 : count <= 3 ? 2 : count <= 6 ? 3 : 4 }
