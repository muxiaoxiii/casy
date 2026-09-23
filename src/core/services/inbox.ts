import { Service } from '../plugin/types'
import { tauriCallSafe } from '../tauriBridge'
import type { InboxItem } from '../../types'
import type { CommandMap } from '../../types/commandMap'
import type { IpcJsonObject } from '../../types/ipc'

type ProcessedInboxResult = CommandMap['process_inbox_item']['result']
type QuickJudgeResult = CommandMap['quick_judge_inbox_item']['result']
type InboxProgress = CommandMap['get_inbox_progress']['result']

/** 收件箱服务：ctx.inbox */
export class InboxService extends Service {
  static inject: string[] = []

  async list(status?: string): Promise<{ ok: boolean; data?: InboxItem[]; error?: string }> {
    return tauriCallSafe('list_inbox_items', { status: status || undefined })
  }

  async add(sourceType: string, contentText?: string, sourcePath?: string): Promise<{ ok: boolean; data?: string; error?: string }> {
    return tauriCallSafe('add_inbox_item', {
      sourceType,
      contentText: contentText ?? null,
      sourcePath: sourcePath ?? null,
    })
  }

  async process(id: string): Promise<{ ok: boolean; data?: ProcessedInboxResult; error?: string }> {
    // 返回类型由 CommandMap 提供（ProcessedInboxResult），此处不再显式泛型
    return tauriCallSafe('process_inbox_item', { id })
  }

  async file(id: string, caseId: string, category?: string): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe('file_inbox_item', {
      itemId: id,
      caseId,
      category: category || '',
    })
  }

  async dismiss(id: string): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe('dismiss_inbox_item', { id })
  }

  /** 即时意图判断（本地规则，0ms）：文件 → 归档推荐；文本 → 任务/期限/知识/案件/提醒推荐 */
  async quickJudge(id: string): Promise<{ ok: boolean; data?: QuickJudgeResult; error?: string }> {
    return tauriCallSafe('quick_judge_inbox_item', { id })
  }

  /** AI 分析（带缓存） */
  async aiAnalyze(id: string): Promise<{ ok: boolean; data?: ProcessedInboxResult; error?: string }> {
    return tauriCallSafe('ai_analyze_inbox_item', { id })
  }

  /**
   * 确认推荐动作并自行推送（设计哲学 §10：推荐按钮 → 一键落地）
   * action: file_to_case | create_task | create_deadline | save_knowledge | create_case | set_reminder
   */
  async confirmAction(opts: {
    inboxItemId: string
    targetCaseId?: string | null
    targetCategory?: string | null
    action?: string
    intent?: IpcJsonObject | null
  }): Promise<{ ok: boolean; data?: IpcJsonObject; error?: string }> {
    const result = await tauriCallSafe('confirm_inbox_action', {
      inboxItemId: opts.inboxItemId,
      targetCaseId: opts.targetCaseId ?? null,
      targetCategory: opts.targetCategory ?? null,
      action: opts.action ?? 'file_to_case',
      intent: opts.intent ?? null,
    })
    if (result.ok) this.ctx.emit('inbox:confirmed', { action: opts.action ?? 'file_to_case', result: result.data })
    return result
  }

  async parseHolidays(content: string) {
    return tauriCallSafe('parse_holiday_notice', { content })
  }

  async actionResult(inboxItemId: string) {
    return tauriCallSafe('get_inbox_action_result', { inboxItemId })
  }

  /** 语音转写（需 OpenAI 兼容 STT） */
  async transcribeVoiceNote(inboxItemId: string): Promise<{ ok: boolean; data?: string; error?: string }> {
    return tauriCallSafe('transcribe_voice_note', { voiceNoteId: inboxItemId })
  }

  /**
   * 拒绝推荐反馈（设计哲学 §10：推荐拒绝 → 记录学习信号）
   * 
   * 记录用户拒绝推荐的原因，供推荐系统学习改进。
   * 后端 reject_inbox_recommendation 写入 inbox_feedback 表。
   */
  async rejectRecommendation(opts: {
    inboxItemId: string
    action: string
    reason?: string
    intent?: IpcJsonObject | null
  }): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe('reject_inbox_recommendation', {
      inboxItemId: opts.inboxItemId,
      action: opts.action,
      reason: opts.reason ?? null,
      intent: opts.intent ?? null,
    })
  }

  /** 批量 AI 处理队列控制 */
  async startBatch(): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe('start_inbox_batch', {})
  }
  async pauseBatch(): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe('pause_inbox_batch', {})
  }
  async resumeBatch(): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe('resume_inbox_batch', {})
  }
  async cancelBatch(): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe('cancel_inbox_batch', {})
  }
  async getBatchProgress(): Promise<{ ok: boolean; data?: InboxProgress; error?: string }> {
    return tauriCallSafe('get_inbox_progress', {})
  }
}
