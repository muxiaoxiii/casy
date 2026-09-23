import { describe, expect, it, vi } from 'vitest'
import { CalendarService } from '../../src/core/services/calendar'
import type { CasyContext } from '../../src/core/plugin/types'

const request = vi.hoisted(() => vi.fn())
vi.mock('../../src/core/tauriBridge', () => ({ tauriCallSafe: request }))

describe('calendar projection contract', () => {
  it('maps the native eventType while preserving timing and case identity', async () => {
    request.mockResolvedValue({ ok: true, data: [{
      id: 'e', date: '2026-09-05', title: 'Hearing', eventType: 'hearing', caseId: 'c', caseName: 'Matter',
      startTime: '09:30', endTime: '10:30', allDay: false,
    }] })
    const result = await new CalendarService({} as CasyContext).events(2026, 9)
    expect(request).toHaveBeenCalledWith('get_calendar_events', { year: 2026, month: 9 })
    expect(result.data?.[0]).toMatchObject({ type: 'hearing', startTime: '09:30', caseId: 'c', color: 'var(--c-danger)' })
  })
  it('preserves a service failure without manufacturing an empty successful month', async () => {
    request.mockResolvedValue({ ok: false, error: 'offline' })
    expect(await new CalendarService({} as CasyContext).events(2026, 9)).toEqual({ ok: false, error: 'offline' })
  })
  it('sends only independent plan dates and revision and broadcasts successful changes', async () => {
    const emit=vi.fn(), service=new CalendarService({ emit } as unknown as CasyContext)
    const data={taskId:'t',startDate:'2026-09-25',endDate:'2026-09-28',expectedRevision:2}
    request.mockResolvedValue({ok:true,data:{...data,revision:3}})
    await service.saveTaskPlan(data)
    expect(request).toHaveBeenLastCalledWith('save_task_plan',{data})
    expect(emit).toHaveBeenCalledWith('plan:changed')
    request.mockResolvedValue({ok:false,error:'PLAN_CONFLICT'});emit.mockClear()
    await service.saveTaskPlan(data)
    expect(emit).not.toHaveBeenCalled()
  })

})
