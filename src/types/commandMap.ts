/**
 * 命令契约注册表（D-3 / B1：tauriBridge 泛型化的数据源）
 *
 * - result 类型来自 src/types/bindings.ts（specta 生成物，禁止手改）
 * - params 类型按各 service 实际传参手写核定；核心写命令禁止退回宽 JSON 参数
 * - 不在本表中的命令无法通过 tauriBridge 调用，避免新命令绕过契约
 */
import type {
  ProcessingCenter,
  AiChatResult,
  AiConfig,
  AppNotification,
  CasePersonDto,
  ContextRef,
  DeadlineRuleAuditDto,
  DeadlineRuleDto,
  FactNodeDto,
  FactHistoryDto,
  ProcedureEvent,
  ProcedureBoard,
  ProcedureAudit,
  FileOcrStateDto,
  LinkDto,
  PersonCaseDto,
  PersonDto,
  ProposalPreviewDto,
  SmartRuleApplyResult,
  SmartRuleDto,
  WhiteboardDto,
  AreaDto,
  AreaStatsDto,
  CalendarEvent,
  CalendarEventRow,
  CalendarSyncReport,
  CaseFile,
  CaseImportConfig,
  CaseImportReport,
  CaseFilter,
  CaseRelation,
  CaseStats,
  CaseTypeMetrics,
  CaseUnifiedView,
  ChatMessage,
  CommandRoute,
  DashboardStats,
  DiffLineDto,
  DeadlineResult,
  DeadlineWarning,
  Draft,
  DraftVersion,
  ExportResponse,
  FeishuSyncReport,
  FieldGroup,
  HolidayNotice,
  CreateAreaInput,
  CreateAreaOutput,
  CreateKnowledgeInput,
  FolderNamingSettingsInput,
  FolderNamingSettingsOutput,
  FolderTemplateInput,
  FolderTemplateOutput,
  ImportReport,
  GraphEdgeDto,
  GraphNodeDto,
  ImportResult,
  InboxItemDto,
  InboxProgress,
  KnowledgeBlockDto,
  KnowledgeDiffCurrentResult,
  KnowledgeDiffVersionsResult,
  KnowledgeGraphDto,
  KnowledgeDocumentSourceDto,
  PageIndexImportResultDto,
  KnowledgeItemDto,
  KnowledgeWithBlocksDto,
  KnowledgeStatsDto,
  KnowledgeTreeBlockDto,
  KnowledgeVersionDto,
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
  SearchKnowledgeDto,
  SearchResult,
  SyncResult,
  SearchTaskDto,
  SyncStatus,
  SubtableImportConfig,
  SubtableImportReport,
  TaskDto,
  TaskPlan,
  TaskPlanInput,
  TaskFilter,
  UpdateAreaInput,
  TaskTemplate,
  TemplateListResponse,
  TimelineEvent,
  TodayStats,
} from './bindings'
import type { InboxStatus } from './index'
import type {
  Case as BusinessCase,
  CaseListResponse,
  DashboardStats as BusinessDashboardStats,
  Task,
} from './index'
import type { AiProposal } from '../core/ai/proposals'
import type { RichTextDocument } from '../core/services/docs'
import type {
  CasePersonDto as BusinessCasePersonDto,
  PersonCaseDto as BusinessPersonCaseDto,
  PersonDto as BusinessPersonDto,
} from '../modules/persons/types'
import type {
  CalendarEventInput,
  CalendarSyncStatus,
  CasePatchInput,
  CreateCasePayload,
  CreateTaskPayload,
  EmailMonitorStatus,
  FeishuBitableFieldInfo,
  FeishuBitableTableInfo,
  FeishuMappingPayload,
  FeishuSyncInfo,
  HolidayCalendarPayload,
  IpcJsonObject,
  IpcJsonScalar,
  KeychainStatus,
  KnowledgePatchInput,
  ReminderRuleInput,
  SmartSummaryRow,
  TodayRecommendations,
  UpdateTaskPayload,
} from './ipc'

