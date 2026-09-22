import { Service } from '../plugin/types'
import { tauriCallSafe } from '../tauriBridge'
import type { ProjectRow } from '../../types/commandMap'

/**
 * 项目服务（A1-1/D-9 垂直拆表阶段一）
 *
 * kind='personal' 直写 projects；kind='legal' 由 cases 触发器镜像、
 * 本阶段只读透出（增删改走案件管理路径）。
 */
export class ProjectsService extends Service {
  static inject: string[] = []

  async list(query?: string): Promise<{ ok: boolean; data?: ProjectRow[]; error?: string }> {
    return tauriCallSafe('list_projects', query ? { query } : {})
  }

  async createPersonal(data: {
    name: string
    description?: string | null
    areaId?: string | null
    color?: string | null
  }): Promise<{ ok: boolean; data?: ProjectRow; error?: string }> {
    return this.mutation('project', tauriCallSafe('create_personal_project', { data }))
  }

  async updatePersonal(
    id: string,
    data: { name?: string; description?: string | null; status?: string; areaId?: string | null; color?: string | null }
  ): Promise<{ ok: boolean; error?: string }> {
    return this.mutation('project', tauriCallSafe('update_personal_project', { id, data }))
  }

  async remove(id: string): Promise<{ ok: boolean; error?: string }> {
    return this.mutation('project', tauriCallSafe('delete_project', { id }))
  }
}
