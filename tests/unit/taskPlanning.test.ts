import { describe, expect, it } from 'vitest'
import { clipPlan, dayDistance, movePlan, planningWarnings, rangeError } from '../../src/modules/calendar/taskPlanning'
import { yearTaskDays } from '../../src/modules/calendar/yearHeatmap'

describe('independent task planning', () => {
  it('moves and resizes across months, leap days and years without modifying deadlines', () => {
    const source={start:'2027-12-30',end:'2028-01-02'}
    expect(movePlan(source,3,'move')).toEqual({start:'2028-01-02',end:'2028-01-05'})
    expect(movePlan(source,10,'start')).toEqual({start:'2028-01-02',end:'2028-01-02'})
    expect(movePlan(source,-10,'end')).toEqual({start:'2027-12-30',end:'2027-12-30'})
    expect(dayDistance('2028-02-28','2028-03-01')).toBe(2)
    expect(dayDistance('2026-03-07','2026-03-10')).toBe(3)
    expect(source).toEqual({start:'2027-12-30',end:'2028-01-02'})
  })
  it('clips long plans without changing their actual dates', () => {
    expect(clipPlan({start:'2026-08-01',end:'2026-12-01'},'2026-09-25',28)).toEqual({offset:0,span:28,clippedStart:true,clippedEnd:true})
    expect(clipPlan({start:'2026-10-23',end:'2026-10-24'},'2026-09-25',28)).toBeNull()
    expect(clipPlan({start:'2026-10-22',end:'2026-10-22'},'2026-09-25',28)?.span).toBe(1)
  })
  it('rejects invalid, inverted and unbounded ranges', () => {
    for(const range of [{start:'2026-02-29',end:'2026-03-01'},{start:'2026-9-01',end:'2026-09-02'},{start:'2026-10-02',end:'2026-10-01'},{start:'2026-01-01',end:'2040-01-01'}])expect(rangeError(range)).toBeTruthy()
    expect(rangeError({start:'2028-02-29',end:'2028-02-29'})).toBe('')
  })
  it('warns on rest, workload overlap and deadline crossing while personal work overrides official rest', () => {
    const task={id:'a',taskName:'A',caseId:'c',dueDate:'2026-09-25'}
    const warnings=planningWarnings(task,{start:'2026-09-25',end:'2026-09-28'},[task,{id:'b',taskName:'B'}],[{taskId:'b',startDate:'2026-09-28',endDate:'2026-09-28',revision:1}],
      [{date:'2026-09-25',source:'official',kind:'holiday',name:'休'},{date:'2026-09-25',source:'personal',kind:'workday',name:'班'}],
      [{date:'2026-09-26',title:'举证期限',caseId:'c',type:'deadline'}])
    expect(warnings).toHaveLength(4)
    expect(warnings.join(' ')).toContain('2 个休息日')
    expect(task.dueDate).toBe('2026-09-25')
  })
  it('uses explicit plans in the year heatmap, including clear without legacy date fallback', () => {
    const task={id:'a',startDate:'2026-01-01',dueDate:'2026-12-31',planDefined:true,plannedStartDate:'2026-09-25',plannedEndDate:'2026-09-27'}
    expect([...yearTaskDays(2026,[task]).keys()]).toEqual(['2026-09-25','2026-09-26','2026-09-27'])
    expect(yearTaskDays(2026,[{...task,plannedStartDate:null,plannedEndDate:null}]).size).toBe(0)
  })
})
