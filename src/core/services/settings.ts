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
}
