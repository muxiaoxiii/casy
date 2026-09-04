// ============================================================
// bindings.Case (wire) ↔ 业务 Case (domain) 边界归一化
// ============================================================
// Rust 端 serde 序列化出的是「可空」的 bindings.Case（如 attorneys: string | null、
// 各日期/文本字段 string | null）；而前端业务类型（types/index.ts Case）把字段建模为
// 非空 string / string[] / 类型联合。本模块在 cases service 的边界把 wire 可空字段
// 归一化到业务 Case，并兼容 attorneys 的三种来源形态：
//   - JSON 字符串（["张律师","李律师"]）
//   - 裸分隔字符串（"张律师,李律师" / "张律师、李律师"）
//   - 数组（["张律师","李律师"]）
// 同时为消费点提供安全的 joinAttorneys，避免对非数组调用 .join 崩溃。
// ============================================================

import type {
  Case as BusinessCase,
  CaseStatus,
  CaseRoute,
  TrackType,
  CaseLevel,
  ProcedureType,
  VerdictType,
  CivilStatus,
  InvalidationStatus,
  AdminStatus,
} from '../types'

/** 空值 → 非空字符串（'' 兜底），保证业务 Case 的字符串字段恒为 string */
function str(v: unknown): string {
  if (typeof v === 'string') return v
  return v == null ? '' : String(v)
}

/** 空值 → null，保留业务 Case 的可空字符串字段（如 deadlineUrgency） */
function maybeString(v: unknown): string | null {
  if (v == null) return null
  if (typeof v === 'string') return v
  return String(v)
}

/** 类型联合字段：非空字符串原样透传（运行时可能为任意值），否则回退默认 */
function enumVal<T extends string>(v: unknown, fallback: T): T {
  if (typeof v === 'string' && v !== '') return v as T
  return fallback
}

/** 清洗单个人名 token：去首尾空白、去包裹的方括号、去包裹的引号 */
function cleanTokenRaw(s: string): string {
  let t = s.trim()
  // 成对/独立的包裹方括号（处理 "[张律师" / "李律师]" / "[张律师]" 这类残缺 JSON 提示）
  if (t.startsWith('[') && t.endsWith(']')) t = t.slice(1, -1).trim()
  else if (t.startsWith('[')) t = t.slice(1).trim()
  else if (t.endsWith(']')) t = t.slice(0, -1).trim()
  if ((t.startsWith('"') && t.endsWith('"')) || (t.startsWith("'") && t.endsWith("'"))) {
    t = t.slice(1, -1).trim()
  }
  return t
}

/**
 * 归一化 attorneys 为 string[]。
 *
 * 兼容输入：
 *  - null / undefined → []
 *  - 数组 → 过滤非字符串元素并 trim
 *  - JSON 数组字符串（'["张律师","李律师"]'）→ 解析为数组
 *  - 裸分隔字符串（中英文逗号、顿号、分号、换行）→ 按分隔符切分并 trim
 */
export function normalizeCaseAttorneys(value: unknown): string[] {
  if (Array.isArray(value)) {
    return value
      .filter((v): v is string => typeof v === 'string' && v.trim() !== '')
      .map((v) => v.trim())
  }

  if (typeof value === 'string' && value.trim() !== '') {
    const trimmed = value.trim()
    // JSON 数组字符串
    if (trimmed.startsWith('[') && trimmed.endsWith(']')) {
      try {
        const parsed: unknown = JSON.parse(trimmed)
        if (Array.isArray(parsed)) {
          return parsed
            .filter((v): v is string => typeof v === 'string' && v.trim() !== '')
            .map((v) => v.trim())
        }
      } catch {
        // 解析失败按裸分隔字符串处理
      }
    }
    // 裸分隔字符串：中英文逗号 / 顿号 / 分号 / 换行
    return trimmed
      .split(/[,，、;；\n\r]+/)
      .map(cleanTokenRaw)
      .filter((s) => s !== '')
  }

  return []
}

/**
 * 安全的 join：无论 attorneys 是数组、JSON 字符串还是裸分隔字符串，都输出可展示字符串。
 * 供消费点替代 `(caseData.attorneys || []).join(sep)`，防止对非数组调用 .join 崩溃。
 */
export function joinAttorneys(value: unknown, sep = '、'): string {
  return normalizeCaseAttorneys(value).join(sep)
}

/**
 * 归一化一条 wire Case（bindings.Case 或任意形状）为业务 Case。
 *
 * - 所有可空字符串字段默认归一为 ''（business Case 声明为非空 string）
 * - 可空枚举字段（caseLevel/procedureType/verdictType/双轨状态）保留 null
 * - 类型联合字段（track/caseStatus/caseRoute）缺失时回退到合法默认值
 * - attorneys 经 normalizeCaseAttorneys 归一为 string[]
 * - 补齐业务 Case 缺失字段：internalNo / folderTemplateId / deadlineUrgency
 */
