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
    return tauriCallSafe<Task>('create_task', { data })
  }

  async toggle(id: string, actualMinutes?: number | null): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe<void>('toggle_task', { id, actualMinutes: actualMinutes ?? null })
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
}
