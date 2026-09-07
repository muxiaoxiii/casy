import { Service } from '../plugin/types'
import { tauriCallSafe } from '../tauriBridge'

export interface BackupFileDto {
  filename: string
  sizeBytes: number
  modifiedAt: string
}

/** 数据备份服务（R-5）：VACUUM INTO 一致性快照 + 覆盖式恢复（需重启） */
export class BackupService extends Service {
  static inject: string[] = []

  async exportFull(destination: string, password: string) {
    return tauriCallSafe('export_full_backup', { destination, password })
  }

  async importFull(source: string, password: string) {
    return tauriCallSafe('import_full_backup', { source, password })
  }

  async create(): Promise<{ ok: boolean; data?: BackupFileDto; error?: string }> {
    return tauriCallSafe('create_backup', {})
  }

  async list(): Promise<{ ok: boolean; data?: BackupFileDto[]; error?: string }> {
    return tauriCallSafe('list_backups', {})
  }

  /** 恢复后必须重启应用；UI 层负责确认与提示 */
  async restore(filename: string): Promise<{ ok: boolean; data?: boolean; error?: string }> {
    return tauriCallSafe('restore_backup', { filename })
  }
}