export function normalizeCase(wire: unknown): BusinessCase {
  const c: Record<string, unknown> =
    wire && typeof wire === 'object' ? (wire as Record<string, unknown>) : {}

  return {
    // 基本信息
    id: str(c.id),
    caseName: str(c.caseName),
    caseNo: str(c.caseNo),
    track: enumVal<TrackType>(c.track, 'other'),
    causeAction: str(c.causeAction),
    internalNo: str(c.internalNo),

    // 当事人
    clientName: str(c.clientName),
    ourRole: str(c.ourRole),
    opponentName: str(c.opponentName),
    opponentRole: str(c.opponentRole),
    opponentFirm: str(c.opponentFirm),
    opponentAgent: str(c.opponentAgent),

    // 审理
    court: str(c.court),
    judgePanel: str(c.judgePanel),
    clerk: str(c.clerk),
    attorneys: normalizeCaseAttorneys(c.attorneys),
    caseLevel: maybeString(c.caseLevel) as CaseLevel | null,
    caseStatus: enumVal<CaseStatus>(c.caseStatus, '未知'),
    caseProgress: str(c.caseProgress),
    caseResult: str(c.caseResult),

    // 双轨状态机
    caseRoute: enumVal<CaseRoute>(c.caseRoute, '民事诉讼'),
    civilStatus: maybeString(c.civilStatus) as CivilStatus | null,
    invalidationStatus: maybeString(c.invalidationStatus) as InvalidationStatus | null,
    adminStatus: maybeString(c.adminStatus) as AdminStatus | null,

    // 专利
    patentName: str(c.patentName),
    patentAppNo: str(c.patentAppNo),
    procedureType: maybeString(c.procedureType) as ProcedureType | null,

    // 日期里程碑
    filingDate: str(c.filingDate),
    complaintReceivedDate: str(c.complaintReceivedDate),
    trialDate: str(c.trialDate),
    trial2Date: str(c.trial2Date),
    trial3Date: str(c.trial3Date),
    verdictType: maybeString(c.verdictType) as VerdictType | null,
    verdictDate: str(c.verdictDate),
    stayDate: str(c.stayDate),
    reliefDeadline: str(c.reliefDeadline),

    // 专利无效专属
    petitionerFirstInvalid: str(c.petitionerFirstInvalid),
    petitionerSuppDeadline: str(c.petitionerSuppDeadline),
    petitionerSubmitDate: str(c.petitionerSubmitDate),
    petitionerReceivedDate: str(c.petitionerReceivedDate),
    petitionerReplyDeadline: str(c.petitionerReplyDeadline),
    patenteeReceivedDate: str(c.patenteeReceivedDate),
    patenteeStatementDeadline: str(c.patenteeStatementDeadline),
    patenteeReceivedSuppDate: str(c.patenteeReceivedSuppDate),
    patenteeSuppDeadline: str(c.patenteeSuppDeadline),
    patenteeSubmitSuppDate: str(c.patenteeSubmitSuppDate),

    // 无效程序新增
    invalidationDecisionDate: str(c.invalidationDecisionDate),
    invalidationDecisionType: str(c.invalidationDecisionType),

    // 行政诉讼新增
    adminFilingDate: str(c.adminFilingDate),
    adminVerdictDate: str(c.adminVerdictDate),
    adminTrial2Date: str(c.adminTrial2Date),

    // 文件夹
    folderPath: str(c.folderPath),
    folderTemplateId: str(c.folderTemplateId),

    // 文书
    lastDocPath: str(c.lastDocPath),
    lastDocAt: str(c.lastDocAt),

    // 进度
    completedText: str(c.completedText),
    notes: str(c.notes),

    // 时间戳
    createdAt: str(c.createdAt),
    updatedAt: str(c.updatedAt),

    // 期限紧急度（列表查询时才填充）
    deadlineUrgency: maybeString(c.deadlineUrgency),
  }
}

/**
 * 归一化案件列表为业务 Case[]。
 */
export function normalizeCaseList(items: unknown[]): BusinessCase[] {
  return Array.isArray(items) ? items.map(normalizeCase) : []
}

/**
 * 归一化 create/update 的入参（业务侧 → wire 侧）。
 *
 * 主要处理 attorneys：业务侧可能是 string[]，而 Rust 端 create_case 以
 * serde 反序列化为 Case（attorneys: Option<String>），数组会导致解析失败；
 * update_case 虽可容忍数组（内部 to_string），但为统一存储格式，这里一律
 * 把数组序列化为 JSON 字符串。其余字段原样透传（缺键跳过/null 清空语义保持在后端）。
 */
export function normalizeCaseInput(data: Record<string, unknown>): Record<string, unknown> {
  if (!data || typeof data !== 'object') return data
  const out: Record<string, unknown> = { ...data }
  if ('attorneys' in out) {
    const attorneys = normalizeCaseAttorneys(out.attorneys)
    out.attorneys = attorneys.length > 0 ? JSON.stringify(attorneys) : null
  }
  return out
}
