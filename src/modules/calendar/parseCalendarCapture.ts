import { parseWhen } from '../../shared/nlp/parseWhen'
import type { CalendarEventInput } from '../../types/ipc'

export function minutes(time: string) {
  const match = time.match(/^(\d{2}):(\d{2})$/)
  if (!match || Number(match[1]) > 23 || Number(match[2]) > 59) return null
  return Number(match[1]) * 60 + Number(match[2])
}
export function timeString(value: number) { return `${String(Math.floor(value / 60)).padStart(2, '0')}:${String(value % 60).padStart(2, '0')}` }

/** Deliberately parse explicit prefixes/suffixes, leaving legal prose intact. */
export function parseCalendarCapture(raw: string, selectedDate: string): { event: CalendarEventInput; error: string } {
  const parsed = parseWhen(raw)
  let title = parsed.taskName
  let error = ''
  let duration = 60
  let endTime: string | null = null
  let location: string | null = null
  const end = title.match(/^(?:-|–|—|至|到)\s*(\d{1,2}[:：]\d{2})\s*/)
  if (end && parsed.time) {
    endTime = end[1].replace('：', ':').padStart(5, '0')
    title = title.slice(end[0].length)
    if (minutes(endTime) === null || minutes(endTime)! <= minutes(parsed.time)!) error = '结束时间需晚于开始时间，并在同一天内'
  }
  const where = title.match(/\s+[@＠]([^@＠]+)$/)
  if (where) { location = where[1].trim(); title = title.slice(0, where.index).trim() }
  const length = title.match(/(?:\s+|持续)(\d+(?:\.\d+)?)\s*(分钟|小时)$/)
  if (length) {
    duration = Number(length[1]) * (length[2] === '小时' ? 60 : 1)
    title = title.slice(0, length.index).trim()
    if (duration <= 0 || !Number.isInteger(duration)) error = '时长需为大于 0 的整分钟'
  }
  if (parsed.time && !endTime) {
    const endMinute = minutes(parsed.time)! + duration
    if (endMinute >= 1440) error = '时间块跨越午夜，请拆分为两条日程'
    else endTime = timeString(endMinute)
  }
  if ((!parsed.time && /^(?:(?:今天|明天|后天)\s*)?(?:上午|下午|晚上|早上|中午)?\s*\d{1,2}(?:[:：]\d{2}|点)/.test(raw.trim())) || (!parsed.date && /^\d{4}[-/]\d{1,2}[-/]\d{1,2}/.test(raw.trim()))) error = '日期或时间无法识别，请检查输入'
  if (!raw.trim() || !title.trim() || ((parsed.date || parsed.time) && parsed.taskName === raw.trim())) error = '请在日期时间后填写日程名称'
  return { event: { title: title.trim(), eventDate: parsed.date || selectedDate, startTime: parsed.time, endTime, allDay: !parsed.time, location }, error }
}

export function overlaps(start: string, end: string, otherStart: string | null | undefined, otherEnd: string | null | undefined) {
  if (!otherStart) return false
  const a = minutes(start), b = minutes(end), c = minutes(otherStart.slice(0, 5)), d = otherEnd ? minutes(otherEnd.slice(0, 5)) : null
  return a !== null && b !== null && c !== null && a < (d ?? c + 60) && c < b
}
