import { Service } from '../plugin/types'
import { tauriCallSafe } from '../tauriBridge'

// ============================================================
// Docs 域类型（对齐后端 drafts.rs / docsy_engine，snake_case → camelCase）
// ============================================================

/** 草稿实体（后端 Draft） */
export interface Draft {
  id: string
  caseId: string | null
  title: string
  content: string | null
  templatePath: string | null
  status: string
  version: number
  createdAt: string
  updatedAt: string
}

/** Docsy 模板字段（后端 TemplateField） */
export interface DocsyTemplateField {
  name: string
  fieldType: string
  defaultValue: string | null
  required: boolean
}

/** Docsy 模板（后端 DocsyTemplate） */
export interface DocsyTemplate {
  id: string
  name: string
  path: string
  category: string
  fieldCount: number
  fields: DocsyTemplateField[]
  description: string
}

/** 模板列表响应（后端 TemplateListResponse） */
export interface DocsyTemplateListResponse {
  templates: DocsyTemplate[]
  total: number
}

/** 渲染结果（后端 RenderResponse） */
export interface DocsyRenderResult {
  html: string
  text: string
  usedFields: Record<string, string>
  missingFields: string[]
}

/** 导出结果（后端 ExportResponse） */
export interface DocsyExportResult {
  outputPath: string
  fileSize: number
  exportedAt: string
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
    return tauriCallSafe<Draft[]>('list_drafts', {})
  }

  /** 获取单个草稿 */
  async getDraft(id: string): Promise<{ ok: boolean; data?: Draft; error?: string }> {
    return tauriCallSafe<Draft>('get_draft', { id })
  }

  /** 新建草稿 */
  async createDraft(data: {
    title: string
    content?: string | null
    caseId?: string | null
    templatePath?: string | null
  }): Promise<{ ok: boolean; data?: Draft; error?: string }> {
    return tauriCallSafe<Draft>('create_draft', {
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
    } = {}
  ): Promise<{ ok: boolean; data?: Draft; error?: string }> {
    return tauriCallSafe<Draft>('update_draft', { id, ...data })
  }

  /** 删除草稿 */
  async deleteDraft(id: string): Promise<{ ok: boolean; data?: boolean; error?: string }> {
    return tauriCallSafe<boolean>('delete_draft', { id })
  }

  // ── Docsy 模板 ──

  /** 列出所有可用 Docsy 模板 */
  async listTemplates(): Promise<{ ok: boolean; data?: DocsyTemplateListResponse; error?: string }> {
    return tauriCallSafe<DocsyTemplateListResponse>('list_docsy_templates', {})
  }

  /** 渲染模板（用案件数据填充占位符，返回 html/text/缺失字段） */
  async renderTemplate(templateId: string, caseId: string): Promise<{ ok: boolean; data?: DocsyRenderResult; error?: string }> {
    return tauriCallSafe<DocsyRenderResult>('render_docsy_template', { templateId, caseId })
  }

  /** 导出 DOCX（可指定输出路径） */
  async exportDocx(
    templateId: string,
    caseId: string,
    outputPath?: string | null
  ): Promise<{ ok: boolean; data?: DocsyExportResult; error?: string }> {
    return tauriCallSafe<DocsyExportResult>('export_docx', {
      templateId,
      caseId,
      outputPath: outputPath ?? null,
    })
  }
}
