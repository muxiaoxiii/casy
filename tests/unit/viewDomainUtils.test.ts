import { describe, expect, it } from 'vitest'
import { filterAndSortFiles, formatFileSize, getFileExt } from '../../src/modules/cases/lib/fileUtils'
import { extractMemoIntent, parseMemosFromNotes } from '../../src/modules/cases/lib/memoUtils'
import { emptyEditForm, toSavePayload } from '../../src/modules/tasks/utils/taskForm'
import { buildGtdStats, tasksForPerspective } from '../../src/modules/tasks/utils/taskFilter'
import { fieldTypeLabel, filterFieldRows, mapToFieldRows } from '../../src/modules/docs/utils/fieldMapping'

const task = (overrides: Record<string, unknown> = {}) => ({
  id: 't1', taskName: '提交答辩状', description: null, createdDate: '2026-01-01', deadline: null,
  priority: 'normal', completed: 0, assignee: null, finishNote: null, taskType: 'action',
  startDate: null, dueDate: null, dueTime: null, waitingFor: null, followUpDate: null,
  context: null, flagged: 0, sequential: 0, blocked: 0, sequenceOrder: 0,
  startBucket: 'inbox', todayIndex: 0, estimatedMinutes: null, actualMinutes: null,
  isOverdue: 0, dueSoon: 0, lastReviewDate: null, nextReviewDate: null,
  areaId: null, knowledgeId: null, caseId: null, ...overrides,
}) as any

describe('view domain extraction helpers', () => {
  it('filters and sorts case files without mutating source', () => {
    const source = [
      { fileName: 'B.pdf', category: 'evidence', createdAt: '2026-01-01', filePath: '/evidence/B.pdf' },
      { fileName: 'A.docx', category: 'submitted', createdAt: '2026-02-01', filePath: '/submitted/A.docx' },
    ]
    expect(filterAndSortFiles(source, { activeCategory: 'evidence' })).toHaveLength(1)
    expect(filterAndSortFiles(source, { sortOrder: 'name' })[0].fileName).toBe('A.docx')
    expect(source[0].fileName).toBe('B.pdf')
    expect(formatFileSize(1024)).toBe('1 KB')
    expect(getFileExt('a.docx')).toBe('DOCX')
  })

  it('parses memo notes and detects task intent', () => {
    expect(parseMemosFromNotes('[{"id":"1","title":"x","content":"y","date":"2026-01-01"}]')).toHaveLength(1)
    expect(extractMemoIntent('请于9月20日提交答辩状').detection).toBe('task')
  })

  it('maps task form and perspectives consistently', () => {
    const form = emptyEditForm('inbox')
    form.taskName = '新任务'
    form.flagged = true
    expect(toSavePayload(null, form)).toMatchObject({ taskName: '新任务', flagged: 1, deferUntil: null })
    const tasks = [task(), task({ id: 't2', completed: 1 })]
    expect(buildGtdStats(tasks, '2026-01-01')).toMatchObject({ all: 1, inbox: 1, completed: 1 })
    expect(tasksForPerspective(tasks, 'completed', { todayStr: '2026-01-01' })).toHaveLength(1)
  })

  it('maps and filters document field rows', () => {
    const rows = mapToFieldRows({ 当事人: [{ name: '张三', suffix: '原告' }], 胜诉: true })
    expect(rows[0].value).toBe('张三(原告)')
    expect(filterFieldRows(rows, '胜诉')).toHaveLength(1)
    expect(fieldTypeLabel('party_list')).toBe('当事人')
  })
})
