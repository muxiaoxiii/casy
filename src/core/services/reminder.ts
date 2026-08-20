import { Service } from '../plugin/types'
import { tauriCallSafe } from '../tauriBridge'

/** 提醒服务：ctx.reminder */
export class ReminderService extends Service {
  static inject: string[] = []

  async rules(): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('list_reminder_rules', {})
  }

  async createRule(data: Record<string, unknown>): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('create_reminder_rule', { data })
  }

  async log(limit?: number): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('get_reminder_log', { limit: limit ?? 50 })
  }

  async startEngine(intervalSecs?: number): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe<void>('start_reminder_engine', { intervalSecs: intervalSecs ?? 300 })
  }

  async updateRule(id: string, data: Record<string, unknown>): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('update_reminder_rule', { id, data })
  }

  async removeRule(id: string): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe<void>('delete_reminder_rule', { id })
  }

  /** 发送本地测试提醒 */
  async test(opts: { ruleId: string; channel: string; message: string }): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe<void>('test_reminder', {
      ruleId: opts.ruleId,
      channel: opts.channel,
      message: opts.message,
    })
  }
}