export interface Cmd<P, R> {
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
export interface HearingInput {
  caseId: string
  hearingDate: string
  hearingName?: string | null
  court?: string | null
  venue?: string | null
  judges?: string | null
  caseLevel?: string | null
  contactInfo?: string | null
  actualStatus?: string | null
  lifecycleStatus?: string | null
  changeReason?: string | null
}

export interface AiUsage {
  usedToday: number
  dailyLimit: number
  remaining: number
}

export interface CaseDirEntry {
  name: string
  relPath: string
  absolutePath?: string
  fileCount: number
  state: 'ok' | 'empty' | 'warn'
}

export type CommandMap = {
  convert_file_to_markdown: Cmd<
    { sourcePath: string; outputDir: string; jobId?: string | null; targetFormat?: 'markdown' | 'pdf' | 'both' | null },
    { outputPath: string; markdownPath?: string | null; pdfPath?: string | null; pages: number; bytes: number }
  >
  // ── 卷宗管理（index-v2 精装版 · 本地文件夹同步）──
  list_case_dirs: Cmd<{ caseId: string }, CaseDirEntry[]>
  create_case_subdir: Cmd<{ caseId: string; parentRel: string | null; name: string }, string>
  import_files_to_case: Cmd<
    { caseId: string; dirRel: string | null; paths: string[]; category?: string },
    Array<{ id: string; fileName: string; archivedPath: string; originalPath: string }>
  >
  scan_unregistered_files: Cmd<{ caseId: string }, Array<{ fileName: string; path: string; sizeBytes: number }>>
  register_existing_files: Cmd<{ caseId: string; paths: string[] }, number>
  get_workspace_sync_status: Cmd<{}, { checkedAt: string; registered: number; queued: number; mirrored: number; renamed: number; missing: number; errors: string[] }>
  list_workspace_sources: Cmd<{ caseIds: string[] }, { fileId: string; caseId: string; fileName: string; filePath: string; jobId: string | null; status: string | null; totalPages: number | null; error: string | null; missing: boolean }[]>
  rollback_document_storage: Cmd<{ fileId: string; jobId: string }, string>
  get_document_storage_state: Cmd<{ fileId: string }, { jobId: string; externalImages: boolean; canOptimize: boolean; canRollback: boolean }>
  optimize_document_storage: Cmd<{ fileId: string; jobId: string }, string>
  read_document_asset: Cmd<{ fileId: string; jobId: string; assetId: string }, string>
  read_knowledge_asset: Cmd<{ noteId: string; assetId: string }, string>
  get_workspace_document: Cmd<{ fileId: string }, { jobId: string; markdown: string; filePath: string; continuations: { fromPage: number; toPage: number; locations: import('./documentRetrieval').SourceLocation[] }[] }>
  reveal_path: Cmd<{ path: string }, null>
  open_file_with_default: Cmd<{ path: string }, null>
  apply_case_file_renames: Cmd<
    { caseId: string; renames: Array<{ id: string; newName: string }> },
    Array<{ id: string; oldName: string; newName: string; warning?: string | null }>
  >
  list_removed_case_files: Cmd<{ caseId: string }, CommandMap['list_case_files']['result']>
  restore_case_file: Cmd<{ id: string }, void>
  move_case_files: Cmd<{ caseId: string; ids: string[]; dirRel: string | null }, CommandMap['apply_case_file_renames']['result']>
  set_case_file_category: Cmd<{ id: string; category: string }, void>

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
  // 说明（P1-7 收口）：get/create/update/search 的 result 使用 index 业务 Case 而非常
  // bindings.Case（bindings 是 specta 生成、字段更宽松/可空）。当前前后端契约以业务类型
  // 为准（store/组件全部基于 index.Case），Case 类型统一迁移留待骨架重构，详见残留风险。
  get_case: Cmd<{ id: string }, BusinessCase>
  list_cases: Cmd<{ filter?: Partial<CaseFilter> }, CaseListResponse>
  search_cases: Cmd<{ query: string }, BusinessCase[]>
  case_stats: Cmd<{}, CaseStats>
  get_dashboard_stats: Cmd<{}, BusinessDashboardStats>
  update_case_status: Cmd<{ caseId: string; track: string; newStatus: string; note?: string | null }, BusinessCase>
  export_cases: Cmd<{ format: string; filter?: Partial<CaseFilter> }, string>
  // B1 类型化：Case PATCH 仍保持缺键跳过 / null 清除语义，但字段集合已收口到后端白名单。
  create_case: Cmd<{ data: CreateCasePayload }, BusinessCase>
  update_case: Cmd<{ id: string; data: CasePatchInput }, BusinessCase>
  // B1 类型化：新增类型化命令
  list_field_groups: Cmd<{ caseType?: string }, FieldGroup[]>
  get_case_unified_view: Cmd<{ filters?: Partial<CaseFilter> }, CaseUnifiedView[]>
  get_today_stats: Cmd<{}, TodayStats>
  get_case_type_metrics: Cmd<{ caseId: string }, CaseTypeMetrics>
  get_all_case_type_metrics: Cmd<{}, CaseTypeMetrics[]>

  // ── 关系 / 时间线 ──
  add_relation: Cmd<{
    caseId: string
    relatedId: string
    relationType: string
    label?: string | null
    mergeData?: boolean
  }, CaseRelation>
  detect_relations: Cmd<{ caseId: string }, CaseRelation[]>
  get_relations: Cmd<{ caseId: string }, RelatedCase[]>
  get_case_timeline: Cmd<{ caseId: string }, TimelineEvent[]>
  remove_relation: Cmd<{ id: string }, void>

  // ── 文件 ──
  list_case_files: Cmd<{ caseId: string; category?: string | null }, CaseFile[]>
  delete_case_file: Cmd<{ id: string }, void>

  // ── 仪表盘（B4 数据可视化）──
  get_project_status_distribution: Cmd<{}, NameCount[]>
  get_track_distribution: Cmd<{}, NameCount[]>
  get_monthly_task_trend: Cmd<{ months?: number }, MonthTrendPoint[]>
  get_upcoming_hearings: Cmd<{ days?: number }, UpcomingHearing[]>
  get_today_kpis: Cmd<{}, TodayKpis>
  seed_demo_data: Cmd<{}, { cases: number; projectsPersonal: number; tasks: number; areas: number; knowledge: number; hearings: number }>
  add_case_file: Cmd<{ caseId: string; fileName: string; filePath: string; category: string }, CaseFile>

  // ── 提醒域 ──
  list_reminder_rules: Cmd<{}, ReminderRule[]>
  create_reminder_rule: Cmd<{ data: ReminderRuleInput }, ReminderRule>
  update_reminder_rule: Cmd<{ id: string; data: ReminderRuleInput }, void>
  delete_reminder_rule: Cmd<{ id: string }, void>
  get_reminder_log: Cmd<{ limit?: number }, ReminderLogEntry[]>
  get_deadline_warnings: Cmd<{}, DeadlineResult[]>
  get_deadline_warnings_with_levels: Cmd<{}, DeadlineWarning[]>
  test_reminder: Cmd<{ channels: string[]; message?: string | null }, ReminderLogEntry[]>
  start_reminder_engine: Cmd<{ intervalSecs?: number | null }, void>
  reminder_recompute_now: Cmd<{}, number>
  record_reminder_feedback: Cmd<{ reminderLogId?: string | null; taskId?: string | null; status: string }, void>

  // ── 备份域 ──
  create_backup: Cmd<{}, import('./bindings').BackupFile>
  list_backups: Cmd<{}, import('./bindings').BackupFile[]>
  restore_backup: Cmd<{ filename: string }, boolean>
  export_full_backup: Cmd<{ destination: string; password: string }, boolean>
  import_full_backup: Cmd<{ source: string; password: string }, boolean>
  save_editor_recovery: Cmd<{ sessionId: string; draft: Record<string, unknown> | null }, void>
  recover_editor_drafts: Cmd<{}, number>
  correct_document_region: Cmd<{ fileId: string; jobId: string; pageNumber: number; regionIndex: number; expectedText: string; text: string }, string>

  // ── 同步域 ──
  get_sync_status: Cmd<{}, SyncStatus>
  test_webdav_connection: Cmd<{ url: string; username: string; password: string }, string>
  webdav_backup_full: Cmd<{ url: string; username: string; password: string; backupPassword: string }, string>
  webdav_restore_full: Cmd<{ url: string; username: string; password: string; backupPassword: string }, boolean>
  webdav_push: Cmd<{ url: string; username: string; password: string }, SyncResult>
  webdav_pull: Cmd<{ url: string; username: string; password: string }, SyncResult>
  webdav_startup_sync: Cmd<{ url: string; username: string; password: string }, SyncResult>
  webdav_resolve_keep_local: Cmd<{ url: string; username: string; password: string }, SyncResult>
  webdav_resolve_keep_remote: Cmd<{ url: string; username: string; password: string }, SyncResult>
  sync_feishu_pull: Cmd<{ appToken: string; tableId: string }, FeishuSyncReport>
  sync_feishu_push: Cmd<{ appToken: string; tableId: string }, FeishuSyncReport>
  get_folder_template: Cmd<{ templateId: string }, FolderTemplateOutput>
  list_folder_templates: Cmd<{}, FolderTemplateOutput[]>
  get_folder_naming_settings: Cmd<{}, FolderNamingSettingsOutput>
  backup_database_key_to_keychain: Cmd<{}, void>
  get_settings: Cmd<{}, Record<string, IpcJsonScalar | IpcJsonObject | IpcJsonScalar[] | IpcJsonObject[]>>
  save_settings: Cmd<{ settings: Record<string, IpcJsonScalar | IpcJsonObject | IpcJsonScalar[] | IpcJsonObject[]> }, void>
  list_saved_filters: Cmd<{ module?: string | null; entityType?: string | null }, IpcJsonObject[]>
  save_filter: Cmd<{ filter: IpcJsonObject }, IpcJsonObject>
  delete_filter: Cmd<{ id: string }, void>
  get_lawyer_profile: Cmd<{}, IpcJsonObject>
  save_lawyer_profile: Cmd<{ profile: IpcJsonObject }, IpcJsonObject>
  save_folder_template: Cmd<{ data: Partial<FolderTemplateInput> }, string>
  delete_folder_template: Cmd<{ templateId: string }, void>
  save_folder_naming_settings: Cmd<{ data: Partial<FolderNamingSettingsInput> }, void>
  get_holidays_summary: Cmd<{}, { holidaysCount: number; workdaysCount: number; yearRange: string | null }>
  import_holidays_json: Cmd<{ jsonPath: string }, IpcJsonObject>
  get_email_monitor_status: Cmd<{}, EmailMonitorStatus>
  list_imap_accounts: Cmd<{}, IpcJsonObject[]>
  configure_imap: Cmd<{ account: import('./bindings').ImapAccountConfig }, string>
  delete_imap_account: Cmd<{ emailAddress: string }, string>
  start_email_monitor: Cmd<{}, string>
  stop_email_monitor: Cmd<{}, string>
  check_keychain_status: Cmd<{}, KeychainStatus>
  approve_mcp_write: Cmd<{ id: string }, IpcJsonObject>
  reject_mcp_write: Cmd<{ id: string }, void>

  feishu_import_all: Cmd<{
    appToken: string
    tableId: string
    localTable: string
    mappings: MappingEntry[]
  }, ImportResult>
  feishu_compare_table: Cmd<{ appToken: string; tableId: string; localTable: string }, SchemaDiff>
  feishu_compare_records: Cmd<{ appToken: string; tableId: string; localTable: string; matchField: string }, RecordDiff>
  feishu_import_incremental: Cmd<{
    appToken: string
    tableId: string
    localTable: string
    sinceTimestamp: string
    mappingsJson: MappingEntry[]
  }, ImportResult>
  feishu_sync_pull: Cmd<{ appToken: string; tableId: string; localTable: string; mappingsJson: MappingEntry[] }, FeishuSyncReport>
  feishu_sync_push: Cmd<{ appToken: string; tableId: string; localTable: string; mappingsJson: MappingEntry[] }, FeishuSyncReport>
  sync_export_persons_to_vcard: Cmd<{}, string>
  sync_export_whiteboards_to_blob: Cmd<{}, string>

  import_feishu_data: Cmd<{ jsonPath: string }, ImportReport>
  sync_reminders_to_calendar: Cmd<{}, CalendarSyncReport>
  configure_feishu_table: Cmd<{ appToken: string; tableId: string }, string>
  configure_feishu: Cmd<{ appId: string; appSecret: string }, string>
  test_feishu_connection: Cmd<{ appId?: string | null; appSecret?: string | null }, string>
  get_feishu_sync_info: Cmd<{}, FeishuSyncInfo>
  feishu_list_tables: Cmd<{ appToken: string }, FeishuBitableTableInfo[]>
  feishu_list_fields: Cmd<{ appToken: string; tableId: string }, FeishuBitableFieldInfo[]>
  feishu_save_mappings: Cmd<{ mappingsJson: FeishuMappingPayload[] }, string>

  // ── AI 域 ──
  get_ai_profiles: Cmd<{}, import('./aiProfiles').AiProfiles>
  save_ai_profiles: Cmd<{ config: import('./aiProfiles').AiProfiles }, import('./aiProfiles').AiProfiles>
  test_ai_profile: Cmd<{ profile: import('./aiProfiles').AiProfile }, string>
  ai_chat: Cmd<{
    messages: Array<Pick<ChatMessage, 'role' | 'content'>>
    mode?: string
    apiUrl?: string
    model?: string
    purpose?: string
    contextRefs?: ContextRef[] | null
    profileId?: string
  }, AiChatResult>
  get_ai_config: Cmd<{}, AiConfig>
  configure_ai: Cmd<{ mode: string; apiUrl?: string | null; apiKey?: string | null; model?: string | null; dailyLimit?: number | null }, string>
  get_command_route_info: Cmd<{ commandName: string }, CommandRoute | null>
  get_today_recommendations: Cmd<{}, TodayRecommendations>
  record_decision: Cmd<{ entityType: string; entityId: string; decisionType: string; decision: string; basis?: string | null; sourceRef?: string | null; status?: string; reviewDue?: string | null }, { id: string }>
  get_learning_analysis: Cmd<{}, IpcJsonObject>
  apply_learning_calibration: Cmd<{}, IpcJsonObject>
  list_pending_memories: Cmd<{}, IpcJsonObject[]>
  confirm_memory: Cmd<{ id: string; sinkToKnowledge?: boolean | null }, IpcJsonObject>
  dismiss_memory: Cmd<{ id: string }, void>
  list_pending_insights: Cmd<{}, IpcJsonObject[]>
  generate_insights_cmd: Cmd<{}, { inserted: number }>
  confirm_insight: Cmd<{ id: string; sinkToKnowledge?: boolean | null }, IpcJsonObject>
  dismiss_insight: Cmd<{ id: string }, void>
  list_summaries: Cmd<{ summaryType?: string | null; limit?: number | null }, IpcJsonObject[]>
  list_decisions: Cmd<{ entityType?: string | null; status?: string | null; limit?: number | null }, IpcJsonObject[]>
  get_pending_decision_reviews: Cmd<{}, IpcJsonObject[]>
  mark_decision_reviewed: Cmd<{ id: string; stillValid: boolean; note?: string | null }, void>
  run_recursive_check: Cmd<{ decisionId: string }, IpcJsonObject>
  get_ai_run_history: Cmd<{ limit?: number | null; purpose?: string | null }, IpcJsonObject[]>
  record_ai_tool_audit: Cmd<{ tool: string; turnId: string; outcome: string; digest?: string | null; runId?: string | null }, void>
  quick_judge_inbox_item: Cmd<{ id: string }, QuickJudgeResult>
  ai_analyze_inbox_item: Cmd<{ id: string }, ProcessedInboxResult>
  list_mcp_pending_writes: Cmd<{}, McpPendingWrite[]>
  get_feishu_auto_push_status: Cmd<{}, { enabled: boolean; pending: boolean; hasTimer: boolean; configured: boolean }>
  set_feishu_auto_push: Cmd<{ enabled: boolean }, string>

  // ── 任务域 ──
  list_tasks: Cmd<{ filter?: Partial<TaskFilter> }, Task[]>
  search_tasks: Cmd<{ query: string }, SearchTaskDto[]>
  // P1-6：撤销删除专用还原命令（保留原 id / completed；snapshot 为前端 Task 快照）
  restore_task: Cmd<{ snapshot: Task }, TaskDto>
  // 任务写命令（B1 类型化）：data 字段集合与后端 create_task / UpdateTaskPatch 对齐。
  // create_task 后端实际仅返回 { id }（非完整 Task），勿误标为 Task；写类命令返回 null。
  create_task: Cmd<{ data: CreateTaskPayload }, { id: string }>
  update_task: Cmd<{ data: UpdateTaskPayload }, null>
  toggle_task: Cmd<{ id: string; actualMinutes?: number | null; origin?: string | null; proposalToken?: string | null }, null>
  delete_task: Cmd<{ id: string; origin?: string | null; proposalToken?: string | null }, null>
  snooze_task: Cmd<{ id: string; option?: string | null; newDueDate?: string | null }, void>


  search_knowledge: Cmd<{ query: string }, SearchKnowledgeDto[]>
  list_knowledge: Cmd<{
    filter?: { category?: string | null; caseId?: string | null; lawName?: string | null } | null
  }, KnowledgeItemDto[]>
  global_search: Cmd<{ query: string }, import('./bindings').GlobalSearchResult[]>
  list_knowledge_blocks: Cmd<{ parentId: string }, KnowledgeBlockDto[]>
  get_knowledge_with_blocks: Cmd<{ id: string }, KnowledgeWithBlocksDto>
  knowledge_stats: Cmd<{}, KnowledgeStatsDto>
  list_knowledge_versions: Cmd<{ itemId: string }, KnowledgeVersionDto[]>
  diff_knowledge_versions: Cmd<{ versionId1: string; versionId2: string }, KnowledgeDiffVersionsResult>
  diff_knowledge_with_current: Cmd<{ versionId: string; itemId: string }, KnowledgeDiffCurrentResult>
  get_knowledge_graph: Cmd<{ limit?: number }, KnowledgeGraphDto>
  list_knowledge_document_sources: Cmd<{}, KnowledgeDocumentSourceDto[]>
  import_pageindex_to_knowledge: Cmd<{ fileId: string }, PageIndexImportResultDto>
  restore_knowledge_version: Cmd<{ itemId: string; versionId: string }, void>
  export_knowledge_markdown: Cmd<
    { itemId: string; outputPath: string },
    { outputPath: string; fileSize: number; exportedAt: string }
  >

  // ── 知识域 ──
  create_knowledge: Cmd<{ data: Partial<CreateKnowledgeInput> }, string>
  update_knowledge: Cmd<{ id: string; data: KnowledgePatchInput }, void>
  delete_knowledge: Cmd<{ id: string }, void>

  // ── 领域域（DomainCommand 样板）──
  list_areas: Cmd<{}, AreaDto[]>
  get_area: Cmd<{ id: string }, AreaDto>
  create_area: Cmd<{ data: CreateAreaInput }, CreateAreaOutput>
  update_area: Cmd<{ id: string; data: Partial<UpdateAreaInput> }, void>
  delete_area: Cmd<{ id: string }, void>
  get_area_stats: Cmd<{ id: string }, AreaStatsDto>

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
  get_inbox_progress: Cmd<{}, InboxProgress>
  get_inbox_action_result: Cmd<{ inboxItemId: string }, IpcJsonObject | null>
  parse_holiday_notice: Cmd<{ content: string }, HolidayNotice>
  confirm_inbox_action: Cmd<{
    inboxItemId: string
    action: string
    targetCaseId?: string | null
    targetCategory?: string | null
    intent?: IpcJsonObject | null
  }, IpcJsonObject>
  reject_inbox_recommendation: Cmd<{ inboxItemId: string; action: string; reason?: string | null; intent?: IpcJsonObject | null }, void>
  transcribe_voice_note: Cmd<{ voiceNoteId: string }, string>
  start_inbox_batch: Cmd<{}, void>
  pause_inbox_batch: Cmd<{}, void>
  resume_inbox_batch: Cmd<{}, void>
  cancel_inbox_batch: Cmd<{}, void>

  // ── 日历域 ──
  get_calendar_events: Cmd<{ year: number; month: number; monthCount?: number }, CalendarEvent[]>
  list_calendar_events: Cmd<{ startDate: string; endDate: string }, CalendarEventRow[]>
  create_calendar_event: Cmd<{ data: CalendarEventInput }, CalendarEventRow>
  update_calendar_event: Cmd<{ id: string; data: CalendarEventInput }, void>
  move_calendar_event: Cmd<{ id: string; newDate: string; newStart?: string | null }, void>
  delete_calendar_event: Cmd<{ id: string }, void>
  list_task_plans: Cmd<Record<string, never>, TaskPlan[]>
  save_task_plan: Cmd<{ data: TaskPlanInput }, TaskPlan>
  get_holiday_calendar: Cmd<{ year: number }, HolidayCalendarPayload>
  send_ics_invitation_cmd: Cmd<{ to: string; subject: string; description: string; startIso: string; durationMinutes: number; alarmMinutes: number }, string>
  test_caldav_connection: Cmd<{}, string>
  get_calendar_sync_status: Cmd<{}, CalendarSyncStatus>
  get_today_brief: Cmd<{}, SmartSummaryRow>
  generate_daily_brief_cmd: Cmd<{}, IpcJsonObject>

  // ── 文书引擎 ──
  list_docsy_templates: Cmd<{}, TemplateListResponse>
  render_docsy_template: Cmd<{ templateId: string; caseId: string }, RenderResponse>
  export_docx: Cmd<{ templateId: string; caseId: string; outputPath?: string | null }, ExportResponse>
  export_editor_document: Cmd<{document:import('../core/services/docs').RichTextDocument;markdown:string;title:string;format:'md'|'pdf'|'docx'|'docx-annotated';outputPath:string;layout?:import('../core/services/docs').DocumentLayout},ExportResponse>
  preview_editor_document: Cmd<{document:import('../core/services/docs').RichTextDocument;layout:import('../core/services/docs').DocumentLayout},import('../core/services/docs').DocumentPreview>
  export_edited_docx: Cmd<{
    document: RichTextDocument
    title: string
    outputPath?: string | null
  }, ExportResponse>

  // ── 知识检索 ──
  hybrid_search_knowledge: Cmd<{ query: string; limit?: number }, SearchResult[]>
  test_embedding_connection: Cmd<{}, string>
  embed_knowledge: Cmd<{ itemId: string; force?: boolean }, string>
  embed_all_knowledge: Cmd<{}, { queued: number; upToDate: number; alreadyQueued: number }>
  get_knowledge_index_status: Cmd<{}, import('./knowledgeIndex').KnowledgeIndexStatus>
  cancel_knowledge_index_job: Cmd<{ jobId: string }, void>
  search_knowledge_index: Cmd<{ query: string; useSemantic: boolean }, import('./knowledgeIndex').KnowledgeSearchResponse>

  // ── 草稿 ──
  list_drafts: Cmd<{}, Draft[]>
  get_draft: Cmd<{ id: string }, Draft>
  list_draft_versions: Cmd<{ id: string; offset?: number }, DraftVersion[]>
  restore_draft_version: Cmd<{ id: string; version: number; expectedVersion: number }, Draft>
  create_draft: Cmd<
    { title: string; content?: string | null; caseId?: string | null; templatePath?: string | null },
    Draft
  >
  update_draft: Cmd<
    { id: string; title?: string; content?: string | null; status?: string; caseId?: string | null; expectedVersion?: number; clearCase?: boolean },
    Draft
  >
  delete_draft: Cmd<{ id: string }, boolean>

  // ── 对标灵感落地（2026-09-01 · Schema v20）──
  // W2 通知中心（Linear 式 Inbox-Zero）
  list_notifications: Cmd<{}, AppNotification[]>
  unread_notification_count: Cmd<{}, number>
  create_notification: Cmd<{ kind: string; title: string; body?: string | null; payloadJson?: string | null }, string>
  mark_notification_read: Cmd<{ id: string }, void>
  dismiss_notification: Cmd<{ id: string }, void>
  dismiss_all_notifications: Cmd<{}, number>
  // W2 Defer Date（OmniFocus 式推迟日）
  defer_task: Cmd<{ taskId: string; until: string }, void>
  clear_task_defer: Cmd<{ taskId: string }, void>
  // W3 期限规则（LawToolBox 式可审计）
  list_deadline_rules: Cmd<{}, DeadlineRuleDto[]>
  upsert_deadline_rule: Cmd<{ id?: string | null; track: string; ruleName: string; legalBasis: string; triggerField: string; offsetValue: number; offsetUnit: string; calcMethod: string; procedureTypes?: string | null; deadlineSource: string; priority: number }, string>
  toggle_deadline_rule: Cmd<{ id: string; enabled: boolean }, void>
  delete_deadline_rule: Cmd<{ id: string }, void>
  list_deadline_rule_audit: Cmd<{ ruleId?: string | null }, DeadlineRuleAuditDto[]>
  recalculate_deadlines_for_track: Cmd<{ track: string }, number>
  // W4 跨模块双链（Hookmark/Obsidian 式）
  create_link: Cmd<{ sourceType: string; sourceId: string; targetType: string; targetId: string; anchor?: string | null; label?: string | null }, LinkDto>
  remove_link: Cmd<{ id: string }, void>
  list_links_for: Cmd<{ sourceType: string; sourceId: string }, LinkDto[]>
  get_backlinks: Cmd<{ targetType: string; targetId: string }, LinkDto[]>
  // W5 Smart Rules + 本地 OCR（DEVONthink 式）
  list_smart_rules: Cmd<{}, Array<Omit<SmartRuleDto, 'matchField' | 'actionType'> & {
    matchField: 'filename' | 'ocr_text'
    actionType: 'set_category' | 'mark_urgent' | 'add_keyword'
  }>>
  upsert_smart_rule: Cmd<{ id?: string | null; name: string; enabled: boolean; matchField: string; matchPattern: string; actionType: string; actionPayload: string }, string>
  delete_smart_rule: Cmd<{ id: string }, void>
  apply_smart_rules: Cmd<{ fileId: string }, SmartRuleApplyResult>
  run_smart_rules_for_all: Cmd<{}, number>
  list_pending_ocr_files: Cmd<{}, [string, string, string][]>
  ocr_case_file: Cmd<{ fileId: string }, string>
  ocr_all_pending: Cmd<{}, number>
  list_case_ocr_states: Cmd<{ caseId: string }, FileOcrStateDto[]>
  get_file_ocr_text: Cmd<{ fileId: string }, string | null>
  // W6 对象化实体（Capacities 式）
  list_persons: Cmd<{ kind?: string | null; keyword?: string | null }, BusinessPersonDto[]>
  upsert_person: Cmd<{ id?: string | null; kind: string; name: string; org?: string | null; phone?: string | null; email?: string | null; preferences?: string | null; notes?: string | null }, string>
  delete_person: Cmd<{ id: string }, void>
  attach_person_to_case: Cmd<{ caseId: string; personId: string; role?: string | null }, string>
  detach_person_from_case: Cmd<{ linkId: string }, void>
  list_case_persons: Cmd<{ caseId: string }, BusinessCasePersonDto[]>
  list_person_cases: Cmd<{ personId: string }, BusinessPersonCaseDto[]>
  // W7 事实白板（LiquidText 式）
  get_editor_tasks: Cmd<{sourceType:string;sourceId:string}, import('./bindings').EditorTask[]>
  bind_editor_task: Cmd<{sourceType:string;sourceId:string;bindingId:string;title:string;taskId:string|null;caseId:string|null},string>
  set_editor_task_completed: Cmd<{sourceType:string;sourceId:string;taskId:string;completed:boolean;expectedCompleted:boolean},null>
  get_whiteboard_document: Cmd<{caseId:string;whiteboardId:string}, import('./bindings').WhiteboardDocument>
  save_whiteboard_document: Cmd<{input:import('./bindings').WhiteboardDocumentInput}, import('./bindings').WhiteboardDocument>
  list_whiteboard_sources: Cmd<{caseId:string;scope:string;query:string;offset:number}, import('./bindings').WhiteboardSources>
  write_whiteboard_export: Cmd<{caseId:string;whiteboardId:string;outputPath:string;format:string;dataBase64:string}, string>
  get_whiteboard_scene: Cmd<{caseId: string; whiteboardId: string}, import('./bindings').WhiteboardSceneDto>
  save_whiteboard_scene: Cmd<{caseId: string; whiteboardId: string; sceneJson: string; preview: string | null; revision: number}, number>
  list_whiteboard_scene_history: Cmd<{caseId: string; whiteboardId: string}, import('./bindings').WhiteboardSceneDto[]>
  list_whiteboards: Cmd<{ caseId: string }, WhiteboardDto[]>
  create_whiteboard: Cmd<{ caseId: string; name: string }, string>
  rename_whiteboard: Cmd<{ id: string; name: string }, void>
  delete_whiteboard: Cmd<{ id: string }, void>
  get_procedure_board: Cmd<{ caseId: string; includeRelated: boolean }, ProcedureBoard>
  save_procedure_event: Cmd<{ event: ProcedureEvent; reason: string }, ProcedureEvent>
  set_procedure_item_state: Cmd<{ caseId: string; itemId: string; fingerprint: string; status: string; note: string }, null>
  get_procedure_history: Cmd<{ caseId: string }, ProcedureAudit[]>
  list_fact_history: Cmd<{ whiteboardId: string; beforeSequence?: number | null }, FactHistoryDto[]>
  list_fact_nodes: Cmd<{ whiteboardId: string }, FactNodeDto[]>
  create_fact_node: Cmd<{ whiteboardId: string; fileId: string | null; page: number | null; excerpt: string; note: string | null; x: number; y: number }, string>
  update_fact_node: Cmd<{ id: string; page?: number | null; excerpt?: string; note?: string | null; x?: number; y?: number }, void>
  delete_fact_node: Cmd<{ id: string }, void>
  list_whiteboard_edges: Cmd<{ whiteboardId: string }, import('./bindings').WhiteboardEdgeDto[]>
  create_whiteboard_edge: Cmd<{ whiteboardId: string; sourceNodeId: string; targetNodeId: string }, string>
  delete_whiteboard_edge: Cmd<{ id: string }, void>

  // ==========================================W1 AI Proposal 预览（Cursor 式 Diff 确认）
  get_proposal_preview: Cmd<{ proposalId: string }, import('../core/ai/proposals').ProposalPreviewDto>
  renew_ai_proposal: Cmd<{ proposalId: string }, AiProposal>
  approve_ai_proposal: Cmd<{ proposalId: string }, string>
  reject_ai_proposal: Cmd<{ proposalId: string }, null>
  create_ai_proposal: Cmd<{
    toolName: string
    targetEntityType: string
    targetEntityId: string | null
    preStateHash: string | null
    payloadJson: string
    ttlSeconds: number | null
  }, AiProposal>

  // ====================================契约门禁补齐（tests/contract.commands.test.ts 防回退）
  // 以下命令此前由前端调用但未登记 CommandMap（frontend ⊄ CommandMap）。
  // 多为复杂命令：params 按调用现场核定，result 优先复用 bindings/手写 DTO，避免漏项回退。
  reasoning_search: Cmd<{ query: string; scope: string[] }, string>
  search_document_passages: Cmd<{ query: string; scope: string[] }, import('./documentRetrieval').DocumentPassage[]>
  get_processing_center: Cmd<{ filter: string; offset: number; limit: number }, ProcessingCenter>
  register_conversion_batch: Cmd<{ sourcePaths: string[] }, string[]>
  cancel_conversion: Cmd<{ jobId: string }, void>
  cancel_queued_conversions: Cmd<{ jobIds: string[] }, void>
  get_document_engine_status: Cmd<{}, import('./bindings').DocumentEngineStatus>
  get_document_page: Cmd<{fileId:string; jobId:string; pageNumber:number}, import('./documentRetrieval').DocumentPageView>
  queue_document_processing: Cmd<{ fileId: string }, import('./bindings').DocumentJobDto>
  list_document_jobs: Cmd<{ fileId: string }, import('./bindings').DocumentJobDto[]>
  list_case_document_jobs: Cmd<{ caseId: string }, import('./bindings').DocumentJobDto[]>
  cancel_document_job: Cmd<{ jobId: string }, void>
  retry_document_job: Cmd<{ jobId: string }, void>
  append_fe_crash: Cmd<{ message: string; stack?: string | null; url?: string | null }, void>
  generate_writing_suggestion: Cmd<{ intent: string; context?: string | null; knowledge?: string | null; style?: string | null }, string>
  link_knowledge_to_case: Cmd<{ knowledgeId: string; caseId: string; relationType?: string | null }, void>
  link_knowledge_to_law: Cmd<{ knowledgeId: string; lawName: string; articleNo?: string | null }, void>
  list_case_hearings: Cmd<{ caseId: string }, import('./bindings').HearingDto[]>
  create_case_hearing: Cmd<{ payload: HearingInput }, import('./bindings').HearingDto>
  update_case_hearing: Cmd<{ id: string; payload: Partial<HearingInput> }, void>
  delete_case_hearing: Cmd<{ id: string }, void>
  excel_get_sheets: Cmd<{ filePath: string }, import('./bindings').SheetInfo[]>
  excel_inspect_sheet: Cmd<{ filePath: string; sheetName: string; headerRowOverride?: number | null }, import('./bindings').ExcelInspectResult>
  feishu_check_config: Cmd<{}, import('./bindings').FeishuConfigStatus>
  feishu_inspect_bitable: Cmd<{ urlOrToken: string; tableIdOverride?: string | null }, import('./bindings').FeishuBitableInspectResult>
  feishu_download_snapshot: Cmd<{ urlOrToken: string }, import('./feishuSnapshot').FeishuSnapshot>
  feishu_import_snapshot: Cmd<{ snapshot: import('./feishuSnapshot').FeishuSnapshot; caseTableId: string; selectedRecordIds: string[] }, import('./feishuSnapshot').SnapshotReport>
  get_case_source_records: Cmd<{ caseId: string }, import('./feishuSnapshot').SourceRecord[]>
  get_imported_asset: Cmd<{ source: string; fileToken: string }, {name: string; mimeType: string; contentBase64: string}>
  export_imported_asset: Cmd<{ source: string; fileToken: string; outputPath: string }, void>
  save_feishu_snapshot: Cmd<{ snapshot: import('./feishuSnapshot').FeishuSnapshot; outputPath: string }, void>
  get_ai_usage: Cmd<{}, AiUsage>
  trigger_feishu_push: Cmd<{}, string>
  start_clipboard_monitor: Cmd<{}, void>
  capture_screenshot: Cmd<{}, string>
  capture_clipboard: Cmd<{}, string>
  save_voice_note: Cmd<{ audioBase64: string; mimeType: string; durationSeconds: number }, string>
  delete_case: Cmd<{ id: string; origin?: string | null; proposalToken?: string | null }, void>
  add_case_log: Cmd<{
    caseId: string
    eventSummary: string
    eventType: string
    eventDate: string
    content: string | null
  }, string>
  excel_import_cases: Cmd<{
    filePath: string
    sheetName: string
    config: Omit<CaseImportConfig, 'defaultTrack'> & { defaultTrack?: string | null }
  }, CaseImportReport>
  feishu_import_bitable_cases: Cmd<{
    appToken: string
    tableId: string
    config: Omit<CaseImportConfig, 'defaultTrack'> & { defaultTrack?: string | null }
  }, CaseImportReport>
  excel_import_subtable: Cmd<{ filePath: string; sheetName: string; config: SubtableImportConfig }, SubtableImportReport>
  feishu_import_bitable_subtable: Cmd<{ appToken: string; tableId: string; config: SubtableImportConfig }, SubtableImportReport>
}
