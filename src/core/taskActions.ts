/**
 * 任务乐观操作 + Undo 栈（M-GTD-1 · A0-2 / A0-3）
 *
 * 原则（设计哲学 §1.6 流动 / 本地优先）：
 * - 本地先行：UI 状态立即变化，IPC 在后台确认
 * - 失败回滚：IPC 失败时恢复本地状态并提示
 * - 可撤销：complete / delete / snooze 三类操作进入 Undo 栈，Cmd+Z 撤销
 *
 * hooks 说明：调用方通过 hooks 把"列表级联变化"（如从当前透视移除行）
 * 与模块解耦——remove() 在乐观阶段调用，restore() 在回滚/撤销时调用。
 */

import { casyContext } from './plugin/context'
import { ElMessage } from 'element-plus'
import type { StartBucket, Task } from '../types'
import type { UpdateTaskPayload } from '../types/ipc'
import { ref } from 'vue'

/**
 * 乐观任务操作的**最小字段子集**（仅用于 complete/restore/snooze 这类
 * 只读取少数字段的函数）。注意：这不是撤销删除的快照契约——
 * `deleteTaskOptimistic` 传的是完整 `Task`（见下），避免后端还原时静默丢列。
 */
export interface TaskLike {
  id: string
  taskName: string
  completed: number
  dueDate?: string | null
  startDate?: string | null
  startBucket?: string | null
  actualMinutes?: number | null
  timeBlock?: string | null
}

interface ListHooks {
  /** 乐观阶段把任务从当前视图移除（若当前透视应隐藏它） */
  remove?: () => void
  /** 回滚或撤销时把任务还原到视图 */
  restore?: () => void
}

interface UndoableAction {
  label: string
  undo: () => Promise<void>
}

const undoStack: UndoableAction[] = []
const MAX_UNDO = 20
const undoRevision = ref(0)
let undoing = false

function registerUndo(action: UndoableAction): void {
  undoStack.push(action)
  if (undoStack.length > MAX_UNDO) undoStack.shift()
  undoRevision.value++
}

/** 是否存在可撤销操作 */
export function canUndo(): boolean {
  void undoRevision.value
  return undoStack.length > 0
}

/** 最近一次可撤销操作的标签（用于提示文案） */
export function peekUndoLabel(): string | null {
  return undoStack.length ? undoStack[undoStack.length - 1].label : null
}

/** 撤销最近一次操作；返回是否执行了撤销 */
export async function undoLast(): Promise<boolean> {
  if (undoing) return false
  const action = undoStack[undoStack.length - 1]
  if (!action) return false
  undoing = true
  try {
    await action.undo()
    const index = undoStack.indexOf(action)
    if (index >= 0) undoStack.splice(index, 1)
    undoRevision.value++
    ElMessage.success(`已撤销：${action.label}`)
    return true
  } catch (err) {
    ElMessage.error('撤销失败')
    return false
  } finally {
    undoing = false
  }
}

/** 一键完成任务：乐观置位 + 失败回滚 + 可撤销 */
export async function completeTaskOptimistic(
  task: TaskLike,
  options: { actualMinutes?: number | null; hooks?: ListHooks } = {}
): Promise<boolean> {
  if (task.completed) return false
  const { actualMinutes = null, hooks = {} } = options
  const prevCompleted = task.completed
  const prevMinutes = task.actualMinutes ?? null

  task.completed = 1
  hooks.remove?.()

  const result = await casyContext.tasks.update({ id: task.id, completed: 1,
    ...(actualMinutes !== null && Number.isFinite(actualMinutes) ? { actualMinutes } : {}) })
  if (!result.ok) {
    task.completed = prevCompleted
    hooks.restore?.()
    ElMessage.error(result.error || '完成失败')
    return false
  }

  if (actualMinutes !== null && Number.isFinite(actualMinutes)) task.actualMinutes = actualMinutes
  registerUndo({
    label: `完成「${task.taskName}」`,
    undo: async () => {
      const result = await casyContext.tasks.update({ id: task.id, completed: prevCompleted, actualMinutes: prevMinutes })
      if (!result.ok) throw new Error(result.error || '撤销完成失败')
      task.completed = prevCompleted
      task.actualMinutes = prevMinutes
      hooks.restore?.()
    },
  })
  return true
}

/** 恢复已完成任务为未完成 */
export async function restoreTaskOptimistic(task: TaskLike): Promise<boolean> {
  if (!task.completed) return false
  const prev = task.completed
  task.completed = 0
  const result = await casyContext.tasks.update({ id: task.id, completed: 0 })
  if (!result.ok) {
    task.completed = prev
    ElMessage.error(result.error || '恢复失败')
    return false
  }
  return true
}

/**
 * 删除任务（无确认框，靠 Undo 兜底）：乐观移除 + 失败还原 + 可撤销。
 * 接收完整 `Task`（而非 TaskLike），因为撤销删除的快照需要全量列，
 * 否则后端 restore_task 会静默丢掉 taskType/priority/context 等字段。
 */
export async function deleteTaskOptimistic(task: Task, hooks: ListHooks = {}): Promise<boolean> {
  const snapshot = { ...task }
  hooks.remove?.()
  const result = await casyContext.tasks.remove(task.id)
  if (!result.ok) {
    hooks.restore?.()
    ElMessage.error(result.error || '删除失败')
    return false
  }
  registerUndo({
    label: `删除「${task.taskName}」`,
    undo: async () => {
      // P1-6：撤销删除不再走 create_task（会另生成 id、completed 清零），
      // 改用专用 restore_task 按快照还原原 id / completed，并用后端返回对象对账，
      // 保证本地对象与 DB 一致。快照即完整 Task，非 TaskLike 子集。
      const restored = await casyContext.tasks.restore(snapshot)
      if (restored.ok) {
        const d = restored.data
        if (d) {
          task.id = d.id
          task.taskName = d.taskName
          task.completed = d.completed
          task.dueDate = d.dueDate ?? null
          task.startDate = d.startDate ?? null
          // d.startBucket 来自后端（schema 校验为合法 StartBucket），窄化到联合类型
          task.startBucket = d.startBucket as StartBucket
          task.actualMinutes = d.actualMinutes ?? null
          // 父关系：前端 Task 用 parentId，后端 DTO 用 parentTaskId，还原后对齐为 parentId
          task.parentId = d.parentTaskId ?? null
        }
        hooks.restore?.()
      } else {
        throw new Error(restored.error || '恢复任务失败')
      }
    },
  })
  return true
}

/** 稍后提醒：IPC 成功后注册撤销（快照原日期字段，用 update 还原） */
export async function snoozeTaskWithUndo(
  task: TaskLike,
  option: string,
  label: string
): Promise<boolean> {
  const snapshot: UpdateTaskPayload = {
    id: task.id,
    startDate: task.startDate ?? null,
    startBucket: task.startBucket || 'anytime',
    timeBlock: task.timeBlock ?? null,
  }
  const result = await casyContext.tasks.snooze(task.id, option)
  if (!result.ok) {
    ElMessage.error(result.error || '操作失败')
    return false
  }
  registerUndo({
    label: `稍后「${task.taskName}」→ ${label}`,
    undo: async () => {
      const result = await casyContext.tasks.update(snapshot)
      if (!result.ok) throw new Error(result.error || '撤销计划失败')
      task.startDate = snapshot.startDate
      task.startBucket = snapshot.startBucket
      task.timeBlock = snapshot.timeBlock
    },
  })
  ElMessage.success(`已计划到${label}`)
  return true
}
