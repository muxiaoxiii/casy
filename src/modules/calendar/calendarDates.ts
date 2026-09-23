export function surroundingMonths(date: Date) {
  return [-1, 0, 1].map(offset => {
    const month = new Date(date.getFullYear(), date.getMonth() + offset, 1)
    return { year: month.getFullYear(), month: month.getMonth() + 1 }
  })
}

export function monthWorkingDays(date: Date, entries: { date: string; kind: string }[]) {
  const overrides = new Map(entries.map(entry => [entry.date, entry.kind]))
  const year = date.getFullYear(), month = date.getMonth()
  let days = 0
  for (let day = 1; day <= new Date(year, month + 1, 0).getDate(); day++) {
    const current = new Date(year, month, day)
    const key = `${year}-${String(month + 1).padStart(2, '0')}-${String(day).padStart(2, '0')}`
    const kind = overrides.get(key)
    if (kind === 'workday' || (kind !== 'holiday' && current.getDay() !== 0 && current.getDay() !== 6)) days++
  }
  return days
}

export function eventDuration(start?: string | null, end?: string | null) {
  if (!start || !end) return ''
  const minutes = (value: string) => Number(value.slice(0, 2)) * 60 + Number(value.slice(3, 5))
  const total = minutes(end) - minutes(start)
  return total > 0 ? `${total} 分钟` : ''
}

export interface AvailabilityEntry { kind: string; source?: string; startTime?: string | null; endTime?: string | null }
export function timeMinutes(value?: string | null) {
  if (!value || !/^(?:[01]\d|2[0-3]):[0-5]\d$/.test(value)) return value === '24:00' ? 1440 : null
  return Number(value.slice(0, 2)) * 60 + Number(value.slice(3, 5))
}
export function availabilityTimeLabel(entry: AvailabilityEntry) {
  return entry.startTime && entry.endTime ? `${entry.startTime}–${entry.endTime}` : '全天'
}
/** Personal availability affects planning, never the statutory calendar. */
export function planningRestIntervals(date: Date, entries: AvailabilityEntry[]): Array<[number, number]> {
  const official = entries.find(e => e.source !== 'personal')
  const rest = official ? official.kind === 'holiday' : [0, 6].includes(date.getDay())
  const personal = entries.find(e => e.source === 'personal')
  if (!personal) return rest ? [[0, 1440]] : []
  const start = timeMinutes(personal.startTime), end = timeMinutes(personal.endTime)
  if (start === null || end === null) return personal.kind === 'holiday' ? [[0, 1440]] : []
  if (personal.kind === 'holiday') return rest ? [[0, 1440]] : [[start, end]]
  return rest ? ([[0, start], [end, 1440]] as Array<[number,number]>).filter(([s,e]) => s < e) : []
}
export function isPlanningWorkday(date: Date, entries: AvailabilityEntry[]) {
  return planningRestIntervals(date, entries).reduce((sum,[s,e]) => sum + e - s,0) < 1440
}
