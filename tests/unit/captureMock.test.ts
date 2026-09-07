import { describe, expect, it } from 'vitest'
import { tryMockCommand } from '../../src/core/mockData'

describe('browser capture contract', () => {
  it('creates a retrievable task before marking the source as filed', () => {
    const id = tryMockCommand('add_inbox_item', { sourceType: 'note', contentText: '核对测试材料' })
    const result = tryMockCommand('confirm_inbox_action', {
      inboxItemId: id, action: 'create_task', targetCaseId: 'c1',
      intent: { taskName: '等待测试反馈', taskType: 'waiting', waitingFor: '测试对象', followUpDate: '2026-09-07' },
    }) as { task: { id: string } }
    const tasks = tryMockCommand('list_tasks', { filter: { perspective: 'waiting' } }) as Array<Record<string, unknown>>
    expect(tasks.find(task => task.id === result.task.id)).toMatchObject({ taskName: '等待测试反馈', caseId: 'c1', followUpDate: '2026-09-07' })
    expect(tryMockCommand('list_inbox_items', { status: 'filed' })).toEqual(expect.arrayContaining([expect.objectContaining({ id })]))
  })

  it('does not claim an unsupported action was completed', () => {
    const id = tryMockCommand('add_inbox_item', { sourceType: 'note', contentText: '保存测试知识' })
    expect(tryMockCommand('confirm_inbox_action', { inboxItemId: id, action: 'save_knowledge' })).toBeUndefined()
    expect(tryMockCommand('list_inbox_items', { status: 'pending' })).toEqual(expect.arrayContaining([expect.objectContaining({ id })]))
  })
})
