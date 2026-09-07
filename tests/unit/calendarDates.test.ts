import { describe, expect, it } from 'vitest'
import { surroundingMonths, monthWorkingDays, eventDuration } from '../../src/modules/calendar/calendarDates'

describe('calendar date boundaries and estimates', () => {
  it('loads neighboring months across year boundaries without month-end overflow', () => {
    expect(surroundingMonths(new Date(2026, 0, 31))).toEqual([
      { year: 2025, month: 12 }, { year: 2026, month: 1 }, { year: 2026, month: 2 },
    ])
  })
  it('uses calendar days and explicit working-day overrides', () => {
    expect(monthWorkingDays(new Date(2026, 8, 1), [])).toBe(22)
    expect(monthWorkingDays(new Date(2026, 8, 1), [{ date: '2026-09-01', kind: 'holiday' }])).toBe(21)
    expect(monthWorkingDays(new Date(2026, 8, 1), [{ date: '2026-09-05', kind: 'workday' }])).toBe(23)
    expect(monthWorkingDays(new Date(2026, 1, 1), [])).toBe(20)
  })
  it('never invents a duration when an endpoint is missing', () => {
    expect(eventDuration('09:30', null)).toBe('')
    expect(eventDuration('09:30', '10:15')).toBe('45 分钟')
    expect(eventDuration('11:00', '10:15')).toBe('')
  })
})
