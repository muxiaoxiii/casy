import { Service } from '../plugin/types'
import { tauriCallSafe } from '../tauriBridge'
import { normalizeCase, normalizeCaseList, normalizeCaseInput } from '../caseNormalize'
import type { Case, CaseListResponse, CaseStats as BusinessCaseStats } from '../../types'
import type { CasePatchInput, CreateCasePayload } from '../../types/ipc'
// list_cases 后端返回 { items,total,page,perPage }，业务类型已补 page/perPage（见 src/types/index.ts
// CaseListResponse）。filter 沿用 bindings.CaseFilter（含 page/perPage，供前端分页透传）。
import type { CaseFilter as BindingsCaseFilter } from '../../types/bindings'
import type { CommandMap } from '../../types/commandMap'
import type { AiAuthCtx } from './tasks'

type WireCaseStats = CommandMap['case_stats']['result']
type TodayStats = CommandMap['get_today_stats']['result']
type CaseTypeMetrics = CommandMap['get_case_type_metrics']['result']
type TimelineEvent = CommandMap['get_case_timeline']['result'][number]
type RelatedCase = CommandMap['get_relations']['result'][number]
type CaseRelation = CommandMap['add_relation']['result']
type SheetInfo = CommandMap['excel_get_sheets']['result'][number]
type ExcelInspectResult = CommandMap['excel_inspect_sheet']['result']
type FeishuConfigStatus = CommandMap['feishu_check_config']['result']
type FeishuInspectResult = CommandMap['feishu_inspect_bitable']['result']
type CaseImportReport = CommandMap['excel_import_cases']['result']
type SubtableImportReport = CommandMap['excel_import_subtable']['result']
type Hearing = CommandMap['list_case_hearings']['result'][number]

function normalizeCaseStats(stats: WireCaseStats): BusinessCaseStats {
  return {
    total: stats.total,
    active: stats.active,
    closed: stats.closed,
    byTrack: stats.byTrack.map(([track, count]) => ({ track, count })),
    byClient: stats.byClient.map(([client, count]) => ({ client, count })),
  }
}

/** 案件服务：ctx.cases（数据通路：视图 → 服务 → tauriBridge → Rust 命令） */
export class CasesService extends Service {
  static inject: string[] = []

  async list(filter: Partial<BindingsCaseFilter> = {}): Promise<{ ok: boolean; data?: CaseListResponse; error?: string }> {
    const result = await tauriCallSafe('list_cases', { filter })
    // 边界：wire Case（可空字段）→ 业务 Case，attorneys 归一为 string[]
    if (result.ok && result.data) {
      return { ...result, data: { ...result.data, items: normalizeCaseList(result.data.items) } }
    }
    return result
  }

  async get(id: string): Promise<{ ok: boolean; data?: Case; error?: string }> {
    const result = await tauriCallSafe('get_case', { id })
    if (result.ok && result.data) {
      return { ...result, data: normalizeCase(result.data) }
    }
    return result
  }

  async create(data: CreateCasePayload): Promise<{ ok: boolean; data?: Case; error?: string }> {
    const result = await tauriCallSafe('create_case', { data: normalizeCaseInput(data) })
    // K-3①：领域事件由 service 层统一发出——人与 AI 触发同一事件流
    if (result.ok) {
      const normalized = result.data ? normalizeCase(result.data) : result.data
      this.ctx.emit('case:created', { id: normalized?.id, ...data })
      return { ...result, data: normalized }
    }
    return result
  }

  async update(id: string, data: CasePatchInput, aiAuth?: AiAuthCtx): Promise<{ ok: boolean; data?: Case; error?: string }> {
    // AI 网关授权信息随 data 透传（后端 update_case 从 data 内读取 origin/proposalToken）
    const payload = aiAuth ? { ...data, ...aiAuth } : data
    const result = await tauriCallSafe('update_case', { id, data: normalizeCaseInput(payload) })
    if (result.ok) {
      const normalized = result.data ? normalizeCase(result.data) : result.data
      this.ctx.emit('case:updated', { id, ...data })
      return { ...result, data: normalized }
    }
    return result
  }

