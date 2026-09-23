// Synthetic services for visual regression checks. No IPC, database or persisted settings.
export const caseName = '测试案件 · 超长名称知识产权许可合同争议与关联证据核查'.repeat(3)
export const tasks = [
  { id: 'task1', taskName: '核对证据目录', startDate: '2026-09-25', dueDate: '2026-09-25', startTime: '09:00', estimatedMinutes: 90, caseId: 'case1', caseName, description: '原始任务说明', completed: 0 },
  { id: 'task2', taskName: '长标题校验_' + 'EvidenceReference'.repeat(10), startDate: '2026-09-25', dueDate: '2026-09-27', estimatedMinutes: 120, caseId: 'case1', caseName, completed: 0 },
  { id: 'task3', taskName: '次日提交材料', startDate: '2026-09-26', dueDate: '2026-09-26', estimatedMinutes: 30, completed: 0 },
]
export const events = [
  { id: 'event1', type: 'event', title: '独立日程', date: '2026-09-25', startTime: '11:00', endTime: '12:00', notes: '原始日程备注', caseId: 'case1', caseName },
  { id: 'hearing1', type: 'hearing', title: '开庭核对', date: '2026-09-25', startTime: '14:00', endTime: '15:30', caseId: 'case1', caseName },
  { id: 'deadline1', type: 'deadline_appeal', title: '上诉期限', date: '2026-09-25', caseId: 'case1', caseName },
  { id: 'task1', type: 'task', title: '核对证据目录', date: '2026-09-25', startTime: '09:00', caseId: 'case1', caseName },
]
export const holidays = [
  { date: '2026-09-20', name: '调休工作日', source: 'official', kind: 'workday' },
  ...[25,26,27].map(day => ({ date: `2026-09-${day}`, name: '中秋节', source: 'official', kind: 'holiday' })),
  { date: '2026-09-25', name: '个人休息安排', source: 'personal', kind: 'holiday' },
  { date: '2026-09-28', name: '个人补班安排', source: 'personal', kind: 'workday' },
]
const ok = data => Promise.resolve({ ok: true, data })
export function fixtureServices(mode = 'mixed') {
  const work = mode === 'mixed' || mode === 'gantt'
  const plans = mode === 'gantt' ? [{taskId:'task1',startDate:'2026-09-25',endDate:'2026-09-27',revision:1},{taskId:'task2',startDate:'2026-09-24',endDate:'2026-10-04',revision:1}] : []
  return {
    calendar: {
      taskPlans: () => ok(plans.map(p=>({...p}))),
      saveTaskPlan: data => { const old=plans.find(p=>p.taskId===data.taskId); if((old?.revision||0)!==data.expectedRevision)return Promise.resolve({ok:false,error:'PLAN_CONFLICT'}); const plan={taskId:data.taskId,startDate:data.startDate,endDate:data.endDate,revision:data.expectedRevision+1}; const index=plans.findIndex(p=>p.taskId===data.taskId); if(index>=0)plans[index]=plan;else plans.push(plan);return ok({...plan}) },
      events: () => ok(work ? events : []), listEvents: () => ok(work ? events.filter(e => e.type === 'event') : []),
      deadlineWarnings: () => ok([]), holidays: year => ok({ year, entries: mode === 'empty' ? [] : holidays.filter(e => e.date.startsWith(`${year}-`)) }),
    },
    tasks: { list: () => ok(work ? tasks : []) },
    cases: { list: () => ok({ items: [{ id: 'case1', caseName }, { id: 'case2', caseName: '另一案件' }], total: 2 }) },
    settings: { get: () => ok(null) },
  }
}
