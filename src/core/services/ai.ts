import { Service } from '../plugin/types'
import { tauriCallSafe } from '../tauriBridge'
import { useAiSettingsStore } from '../../stores/aiSettings'

/**
 * AI 服务：ctx.ai —— AI 模块数据通路
 *
 * 覆盖 AI 智伴 / 决策记录 / AI 审计三个视图用到的命令：
 * 今日推荐、决策留痕、学习洞察、记忆蒸馏、关联洞察、报表历史、决策复核、AI 运行审计。
 * 服务方法名按业务语义命名，内部封装 tauriCallSafe（参数 camelCase → 后端 snake_case）。
 */
export class AiService extends Service {
  static inject: string[] = []

  // ── 通用外部 AI 调用 (OpenAI / DeepSeek 协议) ──
  async askAi(prompt: string, contextData: any = null): Promise<{ ok: boolean; text?: string; error?: string }> {
    const store = useAiSettingsStore()
    store.load()

    if (!store.apiKey) {
      return { ok: false, error: 'AI Provider 未配置 API Key，请前往设置中心配置。' }
    }

    try {
      const messages = [
        { role: 'system', content: store.systemPrompt },
      ]
      
      if (contextData) {
        messages.push({ role: 'system', content: `[附带上下文数据]: ${JSON.stringify(contextData)}` })
      }
      
      messages.push({ role: 'user', content: prompt })

      const response = await fetch(`${store.baseUrl}/chat/completions`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': `Bearer ${store.apiKey}`
        },
        body: JSON.stringify({
          model: store.model,
          messages,
          temperature: 0.7
        })
      })

      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`)
      }

      const data = await response.json()
      return { ok: true, text: data.choices[0].message.content }
    } catch (err: any) {
      console.error('AI API Error:', err)
      return { ok: false, error: err.message || 'AI 请求失败' }
    }
  }

  // ── 今日推荐（§11.6 推荐引擎） ──

  /** 获取今日推荐（recommendations + followupSuggestions + source） */
  async todayRecommendations(): Promise<{ ok: boolean; data?: Record<string, unknown>; error?: string }> {
    return tauriCallSafe<Record<string, unknown>>('get_today_recommendations', {})
  }

  /**
   * 记录决策（推荐采纳/拒绝留痕，§11.6 决策留痕）
   * decisionType 支持 recommend_today / recommend_priority / recommend_estimate 等
   */
  async recordDecision(opts: {
    entityType: string
    entityId: string
    decisionType: string
    decision: string
    basis?: string | null
    sourceRef?: string | null
    status?: string
    reviewDue?: string | null
  }): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('record_decision', {
      entityType: opts.entityType,
      entityId: opts.entityId,
      decisionType: opts.decisionType,
      decision: opts.decision,
      basis: opts.basis ?? null,
      sourceRef: opts.sourceRef ?? null,
      status: opts.status ?? 'proposed',
      reviewDue: opts.reviewDue ?? null,
    })
  }

  // ── 学习洞察（§11.9 行为学习闭环） ──

  /** 获取行为学习分析（耗时校准 / 活跃时段 / 延期模式） */
  async learningAnalysis(): Promise<{ ok: boolean; data?: Record<string, unknown>; error?: string }> {
    return tauriCallSafe<Record<string, unknown>>('get_learning_analysis', {})
  }

  /** 一键校准预估（把偏差 >50% 或未设预估的未完成任务更新为历史均值） */
  async applyCalibration(): Promise<{ ok: boolean; data?: Record<string, unknown>; error?: string }> {
    return tauriCallSafe<Record<string, unknown>>('apply_learning_calibration', {})
  }

  // ── 记忆确认区（蒸馏候选） ──

  /** 列出待确认候选记忆 */
  async pendingMemories(): Promise<{ ok: boolean; data?: unknown[]; error?: string }> {
    return tauriCallSafe<unknown[]>('list_pending_memories', {})
  }

  /** 采纳候选记忆（可选同时沉淀进 knowledge_items 经验类） */
  async confirmMemory(id: string, sinkToKnowledge: boolean): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('confirm_memory', { id, sinkToKnowledge })
  }

  /** 丢弃候选记忆 */
  async dismissMemory(id: string): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe<void>('dismiss_memory', { id })
  }

  // ── 关联洞察（§3.2 通道 B 隐性关联学习） ──

  /** 列出待确认关联洞察 */
  async pendingInsights(): Promise<{ ok: boolean; data?: unknown[]; error?: string }> {
    return tauriCallSafe<unknown[]>('list_pending_insights', {})
  }

  /** 手动触发关联洞察生成（AI 未配置时静默返回 0） */
  async generateInsights(): Promise<{ ok: boolean; data?: Record<string, unknown>; error?: string }> {
    return tauriCallSafe<Record<string, unknown>>('generate_insights_cmd', {})
  }

  /** 确认关联洞察（可选沉淀 knowledge_items 经验类） */
  async confirmInsight(id: string, sinkToKnowledge: boolean): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('confirm_insight', { id, sinkToKnowledge })
  }

  /** 丢弃关联洞察 */
  async dismissInsight(id: string): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe<void>('dismiss_insight', { id })
  }

  // ── 报表历史（smart_summaries，§11.3） ──

  /** 列出报表历史（summaryType: daily / weekly） */
  async listSummaries(summaryType: string, limit?: number | null): Promise<{ ok: boolean; data?: unknown[]; error?: string }> {
    return tauriCallSafe<unknown[]>('list_summaries', { summaryType, limit: limit ?? null })
  }

  // ── 决策记录（§11.6 / §11.7 决策复核） ──

  /** 查询决策记录（可按实体类型 / 状态 / 条数过滤） */
  async listDecisions(opts: { entityType?: string | null; status?: string | null; limit?: number | null } = {}): Promise<{ ok: boolean; data?: unknown[]; error?: string }> {
    return tauriCallSafe<unknown[]>('list_decisions', {
      entityType: opts.entityType ?? null,
      status: opts.status ?? null,
      limit: opts.limit ?? null,
    })
  }

  /** 获取到期待复核决策列表（§11.7："该决策仍有效吗？"） */
  async pendingDecisionReviews(): Promise<{ ok: boolean; data?: unknown[]; error?: string }> {
    return tauriCallSafe<unknown[]>('get_pending_decision_reviews', {})
  }

  /** 复核决策：stillValid=true → 仅写 reviewed_at；false → 作废，可附说明 */
  async markDecisionReviewed(id: string, stillValid: boolean, note?: string | null): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe<void>('mark_decision_reviewed', { id, stillValid, note: note ?? null })
  }

  /** L3 递归核对：AI 核对决策与案件状态的一致性，失败时降级为规则核对 */
  async runRecursiveCheck(decisionId: string): Promise<{ ok: boolean; data?: Record<string, unknown>; error?: string }> {
    return tauriCallSafe<Record<string, unknown>>('run_recursive_check', { decisionId })
  }

  // ── AI 审计（ai_runs 历史） ──

  /** 获取 AI 运行历史（可加 purpose 过滤） */
  async runHistory(limit?: number | null, purpose?: string | null): Promise<{ ok: boolean; data?: unknown[]; error?: string }> {
    return tauriCallSafe<unknown[]>('get_ai_run_history', { limit: limit ?? null, purpose: purpose ?? null })
  }
}
