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

  /** 导出案件（CSV，保存到下载目录，返回文件路径） */
  async exportCases(format: string, filter: Record<string, unknown> = {}): Promise<{ ok: boolean; data?: string; error?: string }> {
    return tauriCallSafe<string>('export_cases', { format, filter })
  }

  /** 今日面板统计（硬性日程/今日到期/等待超时/需回顾） */
  async todayStats(): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('get_today_stats', {})
  }

  /** 案件类型差异化评估指标（get_case_type_metrics） */
  async caseTypeMetrics(caseId: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('get_case_type_metrics', { caseId })
  }

  /** 案件时间线（日志/庭审/任务聚合） */
  async timeline(caseId: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('get_case_timeline', { caseId })
  }

  /** 添加办案日志 */
  async addLog(opts: { caseId: string; eventSummary: string; eventType: string; eventDate: string; content?: string | null }): Promise<{ ok: boolean; data?: string; error?: string }> {
    return tauriCallSafe<string>('add_case_log', {
      caseId: opts.caseId,
      eventSummary: opts.eventSummary,
      eventType: opts.eventType,
      eventDate: opts.eventDate,
      content: opts.content ?? null,
    })
  }

  /** 案件关联关系（双向） */
  async relations(caseId: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('get_relations', { caseId })
  }
}
