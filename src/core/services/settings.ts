import { Service } from '../plugin/types'
import { tauriCallSafe } from '../tauriBridge'

/** 设置服务：ctx.settings */
export class SettingsService extends Service {
  static inject: string[] = []

  async get(): Promise<{ ok: boolean; data?: Record<string, unknown>; error?: string }> {
    return tauriCallSafe<Record<string, unknown>>('get_settings', {})
  }

  async save(settings: Record<string, unknown>): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe<void>('save_settings', { settings })
  }

  async configureAi(opts: { mode: string; apiUrl?: string; apiKey?: string; model?: string; dailyLimit?: number }): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe<void>('configure_ai', {
      mode: opts.mode,
      apiUrl: opts.apiUrl ?? null,
      apiKey: opts.apiKey ?? null,
      model: opts.model ?? null,
      dailyLimit: opts.dailyLimit ?? null,
    })
  }

  // ── AI 配置与用量（AI 智伴状态栏 / 设置页使用） ──

  /** 获取当前 AI 配置（mode / apiUrl / model / dailyLimit） */
  async aiConfig(): Promise<{ ok: boolean; data?: { mode: string; apiUrl?: string | null; apiKey?: string | null; model?: string | null; dailyLimit?: number | null }; error?: string }> {
    return tauriCallSafe('get_ai_config', {})
  }

  /** 获取 AI 调用使用情况（今日用量 / 限额 / 剩余） */
  async aiUsage(): Promise<{ ok: boolean; data?: { usedToday: number; dailyLimit: number; remaining: number }; error?: string }> {
    return tauriCallSafe('get_ai_usage', {})
  }

  /** WebDAV 凭据（供 sync 相关工具读取） */
  async webdavCredentials(): Promise<{ url: string; username: string; password: string } | null> {
    const res = await this.get()
    if (!res.ok || !res.data) return null
    const s = res.data
    const url = (s.webdav_url || s.webdavUrl) as string | undefined
    const username = (s.webdav_username || s.webdavUsername) as string | undefined
    const password = (s.webdav_password || s.webdavPassword) as string | undefined
    if (!url || !username || !password) return null
    return { url, username, password }
  }

  // ── 保存的筛选器（filters store 使用） ──

  async savedFilters(module: string): Promise<{ ok: boolean; data?: unknown[]; error?: string }> {
    return tauriCallSafe<unknown[]>('list_saved_filters', { module })
  }

  async saveFilter(filter: Record<string, unknown>): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('save_filter', { filter })
  }

  async deleteFilter(id: string): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe<void>('delete_filter', { id })
  }

  // ── 律师画像（profile store 使用） ──

  async profile(): Promise<{ ok: boolean; data?: Record<string, unknown>; error?: string }> {
    return tauriCallSafe<Record<string, unknown>>('get_lawyer_profile', {})
  }

  async saveProfile(profile: Record<string, unknown>): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('save_lawyer_profile', { profile })
  }

  // ── 文件夹模板 ──

  async folderTemplates(): Promise<{ ok: boolean; data?: unknown[]; error?: string }> {
    return tauriCallSafe<unknown[]>('list_folder_templates', {})
  }

  async saveFolderTemplate(data: Record<string, unknown>): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('save_folder_template', { data })
  }

  async deleteFolderTemplate(templateId: string): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe<void>('delete_folder_template', { templateId })
  }

  async folderNamingSettings(): Promise<{ ok: boolean; data?: Record<string, unknown>; error?: string }> {
    return tauriCallSafe<Record<string, unknown>>('get_folder_naming_settings', {})
  }

  async saveFolderNamingSettings(data: Record<string, unknown>): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe<void>('save_folder_naming_settings', { data })
  }

  // ── 节假日日历（期限引擎工作日顺延） ──

  async holidaysSummary(): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('get_holidays_summary', {})
  }

  async importHolidaysJson(jsonPath: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('import_holidays_json', { jsonPath })
  }

  // ── 邮件监听（IMAP） ──

  async emailMonitorStatus(): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('get_email_monitor_status', {})
  }

  async imapAccounts(): Promise<{ ok: boolean; data?: unknown[]; error?: string }> {
    return tauriCallSafe<unknown[]>('list_imap_accounts', {})
  }

  async configureImap(account: Record<string, unknown>): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe<void>('configure_imap', { account })
  }

  async deleteImapAccount(emailAddress: string): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe<void>('delete_imap_account', { emailAddress })
  }

  async startEmailMonitor(): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('start_email_monitor', {})
  }

  async stopEmailMonitor(): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('stop_email_monitor', {})
  }

  // ── 钥匙串 / MCP 写操作队列 ──

  async keychainStatus(): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('check_keychain_status', {})
  }

  async mcpPendingWrites(): Promise<{ ok: boolean; data?: unknown[]; error?: string }> {
    return tauriCallSafe<unknown[]>('list_mcp_pending_writes', {})
  }

  async approveMcpWrite(id: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('approve_mcp_write', { id })
  }

  async rejectMcpWrite(id: string): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe<void>('reject_mcp_write', { id })
  }
}
