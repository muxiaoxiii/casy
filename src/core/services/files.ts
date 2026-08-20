import { Service } from '../plugin/types'
import { tauriCallSafe } from '../tauriBridge'

/** 案卷文件服务：ctx.files */
export class FilesService extends Service {
  static inject: string[] = []

  async list(caseId: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('list_case_files', { caseId })
  }

  async add(caseId: string, filePath: string, category?: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('add_case_file', {
      caseId,
      fileName: filePath ? filePath.split('/').pop() : '',
      filePath,
      category: category || '',
    })
  }

  async remove(id: string): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe<void>('delete_case_file', { id })
  }
}
