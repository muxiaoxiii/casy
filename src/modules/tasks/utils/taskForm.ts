/**
 * 任务编辑表单映射（M-UI-0 · 低风险拆分自 TasksView.vue）
 *
 * 纯函数模块——只依赖入参、可单测；把「Task 对象 ↔ 抽屉表单 ↔ 保存载荷」
 * 的三段双向映射从视图抽离，保持 UI 与 Undo 行为不变。
 *
 * 契约要点（与 TasksView.vue 原逻辑逐条对齐）：
 * - 空白表单默认 startBucket='anytime'；新建任务时视图用 'inbox'（原 openNewTask 语义）
 * - toEditForm 把 Task 映射为可编辑字段，flagged 归约为 boolean、estimatedMinutes 归约为 number|null
 * - toSavePayload 走三态 PATCH：空串归一为 null（清除推迟日），非空走原值
 */

import type { Task } from '../../../types'
import type { CreateTaskPayload, UpdateTaskPayload } from '../../../types/ipc'

/** 抽屉编辑表单的可编辑字段（均为表单控件可直接绑定的原语） */
export interface EditForm {
  taskName: string
  description: string
  deadline: string
  priority: string
  caseId: string
  taskType: string
  startDate: string
  dueDate: string
  dueTime: string
  waitingFor: string
  followUpDate: string
  context: string
  flagged: boolean
  areaId: string
  estimatedMinutes: number | null
  startBucket: string
  deferUntil: string
  recurrenceRule: string
  nextReviewDate: string
}

/**
 * 新建任务的空白表单。
 * @param startBucket 新建缺省进入的时间桶（视图传 'inbox'；模块兜底 'anytime'）
 */
export function emptyEditForm(startBucket: string = 'anytime'): EditForm {
  return {
    taskName: '',
    description: '',
    deadline: '',
    priority: 'normal',
    caseId: '',
    taskType: 'action',
    startDate: '',
    dueDate: '',
    dueTime: '',
    waitingFor: '',
    followUpDate: '',
    context: '',
    flagged: false,
    areaId: '',
    estimatedMinutes: null,
    startBucket,
    deferUntil: '',
    recurrenceRule: '',
    nextReviewDate: '',
  }
}

/**
 * Task → 编辑表单映射。空值一律归 ''/默认，flagged 转 boolean，estimatedMinutes 转 number|null。
 * dueDate 缺省时回退到旧字段 deadline（兼容迁移期数据）。
 */
export function toEditForm(task: Task): EditForm {
  return {
    taskName: task.taskName || '',
    description: task.description || '',
    deadline: task.deadline || '',
    priority: task.priority || 'normal',
    caseId: task.caseId || '',
    taskType: task.taskType || 'action',
    startDate: task.startDate || '',
    dueDate: task.dueDate || task.deadline || '',
    dueTime: task.dueTime || '',
    waitingFor: task.waitingFor || '',
    followUpDate: task.followUpDate || '',
    context: task.context || '',
    flagged: task.flagged === 1,
    areaId: task.areaId || '',
    estimatedMinutes: task.estimatedMinutes ?? null,
    startBucket: task.startBucket || 'anytime',
    deferUntil: task.deferUntil || '',
    recurrenceRule: task.recurrenceRule || '',
    nextReviewDate: task.nextReviewDate || '',
  }
}

/**
 * 表单 → create/update 载荷。id 仅在编辑（editingId 非空）时携带；
 * flagged 归一为 0/1；deferUntil 空串归一为 null（清空推迟日，非空走原值）。
 */
export function toSavePayload(editingId: string, form: EditForm): UpdateTaskPayload
export function toSavePayload(editingId: null | undefined, form: EditForm): CreateTaskPayload
export function toSavePayload(editingId: string | null | undefined, form: EditForm): CreateTaskPayload | UpdateTaskPayload {
  return {
    ...(editingId ? { id: editingId } : {}),
    ...form,
    taskName: form.taskName.trim(),
    deadline: form.dueDate || null,
    dueDate: form.dueDate || null,
    dueTime: form.dueTime || null,
    caseId: form.caseId || null,
    areaId: form.areaId || null,
    startDate: form.startDate || null,
    recurrenceRule: form.recurrenceRule || null,
    nextReviewDate: form.nextReviewDate || null,
    flagged: form.flagged ? 1 : 0,
    deferUntil: form.deferUntil || null,
  }
}