  async remove(id: string, aiAuth?: AiAuthCtx): Promise<{ ok: boolean; error?: string }> {
    const result = await tauriCallSafe('delete_case', { id, ...(aiAuth ?? {}) })
    if (result.ok) {
      this.ctx.emit('case:deleted', { id })
    }
    return result
  }

  async search(query: string): Promise<{ ok: boolean; data?: Case[]; error?: string }> {
    const result = await tauriCallSafe('search_cases', { query })
    if (result.ok && result.data) {
      return { ...result, data: normalizeCaseList(result.data) }
    }
    return result
  }
  
  async stats(): Promise<{ ok: boolean; data?: BusinessCaseStats; error?: string }> {
    const result = await tauriCallSafe('case_stats', {})
    if (result.ok && result.data) {
      return { ...result, data: normalizeCaseStats(result.data) }
    }
    return { ok: false, error: result.error }
  }

  /** 导出案件（CSV，保存到下载目录，返回文件路径） */
  async exportCases(format: string, filter: Partial<BindingsCaseFilter> = {}): Promise<{ ok: boolean; data?: string; error?: string }> {
    return tauriCallSafe('export_cases', { format, filter })
  }

  /** 今日面板统计（硬性日程/今日到期/等待超时/需回顾） */
  async todayStats(): Promise<{ ok: boolean; data?: TodayStats; error?: string }> {
    return tauriCallSafe('get_today_stats', {})
  }

  /** 案件类型差异化评估指标（get_case_type_metrics） */
  async caseTypeMetrics(caseId: string): Promise<{ ok: boolean; data?: CaseTypeMetrics; error?: string }> {
    return tauriCallSafe('get_case_type_metrics', { caseId })
  }

  /** 案件时间线（日志/庭审/任务聚合） */
  async timeline(caseId: string): Promise<{ ok: boolean; data?: TimelineEvent[]; error?: string }> {
    return tauriCallSafe('get_case_timeline', { caseId })
  }

  /** 添加办案日志 */
  async addLog(opts: { caseId: string; eventSummary: string; eventType: string; eventDate: string; content?: string | null }): Promise<{ ok: boolean; data?: string; error?: string }> {
    return tauriCallSafe('add_case_log', {
      caseId: opts.caseId,
      eventSummary: opts.eventSummary,
      eventType: opts.eventType,
      eventDate: opts.eventDate,
      content: opts.content ?? null,
    })
  }

  /** 案件关联关系（双向） */
  async relations(caseId: string): Promise<{ ok: boolean; data?: RelatedCase[]; error?: string }> {
    return tauriCallSafe('get_relations', { caseId })
  }

  /** 添加关联关系；旧版合并参数为 true 时后端明确拒绝。 */
  async addRelation(
    caseId: string,
    relatedId: string,
    relationType: string,
    label?: string,
    mergeData?: boolean
  ): Promise<{ ok: boolean; data?: CaseRelation; error?: string }> {
    return tauriCallSafe('add_relation', {
      caseId,
      relatedId,
      relationType,
      label: label || null,
      mergeData: mergeData || false
    })
  }

  /** 获取 Excel 工作簿的 Sheet 列表 */
  async getExcelSheets(filePath: string): Promise<{
    ok: boolean
    data?: SheetInfo[]
    error?: string
  }> {
    return tauriCallSafe('excel_get_sheets', { filePath })
  }

  /** 探测工作表结构、表头与预览数据 */
  async inspectExcelSheet(
    filePath: string,
    sheetName: string,
    headerRowOverride?: number,
  ): Promise<{
    ok: boolean
    data?: ExcelInspectResult
    error?: string
  }> {
    return tauriCallSafe('excel_inspect_sheet', {
      filePath,
      sheetName,
      headerRowOverride: headerRowOverride !== undefined ? headerRowOverride : null,
    })
  }

