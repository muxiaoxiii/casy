/**
 * 命令契约注册表（D-3 / B1：tauriBridge 泛型化的数据源）
 *
 * - result 类型来自 src/types/bindings.ts（specta 生成物，禁止手改）
 * - params 类型按各 service 实际传参手写核定；尚未核定的先 `Record<string, unknown>`
 *   （仍获得返回类型检查），随后续收口逐条收紧
 * - 不在本表中的命令走 tauriBridge 的通用重载，行为不变
 * - 92 个 JSON 动态参数命令随 B1 DomainCommand 化逐步入表
 */
import type {
  AiChatResult,
  AiConfig,
  Case,
  CaseFile,
  CaseFilter,
  CaseListResult,
  CaseRelation,
  CaseStats,
  CalendarSyncReport,
  ChatMessage,
  CommandRoute,
  DashboardStats,
  DeadlineResult,
  DeadlineWarning,
  Draft,
  ExportResponse,
  FeishuSyncReport,
  ImportReport,
  McpPendingWrite,
  QuickJudgeResult,
  RecordDiff,
  RelatedCase,
  ReminderLogEntry,
  ReminderRule,
  RenderResponse,
  SchemaDiff,
  SearchResult,
  SyncResult,
  SyncStatus,
  TaskTemplate,
  TemplateListResponse,
  TimelineEvent,
} from './bindings'

export interface Cmd<P = Record<string, unknown>, R = unknown> {
  readonly params: P
  readonly result: R
}

/** A1-1 项目行（projects 表；specta 覆盖待 CalendarEvent 同批补齐） */
export interface ProjectRow {
  id: string
  name: string
  kind: 'legal' | 'personal'
  description: string | null
  status: string
  areaId: string | null
  color: string | null
  sortOrder: number
  createdAt: string
  updatedAt: string
}

export type CommandMap = {
  // ── 项目域（A1-1 绞杀式阶段一）──
  list_projects: Cmd<{ query?: string }, ProjectRow[]>
  create_personal_project: Cmd<
    { data: { name: string; description?: string | null; areaId?: string | null; color?: string | null } },
    ProjectRow
  >
  update_personal_project: Cmd<
    { id: string; data: { name?: string; description?: string | null; status?: string; areaId?: string | null; color?: string | null } },
    null
  >
  delete_project: Cmd<{ id: string }, null>

  // ── 案件域 ──
  get_case: Cmd<{ id: string }, Case>
  list_cases: Cmd<{ filter?: Partial<CaseFilter> }, CaseListResult>
  search_cases: Cmd<{ query: string }, Case[]>
  case_stats: Cmd<Record<string, unknown>, CaseStats>
  get_dashboard_stats: Cmd<Record<string, unknown>, DashboardStats>
  update_case_status: Cmd<Record<string, unknown>, Case>
  export_cases: Cmd<{ format: string; filter?: Partial<CaseFilter> }, string>

  // ── 关系 / 时间线 ──
  add_relation: Cmd<Record<string, unknown>, CaseRelation>
  detect_relations: Cmd<{ caseId: string }, CaseRelation[]>
  get_relations: Cmd<{ caseId: string }, RelatedCase[]>
  get_case_timeline: Cmd<{ caseId: string }, TimelineEvent[]>

  // ── 文件 ──
  list_case_files: Cmd<{ caseId: string }, CaseFile[]>
  add_case_file: Cmd<{ caseId: string; fileName: string; filePath: string; category: string }, CaseFile>

  // ── 提醒域 ──
  list_reminder_rules: Cmd<Record<string, unknown>, ReminderRule[]>
  get_reminder_log: Cmd<{ limit?: number }, ReminderLogEntry[]>
  get_deadline_warnings: Cmd<Record<string, unknown>, DeadlineResult[]>
  get_deadline_warnings_with_levels: Cmd<Record<string, unknown>, DeadlineWarning[]>
  test_reminder: Cmd<{ ruleId: string; channel: string; message: string }, ReminderLogEntry>

  // ── 同步域 ──
  get_sync_status: Cmd<Record<string, unknown>, SyncStatus>
  webdav_push: Cmd<Record<string, unknown>, SyncResult>
  webdav_pull: Cmd<Record<string, unknown>, SyncResult>
  webdav_startup_sync: Cmd<Record<string, unknown>, SyncResult>
  webdav_resolve_keep_local: Cmd<{ url: string; username: string; password: string }, SyncResult>
  webdav_resolve_keep_remote: Cmd<{ url: string; username: string; password: string }, SyncResult>
  sync_feishu_pull: Cmd<{ appToken: string; tableId: string }, FeishuSyncReport>
  sync_feishu_push: Cmd<{ appToken: string; tableId: string }, FeishuSyncReport>
  feishu_compare_table: Cmd<{ appToken: string; tableId: string; localTable: unknown }, SchemaDiff>
  feishu_compare_records: Cmd<{ appToken: string; tableId: string; localTable: unknown; matchField: string }, RecordDiff>
  import_feishu_data: Cmd<{ jsonPath: string }, ImportReport>
  sync_reminders_to_calendar: Cmd<Record<string, unknown>, CalendarSyncReport>

  // ── AI 域 ──
  ai_chat: Cmd<{
    messages: Array<Pick<ChatMessage, 'role' | 'content'>>
    mode?: string
    apiUrl?: string
    model?: string
    purpose?: string
  }, AiChatResult>
  get_ai_config: Cmd<Record<string, unknown>, AiConfig>
  get_command_route_info: Cmd<Record<string, unknown>, CommandRoute>
  quick_judge_inbox_item: Cmd<{ id: string }, QuickJudgeResult>
  list_mcp_pending_writes: Cmd<Record<string, unknown>, McpPendingWrite[]>

  // ── 文书引擎 ──
  list_docsy_templates: Cmd<Record<string, unknown>, TemplateListResponse>
  render_docsy_template: Cmd<{ templateId: string; caseId: string }, RenderResponse>
  export_docx: Cmd<{ templateId: string; caseId: string; outputPath?: string | null }, ExportResponse>

  // ── 知识检索 ──
  hybrid_search_knowledge: Cmd<{ query: string; limit?: number }, SearchResult>

  // ── 草稿 ──
  list_drafts: Cmd<Record<string, unknown>, Draft[]>
  get_draft: Cmd<{ id: string }, Draft>
}
