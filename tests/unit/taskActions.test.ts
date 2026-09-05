import { describe, it, expect, vi, beforeEach } from 'vitest'
const mocks = vi.hoisted(() => ({ update: vi.fn(), snooze: vi.fn(), success: vi.fn(), error: vi.fn() }))
vi.mock('../../src/core/plugin/context', () => ({ casyContext: { tasks: mocks } }))
vi.mock('element-plus', () => ({ ElMessage: { success: mocks.success, error: mocks.error } }))

beforeEach(() => { vi.resetModules(); vi.clearAllMocks() })
describe('task undo persistence', () => {
  it('keeps a failed undo available and changes local state only after success', async () => {
    const actions = await import('../../src/core/taskActions')
    const task = { id: 't', taskName: '提交证据', completed: 0, actualMinutes: 5 }
    mocks.update.mockResolvedValueOnce({ ok: true }).mockResolvedValueOnce({ ok: false, error: 'locked' }).mockResolvedValueOnce({ ok: true })
    expect(await actions.completeTaskOptimistic(task, { actualMinutes: 20 })).toBe(true)
    expect(task.completed).toBe(1)
    expect(await actions.undoLast()).toBe(false)
    expect(task.completed).toBe(1)
    expect(actions.canUndo()).toBe(true)
    expect(await actions.undoLast()).toBe(true)
    expect(task.completed).toBe(0)
    expect(task.actualMinutes).toBe(5)
    expect(actions.canUndo()).toBe(false)
    expect(mocks.success).toHaveBeenCalledTimes(1)
  })
  it('does not overwrite a legal deadline when undoing a schedule change', async () => {
    const actions = await import('../../src/core/taskActions')
    const task = { id: 't', taskName: '准备材料', completed: 0, startDate: '2026-09-01', dueDate: '2026-09-30', startBucket: 'today', timeBlock: 'morning' }
    mocks.snooze.mockResolvedValue({ ok: true })
    mocks.update.mockResolvedValue({ ok: true })
    await actions.snoozeTaskWithUndo(task, 'tomorrow', '明天')
    task.dueDate = '2026-10-01'
    await actions.undoLast()
    expect(mocks.update).toHaveBeenCalledWith({ id: 't', startDate: '2026-09-01', startBucket: 'today', timeBlock: 'morning' })
    expect(task.dueDate).toBe('2026-10-01')
  })
})
