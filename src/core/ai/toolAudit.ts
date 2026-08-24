/**
 * AI 工具调用审计（K-3 归因消费者）
 *
 * 订阅内核内部事件 tool:executed（executeTool 在 origin='ai' 时发出），
 * 将 AI 发起的工具执行结果写入既有 audit_events 表（actor='ai'）：
 *   - ai_runs 记过程（对话级，ai_chat 命令负责）
 *   - audit_events 记结果（工具级，本模块负责）
 * turnId 为一次工具循环的关联键；与 ai_runs 的外键关联待 ai_chat
 * 返回结构化 run_id 后替换（随 B1 AI 域收口）。
 * 失败不阻塞主流程：审计写失败仅记 console。
 */
import { casyContext } from '../plugin/context'
import { tauriCallSafe } from '../tauriBridge'

interface ToolExecutedPayload {
  name?: string
  turnId?: string | null
  ok?: boolean
  declined?: boolean
  digest?: string
}

export function installToolAuditWriter(): void {
  casyContext.on('tool:executed', (raw) => {
    const p = (raw ?? {}) as ToolExecutedPayload
    if (!p.name) return
    const outcome = p.declined ? 'cancelled' : p.ok ? 'executed' : 'failed'
    void tauriCallSafe<void>('record_ai_tool_audit', {
      tool: p.name,
      turnId: p.turnId ?? '',
      outcome,
      digest: p.digest ?? null,
    }).catch((e: unknown) => {
      console.warn('[Casy] AI 工具审计写入失败（不阻塞）:', e)
    })
  })
}
