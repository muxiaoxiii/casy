import { describe, expect, it } from 'vitest'
import {
  parseLocalDate,
  toLocalISODate,
  todayLocalISO,
  addDaysLocalISO,
  daysUntil,
  isBeforeToday,
} from '../src/shared/utils/date'

describe('shared/utils/date · parseLocalDate', () => {
  it('合法日期按本地午夜解析，年月日一致', () => {
    const d = parseLocalDate('2026-09-04')!
    expect(d.getFullYear()).toBe(2026)
    expect(d.getMonth()).toBe(8) // 0-based: 9月
    expect(d.getDate()).toBe(4)
  })

  it('空值/非法/非 YYYY-MM-DD 输入返回 null', () => {
    expect(parseLocalDate(null)).toBeNull()
    expect(parseLocalDate(undefined)).toBeNull()
    expect(parseLocalDate('')).toBeNull()
    expect(parseLocalDate('not-a-date')).toBeNull()
    expect(parseLocalDate('2026-9-4')).toBeNull() // 未补零
    expect(parseLocalDate('2026/09/04')).toBeNull() // 分隔符不对
  })

  it('越界日期不回滚（2026-02-31 / 2026-04-31 / 2026-13-01 应为 null）', () => {
    // JS Date 默认会把 2/31 rollover 成 3/3，parseLocalDate 必须拒绝
    expect(parseLocalDate('2026-02-31')).toBeNull()
    expect(parseLocalDate('2026-04-31')).toBeNull()
    expect(parseLocalDate('2026-13-01')).toBeNull()
    expect(parseLocalDate('2026-00-10')).toBeNull()
    expect(parseLocalDate('2026-09-32')).toBeNull()
  })
})

describe('shared/utils/date · 日期加减与比较', () => {
  it('todayLocalISO 与 toLocalISODate 一致（均为本地日期）', () => {
    expect(todayLocalISO()).toBe(toLocalISODate(new Date()))
  })

  it('addDaysLocalISO: 加减天数', () => {
    expect(addDaysLocalISO('2026-09-04', 1)).toBe('2026-09-05')
    expect(addDaysLocalISO('2026-09-04', -1)).toBe('2026-09-03')
    // 跨月
    expect(addDaysLocalISO('2026-09-30', 1)).toBe('2026-10-01')
  })

  it('daysUntil: 今天=0，昨天=-1，明天=1', () => {
    const d = (offset: number) => {
      const t = new Date()
      t.setDate(t.getDate() + offset)
      return toLocalISODate(t)
    }
    expect(daysUntil(d(0))).toBe(0)
    expect(daysUntil(d(1))).toBe(1)
    expect(daysUntil(d(-1))).toBe(-1)
    expect(daysUntil('not-a-date')).toBeNull()
  })

  it('isBeforeToday: 早于今天为真，今天/未来为假', () => {
    const d = (offset: number) => {
      const t = new Date()
      t.setDate(t.getDate() + offset)
      return toLocalISODate(t)
    }
    expect(isBeforeToday(d(-1))).toBe(true)
    expect(isBeforeToday(d(0))).toBe(false)
    expect(isBeforeToday(d(1))).toBe(false)
  })
})
