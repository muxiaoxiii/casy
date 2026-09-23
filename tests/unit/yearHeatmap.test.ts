import { expect, it } from 'vitest'
import { yearTaskDays, heatLevel } from '../../src/modules/calendar/yearHeatmap'
import { parseHolidayDraft } from '../../src/shared/holidayNotice'
it('counts dated tasks once per day, including completed and cross-year work, without undated tasks', () => {
  const tasks = [{ id: 'a', startDate: '2027-12-31', dueDate: '2028-01-02' }, { id: 'b', dueDate: '2028-01-02', completed: true }, { id: 'b', dueDate: '2028-01-02' }, { id: 'c' }]
  const days = yearTaskDays(2028, tasks)
  expect([...days.keys()]).toEqual(['2028-01-01', '2028-01-02'])
  expect(days.get('2028-01-02')).toHaveLength(2)
  expect(yearTaskDays(2028, [{ id: 'leap', startDate: '2028-02-28', dueDate: '2028-03-01' }]).size).toBe(3)
  expect(yearTaskDays(2027, [{ id: 'long', startDate: '2000-01-01', dueDate: '2040-01-01' }]).size).toBe(365)
  expect([0,1,2,3,4,6,7,100].map(heatLevel)).toEqual([0,1,2,2,3,3,4,4])
})
it('rejects invalid or contradictory AI JSON and preserves reviewed dates', () => {
  expect(() => parseHolidayDraft('{"year":2027,"holidays":["2027-02-29"],"workdays":[]}')).toThrow('无效日期')
  expect(() => parseHolidayDraft('{"year":2027,"holidays":["2027-01-01"],"workdays":["2027-01-01"]}')).toThrow('同一天')
  expect(() => parseHolidayDraft('{"error":"未识别"}')).toThrow()
  expect(parseHolidayDraft('```json\n{"year":2028,"holidays":["2028-02-29","2028-02-29"],"workdays":[]}\n```').holidays).toEqual(['2028-02-29'])
})
