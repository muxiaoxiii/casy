import { describe, expect, it } from 'vitest'
import { applyTaskCardFilters, buildGtdStats, tasksForPerspective } from '../../src/modules/tasks/utils/taskFilter'

describe('GTD availability', () => {
  it('matches overdue waiting metrics for both waiting types and named waiting parties', () => {
    const tasks = [
      { id: 'type', taskType: 'waiting', followUpDate: '2026-09-05' },
      { id: 'person', taskType: 'action', waitingFor: '代理人', followUpDate: '2026-09-05' },
      { id: 'blank', taskType: 'waiting', followUpDate: '' },
      { id: 'today', taskType: 'waiting', followUpDate: '2026-09-06' },
      { id: 'done', taskType: 'waiting', followUpDate: '2026-09-05', completed: 1 },
    ] as any
    expect(applyTaskCardFilters(tasks, { metric: 'waitingOverdue', todayStr: '2026-09-06' }).map(task => task.id)).toEqual(['type', 'person'])
  })
  it('keeps clarified tasks out of inbox and future/blocked tasks out of next actions', () => {
    const tasks = [
      { id:'inbox',startBucket:'inbox' }, {id:'ready',startBucket:'anytime'},
      { id:'blocked',startBucket:'anytime',blocked:1 }, {id:'future',startBucket:'anytime',startDate:'2026-10-01'},
      { id:'someday',startBucket:'someday' }, {id:'waiting',startBucket:'anytime',waitingFor:'客户'},
      { id:'deadline',startBucket:'anytime',dueDate:'2026-09-01',deferUntil:'2026-10-01'},
    ].map(task => ({ completed:0,taskType:'action',...task })) as any
    expect(tasksForPerspective(tasks,'inbox',{todayStr:'2026-09-06'}).map(t=>t.id)).toEqual(['inbox'])
    expect(tasksForPerspective(tasks,'next',{todayStr:'2026-09-06'}).map(t=>t.id)).toEqual(['ready'])
    expect(tasksForPerspective(tasks,'today',{todayStr:'2026-09-06'}).map(t=>t.id)).toEqual(['deadline'])
    expect(buildGtdStats(tasks,'2026-09-06')).toMatchObject({inbox:1,next:1,today:1,waiting:1})
  })
})
