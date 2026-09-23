import type {
  CaseLevel,
  CaseRoute,
  CivilStatus,
  InvalidationStatus,
  AdminStatus,
  ProcedureType,
  TaskPriority,
  StartBucket,
  TrackType,
  VerdictType,
} from './index'

export type IpcJsonScalar = string | number | boolean | null
export type IpcJsonObject = { [key: string]: IpcJsonScalar | IpcJsonObject | IpcJsonScalar[] | IpcJsonObject[] }

export type IpcJsonValue = IpcJsonScalar | IpcJsonObject | IpcJsonScalar[] | IpcJsonObject[]

export interface HolidayCalendarEntry {
  source?: 'official' | 'personal'
  date: string
  name: string
  kind: 'holiday' | 'workday' | string
}

export interface HolidayCalendarPayload {
  year: number
  entries: HolidayCalendarEntry[]
}

export interface FeishuBitableTableInfo {
  tableId: string
  name: string
  revision: number | null
}

export interface FeishuBitableFieldInfo {
  fieldId: string
  fieldName: string
  type: number
  ui_type?: string | null
  isPrimary?: boolean | null
  property?: IpcJsonValue
  description?: IpcJsonValue
}

export interface NormalizedFeishuBitableFieldInfo extends FeishuBitableFieldInfo {
  fieldType: number
}

export interface FeishuSyncInfo {
  configured: boolean
  appId: string | null
  lastPullAt: string | null
  lastPushAt: string | null
  lastPullCount: string | null
  lastPushCount: string | null
  appToken: string | null
  tableId: string | null
}

export interface CalendarSyncStatus {
  enabled: boolean
  configured: boolean
  syncedCount: number
  pendingCount: number
  failedCount: number
  lastSyncAt: string | null
}

export interface SmartSummaryRow {
  id: string
  title: string
  content: string | null
  structuredData: string | null
  periodStart: string | null
  periodEnd: string | null
  createdAt: string | null
  narrativeSource: string | null
}

export interface RecommendationItem {
  taskId: string
  taskName: string
  caseId: string | null
  caseName: string | null
  reason: string
  score: number
  priority: string
  dueDate: string | null
  estimatedMinutes: number | null
  context: string | null
}

export interface FollowupSuggestion {
  taskId: string
  taskName: string
  waitingFor: string | null
  waitingDays: number
  reason: string
  action: string
}

export interface TodayRecommendations {
  recommendations: RecommendationItem[]
  followupSuggestions: FollowupSuggestion[]
  source: string
  generatedAt: string
}

export interface EmailMonitorAccountStatus {
  id: string
  email: string
  server: string
  enabled: boolean
}

export interface EmailMonitorStatus {
  running: boolean
  accountCount: number
  accounts: EmailMonitorAccountStatus[]
}

export interface KeychainAccountStatus {
  id: string
  email: string
  hasLegacyPassword: boolean
  hasKeychainPassword: boolean
}

export interface KeychainStatus {
  keychainAvailable: boolean
  accounts: KeychainAccountStatus[]
}

