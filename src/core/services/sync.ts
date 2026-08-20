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

  // ── 飞书同步（导入/凭证/表结构/映射/比较） ──

  async feishuSyncInfo(): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('get_feishu_sync_info', {})
  }

  /** legacy JSON dump 导入 */
  async importFeishuData(jsonPath: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('import_feishu_data', { jsonPath })
  }

  async configureFeishu(appId: string, appSecret: string): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe<void>('configure_feishu', { appId, appSecret })
  }

  async testFeishuConnection(): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('test_feishu_connection', {})
  }

  /** v3.0: 表结构发现 */
  async feishuListTables(appToken: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('feishu_list_tables', { appToken })
  }

  async feishuListFields(appToken: string, tableId: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('feishu_list_fields', { appToken, tableId })
  }

  /** v3.0: Schema 比较 */
  async feishuCompareTable(appToken: string, tableId: string, localTable: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('feishu_compare_table', { appToken, tableId, localTable })
  }

  /** v3.0: 记录比较 */
  async feishuCompareRecords(appToken: string, tableId: string, localTable: string, matchField: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('feishu_compare_records', { appToken, tableId, localTable, matchField })
  }

  async feishuSaveMappings(mappingsJson: unknown[]): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('feishu_save_mappings', { mappingsJson })
  }

  /** v3.0: 全量导入 */
  async feishuImportAll(appToken: string, tableId: string, localTable: string, mappingsJson: unknown[]): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('feishu_import_all', { appToken, tableId, localTable, mappingsJson })
  }

  /** v3.0: 增量导入 */
  async feishuImportIncremental(appToken: string, tableId: string, localTable: string, sinceTimestamp: string, mappingsJson: unknown[]): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('feishu_import_incremental', {
      appToken,
      tableId,
      localTable,
      sinceTimestamp,
      mappingsJson,
    })
  }

  // ── WebDAV 启动检查与冲突解决 ──

  /** WebDAV 启动同步检查（检测冲突 / 同步方向） */
  async startupSync(url: string, username: string, password: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('webdav_startup_sync', { url, username, password })
  }

  /** 冲突解决：保留本地版本并上传 */
  async resolveKeepLocal(url: string, username: string, password: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('webdav_resolve_keep_local', { url, username, password })
  }

  /** 冲突解决：保留远程版本 */
  async resolveKeepRemote(url: string, username: string, password: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('webdav_resolve_keep_remote', { url, username, password })
  }

  // ── 飞书双向同步（SyncStatusView 使用） ──

  /** 飞书拉取 */
  async feishuPull(appToken: string, tableId: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('sync_feishu_pull', { appToken, tableId })
  }

  /** 飞书推送 */
  async feishuPush(appToken: string, tableId: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('sync_feishu_push', { appToken, tableId })
  }
}
