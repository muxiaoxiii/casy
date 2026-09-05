import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { parseWhen } from '../../src/shared/nlp/parseWhen'

beforeEach(() => { vi.useFakeTimers(); vi.setSystemTime(new Date(2026, 8, 6, 10)) })
afterEach(() => vi.useRealTimers())
describe('capture date and time prefixes', () => {
  it.each([
    ['明天下午三点半提交材料', '2026-09-07', '15:30', '提交材料'],
    ['上午十一点十五分 下周一联系客户', '2026-09-07', '11:15', '联系客户'],
    ['下周一下午3:45 开庭', '2026-09-07', '15:45', '开庭'],
    ['2026-12-31 09:30 核对期限', '2026-12-31', '09:30', '核对期限'],
    ['十一天后回访', '2026-09-17', null, '回访'],
    ['2026-02-30核对材料', null, null, '2026-02-30核对材料'],
    ['02/30核对材料', null, null, '02/30核对材料'],
    ['25:00核对材料', null, null, '25:00核对材料'],
    ['9:70核对材料', null, null, '9:70核对材料'],
    ['合同第09-07号审查', null, null, '合同第09-07号审查'],
  ])('parses %s without silently rolling invalid dates', (input, date, time, taskName) => {
    expect(parseWhen(input)).toEqual({ date, time, taskName })
  })
  it('keeps next week within the next calendar week even when the weekday has not passed', () => {
    vi.setSystemTime(new Date(2026, 8, 7, 10))
    expect(parseWhen('下周五提交').date).toBe('2026-09-18')
    expect(parseWhen('下周一提交').date).toBe('2026-09-14')
    expect(parseWhen('本周一提交').date).toBe('2026-09-07')
  })
})
