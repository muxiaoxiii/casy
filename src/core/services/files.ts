import { Service } from '../plugin/types'
import { tauriCallSafe } from '../tauriBridge'

/** 案卷文件服务：ctx.files */
export class FilesService extends Service {
  static inject: string[] = []

  /** 列出案件文件；category 缺省时返回全部（list_case_files 支持按分类过滤） */
  async list(caseId: string, category?: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('list_case_files', {
      caseId,
      category: category || null,
    })
  }

  async add(caseId: string, filePath: string, category?: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('add_case_file', {
      caseId,
      fileName: filePath ? (filePath.split('/').pop() || filePath.split('\\').pop() || '') : '',
      filePath,
      category: category || '',
    })
  }

  async remove(id: string): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe<void>('delete_case_file', { id })
  }

  /** 打开文件/目录（open_path，供导出 DOCX 后打开文件等场景） */
  async open(path: string): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe<void>('open_path', { path })
  }
}
