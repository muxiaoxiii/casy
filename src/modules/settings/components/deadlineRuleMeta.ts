/**
 * 法定期限规则（W3）共享类型与展示元数据
 * 供 DeadlineRulesSettings.vue / DeadlineRuleAuditDrawer.vue 使用
 * 与 src-tauri/src/commands/deadline_rules.rs 的 DTO（camelCase）对应
 */

export interface DeadlineRuleDto {
  id: string
  track: string
  ruleName: string
  legalBasis: string
  triggerField: string
  offsetValue: number
  offsetUnit: string
  calcMethod: string
  procedureTypes: string | null
  deadlineSource: string
  autoCalculate: boolean
  priority: number
}

export interface DeadlineRuleAuditDto {
  id: string
  ruleId: string
  action: string
  beforeJson: string | null
  afterJson: string | null
  actor: string
  createdAt: string | null
}

/** 轨道展示（与 cases 模块既有取值一致） */
export const TRACK_LABELS: Record<string, string> = {
  patent_invalidation: '专利无效',
  admin_litigation: '行政诉讼',
  civil_tort: '民事侵权',
  other: '其他',
}

export const TRACK_TAG_TYPES: Record<string, 'primary' | 'warning' | 'success' | 'info'> = {
  patent_invalidation: 'primary',
  admin_litigation: 'warning',
  civil_tort: 'success',
}

export function trackLabel(track: string): string {
  return TRACK_LABELS[track] || track
}

/**
 * 偏移单位（值与 deadline_rules 表 CHECK 约束 / engine.rs 一致：
 * 'day' | 'calendar_month'；calendar_day/workday 仅为兼容展示）
 */
export const OFFSET_UNIT_OPTIONS = [
  { value: 'day', label: '自然日' },
  { value: 'calendar_month', label: '自然月' },
]

export const OFFSET_UNIT_LABELS: Record<string, string> = {
  day: '自然日',
  calendar_month: '自然月',
  calendar_day: '自然日',
  workday: '工作日',
}

const OFFSET_UNIT_SHORT: Record<string, string> = {
  day: '天',
  calendar_day: '天',
  workday: '个工作日',
  calendar_month: '个月',
}

export function formatOffset(rule: Pick<DeadlineRuleDto, 'offsetValue' | 'offsetUnit'>): string {
  const sign = rule.offsetValue > 0 ? '+' : ''
  const unit = OFFSET_UNIT_SHORT[rule.offsetUnit] || rule.offsetUnit
  return `${sign}${rule.offsetValue} ${unit}`
}

/** 计算算法（engine.rs：patent 走专利细则算法，其余默认诉讼算法） */
export const CALC_METHOD_OPTIONS = [
  { value: 'civil', label: '诉讼法算法（届满顺延至工作日）' },
  { value: 'patent', label: '专利实施细则算法' },
]

export const CALC_METHOD_LABELS: Record<string, string> = {
  civil: '诉讼法算法',
  patent: '专利细则算法',
}

/** 期限来源（CHECK: statutory | recommended） */
export const DEADLINE_SOURCE_OPTIONS = [
  { value: 'statutory', label: '法定期限' },
  { value: 'recommended', label: '建议期限' },
]

export const DEADLINE_SOURCE_LABELS: Record<string, string> = {
  statutory: '法定',
  recommended: '建议',
}

/** 触发字段（engine.rs get_case_date_field 支持的取值） */
export const TRIGGER_FIELD_OPTIONS = [
  { value: 'filing_date', label: '立案日期' },
  { value: 'complaint_received_date', label: '收到起诉状日期' },
  { value: 'trial_date', label: '开庭/口审日期' },
  { value: 'trial2_date', label: '二次开庭日期' },
  { value: 'trial3_date', label: '三次开庭日期' },
  { value: 'verdict_date', label: '收到判决/裁定日期' },
  { value: 'stay_date', label: '中止审理日期' },
  { value: 'relief_deadline', label: '权利救济期限' },
  { value: 'petitioner_first_invalid', label: '请求人首次提出无效日期' },
  { value: 'petitioner_submit_date', label: '请求人提交意见日期' },
  { value: 'petitioner_received_date', label: '请求人收到通知日期' },
  { value: 'patentee_received_date', label: '专利权人收到通知日期' },
  { value: 'patentee_received_supp_date', label: '专利权人收到补充意见日期' },
]

export const TRIGGER_FIELD_LABELS: Record<string, string> = Object.fromEntries(
  TRIGGER_FIELD_OPTIONS.map((o) => [o.value, o.label]),
)

export function triggerFieldLabel(field: string): string {
  return TRIGGER_FIELD_LABELS[field] || field
}

/** 适用程序展示：JSON 数组 → 顿号连接；对象/原文 → 原样摘要 */
export function formatProcedureTypes(raw: string | null | undefined): string {
  if (!raw) return '全部适用'
  try {
    const parsed: unknown = JSON.parse(raw)
    if (Array.isArray(parsed)) return parsed.map(String).join('、') || '全部适用'
    if (parsed && typeof parsed === 'object') {
      return Object.entries(parsed as Record<string, unknown>)
        .map(([k, v]) => `${k}=${String(v)}`)
        .join('、')
    }
    return String(parsed)
  } catch {
    return raw
  }
}

/** 留痕动作徽标 */
export const AUDIT_ACTION_META: Record<string, { label: string; type: 'success' | 'warning' | 'info' | 'danger' }> = {
  create: { label: '新建', type: 'success' },
  update: { label: '更新', type: 'warning' },
  toggle: { label: '启停', type: 'info' },
  delete: { label: '删除', type: 'danger' },
}

/** 留痕字段级对比用的字段元数据（键为 beforeJson/afterJson 的 camelCase 键） */
export const AUDIT_FIELD_META: Array<{
  key: string
  label: string
  format?: (v: unknown) => string
}> = [
  { key: 'ruleName', label: '规则名称' },
  { key: 'track', label: '所属轨道', format: (v) => trackLabel(String(v ?? '')) },
  { key: 'legalBasis', label: '法条依据' },
  { key: 'triggerField', label: '触发字段', format: (v) => triggerFieldLabel(String(v ?? '')) },
  { key: 'offsetValue', label: '偏移值', format: (v) => String(v ?? '') },
  { key: 'offsetUnit', label: '偏移单位', format: (v) => OFFSET_UNIT_LABELS[String(v ?? '')] || String(v ?? '') },
  { key: 'calcMethod', label: '计算算法', format: (v) => CALC_METHOD_LABELS[String(v ?? '')] || String(v ?? '') },
  { key: 'procedureTypes', label: '适用程序', format: (v) => formatProcedureTypes(v as string | null) },
  { key: 'deadlineSource', label: '期限来源', format: (v) => DEADLINE_SOURCE_LABELS[String(v ?? '')] || String(v ?? '') },
  { key: 'autoCalculate', label: '自动计算', format: (v) => (v ? '启用' : '停用') },
  { key: 'priority', label: '优先级', format: (v) => String(v ?? '') },
]

export function safeParseJson(raw: string | null | undefined): Record<string, unknown> | null {
  if (!raw) return null
  try {
    const parsed: unknown = JSON.parse(raw)
    return parsed && typeof parsed === 'object' && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : null
  } catch {
    return null
  }
}