export interface CasePatchInput {
  thirdParties?: string | null
  caseAmount?: string | null
  legalFees?: string | null
  feePayment?: string | null
  claims?: string | null
  jurisdictionObjection?: string | null
  externalCaseNo?: string | null
  defenseDeadline?: string | null
  estimatedTrialEnd?: string | null
  relatedCases?: Array<{ caseId: string; relationType: string; label?: string }>
  hearings?: IpcJsonObject[]
  logs?: IpcJsonObject[]
  tasks?: IpcJsonObject[]
  officials?: IpcJsonObject[]
  track?: TrackType | string | null
  caseName?: string | null
  caseNo?: string | null
  internalNo?: string | null
  causeAction?: string | null
  clientName?: string | null
  ourRole?: string | null
  opponentName?: string | null
  opponentRole?: string | null
  opponentFirm?: string | null
  opponentAgent?: string | null
  court?: string | null
  judgePanel?: string | null
  clerk?: string | null
  attorneys?: string | string[] | null
  caseLevel?: CaseLevel | string | null
  caseProgress?: string | null
  caseResult?: string | null
  caseGoal?: string | null
  patentName?: string | null
  patentAppNo?: string | null
  patentNo?: string | null
  procedureType?: ProcedureType | string | null
  filingDate?: string | null
  complaintReceivedDate?: string | null
  trialDate?: string | null
  trial2Date?: string | null
  trial3Date?: string | null
  verdictType?: VerdictType | string | null
  verdictDate?: string | null
  stayDate?: string | null
  reliefDeadline?: string | null
  notes?: string | null
  completedText?: string | null
  petitionerFirstInvalid?: string | null
  petitionerSuppDeadline?: string | null
  petitionerSubmitDate?: string | null
  petitionerReceivedDate?: string | null
  petitionerReplyDeadline?: string | null
  patenteeReceivedDate?: string | null
  patenteeStatementDeadline?: string | null
  patenteeReceivedSuppDate?: string | null
  patenteeSuppDeadline?: string | null
  patenteeSubmitSuppDate?: string | null
  folderTemplateId?: string | null
  caseRoute?: CaseRoute | string | null
  civilStatus?: CivilStatus | string | null
  invalidationStatus?: InvalidationStatus | string | null
  adminStatus?: AdminStatus | string | null
  invalidationDecisionDate?: string | null
  invalidationDecisionType?: string | null
  adminFilingDate?: string | null
  adminVerdictDate?: string | null
  adminTrial2Date?: string | null
  origin?: string | null
  proposalToken?: string | null
}

export type CreateCasePayload = CasePatchInput & {
  caseName: string
  clientName: string
}

export interface TaskMutationInput {
  id?: string
  origin?: string | null
  proposalToken?: string | null
  taskName?: string | null
  description?: string | null
  createdDate?: string | null
  deadline?: string | null
  priority?: TaskPriority | string | null
  completed?: number | boolean | null
  assignee?: string | null
  finishNote?: string | null
  taskType?: string | null
  startDate?: string | null
  dueDate?: string | null
  dueTime?: string | null
  waitingFor?: string | null
  followUpDate?: string | null
  context?: string | null
  flagged?: number | boolean | null
  sequential?: number | boolean | null
  blocked?: number | boolean | null
  blockedReason?: string | null
  sequenceOrder?: number | null
  startBucket?: StartBucket | string | null
  todayIndex?: number | null
  estimatedMinutes?: number | null
  actualMinutes?: number | null
  areaId?: string | null
  caseId?: string | null
  knowledgeId?: string | null
  timeBlock?: string | null
  parentId?: string | null
  parentTaskId?: string | null
  recurrenceRule?: string | null
  isFocus?: number | boolean | null
  deferUntil?: string | null
  nextReviewDate?: string | null
  lastReviewDate?: string | null
  inboxSourceId?: string | null
}

export type CreateTaskPayload = Omit<TaskMutationInput, 'id'>

export type UpdateTaskPayload = TaskMutationInput & {
  id: string
}

export interface CalendarEventInput {
  title: string
  eventDate: string
  startTime?: string | null
  endTime?: string | null
  allDay?: boolean | number | null
  color?: string | null
  location?: string | null
  notes?: string | null
  caseId?: string | null
  taskId?: string | null
  eventType?: string | null
  reminderDate?: string | null
}

export interface KnowledgePatchInput {
  expectedContent?: string
  title?: string | null
  category?: string | null
  content?: string | null
  tags?: string | null
  lawName?: string | null
  articleNo?: string | null
  effectiveDate?: string | null
  status?: string | null
  linkedCaseId?: string | null
  parentId?: string | null
}

export interface ReminderRuleInput {
  id?: string | null
  name?: string | null
  triggerType?: string | null
  triggerValue?: number | null
  channels?: string[] | string | null
  messageTemplate?: string | null
  caseTypes?: string | null
  enabled?: number | boolean | null
}

export interface FeishuMappingPayload {
  id?: string | null
  connectionId?: string | null
  feishuTableId?: string | null
  feishuFieldId?: string | null
  feishuFieldName?: string | null
  feishuFieldType?: number | null
  localTable?: string | null
  localColumn?: string | null
  transformRule?: string | null
  syncDirection?: string | null
  isFormula?: number | boolean | null
  isLink?: number | boolean | null
  isLookup?: number | boolean | null
}
