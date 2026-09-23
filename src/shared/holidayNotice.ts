export interface HolidayDraft { year: number; holidays: string[]; workdays: string[] }
export function parseHolidayDraft(text: string): HolidayDraft {
  const raw = JSON.parse(text.replace(/^\s*```(?:json)?\s*/i, '').replace(/\s*```\s*$/, ''))
  if (!Number.isInteger(raw.year) || raw.year < 1900 || raw.year > 2200) throw new Error('请提供 1900—2200 之间的通知年份')
  const dates = (value: unknown): string[] => {
    if (!Array.isArray(value) || value.length > 1500) throw new Error('holidays / workdays 必须是日期数组')
    return [...new Set(value.map(date => {
      if (typeof date !== 'string' || !/^\d{4}-\d{2}-\d{2}$/.test(date)) throw new Error('日期须为 YYYY-MM-DD')
      const [y, m, d] = date.split('-').map(Number)
      const parsed = new Date(y, m - 1, d)
      if (y < 1900 || y > 2200 || parsed.getFullYear() !== y || parsed.getMonth() !== m - 1 || parsed.getDate() !== d) throw new Error(`无效日期：${date}`)
      return date
    }))].sort()
  }
  const holidays = dates(raw.holidays), workdays = dates(raw.workdays)
  if (!holidays.length && !workdays.length) throw new Error('请至少提供一个放假或补班日期')
  if (holidays.some(date => workdays.includes(date))) throw new Error('同一天不能既放假又补班')
  return { year: raw.year, holidays, workdays }
}
