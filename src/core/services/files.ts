import { Service } from '../plugin/types'
import { tauriCallSafe } from '../tauriBridge'
import type { TauriResult } from '../../types'

/** 案卷文件服务：ctx.files */
export class FilesService extends Service {
  static inject: string[] = []

  /** 列出案件文件；category 缺省时返回全部（list_case_files 支持按分类过滤） */
  async list(caseId: string, category?: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe('list_case_files', {
      caseId,
      category: category || null,
    })
  }

  async add(caseId: string, filePath: string, category?: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe('add_case_file', {
      caseId,
      fileName: filePath ? (filePath.split('/').pop() || filePath.split('\\').pop() || '') : '',
      filePath,
      category: category || '',
    })
  }

  async remove(id: string): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe('delete_case_file', { id })
  }

  /** 打开文件/目录（open_file_with_default，供导出 DOCX 后打开文件等场景） */
  async open(path: string): Promise<TauriResult<null>> {
    return tauriCallSafe('open_file_with_default', { path })
  }

  // ── 案卷管理（index-v2 精装版 · 本地文件夹同步）──

  /** 12 阶段目录树扫描 */
  async listCaseDirs(caseId: string) {
    return tauriCallSafe('list_case_dirs', { caseId })
  }

  /** 新建子文件夹（parentRel 空 = 卷宗根） */
  async createSubdir(caseId: string, parentRel: string | null, name: string) {
    return tauriCallSafe('create_case_subdir', { caseId, parentRel: parentRel ?? null, name })
  }

  /** 系统拖入/选择文件 → 复制进子目录并登记 */
  async importToCase(caseId: string, dirRel: string | null, paths: string[]) {
    return tauriCallSafe('import_files_to_case', { caseId, dirRel: dirRel ?? null, paths })
  }

  /** 扫描磁盘未登记文件（既有卷宗导入） */
  async scanUnregistered(caseId: string) {
    return tauriCallSafe('scan_unregistered_files', { caseId })
  }

  /** 批量登记既有文件 */
  async registerExisting(caseId: string, paths: string[]) {
    return tauriCallSafe('register_existing_files', { caseId, paths })
  }

  /** Finder 定位 */
  async reveal(path: string): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe('reveal_path', { path })
  }

  /** 系统默认应用打开 */
  async openDefault(path: string): Promise<TauriResult<null>> {
    return this.open(path)
  }

  /** 批量重命名（磁盘 + DB 同步，冲突自动去重） */
  async applyRenames(
    caseId: string,
    renames: Array<{ id: string; newName: string }>,
  ) {
    return tauriCallSafe('apply_case_file_renames', { caseId, renames })
  }

  /** 按案件列出已登记文件（注册表类型：CaseFile[]） */
  async listByCase(caseId: string) {
    return tauriCallSafe('list_case_files', { caseId })
  }
}
