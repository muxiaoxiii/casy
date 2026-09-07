import { Service } from '../plugin/types'
import { tauriCallSafe } from '../tauriBridge'
import type { MappingEntry } from '../../types/bindings'
import type { CommandMap } from '../../types/commandMap'
import type {
  FeishuBitableTableInfo,
  FeishuMappingPayload,
  NormalizedFeishuBitableFieldInfo,
} from '../../types/ipc'

type SyncStatus = CommandMap['get_sync_status']['result']
type SyncResult = CommandMap['webdav_push']['result']
type FeishuSyncInfo = CommandMap['get_feishu_sync_info']['result']
type FeishuImportReport = CommandMap['import_feishu_data']['result']
type SchemaDiff = CommandMap['feishu_compare_table']['result']
type RecordDiff = CommandMap['feishu_compare_records']['result']
type FeishuImportResult = CommandMap['feishu_import_all']['result']
type FeishuSyncReport = CommandMap['sync_feishu_pull']['result']

/** 同步服务：ctx.sync */
export class SyncService extends Service {
  static inject: string[] = []

  async status(): Promise<{ ok: boolean; data?: SyncStatus; error?: string }> {
    return tauriCallSafe('get_sync_status', {})
  }

  async testWebdav(url: string, username: string, password: string): Promise<{ ok: boolean; data?: string; error?: string }> {
    return tauriCallSafe('test_webdav_connection', { url, username, password })
  }

  async push(url: string, username: string, password: string): Promise<{ ok: boolean; data?: SyncResult; error?: string }> {
    return tauriCallSafe('webdav_push', { url, username, password })
  }

  async pull(url: string, username: string, password: string): Promise<{ ok: boolean; data?: SyncResult; error?: string }> {
    return tauriCallSafe('webdav_pull', { url, username, password })
  }

  // ── 飞书同步（导入/凭证/表结构/映射/比较） ──

  async feishuSyncInfo(): Promise<{ ok: boolean; data?: FeishuSyncInfo; error?: string }> {
    return tauriCallSafe('get_feishu_sync_info', {})
  }

  /** legacy JSON dump 导入 */
  async importFeishuData(jsonPath: string): Promise<{ ok: boolean; data?: FeishuImportReport; error?: string }> {
    return tauriCallSafe('import_feishu_data', { jsonPath })
  }

  async configureFeishu(appId: string, appSecret: string): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe('configure_feishu', { appId, appSecret })
  }

  async testFeishuConnection(appId?: string, appSecret?: string): Promise<{ ok: boolean; data?: string; error?: string }> {
    return tauriCallSafe('test_feishu_connection', {
      appId: appId || null,
      appSecret: appSecret || null,
    })
  }

  /** v3.0: 表结构发现 */
  async feishuListTables(appToken: string): Promise<{ ok: boolean; data?: FeishuBitableTableInfo[]; error?: string }> {
    return tauriCallSafe('feishu_list_tables', { appToken })
  }

  async feishuListFields(appToken: string, tableId: string): Promise<{ ok: boolean; data?: NormalizedFeishuBitableFieldInfo[]; error?: string }> {
    const result = await tauriCallSafe('feishu_list_fields', { appToken, tableId })
    if (result.ok && result.data) {
      return {
        ok: true,
        data: result.data.map((field) => ({
          ...field,
          fieldType: field.type,
        })),
      }
    }
    return { ok: false, error: result.error }
  }

  /** v3.0: Schema 比较 */
  async feishuCompareTable(appToken: string, tableId: string, localTable: string): Promise<{ ok: boolean; data?: SchemaDiff; error?: string }> {
    return tauriCallSafe('feishu_compare_table', { appToken, tableId, localTable })
  }

  /** v3.0: 记录比较 */
  async feishuCompareRecords(appToken: string, tableId: string, localTable: string, matchField: string): Promise<{ ok: boolean; data?: RecordDiff; error?: string }> {
    return tauriCallSafe('feishu_compare_records', { appToken, tableId, localTable, matchField })
  }

  async feishuSaveMappings(mappingsJson: FeishuMappingPayload[]): Promise<{ ok: boolean; data?: string; error?: string }> {
    return tauriCallSafe('feishu_save_mappings', { mappingsJson })
  }

  /** v3.0: 全量导入 */
  async feishuImportAll(appToken: string, tableId: string, localTable: string, mappingsJson: MappingEntry[]): Promise<{ ok: boolean; data?: FeishuImportResult; error?: string }> {
    return tauriCallSafe('feishu_import_all', { appToken, tableId, localTable, mappings: mappingsJson })
  }

  /** v3.0: 增量导入 */
  async feishuImportIncremental(appToken: string, tableId: string, localTable: string, sinceTimestamp: string, mappingsJson: MappingEntry[]): Promise<{ ok: boolean; data?: FeishuImportResult; error?: string }> {
    return tauriCallSafe('feishu_import_incremental', {
      appToken,
      tableId,
      localTable,
      sinceTimestamp,
      mappingsJson,
    })
  }

  // ── WebDAV 启动检查与冲突解决 ──

  /** WebDAV 启动同步检查（检测冲突 / 同步方向） */
  async startupSync(url: string, username: string, password: string): Promise<{ ok: boolean; data?: SyncResult; error?: string }> {
    return tauriCallSafe('webdav_startup_sync', { url, username, password })
  }

  /** 冲突解决：保留本地版本并上传 */
  async resolveKeepLocal(url: string, username: string, password: string): Promise<{ ok: boolean; data?: SyncResult; error?: string }> {
    return tauriCallSafe('webdav_resolve_keep_local', { url, username, password })
  }

  /** 冲突解决：保留远程版本 */
  async resolveKeepRemote(url: string, username: string, password: string): Promise<{ ok: boolean; data?: SyncResult; error?: string }> {
    return tauriCallSafe('webdav_resolve_keep_remote', { url, username, password })
  }

  // ── 飞书双向同步（SyncStatusView 使用） ──

  /** 飞书拉取 */
  async feishuPull(appToken: string, tableId: string): Promise<{ ok: boolean; data?: FeishuSyncReport; error?: string }> {
    return tauriCallSafe('sync_feishu_pull', { appToken, tableId })
  }

  /** 飞书推送 */
  async feishuPush(appToken: string, tableId: string): Promise<{ ok: boolean; data?: FeishuSyncReport; error?: string }> {
    return tauriCallSafe('sync_feishu_push', { appToken, tableId })
  }
}
