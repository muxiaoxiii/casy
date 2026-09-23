import { Service } from '../plugin/types'
import { tauriCallSafe } from '../tauriBridge'
import type { FolderNamingSettingsInput, FolderNamingSettingsOutput, FolderTemplateInput, FolderTemplateOutput, ImapAccountConfig } from '../../types/bindings'
import type { CommandMap } from '../../types/commandMap'
import type { IpcJsonObject, IpcJsonScalar } from '../../types/ipc'

type SettingsValue = IpcJsonScalar | IpcJsonObject | IpcJsonScalar[] | IpcJsonObject[]
type EmailMonitorStatus = CommandMap['get_email_monitor_status']['result']
type KeychainStatus = CommandMap['check_keychain_status']['result']
type McpPendingWrite = CommandMap['list_mcp_pending_writes']['result'][number]

/** 设置服务：ctx.settings */
export class SettingsService extends Service {
  static inject: string[] = []

  async get(): Promise<{ ok: boolean; data?: Record<string, SettingsValue>; error?: string }> {
    return tauriCallSafe('get_settings', {})
  }

  async save(settings: Record<string, SettingsValue>): Promise<{ ok: boolean; error?: string }> {
    const result = await tauriCallSafe('save_settings', { settings })
    if (result.ok && 'personal_calendar_days' in settings) this.ctx.emit('holiday:updated', {})
    return result
  }

  async configureAi(opts: { mode: string; apiUrl?: string; apiKey?: string; model?: string; dailyLimit?: number }): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe('configure_ai', {
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
    if (!url || !username || !(password || s.webdavPassword_configured || s.webdav_password_configured)) return null
    return { url, username, password: password || '' }
  }

  // ── 保存的筛选器（filters store 使用） ──

  async savedFilters(module: string): Promise<{ ok: boolean; data?: IpcJsonObject[]; error?: string }> {
    return tauriCallSafe('list_saved_filters', { module })
  }

  async saveFilter(filter: IpcJsonObject): Promise<{ ok: boolean; data?: IpcJsonObject; error?: string }> {
    return tauriCallSafe('save_filter', { filter })
  }

  async deleteFilter(id: string): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe('delete_filter', { id })
  }

  // ── 律师画像（profile store 使用） ──

  async profile(): Promise<{ ok: boolean; data?: IpcJsonObject; error?: string }> {
    return tauriCallSafe('get_lawyer_profile', {})
  }

  async saveProfile(profile: IpcJsonObject): Promise<{ ok: boolean; data?: IpcJsonObject; error?: string }> {
    return tauriCallSafe('save_lawyer_profile', { profile })
  }

  // ── 文件夹模板 ──

  /** 载入演示数据（仅空库可用；Dogfooding/可视化验证辅助） */
  async seedDemoData() {
    return tauriCallSafe('seed_demo_data', {})
  }

  async folderTemplates(): Promise<{ ok: boolean; data?: FolderTemplateOutput[]; error?: string }> {
    return tauriCallSafe('list_folder_templates', {})
  }

  async saveFolderTemplate(data: Partial<FolderTemplateInput>): Promise<{ ok: boolean; data?: string; error?: string }> {
    return tauriCallSafe('save_folder_template', { data })
  }

  async deleteFolderTemplate(templateId: string): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe('delete_folder_template', { templateId })
  }

  async folderNamingSettings(): Promise<{ ok: boolean; data?: FolderNamingSettingsOutput; error?: string }> {
    return tauriCallSafe('get_folder_naming_settings', {})
  }

  async saveFolderNamingSettings(data: Partial<FolderNamingSettingsInput>): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe('save_folder_naming_settings', { data })
  }

  // ── 节假日日历（期限引擎工作日顺延） ──

  async holidaysSummary(): Promise<{ ok: boolean; data?: { holidaysCount: number; workdaysCount: number; yearRange: string | null }; error?: string }> {
    return tauriCallSafe('get_holidays_summary', {})
  }

  async importHolidaysJson(jsonPath: string): Promise<{ ok: boolean; data?: IpcJsonObject; error?: string }> {
    const result = await tauriCallSafe('import_holidays_json', { jsonPath })
    if (result.ok) this.ctx.emit('holiday:imported', result.data)
    return result
  }

  // ── 邮件监听（IMAP） ──

  async emailMonitorStatus(): Promise<{ ok: boolean; data?: EmailMonitorStatus; error?: string }> {
    return tauriCallSafe('get_email_monitor_status', {})
  }

  async imapAccounts(): Promise<{ ok: boolean; data?: IpcJsonObject[]; error?: string }> {
    return tauriCallSafe('list_imap_accounts', {})
  }

  async configureImap(account: ImapAccountConfig): Promise<{ ok: boolean; data?: string; error?: string }> {
    return tauriCallSafe('configure_imap', { account })
  }

  async deleteImapAccount(emailAddress: string): Promise<{ ok: boolean; data?: string; error?: string }> {
    return tauriCallSafe('delete_imap_account', { emailAddress })
  }

  async startEmailMonitor(): Promise<{ ok: boolean; data?: string; error?: string }> {
    return tauriCallSafe('start_email_monitor', {})
  }

  async stopEmailMonitor(): Promise<{ ok: boolean; data?: string; error?: string }> {
    return tauriCallSafe('stop_email_monitor', {})
  }

  // ── 钥匙串 / MCP 写操作队列 ──

  async keychainStatus(): Promise<{ ok: boolean; data?: KeychainStatus; error?: string }> {
    return tauriCallSafe('check_keychain_status', {})
  }

  async mcpPendingWrites(): Promise<{ ok: boolean; data?: McpPendingWrite[]; error?: string }> {
    return tauriCallSafe('list_mcp_pending_writes', {})
  }

  async approveMcpWrite(id: string): Promise<{ ok: boolean; data?: IpcJsonObject; error?: string }> {
    return tauriCallSafe('approve_mcp_write', { id })
  }

  async rejectMcpWrite(id: string): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe('reject_mcp_write', { id })
  }
}
