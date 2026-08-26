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
  CalendarEvent,
  CalendarEventRow,
  CalendarSyncReport,
  Case,
  CaseFile,
  CaseFilter,
  CaseListResult,
  CaseRelation,
  CaseStats,
  CaseTypeMetrics,
  CaseUnifiedView,
  ChatMessage,
  CommandRoute,
  DashboardStats,
  DeadlineResult,
  DeadlineWarning,
  Draft,
  ExportResponse,
  FeishuSyncReport,
  FieldGroup,
  HolidayNotice,
  CreateKnowledgeInput,
  FolderNamingSettingsInput,
  FolderTemplateInput,
  ImportReport,
  ImportResult,
  InboxItemDto,
  InboxProgress,
  MappingEntry,
  McpPendingWrite,
  TodayKpis,
  MonthTrendPoint,
  NameCount,
  ProcessedInboxResult,
  QuickJudgeResult,
  UpcomingHearing,
  RecordDiff,
  RelatedCase,
  ReminderLogEntry,
  ReminderRule,
  RenderResponse,
  SchemaDiff,
  SearchResult,
  SyncResult,
  SearchTaskDto,
  SyncStatus,
  TaskDto,
  TaskFilter,
  TaskTemplate,
  TemplateListResponse,
  TimelineEvent,
  TodayStats,
} from './bindings'
import type { InboxStatus } from './index'

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

/** 案卷目录树条目 */
export interface CaseDirEntry {
  name: string
  relPath: string
  fileCount: number
  state: 'ok' | 'empty' | 'warn'
}

export type CommandMap = {
  // ── 卷宗管理（index-v2 精装版 · 本地文件夹同步）──
  list_case_dirs: Cmd<{ caseId: string }, CaseDirEntry[]>
  create_case_subdir: Cmd<{ caseId: string; parentRel: string | null; name: string }, string>
  import_files_to_case: Cmd<
    { caseId: string; dirRel: string | null; paths: string[] },
    Array<{ id: string; fileName: string; archivedPath: string; originalPath: string }>
  >
  scan_unregistered_files: Cmd<{ caseId: string }, Array<{ fileName: string; path: string; sizeBytes: number }>>
  register_existing_files: Cmd<{ caseId: string; paths: string[] }, number>
  reveal_path: Cmd<{ path: string }, null>
  open_file_with_default: Cmd<{ path: string }, null>
  apply_case_file_renames: Cmd<
    { caseId: string; renames: Array<{ id: string; newName: string }> },
    Array<{ id: string; oldName: string; newName: string }>
  >

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
  // B1 类型化：create_case/update_case 参数因「缺键跳过 vs null 清除」三态语义复杂，保持 Record
  create_case: Cmd<{ data: Record<string, unknown> }, Case>
  update_case: Cmd<{ id: string; data: Record<string, unknown> }, Case>
  // B1 类型化：新增类型化命令
  list_field_groups: Cmd<{ caseType?: string }, FieldGroup[]>
  get_case_unified_view: Cmd<{ filters?: Record<string, unknown> }, CaseUnifiedView[]>
  get_today_stats: Cmd<Record<string, unknown>, TodayStats>
  get_case_type_metrics: Cmd<{ caseId: string }, CaseTypeMetrics>
  get_all_case_type_metrics: Cmd<Record<string, unknown>, CaseTypeMetrics[]>

  // ── 关系 / 时间线 ──
  add_relation: Cmd<Record<string, unknown>, CaseRelation>
  detect_relations: Cmd<{ caseId: string }, CaseRelation[]>
  get_relations: Cmd<{ caseId: string }, RelatedCase[]>
  get_case_timeline: Cmd<{ caseId: string }, TimelineEvent[]>

  // ── 文件 ──
  list_case_files: Cmd<{ caseId: string }, CaseFile[]>

  // ── 仪表盘（B4 数据可视化）──
  get_project_status_distribution: Cmd<Record<string, unknown>, NameCount[]>
  get_track_distribution: Cmd<Record<string, unknown>, NameCount[]>
  get_monthly_task_trend: Cmd<{ months?: number }, MonthTrendPoint[]>
  get_upcoming_hearings: Cmd<{ days?: number }, UpcomingHearing[]>
  get_today_kpis: Cmd<Record<string, unknown>, TodayKpis>
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
  feishu_import_all: Cmd<{
    appToken: string
    tableId: string
    localTable: string
    mappings: MappingEntry[]
  }, ImportResult>
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

  // ── 任务域 ──
  list_tasks: Cmd<{ filter?: Partial<TaskFilter> }, TaskDto[]>
  search_tasks: Cmd<{ query: string }, SearchTaskDto[]>

  // ── 知识域 ──
  create_knowledge: Cmd<{ data: CreateKnowledgeInput }, string>

  // ── 收件箱域（B1 类型化首批）──
  add_inbox_item: Cmd<{
    sourceType: string
    title?: string | null
    contentText?: string | null
    sourcePath?: string | null
  }, string>
  // status 由 schema CHECK 约束枚举，bindings 侧为 string——此处收窄为手写联合
  list_inbox_items: Cmd<{ status?: string }, (Omit<InboxItemDto, 'status'> & { status: InboxStatus })[]>
  process_inbox_item: Cmd<{ id: string }, ProcessedInboxResult>
  file_inbox_item: Cmd<{ itemId: string; caseId: string; category: string }, void>
  dismiss_inbox_item: Cmd<{ id: string }, void>
  get_inbox_progress: Cmd<Record<string, unknown>, InboxProgress>
  parse_holiday_notice: Cmd<{ content: string }, HolidayNotice>

  // ── 日历域 ──
  get_calendar_events: Cmd<{ year: number; month: number }, CalendarEvent[]>
  list_calendar_events: Cmd<{ startDate: string; endDate: string }, CalendarEventRow[]>
  create_calendar_event: Cmd<{ data: Record<string, unknown> }, CalendarEventRow>
  update_calendar_event: Cmd<{ id: string; data: Record<string, unknown> }, void>
  move_calendar_event: Cmd<{ id: string; newDate: string; newStart?: string }, void>
  delete_calendar_event: Cmd<{ id: string }, void>

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
