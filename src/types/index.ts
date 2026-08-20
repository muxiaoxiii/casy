// ============================================================
// Casy 共享类型定义
// ============================================================
// 所有实体类型与 Rust 端 serde 序列化对齐（camelCase）
// 前端 stores、tauriBridge、组件统一从此文件引用类型
// ============================================================

// ==================== 状态机枚举 ====================

/** 民事诉讼轨状态（14 档） */
export type CivilStatus =
  | 'intake'           // 接案
  | 'filed'            // 立案受理
  | 'pre_hearing'      // 待开庭
  | 'in_trial'         // 审理中
  | 'settled'          // 已调解
  | 'awaiting_verdict' // 待判决
  | 'verdict_issued'   // 判决已出
  | 'appeal_period'    // 上诉期
  | 'second_instance'  // 二审中
  | 'second_verdict'   // 二审判决已出
  | 'retrial'          // 再审
  | 'enforcement'      // 执行中
  | 'suspended'        // 中止
  | 'closed'           // 已结案

/** 专利无效轨状态（6 档） */
export type InvalidationStatus =
  | 'preparing'          // 待提无效
  | 'filed'              // 无效已受理
  | 'pre_oral'           // 待口审
  | 'oral_done'          // 口审完成
  | 'awaiting_decision'  // 待无效决定
  | 'decision_issued'    // 决定已出

/** 行政诉讼轨状态（7 档） */
export type AdminStatus =
  | 'filed'              // 行政诉讼立案
  | 'pre_hearing'        // 行政诉讼待开庭
  | 'in_trial'           // 行政诉讼审理中
  | 'awaiting_verdict'   // 行政诉讼待判决
  | 'verdict_issued'     // 行政诉讼判决已出
  | 'second_instance'    // 行政诉讼二审中
  | 'closed'             // 行政诉讼已结案

/** 轨道路由 */
export type CaseRoute =
  | '民事诉讼'
  | '专利无效'
  | '行政诉讼'
  | '民事诉讼+专利无效'
  | '专利无效+行政诉讼'
  | '三轨并行'

/** 聚合状态（向后兼容） */
export type CaseStatus = '已完结' | '进行中' | '未知'

/** 案件轨道类型（旧字段，迁移期间保留） */
export type TrackType =
  | 'patent_invalidation'  // 专利无效
  | 'admin_litigation'     // 行政诉讼
  | 'civil_tort'           // 民事侵权
  | 'other'                // 其他

/** 审级 */
export type CaseLevel = '一审' | '二审' | '再审' | '结案'

/** 程序类型 */
export type ProcedureType = '普通' | '简易'

/** 判决类型（影响上诉期天数） */
export type VerdictType = '判决' | '裁定'

/** 轨道名称（用于 history 表） */
export type TrackName = '民事诉讼' | '专利无效' | '行政诉讼'

/** 状态变更来源 */
export type StatusChangeSource = 'manual' | 'auto' | 'ai'

// ==================== 案件实体 ====================

/** 案件主实体 */
export interface Case {
  // 基本信息
  id: string
  caseName: string
  caseNo: string
  track: TrackType           // 旧字段，迁移期间保留
  causeAction: string        // 案由

  // 当事人
  clientName: string
  ourRole: string
  opponentName: string
  opponentRole: string
  opponentFirm: string
  opponentAgent: string

  // 审理
  court: string
  judgePanel: string
  clerk: string
  attorneys: string[]        // JSON array
  caseLevel: CaseLevel | null
  caseStatus: CaseStatus     // 聚合状态（向后兼容）
  caseProgress: string
  caseResult: string

  // 双轨状态机（新增）
  caseRoute: CaseRoute
  civilStatus: CivilStatus | null
  invalidationStatus: InvalidationStatus | null
  adminStatus: AdminStatus | null

  // 专利
  patentName: string
  patentAppNo: string
  procedureType: ProcedureType | null

  // 日期里程碑
  filingDate: string
  complaintReceivedDate: string
  trialDate: string
  trial2Date: string
  trial3Date: string
  verdictType: VerdictType | null
  verdictDate: string
  stayDate: string
  reliefDeadline: string

  // 专利无效专属
  petitionerFirstInvalid: string
  petitionerSuppDeadline: string
  petitionerSubmitDate: string
  petitionerReceivedDate: string
  petitionerReplyDeadline: string
  patenteeReceivedDate: string
  patenteeStatementDeadline: string
  patenteeReceivedSuppDate: string
  patenteeSuppDeadline: string
  patenteeSubmitSuppDate: string

