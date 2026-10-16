import { Service } from '../plugin/types'
import { tauriCallSafe } from '../tauriBridge'
import type { TauriResult } from '../../types'
import type { DocumentFacts, IntakeProposal } from '../../types/bindings'
import type { CommandMap } from '../../types/commandMap'

type CaseFile = CommandMap['list_case_files']['result'][number]
type CaseDirEntry = CommandMap['list_case_dirs']['result'][number]
type ImportedCaseFile = CommandMap['import_files_to_case']['result'][number]
type UnregisteredCaseFile = CommandMap['scan_unregistered_files']['result'][number]
type CaseFileRenameResult = CommandMap['apply_case_file_renames']['result'][number]

/** 案卷文件服务：ctx.files */
export class FilesService extends Service {
  static inject: string[] = []

  /** 列出案件文件；category 缺省时返回全部（list_case_files 支持按分类过滤） */
  async list(caseId: string, category?: string): Promise<{ ok: boolean; data?: CaseFile[]; error?: string }> {
    return tauriCallSafe('list_case_files', {
      caseId,
      category: category || null,
    })
  }

  async add(caseId: string, filePath: string, category?: string): Promise<{ ok: boolean; data?: CaseFile; error?: string }> {
    return this.mutation('file', tauriCallSafe('add_case_file', {
      caseId,
      fileName: filePath.replaceAll('\\', '/').split('/').pop() || '',
      filePath,
      category: category || 'other',
    }))
  }

  async remove(id: string): Promise<{ ok: boolean; error?: string }> {
    return this.mutation('file', tauriCallSafe('delete_case_file', { id }))
  }

  /** 打开文件/目录（open_file_with_default，供导出 DOCX 后打开文件等场景） */
  async open(path: string): Promise<TauriResult<null>> {
    return tauriCallSafe('open_file_with_default', { path })
  }

  // ── 案卷管理（index-v2 精装版 · 本地文件夹同步）──

  /** 12 阶段目录树扫描 */
  async listCaseDirs(caseId: string): Promise<{ ok: boolean; data?: CaseDirEntry[]; error?: string }> {
    return tauriCallSafe('list_case_dirs', { caseId })
  }

  /** 新建子文件夹（parentRel 空 = 卷宗根） */
  async createSubdir(caseId: string, parentRel: string | null, name: string): Promise<{ ok: boolean; data?: string; error?: string }> {
    return tauriCallSafe('create_case_subdir', { caseId, parentRel: parentRel ?? null, name })
  }

  /** 系统拖入/选择文件 → 复制进子目录并登记 */
  async importToCase(caseId: string, dirRel: string | null, paths: string[], category?: string): Promise<{ ok: boolean; data?: ImportedCaseFile[]; error?: string }> {
    return this.mutation('file', tauriCallSafe('import_files_to_case', { caseId, dirRel: dirRel ?? null, paths, category }))
  }

  async removed(caseId: string) { return tauriCallSafe('list_removed_case_files', { caseId }) }
  async restore(id: string) { return this.mutation('file', tauriCallSafe('restore_case_file', { id })) }
  async move(caseId: string, ids: string[], dirRel: string | null) { return this.mutation('file', tauriCallSafe('move_case_files', { caseId, ids, dirRel })) }
  async setCategory(id: string, category: string) { return this.mutation('file', tauriCallSafe('set_case_file_category', { id, category })) }

  /** O5：把某个已完成 OCR 版本设为当前生效版本（校订链回退，不删除任何版本） */
  async pinDocumentJob(fileId: string, jobId: string) { return this.mutation('file', tauriCallSafe('pin_document_job', { fileId, jobId })) }
  /** O7：轻量元数据（备注 / 文档日期 / 关键词） */
  async setMeta(id: string, meta: { documentDate?: string | null; summary?: string | null; keywords?: string | null }) {
    return this.mutation('file', tauriCallSafe('update_case_file_meta', { id, ...meta }))
  }

  /** UPDATE_PLAN Phase 1：抽取（或重跑）一份文书的语义事实（开庭日期带页码出处）。
   *  OCR 未完成时后端明确报错，不返回空事实。 */
  async extractFacts(refType: string, refId: string): Promise<TauriResult<DocumentFacts>> {
    return this.mutation('file', tauriCallSafe('extract_document_facts', { refType, refId }))
  }

  /** 读取已抽取的事实（未抽取则报错，由调用方决定是否触发抽取） */
  async facts(refType: string, refId: string): Promise<TauriResult<DocumentFacts>> {
    return tauriCallSafe('get_document_facts', { refType, refId })
  }

  /** UPDATE_PLAN Phase 1：从已抽取事实生成待确认建议（开庭日期带页码出处）。
   *  命中 C1 冲突（同案已有开庭且日期不同）时建议带三选一冲突，由用户裁决。 */
  async generateProposals(refType: string, refId: string): Promise<TauriResult<IntakeProposal[]>> {
    return this.mutation('file', tauriCallSafe('generate_intake_proposals', { refType, refId }))
  }

  /** 列出建议（status 为空 = 全部，新的在前） */
  async proposals(status?: string): Promise<TauriResult<IntakeProposal[]>> {
    return tauriCallSafe('list_intake_proposals', { status: status ?? null })
  }

  /** 确认一条建议。resolution 至少含 option；C1 选 reschedule 时需
   *  supersedeHearingId 与 reason；无案件时需 caseId。 */
  async confirmProposal(id: string, resolution: Record<string, unknown>): Promise<TauriResult<IntakeProposal>> {
    return this.mutation('file', tauriCallSafe('confirm_intake_proposal', { id, resolution }))
  }

  /** 拒绝一条建议（记录原因，供学习信号） */
  async rejectProposal(id: string, reason?: string): Promise<TauriResult<void>> {
    return this.mutation('file', tauriCallSafe('reject_intake_proposal', { id, reason: reason ?? null }))
  }

  /** 扫描磁盘未登记文件（既有卷宗导入） */
  async scanUnregistered(caseId: string): Promise<{ ok: boolean; data?: UnregisteredCaseFile[]; error?: string }> {
    return tauriCallSafe('scan_unregistered_files', { caseId })
  }

  /** 批量登记既有文件 */
  async registerExisting(caseId: string, paths: string[]): Promise<{ ok: boolean; data?: number; error?: string }> {
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
  ): Promise<{ ok: boolean; data?: CaseFileRenameResult[]; error?: string }> {
    return tauriCallSafe('apply_case_file_renames', { caseId, renames })
  }

  /** 按案件列出已登记文件（注册表类型：CaseFile[]） */
  async listByCase(caseId: string): Promise<{ ok: boolean; data?: CaseFile[]; error?: string }> {
    return tauriCallSafe('list_case_files', { caseId })
  }
}
