import { Service } from '../plugin/types'
import { tauriCallSafe } from '../tauriBridge'
import type { Task } from '../../types'
import type { TaskDto } from '../../types/bindings'

/**
 * 撤销删除（restore_task）快照：不能再是 TaskLike 那种 7 字段子集。
 * 必须是完整 Task 全量列，否则后端会静默丢掉 taskType/priority/context 等列。
 * 额外声明后端 DTO 字段名（parentTaskId/deferUntil）与前端 Task 的 parentId 等价，
 * 确保父关系不会被丢弃；这些为可选扩展，不影响 `Task` 本身的可赋值性。
 */
export type RestoreTaskSnapshot = Task & {
  /** 后端 DTO 字段名，与前端 Task 的 parentId 等价（二者映射到同一列） */
  parentTaskId?: string | null
  /** 后端 DTO 字段名（Defer 日期） */
  deferUntil?: string | null
}

/** AI 授权上下文（P0-2 网关：提案批准后重放时携带一次性 proposal token） */
export interface AiAuthCtx {
  origin?: string
  proposalToken?: string
}

/** 任务服务：ctx.tasks */
export class TasksService extends Service {
  static inject: string[] = []

  async list(filter: Record<string, unknown> = {}): Promise<{ ok: boolean; data?: Task[]; error?: string }> {
    // TODO(B1 类型对齐)：切 CommandMap 注册表前需统一手写 Task 的字面量联合
    // （priority/taskType/context/startBucket）与缺失字段 dueTime——涉全消费面，随主线协同
    return tauriCallSafe<Task[]>('list_tasks', { filter })
  }

  async create(data: Record<string, unknown>): Promise<{ ok: boolean; data?: { id: string }; error?: string }> {
    // B1：create_task 后端实际仅返回 { id }（非完整 Task），按真实契约标注返回类型
    const result = await tauriCallSafe('create_task', { data })
    // K-3①：领域事件由 service 层统一发出——人与 AI 触发同一事件流
    if (result.ok) {
      this.ctx.emit('task:created', { id: result.data?.id, ...data })
    }
    return result
  }

  async toggle(id: string, actualMinutes?: number | null, aiAuth?: AiAuthCtx): Promise<{ ok: boolean; error?: string }> {
    const result = await tauriCallSafe('toggle_task', { id, actualMinutes: actualMinutes ?? null, ...(aiAuth ?? {}) })
    if (result.ok) {
      this.ctx.emit('task:completed', { id })
    }
    return result
  }

  async update(data: Record<string, unknown>): Promise<{ ok: boolean; error?: string }> {
    // id 必须在 data 内（后端 update_task 只收 data）
    return tauriCallSafe('update_task', { data })
  }

  async remove(id: string, aiAuth?: AiAuthCtx): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe('delete_task', { id, ...(aiAuth ?? {}) })
  }

  /**
   * 撤销删除：按快照还原任务（保留原 id 与 completed，不走 create_task 的
   * sequential 继承 / AI token 消耗）。快照必须是完整 RestoreTaskSnapshot，
   * 避免仅满足 TaskLike 的瘦对象让后端静默丢列。后端返回还原后的完整 TaskDto 供前端对账。
   */
  async restore(snapshot: RestoreTaskSnapshot): Promise<{ ok: boolean; data?: TaskDto; error?: string }> {
    return tauriCallSafe('restore_task', { snapshot })
  }

  /** GTD 领域列表 */
  async areas(): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('list_areas', {})
  }

  /** 新建领域（A1-2：name 必填，description/icon 可选） */
  async createArea(data: { name: string; description?: string | null; icon?: string | null }):
    Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('create_area', { data })
  }

  /**
   * 更新领域。⚠️ 后端仅 name/sort_order 走 COALESCE，description/icon 为直接赋值
   * （缺省即清空）——调用方编辑时必须整组提交 name/description/icon
   */
  async updateArea(id: string, data: { name?: string; description?: string | null; icon?: string | null }):
    Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe<void>('update_area', { id, data })
  }

  /**
   * 删除领域。后端保护：领域下仍有任务时拒绝（错误信息含任务数，UI 直接透出）
   */
  async removeArea(id: string): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe<void>('delete_area', { id })
  }

  /** 领域统计（任务总数/完成数/案件数），供管理界面展示 */
  async areaStats(id: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('get_area_stats', { id })
  }

  /** 稍后提醒（写 snoozed 行为事件，支撑"懂你的节奏"学习） */
  async snooze(id: string, option: string, newDueDate?: string | null): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe<void>('snooze_task', { id, option, newDueDate: newDueDate ?? null })
  }

  /** ⌘K 全局搜索：任务域（LIKE，本地规模足够；FTS 升级待 tasks_fts） */
  async searchTasks(query: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe('search_tasks', { query })
  }
}