  /** 批量导入 Excel 案件 */
  async importExcelCases(
    filePath: string,
    sheetName: string,
    config: {
      headerRow: number
      columnMappings: Record<number, string>
      forwardFillColumns: number[]
      conflictStrategy: string
      defaultTrack?: string | null
    },
  ): Promise<{
    ok: boolean
    data?: CaseImportReport
    error?: string
  }> {
    const result = await tauriCallSafe('excel_import_cases', { filePath, sheetName, config })
    if (result.ok) {
      this.ctx.emit('case:imported', result.data)
    }
    return result
  }

  /** 检查飞书自建应用配置状态 */
  async checkFeishuConfig(): Promise<{
    ok: boolean
    data?: FeishuConfigStatus
    error?: string
  }> {
    return tauriCallSafe('feishu_check_config')
  }

  /** 探测飞书多维表格结构、字段列表与样本数据 */
  async inspectFeishuBitable(
    urlOrToken: string,
    tableIdOverride?: string,
  ): Promise<{
    ok: boolean
    data?: FeishuInspectResult
    error?: string
  }> {
    return tauriCallSafe('feishu_inspect_bitable', {
      urlOrToken,
      tableIdOverride: tableIdOverride !== undefined ? tableIdOverride : null,
    })
  }

  /** 批量导入飞书多维表格案件 */
  async importFeishuBitableCases(
    appToken: string,
    tableId: string,
    config: {
      headerRow: number
      columnMappings: Record<number, string>
      forwardFillColumns: number[]
      conflictStrategy: string
      defaultTrack?: string | null
    },
  ): Promise<{
    ok: boolean
    data?: CaseImportReport
    error?: string
  }> {
    const result = await tauriCallSafe('feishu_import_bitable_cases', {
      appToken,
      tableId,
      config,
    })
    if (result.ok) {
      this.ctx.emit('case:imported', result.data)
    }
    return result
  }

  /** 批量导入 Excel 关联分表 (任务/庭审/日志) */
  async importExcelSubtable(
    filePath: string,
    sheetName: string,
    config: {
      targetEntity: string
      headerRow: number
      columnMappings: Record<number, string>
      forwardFillColumns: number[]
    },
  ): Promise<{
    ok: boolean
    data?: SubtableImportReport
    error?: string
  }> {
    const result = await tauriCallSafe('excel_import_subtable', { filePath, sheetName, config })
    if (result.ok) {
      this.ctx.emit('case:imported', result.data)
    }
    return result
  }

  /** 批量导入飞书关联分表 (任务/庭审/日志) */
  async importFeishuSubtable(
    appToken: string,
    tableId: string,
    config: {
      targetEntity: string
      headerRow: number
      columnMappings: Record<number, string>
      forwardFillColumns: number[]
    },
  ): Promise<{
    ok: boolean
    data?: SubtableImportReport
    error?: string
  }> {
    const result = await tauriCallSafe('feishu_import_bitable_subtable', {
      appToken,
      tableId,
      config,
    })
    if (result.ok) {
      this.ctx.emit('case:imported', result.data)
    }
    return result
  }

  /** 获取案件所有庭审记录 */
  async listHearings(caseId: string): Promise<{
    ok: boolean
    data?: Hearing[]
    error?: string
  }> {
    return tauriCallSafe('list_case_hearings', { caseId })
  }

  /** 创建庭审记录 */
  async createHearing(payload: {
    caseId: string
    hearingDate: string
    hearingName?: string
    court?: string
    venue?: string
    judges?: string
    caseLevel?: string
    contactInfo?: string
    actualStatus?: string
  }): Promise<{ ok: boolean; data?: Hearing; error?: string }> {
    return tauriCallSafe('create_case_hearing', { payload })
  }

  /** 更新庭审记录 */
  async updateHearing(
    id: string,
    payload: {
      hearingName?: string
      hearingDate?: string
      court?: string
      venue?: string
      judges?: string
      caseLevel?: string
      contactInfo?: string
      actualStatus?: string
    },
  ): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe('update_case_hearing', { id, payload })
  }

  /** 删除庭审记录 */
  async deleteHearing(id: string): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe('delete_case_hearing', { id })
  }
}
