import { Service } from '../plugin/types'
import { tauriCallSafe } from '../tauriBridge'
import type { CreateKnowledgeInput, KnowledgeDocumentSourceDto, PageIndexImportResultDto } from '../../types/bindings'
import type { CommandMap } from '../../types/commandMap'
import type { KnowledgePatchInput } from '../../types/ipc'

type KnowledgeFilter = NonNullable<CommandMap['list_knowledge']['params']['filter']>
type KnowledgeList = CommandMap['list_knowledge']['result']
type KnowledgeSearchResult = CommandMap['search_knowledge']['result']
type GlobalSearchResult = CommandMap['global_search']['result']
type KnowledgeWithBlocks = CommandMap['get_knowledge_with_blocks']['result']
type KnowledgeVersion = CommandMap['list_knowledge_versions']['result'][number]
type KnowledgeDiffCurrent = CommandMap['diff_knowledge_with_current']['result']
type KnowledgeDiffVersions = CommandMap['diff_knowledge_versions']['result']
type KnowledgeGraph = CommandMap['get_knowledge_graph']['result']

export interface KnowledgeExportResult {
  outputPath: string
  fileSize: number
  exportedAt: string
}

/** 知识库服务：ctx.knowledge */
export class KnowledgeService extends Service {
  static inject: string[] = []

  async list(filter: KnowledgeFilter = {}): Promise<{ ok: boolean; data?: KnowledgeList; error?: string }> {
    return tauriCallSafe('list_knowledge', { filter })
  }

  async search(query: string): Promise<{ ok: boolean; data?: KnowledgeSearchResult; error?: string }> {
    return tauriCallSafe('search_knowledge', { query })
  }

  async globalSearch(query: string): Promise<{ ok: boolean; data?: GlobalSearchResult; error?: string }> {
    return tauriCallSafe('global_search', { query })
  }

  async searchIndex(query: string, useSemantic: boolean) {
    return tauriCallSafe('search_knowledge_index', { query, useSemantic })
  }

  async create(data: Partial<CreateKnowledgeInput>): Promise<{ ok: boolean; data?: string; error?: string }> {
    return tauriCallSafe('create_knowledge', { data })
  }

  async update(id: string, data: KnowledgePatchInput): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe('update_knowledge', { id, data })
  }

  async remove(id: string): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe('delete_knowledge', { id })
  }

  /** 获取条目及其块树（§8.2 块级引用；后端 get_knowledge_with_blocks 返回 { item, blocks }） */
  async getWithBlocks(id: string): Promise<{ ok: boolean; data?: KnowledgeWithBlocks; error?: string }> {
    return tauriCallSafe('get_knowledge_with_blocks', { id })
  }

  /** 版本历史 */
  async versions(itemId: string): Promise<{ ok: boolean; data?: KnowledgeVersion[]; error?: string }> {
    return tauriCallSafe('list_knowledge_versions', { itemId })
  }

  /** 版本与当前内容差异 */
  async diffWithCurrent(versionId: string, itemId: string): Promise<{ ok: boolean; data?: KnowledgeDiffCurrent; error?: string }> {
    return tauriCallSafe('diff_knowledge_with_current', { versionId, itemId })
  }

  /** 两个版本差异 */
  async diffVersions(versionId1: string, versionId2: string): Promise<{ ok: boolean; data?: KnowledgeDiffVersions; error?: string }> {
    return tauriCallSafe('diff_knowledge_versions', { versionId1, versionId2 })
  }

  async restoreVersion(itemId: string, versionId: string): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe('restore_knowledge_version', { itemId, versionId })
  }

  async documentSources(): Promise<{ ok: boolean; data?: KnowledgeDocumentSourceDto[]; error?: string }> {
    return tauriCallSafe('list_knowledge_document_sources', {})
  }

  async importPageIndex(fileId: string): Promise<{ ok: boolean; data?: PageIndexImportResultDto; error?: string }> {
    return tauriCallSafe('import_pageindex_to_knowledge', { fileId })
  }

  /** 导出单篇 Markdown；调用前必须先保存当前编辑态。 */
  async exportMarkdown(itemId: string, outputPath: string): Promise<{ ok: boolean; data?: KnowledgeExportResult; error?: string }> {
    return tauriCallSafe('export_knowledge_markdown', { itemId, outputPath })
  }

  /** 知识图谱数据（知识 ↔ 案件 ↔ 任务；后端 get_knowledge_graph 返回 { nodes, edges }） */
  async graph(limit = 100): Promise<{ ok: boolean; data?: KnowledgeGraph; error?: string }> {
    return tauriCallSafe('get_knowledge_graph', { limit })
  }
}
