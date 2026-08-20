import { Service } from '../plugin/types'
import { tauriCallSafe } from '../tauriBridge'
import type { Case, CaseListResponse } from '../../types'

/** 案件服务：ctx.cases（数据通路：视图 → 服务 → tauriBridge → Rust 命令） */
export class CasesService extends Service {
  static inject: string[] = []

  async list(filter: Record<string, unknown> = {}): Promise<{ ok: boolean; data?: CaseListResponse; error?: string }> {
    return tauriCallSafe<CaseListResponse>('list_cases', { filter })
  }

  async get(id: string): Promise<{ ok: boolean; data?: Case; error?: string }> {
    return tauriCallSafe<Case>('get_case', { id })
  }

  async create(data: Record<string, unknown>): Promise<{ ok: boolean; data?: Case; error?: string }> {
    return tauriCallSafe<Case>('create_case', { data })
  }

  async update(id: string, data: Record<string, unknown>): Promise<{ ok: boolean; data?: Case; error?: string }> {
    return tauriCallSafe<Case>('update_case', { id, data })
  }

  async remove(id: string): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe<void>('delete_case', { id })
  }

  async search(query: string): Promise<{ ok: boolean; data?: Case[]; error?: string }> {
    return tauriCallSafe<Case[]>('search_cases', { query })
  }
  
  async stats(): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('case_stats', {})
  }
}
