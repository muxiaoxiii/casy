// @vitest-environment node
import { describe, it, expect, vi, beforeEach } from 'vitest'
import type { Task } from '../src/types'

/**
 * 乐观任务操作 + Undo 栈 单测（M-GTD-1 · A0-2/A0-3）
 *
 * 覆盖 P1-6 的关键契约：
 * - deleteTaskOptimistic 乐观移除（remove hook）、失败回滚（restore hook + 提示）
 * - 撤销删除走 restore_task 快照还原：保留原 id / completed，并回写前端 Task 字段
 * - undoLast 的栈弹出与成功/失败提示语义
 */
const h = vi.hoisted(() => ({
  remove: vi.fn(),
  restore: vi.fn(),
  toggle: vi.fn(),
  snooze: vi.fn(),
  update: vi.fn(),
  message: {
    success: vi.fn(),
    error: vi.fn(),
    info: vi.fn(),
    warning: vi.fn(),
  },
  box: { confirm: vi.fn() },
}))

vi.mock('../src/core/plugin/context', () => ({
  casyContext: {
    tasks: {
      remove: h.remove,
      restore: h.restore,
      toggle: h.toggle,
      snooze: h.snooze,
      update: h.update,
    },
  },
}))

vi.mock('element-plus', () => ({
  ElMessage: h.message,
  ElMessageBox: h.box,
}))

import { deleteTaskOptimistic, undoLast, canUndo, peekUndoLabel } from '../src/core/taskActions'

function makeTask(overrides: Partial<Task> = {}): Task {
  return {
    id: 't-1',
    taskName: '旧任务',
    description: null,
    createdDate: '2026-09-01',
    deadline: null,
    priority: 'important',
    completed: 1,
    assignee: null,
    finishNote: null,
    taskType: 'action',
    startDate: '2026-09-05',
    dueDate: '2026-09-05',
    dueTime: null,
    waitingFor: null,
    followUpDate: null,
    context: '@办公室',
    flagged: 1,
    sequential: 0,
    blocked: 0,
    sequenceOrder: 0,
    startBucket: 'today',
    todayIndex: 2,
    estimatedMinutes: 30,
    actualMinutes: null,
    isOverdue: 0,
    dueSoon: 0,
    lastReviewDate: null,
    nextReviewDate: null,
    areaId: null,
    knowledgeId: null,
    caseId: 'case1',
    parentId: null,
    ...overrides,
  }
}

describe('taskActions · 删除 + Undo（P1-6）', () => {
  beforeEach(async () => {
    // 排空跨用例残留的 Undo 栈（撤销会触发 restore mock，随后统一清空断言）
    while (canUndo()) await undoLast()
    vi.clearAllMocks()
  })

  it('乐观删除成功：调用 remove、调用 remove hook、不入 restore、注册撤销', async () => {
    const task = makeTask()
    const remove = vi.fn()
    const restore = vi.fn()
    h.remove.mockResolvedValue({ ok: true })

    const result = await deleteTaskOptimistic(task, { remove, restore })

    expect(result).toBe(true)
    expect(remove).toHaveBeenCalledTimes(1)
    expect(h.remove).toHaveBeenCalledWith(task.id)
    expect(restore).not.toHaveBeenCalled()
    expect(canUndo()).toBe(true)
    expect(peekUndoLabel()).toBe(`删除「${task.taskName}」`)
  })

  it('删除失败：调用 restore hook、提示错误、不注册撤销', async () => {
    const task = makeTask()
    const restore = vi.fn()
    h.remove.mockResolvedValue({ ok: false, error: 'db down' })

    const result = await deleteTaskOptimistic(task, { restore })

    expect(result).toBe(false)
    expect(restore).toHaveBeenCalledTimes(1)
    expect(h.message.error).toHaveBeenCalledWith('db down')
    expect(canUndo()).toBe(false)
  })

  it('undoLast 撤销删除：restore_task 还原快照并回写前端 Task 字段', async () => {
    const task = makeTask()
    const restore = vi.fn()
    h.remove.mockResolvedValue({ ok: true })
    const label = `删除「${task.taskName}」`
    h.restore.mockResolvedValue({
      ok: true,
      data: {
        id: 't-orig',
        taskName: '已还原任务',
        completed: 1,
        dueDate: '2026-09-05',
        startDate: null,
        startBucket: 'today',
        actualMinutes: 30,
        parentTaskId: 'p-9',
      },
    })

    await deleteTaskOptimistic(task, { restore })
    expect(canUndo()).toBe(true)

    const done = await undoLast()

    expect(done).toBe(true)
    expect(h.restore).toHaveBeenCalledWith(task)
    expect(restore).toHaveBeenCalledTimes(1)
    // 快照字段回写本地 Task
    expect(task.id).toBe('t-orig')
    expect(task.taskName).toBe('已还原任务')
    expect(task.completed).toBe(1)
    expect(task.dueDate).toBe('2026-09-05')
    expect(task.startDate).toBe(null)
    expect(task.startBucket).toBe('today')
    expect(task.actualMinutes).toBe(30)
    expect(task.parentId).toBe('p-9')
    // 撤销后栈清空
    expect(canUndo()).toBe(false)
    expect(h.message.success).toHaveBeenCalledWith(`已撤销：${label}`)
  })

  it('撤销时 restore_task 失败：undoLast 返回 false 并提示撤销失败', async () => {
    const task = makeTask()
    h.remove.mockResolvedValue({ ok: true })
    h.restore.mockResolvedValue({ ok: false, error: 'id 已被占用' })

    await deleteTaskOptimistic(task)
    expect(canUndo()).toBe(true)

    const done = await undoLast()

    expect(done).toBe(false)
    expect(h.message.error).toHaveBeenCalledWith('撤销失败')
    expect(canUndo()).toBe(false)
  })

  it('无撤销操作时 undoLast 为空操作', async () => {
    expect(canUndo()).toBe(false)
    const done = await undoLast()
    expect(done).toBe(false)
    expect(h.message.error).not.toHaveBeenCalled()
  })
})
