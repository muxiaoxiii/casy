import { Service } from '../plugin/types'
import { tauriCallSafe } from '../tauriBridge'
import type { Task } from '../../types'

/** 任务服务：ctx.tasks */
export class TasksService extends Service {
  static inject: string[] = []

  async list(filter: Record<string, unknown> = {}): Promise<{ ok: boolean; data?: Task[]; error?: string }> {
    return tauriCallSafe<Task[]>('list_tasks', { filter })
  }

  async create(data: Record<string, unknown>): Promise<{ ok: boolean; data?: Task; error?: string }> {
    const result = await tauriCallSafe<Task>('create_task', { data })
    // K-3①：领域事件由 service 层统一发出——人与 AI 触发同一事件流
    if (result.ok) {
      this.ctx.emit('task:created', { id: result.data?.id, ...data })
    }
    return result
  }

  async toggle(id: string, actualMinutes?: number | null): Promise<{ ok: boolean; error?: string }> {
    const result = await tauriCallSafe<void>('toggle_task', { id, actualMinutes: actualMinutes ?? null })
    if (result.ok) {
      this.ctx.emit('task:completed', { id })
    }
    return result
  }

  async update(data: Record<string, unknown>): Promise<{ ok: boolean; error?: string }> {
    // id 必须在 data 内（后端 update_task 只收 data）
    return tauriCallSafe<void>('update_task', { data })
  }

  async remove(id: string): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe<void>('delete_task', { id })
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
}
