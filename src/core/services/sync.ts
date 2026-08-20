import { Service } from '../plugin/types'
import { tauriCallSafe } from '../tauriBridge'

/** 同步服务：ctx.sync */
export class SyncService extends Service {
  static inject: string[] = []

  async status(): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('get_sync_status', {})
  }

  async testWebdav(url: string, username: string, password: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('test_webdav_connection', { url, username, password })
  }

  async push(url: string, username: string, password: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('webdav_push', { url, username, password })
  }

  async pull(url: string, username: string, password: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('webdav_pull', { url, username, password })
  }
}
