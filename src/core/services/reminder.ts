import { Service } from '../plugin/types'
import { tauriCallSafe } from '../tauriBridge'
import type { ReminderLogEntry, ReminderRule } from '../../types/bindings'
import type { ReminderRuleInput } from '../../types/ipc'

/** 提醒服务：ctx.reminder */
export class ReminderService extends Service {
  static inject: string[] = []

  async rules(): Promise<{ ok: boolean; data?: ReminderRule[]; error?: string }> {
    return tauriCallSafe('list_reminder_rules', {})
  }

  async createRule(data: ReminderRuleInput): Promise<{ ok: boolean; data?: ReminderRule; error?: string }> {
    return tauriCallSafe('create_reminder_rule', { data })
  }

  async log(limit?: number): Promise<{ ok: boolean; data?: ReminderLogEntry[]; error?: string }> {
    return tauriCallSafe('get_reminder_log', { limit: limit ?? 50 })
  }

  async startEngine(intervalSecs?: number): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe('start_reminder_engine', { intervalSecs: intervalSecs ?? 300 })
  }

  /** 立即重算一次（K-3：task:completed 事件消费者调用，不等引擎周期） */
  async recomputeNow(): Promise<{ ok: boolean; data?: number; error?: string }> {
    return tauriCallSafe('reminder_recompute_now', {})
  }

  async updateRule(id: string, data: ReminderRuleInput): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe('update_reminder_rule', { id, data })
  }

  async removeRule(id: string): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe('delete_reminder_rule', { id })
  }

  /** 发送本地测试提醒 */
  async test(opts: { ruleId: string; channel: string; message: string }): Promise<{ ok: boolean; data?: ReminderLogEntry[]; error?: string }> {
    return tauriCallSafe('test_reminder', {
      channels: [opts.channel],
      message: opts.message,
    })
  }

  /** 提醒处理反馈（写 reminded 行为事件，支撑"懂你的节奏"学习） */
  async recordFeedback(opts: { reminderLogId?: string | null; taskId?: string | null; status: string }): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe('record_reminder_feedback', {
      reminderLogId: opts.reminderLogId ?? null,
      taskId: opts.taskId ?? null,
      status: opts.status,
    })
  }
}
