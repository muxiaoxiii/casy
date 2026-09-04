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
  AppNotification,
  CasePersonDto,
  ContextRef,
  DeadlineRuleAuditDto,
  DeadlineRuleDto,
  FactNodeDto,
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
  TaskDto,
  TaskFilter,
  UpdateAreaInput,
  TaskTemplate,
  TemplateListResponse,
  TimelineEvent,
  TodayStats,
} from './bindings'
import type { InboxStatus } from './index'
import type { Case as BusinessCase, CaseListResponse, Task } from './index'
import type { AiProposal } from '../core/ai/proposals'

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
  // 说明（P1-7 收口）：get/create/update/search 的 result 使用 index 业务 Case 而非常
  // bindings.Case（bindings 是 specta 生成、字段更宽松/可空）。当前前后端契约以业务类型
  // 为准（store/组件全部基于 index.Case），Case 类型统一迁移留待骨架重构，详见残留风险。
  get_case: Cmd<{ id: string }, BusinessCase>
  list_cases: Cmd<{ filter?: Partial<CaseFilter> }, CaseListResponse>
  search_cases: Cmd<{ query: string }, BusinessCase[]>
  case_stats: Cmd<Record<string, unknown>, CaseStats>
  get_dashboard_stats: Cmd<Record<string, unknown>, DashboardStats>
  update_case_status: Cmd<Record<string, unknown>, BusinessCase>
  export_cases: Cmd<{ format: string; filter?: Partial<CaseFilter> }, string>
  // B1 类型化：create_case/update_case 参数因「缺键跳过 vs null 清除」三态语义复杂，保持 Record
  create_case: Cmd<{ data: Record<string, unknown> }, BusinessCase>
  update_case: Cmd<{ id: string; data: Record<string, unknown> }, BusinessCase>
  // B1 类型化：新增类型化命令
  list_field_groups: Cmd<{ caseType?: string }, FieldGroup[]>
  get_case_unified_view: Cmd<{ filters?: Record<string, unknown> }, CaseUnifiedView[]>
  get_today_stats: Cmd<Record<string, unknown>, TodayStats>
  get_case_type_metrics: Cmd<{ caseId: string }, CaseTypeMetrics>
  get_all_case_type_metrics: Cmd<Record<string, unknown>, CaseTypeMetrics[]>

  // ── 关系 / 时间线 ──
  add_relation: Cmd<{ caseId: string; targetId: string; relationType: string; merge_data?: Record<string, unknown> }, CaseRelation>
  detect_relations: Cmd<{ caseId: string }, CaseRelation[]>
  get_relations: Cmd<{ caseId: string }, RelatedCase[]>
  get_case_timeline: Cmd<{ caseId: string }, TimelineEvent[]>
  remove_relation: Cmd<{ id: string }, void>

  // ── 文件 ──
  list_case_files: Cmd<{ caseId: string }, CaseFile[]>

  // ── 仪表盘（B4 数据可视化）──
  get_project_status_distribution: Cmd<Record<string, unknown>, NameCount[]>
  get_track_distribution: Cmd<Record<string, unknown>, NameCount[]>
  get_monthly_task_trend: Cmd<{ months?: number }, MonthTrendPoint[]>
  get_upcoming_hearings: Cmd<{ days?: number }, UpcomingHearing[]>
  get_today_kpis: Cmd<Record<string, unknown>, TodayKpis>
  seed_demo_data: Cmd<Record<string, unknown>, { cases: number; projectsPersonal: number; tasks: number; areas: number; knowledge: number; hearings: number }>
  add_case_file: Cmd<{ caseId: string; fileName: string; filePath: string; category: string }, CaseFile>

  // ── 提醒域 ──
  list_reminder_rules: Cmd<Record<string, unknown>, ReminderRule[]>
  get_reminder_log: Cmd<{ limit?: number }, ReminderLogEntry[]>
  get_deadline_warnings: Cmd<Record<string, unknown>, DeadlineResult[]>
  get_deadline_warnings_with_levels: Cmd<Record<string, unknown>, DeadlineWarning[]>
  test_reminder: Cmd<{ ruleId: string; channel: string; message: string }, ReminderLogEntry>

  // ── 备份域 ──
  create_backup: Cmd<{}, import('./bindings').BackupFile>
  list_backups: Cmd<{}, import('./bindings').BackupFile[]>
  restore_backup: Cmd<{ filename: string }, boolean>

  // ── 同步域 ──
  get_sync_status: Cmd<Record<string, unknown>, SyncStatus>
  webdav_push: Cmd<Record<string, unknown>, SyncResult>
  webdav_pull: Cmd<Record<string, unknown>, SyncResult>
  webdav_startup_sync: Cmd<Record<string, unknown>, SyncResult>
  webdav_resolve_keep_local: Cmd<{ url: string; username: string; password: string }, SyncResult>
  webdav_resolve_keep_remote: Cmd<{ url: string; username: string; password: string }, SyncResult>
  sync_feishu_pull: Cmd<{ appToken: string; tableId: string }, FeishuSyncReport>
  sync_feishu_push: Cmd<{ appToken: string; tableId: string }, FeishuSyncReport>
  get_folder_template: Cmd<{ templateId: string }, FolderTemplateOutput>
  list_folder_templates: Cmd<Record<string, unknown>, FolderTemplateOutput[]>
  get_folder_naming_settings: Cmd<Record<string, unknown>, FolderNamingSettingsOutput>

  feishu_import_all: Cmd<{
    appToken: string
    tableId: string
    localTable: string
    mappings: MappingEntry[]
  }, ImportResult>
  feishu_compare_table: Cmd<{ appToken: string; tableId: string; localTable: unknown }, SchemaDiff>
  feishu_compare_records: Cmd<{ appToken: string; tableId: string; localTable: unknown; matchField: string }, RecordDiff>
  feishu_import_incremental: Cmd<{ appToken: string; tableId: string; localTable: string; mappingsJson: any }, any>
  feishu_sync_pull: Cmd<{ appToken: string; tableId: string; localTable: string; mappingsJson: any }, any>
  feishu_sync_push: Cmd<{ appToken: string; tableId: string; localTable: string; mappingsJson: any }, any>
  sync_export_persons_to_vcard: Cmd<{}, string>
  sync_export_whiteboards_to_blob: Cmd<{}, string>

  import_feishu_data: Cmd<{ jsonPath: string }, ImportReport>
  sync_reminders_to_calendar: Cmd<Record<string, unknown>, CalendarSyncReport>

  // ── AI 域 ──
  ai_chat: Cmd<{
    messages: Array<Pick<ChatMessage, 'role' | 'content'>>
    mode?: string
    apiUrl?: string
    model?: string
    purpose?: string
    contextRefs?: ContextRef[]
  }, AiChatResult>
  get_ai_config: Cmd<Record<string, unknown>, AiConfig>
  get_command_route_info: Cmd<Record<string, unknown>, CommandRoute>
  quick_judge_inbox_item: Cmd<{ id: string }, QuickJudgeResult>
  list_mcp_pending_writes: Cmd<Record<string, unknown>, McpPendingWrite[]>

  // ── 任务域 ──
  list_tasks: Cmd<{ filter?: Partial<TaskFilter> }, TaskDto[]>
  search_tasks: Cmd<{ query: string }, SearchTaskDto[]>
  // P1-6：撤销删除专用还原命令（保留原 id / completed；snapshot 为前端 Task 快照）
  restore_task: Cmd<{ snapshot: Task }, TaskDto>
  // 任务写命令（B1 类型化）：data 参数当前按 Record 收口，后续逐步收紧为强类型输入。
  // create_task 后端实际仅返回 { id }（非完整 Task），勿误标为 Task；写类命令返回 null。
  create_task: Cmd<{ data: Record<string, unknown> }, { id: string }>
  update_task: Cmd<{ data: Record<string, unknown> }, null>
  toggle_task: Cmd<{ id: string; actualMinutes?: number | null; origin?: string | null; proposalToken?: string | null }, null>
  delete_task: Cmd<{ id: string; origin?: string | null; proposalToken?: string | null }, null>


  search_knowledge: Cmd<{ query: string }, SearchKnowledgeDto[]>
  global_search: Cmd<{ query: string }, any[]>
  list_knowledge_blocks: Cmd<{ parentId: string }, KnowledgeBlockDto[]>
  get_knowledge_with_blocks: Cmd<{ id: string }, KnowledgeWithBlocksDto>
  knowledge_stats: Cmd<Record<string, unknown>, KnowledgeStatsDto>
  list_knowledge_versions: Cmd<{ itemId: string }, KnowledgeVersionDto[]>
  diff_knowledge_versions: Cmd<{ versionId1: string; versionId2: string }, KnowledgeDiffVersionsResult>
  diff_knowledge_with_current: Cmd<{ versionId: string; itemId: string }, KnowledgeDiffCurrentResult>
  get_knowledge_graph: Cmd<{ limit?: number }, KnowledgeGraphDto>
  list_knowledge_document_sources: Cmd<Record<string, unknown>, KnowledgeDocumentSourceDto[]>
  import_pageindex_to_knowledge: Cmd<{ fileId: string }, PageIndexImportResultDto>
  restore_knowledge_version: Cmd<{ itemId: string; versionId: string }, void>
  export_knowledge_markdown: Cmd<
    { itemId: string; outputPath: string },
    { outputPath: string; fileSize: number; exportedAt: string }
  >

  // ── 知识域 ──
  create_knowledge: Cmd<{ data: CreateKnowledgeInput }, string>

  // ── 领域域（DomainCommand 样板）──
  list_areas: Cmd<Record<string, unknown>, AreaDto[]>
  get_area: Cmd<{ id: string }, AreaDto>
  create_area: Cmd<{ data: CreateAreaInput }, CreateAreaOutput>
  update_area: Cmd<{ id: string; data: UpdateAreaInput }, void>
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
  export_edited_docx: Cmd<{
    document: Record<string, unknown>
    title: string
    outputPath?: string | null
  }, ExportResponse>

  // ── 知识检索 ──
  hybrid_search_knowledge: Cmd<{ query: string; limit?: number }, SearchResult>

  // ── 草稿 ──
  list_drafts: Cmd<Record<string, unknown>, Draft[]>
  get_draft: Cmd<{ id: string }, Draft>
  create_draft: Cmd<
    { title: string; content?: string | null; caseId?: string | null; templatePath?: string | null },
    Draft
  >
  update_draft: Cmd<
    { id: string; title?: string; content?: string | null; status?: string; caseId?: string | null },
    Draft
  >
  delete_draft: Cmd<{ id: string }, boolean>

  // ── 对标灵感落地（2026-09-01 · Schema v20）──
  // W2 通知中心（Linear 式 Inbox-Zero）
  list_notifications: Cmd<Record<string, unknown>, AppNotification[]>
  unread_notification_count: Cmd<Record<string, unknown>, number>
  create_notification: Cmd<{ kind: string; title: string; body?: string | null; payloadJson?: string | null }, string>
  mark_notification_read: Cmd<{ id: string }, void>
  dismiss_notification: Cmd<{ id: string }, void>
  dismiss_all_notifications: Cmd<Record<string, unknown>, number>
  // W2 Defer Date（OmniFocus 式推迟日）
  defer_task: Cmd<{ taskId: string; until: string }, void>
  clear_task_defer: Cmd<{ taskId: string }, void>
  // W3 期限规则（LawToolBox 式可审计）
  list_deadline_rules: Cmd<Record<string, unknown>, DeadlineRuleDto[]>
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
  list_smart_rules: Cmd<Record<string, unknown>, SmartRuleDto[]>
  upsert_smart_rule: Cmd<{ id?: string | null; name: string; enabled: boolean; matchField: string; matchPattern: string; actionType: string; actionPayload: string }, string>
  delete_smart_rule: Cmd<{ id: string }, void>
  apply_smart_rules: Cmd<{ fileId: string }, SmartRuleApplyResult>
  run_smart_rules_for_all: Cmd<Record<string, unknown>, number>
  list_pending_ocr_files: Cmd<Record<string, unknown>, [string, string, string][]>
  ocr_case_file: Cmd<{ fileId: string }, string>
  ocr_all_pending: Cmd<Record<string, unknown>, number>
  list_case_ocr_states: Cmd<{ caseId: string }, FileOcrStateDto[]>
  get_file_ocr_text: Cmd<{ fileId: string }, string | null>
  // W6 对象化实体（Capacities 式）
  list_persons: Cmd<{ kind?: string | null; keyword?: string | null }, PersonDto[]>
  upsert_person: Cmd<{ id?: string | null; kind: string; name: string; org?: string | null; phone?: string | null; email?: string | null; preferences?: string | null; notes?: string | null }, string>
  delete_person: Cmd<{ id: string }, void>
  attach_person_to_case: Cmd<{ caseId: string; personId: string; role?: string | null }, string>
  detach_person_from_case: Cmd<{ linkId: string }, void>
  list_case_persons: Cmd<{ caseId: string }, CasePersonDto[]>
  list_person_cases: Cmd<{ personId: string }, PersonCaseDto[]>
  // W7 事实白板（LiquidText 式）
  list_whiteboards: Cmd<{ caseId: string }, WhiteboardDto[]>
  create_whiteboard: Cmd<{ caseId: string; name: string }, string>
  rename_whiteboard: Cmd<{ id: string; name: string }, void>
  delete_whiteboard: Cmd<{ id: string }, void>
  list_fact_nodes: Cmd<{ whiteboardId: string }, FactNodeDto[]>
  create_fact_node: Cmd<{ whiteboardId: string; fileId: string | null; page: number | null; excerpt: string; note: string | null; x: number; y: number }, string>
  update_fact_node: Cmd<{ id: string; page?: number | null; excerpt?: string; note?: string | null; x?: number; y?: number }, void>
  delete_fact_node: Cmd<{ id: string }, void>
  list_whiteboard_edges: Cmd<{ whiteboardId: string }, import('./bindings').WhiteboardEdgeDto[]>
  create_whiteboard_edge: Cmd<{ whiteboardId: string; sourceNodeId: string; targetNodeId: string }, string>
  delete_whiteboard_edge: Cmd<{ id: string }, void>

  // ==========================================W1 AI Proposal 预览（Cursor 式 Diff 确认）
  get_proposal_preview: Cmd<{ proposalId: string }, import('../core/ai/proposals').ProposalPreviewDto>
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
  // 多为复杂/尚未收口的契约：params 按调用现场尽量核定，result 以 any/unknown
  // 收口避免漏项（仍能为已登记调用提供参数检查），随后续收口逐步收紧。
  reasoning_search: Cmd<{ query: string; scope: string[] }, unknown>
  get_document_engine_status: Cmd<Record<string, unknown>, unknown>
  queue_document_processing: Cmd<{ fileId: string }, unknown>
  list_document_jobs: Cmd<{ fileId: string }, unknown>
  retry_document_job: Cmd<{ jobId: string }, unknown>
  list_case_hearings: Cmd<{ caseId: string }, any>
  create_case_hearing: Cmd<{ payload: Record<string, unknown> }, any>
  update_case_hearing: Cmd<{ id: string; payload: Record<string, unknown> }, void>
  delete_case_hearing: Cmd<{ id: string }, void>
  excel_get_sheets: Cmd<{ filePath: string }, any>
  excel_inspect_sheet: Cmd<{ filePath: string; sheetName: string; headerRowOverride?: number | null }, any>
  feishu_check_config: Cmd<Record<string, unknown>, any>
  feishu_inspect_bitable: Cmd<{ urlOrToken: string; tableIdOverride?: string | null }, any>
  get_ai_usage: Cmd<Record<string, unknown>, any>
  trigger_feishu_push: Cmd<Record<string, unknown>, unknown>
  start_clipboard_monitor: Cmd<Record<string, unknown>, any>
}
