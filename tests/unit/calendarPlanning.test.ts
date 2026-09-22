import { afterEach, describe, expect, it, vi } from 'vitest'
import { parseCalendarCapture, overlaps } from '../../src/modules/calendar/parseCalendarCapture'
import { layoutTimedItems, type TimedItem } from '../../src/modules/calendar/timeLayout'

afterEach(() => vi.useRealTimers())
describe('calendar capture', () => {
  it('previews explicit date, duration and location without retaining syntax in the title', () => {
    vi.useFakeTimers(); vi.setSystemTime(new Date(2026, 8, 13, 8))
    expect(parseCalendarCapture('明天下午3点 讨论证据 1小时 @会议室', '2026-10-01')).toEqual({ error: '', event: {
      title: '讨论证据', eventDate: '2026-09-14', startTime: '15:00', endTime: '16:00', allDay: false, location: '会议室',
    } })
  })
  it('supports explicit time ranges and preserves date-like legal prose', () => {
    expect(parseCalendarCapture('14:00–15:30 核对证据', '2026-09-13').event).toMatchObject({ title: '核对证据', startTime: '14:00', endTime: '15:30' })
    expect(parseCalendarCapture('研究三天后送达的法律效果', '2026-09-13').event).toMatchObject({ title: '研究三天后送达的法律效果', allDay: true })
    expect(parseCalendarCapture('23:30 核对证据 1小时', '2026-09-13').error).toBeTruthy()
    expect(parseCalendarCapture('14:00–13:30 核对证据', '2026-09-13').error).toBeTruthy()
  })
  it('does not report adjacent events as overlapping', () => {
    expect(overlaps('09:00', '10:00', '10:00', '11:00')).toBe(false)
    expect(overlaps('09:00', '10:00', '09:30', '11:00')).toBe(true)
  })
})
describe('calendar time layout', () => {
  const item = (id: string, startTime: string, endTime: string): TimedItem => ({ id, startTime, endTime, title: id, kind: 'event' })
  it('allocates lanes across a connected overlap cluster and resets for the next group', () => {
    const result = layoutTimedItems([item('a', '09:00', '10:30'), item('b', '10:00', '11:00'), item('c', '10:30', '11:30'), item('d', '13:00', '14:00')])
    expect(result.map(e => [e.id, e.lane, e.lanes])).toEqual([['a', 0, 2], ['b', 1, 2], ['c', 0, 2], ['d', 0, 1]])
    expect(result[0].end - result[0].start).toBe(90)
  })
  it('discards invalid times and keeps three simultaneous events visible', () => {
    const result = layoutTimedItems([item('bad', '25:00', '26:00'), item('a', '09:00', '11:00'), item('b', '09:00', '10:00'), item('c', '09:30', '10:30')])
    expect(result).toHaveLength(3)
    expect(new Set(result.map(e => e.lane)).size).toBe(3)
    expect(result.every(e => e.lanes === 3)).toBe(true)
  })
})