  // 无效程序新增
  invalidationDecisionDate: string
  invalidationDecisionType: string  // 全部无效/部分无效/维持有效

  // 行政诉讼新增
  adminFilingDate: string
  adminVerdictDate: string
  adminTrial2Date: string

  // 文件夹
  folderPath: string

  // 文书
  lastDocPath: string
  lastDocAt: string

  // 进度
  completedText: string
  notes: string

  // 时间戳
  createdAt: string
  updatedAt: string
}

/** 创建案件的输入（必填字段） */
export interface CreateCaseInput {
  caseName: string
  clientName: string
  opponentName?: string
  track?: TrackType
  causeAction?: string
  court?: string
  caseNo?: string
  caseRoute?: CaseRoute
}

/** 更新案件的输入（所有字段可选） */
export type UpdateCaseInput = Partial<Omit<Case, 'id' | 'createdAt' | 'updatedAt'>>

// ==================== 审级历程 ====================

/** 审级历程记录 */
export interface TrackHistoryEntry {
  id: string
  caseId: string
  track: TrackName
  fromStatus: string | null
  toStatus: string
  changedAt: string
  source: StatusChangeSource
  note: string | null
}

// ==================== 案件筛选 ====================

/** 案件筛选条件 */
export interface CaseFilter {
  track: TrackType | null
  client: string | null
  court: string | null
  status: CaseStatus | null
  search: string
  sortBy: string
  dateFrom: string | null
  dateTo: string | null
  // 新增筛选维度
  civilStatus: CivilStatus | null
  invalidationStatus: InvalidationStatus | null
  adminStatus: AdminStatus | null
  caseRoute: CaseRoute | null
}

/** 案件列表响应 */
export interface CaseListResponse {
  items: Case[]
  total: number
}

/** 案件统计 */
export interface CaseStats {
  total: number
  active: number
  closed: number
  byTrack: Array<{ track: string; count: number }>
  byClient: Array<{ client: string; count: number }>
}

// ==================== 收件箱 ====================

export type InboxStatus = 'pending' | 'processed' | 'filed' | 'archived' | 'ignored'

export interface InboxItem {
  id: string
  title: string
  contentText: string
  sourceType: string
  sourcePath: string
  aiCategory: string
  aiConfidence: number
  aiExtracted: unknown
  aiSuggestedCaseId: string | null
  status: InboxStatus
  linkedCaseId: string | null
  userCategory: string
  createdAt: string
  processedAt: string | null
}

// ==================== 任务 ====================

export type TaskPriority = 'urgent_important' | 'important' | 'urgent' | 'high' | 'normal' | 'low'
export type TaskStatus = 'pending' | 'in_progress' | 'done' | 'cancelled'
export type TaskType = 'action' | 'waiting' | 'delegated' | 'someday' | 'note'
export type Context = 'office' | 'phone' | 'court' | 'home' | 'anywhere' | 'computer' | 'outside'
export type StartBucket = 'inbox' | 'anytime' | 'someday' | 'today'

/** 任务实体（GTD 字段 · 对齐后端 tasks.rs 返回） */
export interface Task {
  id: string
  taskName: string
  description: string | null
  createdDate: string
  deadline: string | null          // 兼容旧字段 = dueDate
  priority: TaskPriority
  completed: number                // 0/1
  assignee: string | null
  finishNote: string | null
  // GTD 字段
  taskType: TaskType
  startDate: string | null         // When
  dueDate: string | null           // Deadline
  dueTime: string | null           // 具体时间点 HH:MM（设计哲学 §7 时间分配）
  waitingFor: string | null        // 等谁
  followUpDate: string | null      // 跟进日期
  context: Context | null          // @办公室 等
  flagged: number                  // 旗标
  sequential: number               // 顺序项目
  blocked: number                  // 0=解锁 1=锁定
  sequenceOrder: number
  startBucket: StartBucket         // 时间桶
  todayIndex: number
  estimatedMinutes: number | null
  actualMinutes: number | null
  isOverdue: number                // 缓存标志
  dueSoon: number                  // 缓存标志
  lastReviewDate: string | null
  nextReviewDate: string | null    // 回顾周期
  areaId: string | null
  knowledgeId: string | null
  caseId: string | null
}

