/**
 * 日历任务富化（审查 3.1-5b）
 *
 * TaskDto 没有 case_name 字段，而时间线卡片、案件名搜索、未排期池与移动条都要读
 * task.caseName（旧代码因此恒显示「常规待办」、案件名搜索恒 0）。用已加载案件构造
 * caseId→案件名映射，在任务装载后补上该字段；不改原对象，返回新数组。
 */

interface CaseNameSource {
  id: string
  caseName?: string | null
  caseNo?: string | null
}

interface CaseNameTarget {
  caseId?: string | null
  caseName?: string | null
}

/** caseId → 展示名（案件名优先，缺失时退回案号） */
export function buildCaseNameMap(cases: CaseNameSource[]): Map<string, string> {
  const map = new Map<string, string>()
  for (const item of cases) {
    if (item?.id) map.set(item.id, item.caseName || item.caseNo || '')
  }
  return map
}

/** 为任务补齐 caseName；已有 caseName、无 caseId 或案件未加载时保持原样 */
export function enrichTasksWithCaseName<T extends CaseNameTarget>(tasks: T[], cases: CaseNameSource[]): T[] {
  if (!Array.isArray(tasks) || !Array.isArray(cases) || !cases.length) return tasks
  const nameByCaseId = buildCaseNameMap(cases)
  return tasks.map(task => {
    if (task.caseName || !task.caseId) return task
    const caseName = nameByCaseId.get(task.caseId)
    return caseName ? { ...task, caseName } : task
  })
}
