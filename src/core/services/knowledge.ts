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

  /** 获取条目及其块树（§8.2 块级引用；后端 get_knowledge_with_blocks 返回 { item, blocks }） */
  async getWithBlocks(id: string): Promise<{ ok: boolean; data?: { item?: Record<string, unknown>; blocks?: unknown[] }; error?: string }> {
    return tauriCallSafe<{ item?: Record<string, unknown>; blocks?: unknown[] }>('get_knowledge_with_blocks', { id })
  }

  /** 版本历史 */
  async versions(itemId: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('list_knowledge_versions', { itemId })
  }

  /** 版本与当前内容差异 */
  async diffWithCurrent(versionId: string, itemId: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('diff_knowledge_with_current', { versionId, itemId })
  }

  /** 两个版本差异 */
  async diffVersions(versionId1: string, versionId2: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('diff_knowledge_versions', { versionId1, versionId2 })
  }

  /** 知识图谱数据（知识 ↔ 案件 ↔ 任务；后端 get_knowledge_graph 返回 { nodes, edges }） */
  async graph(limit = 100): Promise<{ ok: boolean; data?: { nodes?: unknown[]; edges?: unknown[] }; error?: string }> {
    return tauriCallSafe<{ nodes?: unknown[]; edges?: unknown[] }>('get_knowledge_graph', { limit })
  }
}
