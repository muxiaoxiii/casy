import { Service } from '../plugin/types'
import { tauriCallSafe } from '../tauriBridge'
import type { KnowledgeItem } from '../../types'

/** 知识库服务：ctx.knowledge */
export class KnowledgeService extends Service {
  static inject: string[] = []

  async list(filter: Record<string, unknown> = {}): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('list_knowledge', { filter })
  }

  async search(query: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('search_knowledge', { query })
  }

  async create(data: Record<string, unknown>): Promise<{ ok: boolean; data?: string; error?: string }> {
    return tauriCallSafe<string>('create_knowledge', { data })
  }

  async update(id: string, data: Record<string, unknown>): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe<void>('update_knowledge', { id, data })
  }

  async remove(id: string): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe<void>('delete_knowledge', { id })
  }
}
