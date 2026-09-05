import { describe, expect, it } from 'vitest'
import { buildGtdStats, tasksForPerspective } from '../../src/modules/tasks/utils/taskFilter'

describe('GTD availability', () => {
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
