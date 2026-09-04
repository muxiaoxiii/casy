/**
 * AI 提案网关前端封装（W1：Cursor 式 Diff 确认视图 + @ 引用沙箱）
 *
 * - 命令、参数与返回值均由 CommandMap 推导
 * - TS interface 与后端 #[serde(rename_all = "camelCase")] 对齐
 */

import { tauriCall } from '../tauriBridge'

// ============================================================
// 类型（对齐 Rust：ai::gateway::AiProposal / commands::ai_routes::ProposalPreviewDto）
// ============================================================

export type ProposalStatus = 'pending' | 'approved' | 'executed' | 'rejected' | 'expired'

export interface AiProposal {
  id: string
  toolName: string
  targetEntityType: string
  targetEntityId: string | null
  preStateHash: string | null
  payloadJson: string
  authToken: string
  expiresAt: string
  status: ProposalStatus
  createdAt: string
  executedAt: string | null
}

export interface ProposalFieldDiff {
  /** 字段名（payload 中的 camelCase key） */
  field: string
  before: unknown
  after: unknown
  changed: boolean
}

export interface ProposalPreviewDto {
  proposal: AiProposal
  targetEntityName: string | null
  currentState: Record<string, unknown> | null
  fieldDiffs: ProposalFieldDiff[]
}

export type ContextRefKind = 'file' | 'knowledge' | 'task' | 'case'

export interface ContextRef {
  kind: ContextRefKind
  id: string
}

/** 实际注入成功的引用（后端 ai_chat 返回） */
export interface UsedRef {
  kind: string
  id: string
  title: string
}

/** 输入框上方的引用 chip */
export interface RefChip extends ContextRef {
  title: string
}

// ============================================================
// 命令封装
// ============================================================

/** 获取提案预览（字段级 before→after diff） */
export function getProposalPreview(proposalId: string) {
  return tauriCall('get_proposal_preview', { proposalId })
}

/** 用户授权通过提案，返回一次性 auth_token */
export function approveProposal(proposalId: string) {
  return tauriCall('approve_ai_proposal', { proposalId })
}

/** 用户拒绝提案 */
export function rejectProposal(proposalId: string) {
  return tauriCall('reject_ai_proposal', { proposalId })
}

export interface CreateProposalInput {
  toolName: string
  targetEntityType: string
  targetEntityId?: string | null
  preStateHash?: string | null
  payloadJson: string
  ttlSeconds?: number | null
}

/** 创建 AI 写操作提案（初始 pending，默认 5 分钟有效期） */
export function createAiProposal(input: CreateProposalInput) {
  return tauriCall('create_ai_proposal', {
    toolName: input.toolName,
    targetEntityType: input.targetEntityType,
    targetEntityId: input.targetEntityId ?? null,
    preStateHash: input.preStateHash ?? null,
    payloadJson: input.payloadJson,
    ttlSeconds: input.ttlSeconds ?? null,
  })
}

// ============================================================
// 工具名 → 目标实体类型推断
// ============================================================

const ENTITY_TYPE_BY_CATEGORY: Record<string, string> = {
  tasks: 'task',
  cases: 'case',
  knowledge: 'knowledge',
  files: 'file',
  calendar: 'calendar_event',
  inbox: 'inbox_item',
  reminder: 'reminder',
  settings: 'setting',
}

/** 由工具分类/名称推断提案的目标实体类型（与后端 preview 白名单对齐） */
export function inferEntityType(toolName: string, category?: string): string {
  if (category && ENTITY_TYPE_BY_CATEGORY[category]) {
    return ENTITY_TYPE_BY_CATEGORY[category]
  }
  const parts = toolName.split('_')
  return parts[parts.length - 1] || toolName
}
