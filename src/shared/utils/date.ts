/**
 * 本地时区日期工具（审查 P1-2）
 *
 * 背景：全仓原有 19 处用 `new Date(str)` + `toISOString().split('T')[0]` 推导日期。
 * 这个组合在东八区（全部目标用户）有 8 小时的系统性错误——
 *
 *   `new Date('2026-09-04')`        → 按 **UTC 午夜**解析
 *   `new Date()`                    → 本地当下时刻
 *   `.toISOString()`                → 转回 UTC
 *
 * 于是北京时间 00:00–07:59 期间，`new Date().toISOString()` 得到的是**前一天**
 * 的 UTC 时间戳，"今天"整体偏早一天。后果包括：逾期任务不被判为逾期、
 * 早报在最该出现的清晨被跳过、"逾期"筛选筛不出今日刚逾期的案件。
 *
 * 统一约定：
 *   - 数据库与前端流转的日期一律是 `'YYYY-MM-DD'` 纯日期串，按**本地午夜**解析
 *   - 取"今天"一律用 `todayLocalISO()`，绝不用 `toISOString()`
 */

/** 按本地午夜解析 'YYYY-MM-DD'；非法输入返回 null */
export function parseLocalDate(value: string | null | undefined): Date | null {
  if (!value) return null
  const match = String(value).trim().match(/^(\d{4})-(\d{2})-(\d{2})/)
  if (!match) return null
  const [, y, m, d] = match
  const year = Number(y)
  const month = Number(m)
  const day = Number(d)
  const date = new Date(year, month - 1, day)
  // JS Date 会把越界日期 rollover（如 2026-02-31 → 2026-03-03、2026-13-01 → 2027-01-01）。
  // 需回验 年/月/日 三者一致，才认定是合法日期；否则返回 null。
  if (
    date.getFullYear() !== year ||
    date.getMonth() !== month - 1 ||
    date.getDate() !== day
  ) {
    return null
  }
  return Number.isNaN(date.getTime()) ? null : date
}

/** Date → 本地 'YYYY-MM-DD'（不是 toISOString，后者是 UTC） */
export function toLocalISODate(date: Date): string {
  const y = date.getFullYear()
  const m = String(date.getMonth() + 1).padStart(2, '0')
  const d = String(date.getDate()).padStart(2, '0')
  return `${y}-${m}-${d}`
}

/** 本地今天的 'YYYY-MM-DD' */
export function todayLocalISO(): string {
  return toLocalISODate(new Date())
}

/** 在给定日期上加减天数，返回本地 'YYYY-MM-DD' */
export function addDaysLocalISO(iso: string, days: number): string {
  const date = parseLocalDate(iso)
  if (!date) return iso
  date.setDate(date.getDate() + days)
  return toLocalISODate(date)
}

/**
 * 截止日距今天的天数（负数=已逾期）；无效日期返回 null。
 * 两侧均按本地午夜解析——原本 `new Date('YYYY-MM-DD')` 按 UTC 解析，
 * 在东八区会把"今天"算成 +1 天。
 */
export function daysUntil(deadline: string | null | undefined): number | null {
  const d = parseLocalDate(deadline)
  if (!d) return null
  const today = parseLocalDate(todayLocalISO())!
  return Math.round((d.getTime() - today.getTime()) / 86400000)
}

/** 是否早于今天（用于逾期判定） */
export function isBeforeToday(dateStr: string | null | undefined): boolean {
  if (!dateStr) return false
  return dateStr < todayLocalISO()
}

/** SQLite timestamps without an offset are local; explicit offsets retain their meaning. */
export function parseTimestamp(value: string | null | undefined): Date | null {
  if (!value) return null
  const text = value.trim()
  if (!/^\d{4}-\d{2}-\d{2}(?:[T ]\d{2}:\d{2}(?::\d{2}(?:\.\d+)?)?(?:Z|[+-]\d{2}:?\d{2})?)?$/.test(text)) return null
  if (!parseLocalDate(text)) return null
  const time = text.match(/[T ](\d{2}):(\d{2})(?::(\d{2}))?/)
  if (time && (Number(time[1]) > 23 || Number(time[2]) > 59 || Number(time[3] || 0) > 59)) return null
  const date = text.length === 10 ? parseLocalDate(text) : new Date(text.replace(' ', 'T'))
  return date && Number.isFinite(date.getTime()) ? date : null
}
export function formatTimestamp(value: string | null | undefined): string {
  const date = parseTimestamp(value)
  if (!date) return '时间未知'
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${toLocalISODate(date)} ${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
}
export function relativeTimestamp(value: string | null | undefined): string {
  const date = parseTimestamp(value)
  if (!date) return '时间未知'
  const elapsed = Date.now() - date.getTime()
  if (elapsed < 0) return formatTimestamp(value)
  if (elapsed < 60000) return '刚刚'
  if (elapsed < 3600000) return `${Math.floor(elapsed / 60000)} 分钟前`
  if (elapsed < 86400000) return `${Math.floor(elapsed / 3600000)} 小时前`
  return formatTimestamp(value)
}
