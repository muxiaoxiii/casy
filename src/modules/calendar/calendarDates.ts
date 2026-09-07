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
