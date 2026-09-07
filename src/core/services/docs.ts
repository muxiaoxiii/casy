import { Service } from '../plugin/types'
import { tauriCallSafe } from '../tauriBridge'
// Docsy 模板/渲染/导出契约来自 bindings（specta 生成物，禁止手改）。
// 此前 docs.ts 手写的 DocsyTemplateListResponse / DocsyRenderResult / DocsyExportResult
// 与生成类型重复，且已确认无外部 import 引用，随本次收口统一到 bindings（删除重复）。
import type {
  Draft,
  ExportResponse,
  RenderResponse,
  TemplateListResponse,
} from '../../types/bindings'

// ============================================================
// Docs 域类型
// ============================================================

/** Tiptap/ProseMirror JSON 文档；节点由后端白名单解析，未知节点会拒绝导出。 */
export interface RichTextDocument {
  type: string
  attrs?: Record<string, unknown>
  content?: RichTextDocument[]
  text?: string
  marks?: Array<{ type: string; attrs?: Record<string, unknown> }>
}

/**
 * 文书服务：ctx.docs —— docs 模块数据通路
 *
 * 覆盖草稿（list/get/create/update/delete_draft）与 Docsy 模板
 * （list_docsy_templates / render_docsy_template / export_docx）。
 * 服务方法名按业务语义命名，内部封装 tauriCallSafe
 * （参数 camelCase → 后端 snake_case，Tauri 自动转换）。
 */
export class DocsService extends Service {
  static inject: string[] = []

  // ── 草稿 ──

  /** 列出所有草稿（按 updated_at 倒序） */
  async listDrafts(): Promise<{ ok: boolean; data?: Draft[]; error?: string }> {
    return tauriCallSafe('list_drafts', {})
  }

  /** 获取单个草稿 */
  async getDraft(id: string): Promise<{ ok: boolean; data?: Draft; error?: string }> {
    return tauriCallSafe('get_draft', { id })
  }

  /** 新建草稿 */
  async createDraft(data: {
    title: string
    content?: string | null
    caseId?: string | null
    templatePath?: string | null
  }): Promise<{ ok: boolean; data?: Draft; error?: string }> {
    return tauriCallSafe('create_draft', {
      title: data.title,
      content: data.content ?? null,
      caseId: data.caseId ?? null,
      templatePath: data.templatePath ?? null,
    })
  }

  /** 更新草稿（title/content/status/caseId 均可选，缺省保留原值） */
  async updateDraft(
    id: string,
    data: {
      title?: string
      content?: string | null
      status?: string
      caseId?: string | null
      expectedVersion?: number
    } = {}
  ): Promise<{ ok: boolean; data?: Draft; error?: string }> {
    return tauriCallSafe('update_draft', { id, ...data })
  }

  /** 删除草稿 */
  async deleteDraft(id: string): Promise<{ ok: boolean; data?: boolean; error?: string }> {
    return tauriCallSafe('delete_draft', { id })
  }

  // ── Docsy 模板 ──

  /** 列出所有可用 Docsy 模板 */
  async listTemplates(): Promise<{ ok: boolean; data?: TemplateListResponse; error?: string }> {
    return tauriCallSafe('list_docsy_templates', {})
  }

  /** 渲染模板（用案件数据填充占位符，返回 html/text/缺失字段） */
  async renderTemplate(templateId: string, caseId: string): Promise<{ ok: boolean; data?: RenderResponse; error?: string }> {
    return tauriCallSafe('render_docsy_template', { templateId, caseId })
  }

  /** 导出 DOCX（可指定输出路径） */
  async exportDocx(
    templateId: string,
    caseId: string,
    outputPath?: string | null
  ): Promise<{ ok: boolean; data?: ExportResponse; error?: string }> {
    return tauriCallSafe('export_docx', {
      templateId,
      caseId,
      outputPath: outputPath ?? null,
    })
  }

  /** 将所见即所得编辑器的结构化文档交给 Rust 原生导出。 */
  async exportEditedDocx(data: {
    document: RichTextDocument
    title: string
    outputPath?: string | null
  }): Promise<{ ok: boolean; data?: ExportResponse; error?: string }> {
    return tauriCallSafe('export_edited_docx', {
      document: data.document,
      title: data.title,
      outputPath: data.outputPath ?? null,
    })
  }
}
