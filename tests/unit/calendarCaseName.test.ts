import { describe, expect, it } from 'vitest'
import { buildCaseNameMap, enrichTasksWithCaseName } from '../../src/modules/calendar/enrichTasks'

// 审查 3.1-5b：TaskDto 没有 case_name，日历各处读 task.caseName 曾恒为兜底文案。
describe('calendar task caseName enrichment', () => {
  const cases = [
    { id: 'case1', caseName: '知识产权许可合同争议' },
    { id: 'case2', caseName: null, caseNo: '（2026）京01民初1号' },
  ]

  it('attaches the loaded case name to tasks that only carry caseId', () => {
    const tasks = [{ id: 't1', caseId: 'case1' }, { id: 't2', caseId: 'case2' }]
    expect(enrichTasksWithCaseName(tasks, cases)).toEqual([
      { id: 't1', caseId: 'case1', caseName: '知识产权许可合同争议' },
      { id: 't2', caseId: 'case2', caseName: '（2026）京01民初1号' },
    ])
  })

  it('leaves already-enriched, case-less and unknown-case tasks untouched', () => {
    const tasks = [
      { id: 't1', caseId: 'case1', caseName: '后端已提供' },
      { id: 't2' },
      { id: 't3', caseId: 'missing' },
    ]
    expect(enrichTasksWithCaseName(tasks, cases)).toEqual(tasks)
  })

  it('is a no-op while no case is loaded', () => {
    const tasks = [{ id: 't1', caseId: 'case1' }]
    expect(enrichTasksWithCaseName(tasks, [])).toBe(tasks)
  })

  it('falls back to the case number when the case name is empty', () => {
    expect(buildCaseNameMap(cases).get('case2')).toBe('（2026）京01民初1号')
    expect(buildCaseNameMap(cases).get('nope')).toBeUndefined()
  })
})