/** 任务过滤条件 */
export interface TaskFilter {
  caseId?: string | null
  areaId?: string | null
  taskType?: string | null
  startBucket?: string | null
  completed?: number | null
}

// ==================== 时间线 ====================

export interface TimelineEvent {
  id: string
  caseId: string
  eventSummary: string
  eventType: string
  eventDate: string
  content: string
  files: string[] | null
  createdAt: string
}

// ==================== 知识库 ====================

export interface KnowledgeItem {
  id: string
  title: string
  content: string
  category: string
  tags: string[]
  sourcePath: string
  linkedCaseId: string | null
  linkedLawId: string | null
  version: number
  createdAt: string
  updatedAt: string
}

// ==================== 日历 ====================

export interface CalendarEvent {
  id: string
  title: string
  date: string
  type: 'hearing' | 'deadline' | 'task' | 'other'
  caseId: string | null
  color: string
}

// ==================== Dashboard ====================

export interface DashboardStats {
  activeCount: number
  totalCount: number
  closedCount: number
  deadlineWarnings: Array<{
    caseId: string
    caseName: string
    deadline: string
    daysLeft: number
  }>
  recentActivities: TimelineEvent[]
  byTrack: Array<{ track: string; count: number }>
}

// ==================== Tauri Bridge ====================

/** Tauri 调用安全返回值 */
export interface TauriResult<T = unknown> {
  ok: boolean
  data?: T
  error?: string
}

/** Tauri 调用选项 */
export interface TauriCallOptions {
  silent?: boolean
  errorMessage?: string
}

// ==================== 同步 ====================

export type SyncStatus = 'synced' | 'local_newer' | 'remote_newer' | 'conflict' | 'push_failed'

export interface SyncMapEntry {
  id: string
  localId: string
  remoteId: string
  tableName: string
  syncStatus: SyncStatus
  lastSyncAt: string
}

// ==================== 设置 ====================

export interface AiConfig {
  mode: 'ollama' | 'openai' | 'noop'
  endpoint: string
  model: string
  apiKey: string
}

export interface WebDAVConfig {
  url: string
  username: string
  password: string
  enabled: boolean
}

export interface FeishuConfig {
  appId: string
  appSecret: string
  enabled: boolean
}

export interface ImapConfig {
  host: string
  port: number
  username: string
  password: string
  enabled: boolean
  whitelist: string[]
}

export interface AppSettings {
  ai: AiConfig
  webdav: WebDAVConfig
  feishu: FeishuConfig
  imap: ImapConfig
  general: {
    theme: 'light' | 'dark' | 'system'
    language: string
    caseFolderPath: string
    inboxFolderPath: string
  }
}

// ==================== 状态机显示映射 ====================

/** 民事诉讼轨状态中文映射 */
export const CIVIL_STATUS_LABELS: Record<CivilStatus, string> = {
  intake: '接案',
  filed: '立案受理',
  pre_hearing: '待开庭',
  in_trial: '审理中',
  settled: '已调解',
  awaiting_verdict: '待判决',
  verdict_issued: '判决已出',
  appeal_period: '上诉期',
  second_instance: '二审中',
  second_verdict: '二审判决已出',
  retrial: '再审',
  enforcement: '执行中',
  suspended: '中止',
  closed: '已结案',
}

/** 专利无效轨状态中文映射 */
export const INVALIDATION_STATUS_LABELS: Record<InvalidationStatus, string> = {
  preparing: '待提无效',
  filed: '无效已受理',
  pre_oral: '待口审',
  oral_done: '口审完成',
  awaiting_decision: '待无效决定',
  decision_issued: '决定已出',
}

/** 行政诉讼轨状态中文映射 */
export const ADMIN_STATUS_LABELS: Record<AdminStatus, string> = {
  filed: '行政诉讼立案',
  pre_hearing: '行政诉讼待开庭',
  in_trial: '行政诉讼审理中',
  awaiting_verdict: '行政诉讼待判决',
  verdict_issued: '行政诉讼判决已出',
  second_instance: '行政诉讼二审中',
  closed: '行政诉讼已结案',
}

/** 轨道路由中文映射 */
export const CASE_ROUTE_LABELS: Record<CaseRoute, string> = {
  '民事诉讼': '民事诉讼',
  '专利无效': '专利无效',
  '行政诉讼': '行政诉讼',
  '民事诉讼+专利无效': '诉讼+无效',
  '专利无效+行政诉讼': '无效+行政诉讼',
  '三轨并行': '三轨并行',
}
