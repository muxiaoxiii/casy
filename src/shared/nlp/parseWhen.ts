/** Local prefix parsing shared by task capture and reviewed inbox capture. */
export interface ParsedWhen {
  taskName: string
  date: string | null
  time: string | null
}

const WEEKDAYS: Record<string, number> = { 一: 1, 二: 2, 三: 3, 四: 4, 五: 5, 六: 6, 日: 0, 天: 0 }
const DIGITS: Record<string, number> = { 零: 0, 一: 1, 二: 2, 两: 2, 三: 3, 四: 4, 五: 5, 六: 6, 七: 7, 八: 8, 九: 9 }
function number(value: string): number | null {
  if (/^\d+$/.test(value)) return Number(value)
  if (value in DIGITS) return DIGITS[value]
  const parts = value.match(/^([一二三四五六七八九])?十([一二三四五六七八九])?$/)
  return parts ? (parts[1] ? DIGITS[parts[1]] : 1) * 10 + (parts[2] ? DIGITS[parts[2]] : 0) : null
}
function today(): Date {
  const now = new Date()
  return new Date(now.getFullYear(), now.getMonth(), now.getDate())
}
function dateString(date: Date): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`
}
function calendarDate(year: number, month: number, day: number): Date | null {
  const date = new Date(year, month - 1, day)
  return date.getFullYear() === year && date.getMonth() === month - 1 && date.getDate() === day ? date : null
}
type Token = { value: string; length: number }
function datePrefix(text: string, base: Date): Token | null {
  let match = text.match(/^(今天|明天|后天|大后天)/)
  const result = new Date(base)
  if (match) {
    result.setDate(result.getDate() + ({ 今天: 0, 明天: 1, 后天: 2, 大后天: 3 }[match[1]] ?? 0))
  } else if ((match = text.match(/^(下|本)?(?:周|星期)([一二三四五六日天])/))) {
    const target = WEEKDAYS[match[2]]
    const offset = match[1]
      ? -((base.getDay() + 6) % 7) + (match[1] === '下' ? 7 : 0) + ((target + 6) % 7)
      : (target - base.getDay() + 7) % 7
    result.setDate(result.getDate() + offset)
  } else if ((match = text.match(/^([0-9一二两三四五六七八九十]{1,3})\s*(天|周)后/))) {
    const count = number(match[1])
    if (count === null) return null
    result.setDate(result.getDate() + count * (match[2] === '周' ? 7 : 1))
  } else {
    match = text.match(/^(?:(\d{4})[-/])?(\d{1,2})[-/](\d{1,2})(?!\d|[-/])/)
    if (!match) return null
    let date = calendarDate(match[1] ? Number(match[1]) : base.getFullYear(), Number(match[2]), Number(match[3]))
    if (!date) return null
    if (!match[1] && date < base) date = calendarDate(base.getFullYear() + 1, Number(match[2]), Number(match[3]))
    return date ? { value: dateString(date), length: match[0].length } : null
  }
  return { value: dateString(result), length: match[0].length }
}
function timePrefix(text: string): Token | null {
  const match = text.match(/^(上午|早上|下午|晚上|中午)?\s*(?:(\d{1,2})[:：](\d{2})(?!\d|[:：])|([0-9零一二两三四五六七八九十]{1,3})点(?:(半|整)|([0-9零一二两三四五六七八九十]{1,3})分?)?)/)
  if (!match) return null
  let hour = number(match[2] || match[4])
  const minute = match[3] ? Number(match[3]) : match[5] === '半' ? 30 : match[6] ? number(match[6]) : 0
  if (hour === null || minute === null || hour > 23 || minute > 59) return null
  if (['下午', '晚上', '中午'].includes(match[1]) && hour < 12) hour += 12
  if (['上午', '早上'].includes(match[1]) && hour === 12) hour = 0
  return { value: `${String(hour).padStart(2, '0')}:${String(minute).padStart(2, '0')}`, length: match[0].length }
}

export function parseWhen(raw: string): ParsedWhen {
  let text = raw.trim()
  let date: string | null = null
  let time: string | null = null
  const base = today()
  // Accept either date-time or time-date order, without searching inside legal text.
  for (let count = 0; count < 2; count++) {
    const dateToken: Token | null = !date ? datePrefix(text, base) : null
    const timeToken: Token | null = !dateToken && !time ? timePrefix(text) : null
    if (dateToken) date = dateToken.value
    else if (timeToken) time = timeToken.value
    else break
    text = text.slice((dateToken || timeToken)!.length).replace(/^[\s，,]+/, '')
  }
  return { taskName: text || raw.trim(), date, time }
}

export function bucketForDate(date: string | null): 'inbox' | 'today' | 'anytime' {
  return !date ? 'inbox' : date === dateString(today()) ? 'today' : 'anytime'
}
