/**
 * 知识库插件
 * 
 * 将知识库功能封装为插件
 */

import type { CasyPlugin, CasyContext, CasyTool } from '../plugin/types'
import { defineTool } from '../plugin/defineTool'
import type { CreateKnowledgeInput } from '../../types/bindings'
import type { CommandMap } from '../../types/commandMap'
import type { KnowledgePatchInput } from '../../types/ipc'

type ListKnowledgeToolParams = CommandMap['list_knowledge']['params']

export class KnowledgePlugin implements CasyPlugin {
  name = 'knowledge'
  version = '1.0.0'
  description = '知识库模块'
  
  async install(ctx: CasyContext): Promise<void> {
    // 注册工具
    ctx.registerTool(this.createListKnowledgeTool(ctx))
    ctx.registerTool(this.createSearchKnowledgeTool(ctx))
    ctx.registerTool(this.createReadDocumentTool(ctx))
    ctx.registerTool(this.createListDocumentsTool(ctx))
    ctx.registerTool(this.createCreateKnowledgeTool(ctx))
    ctx.registerTool(this.createUpdateKnowledgeTool(ctx))
    ctx.registerTool(this.createDeleteKnowledgeTool(ctx))

    console.log('KnowledgePlugin installed')
  }

  async uninstall(ctx: CasyContext): Promise<void> {
    ctx.unregisterTool('list_knowledge')
    ctx.unregisterTool('search_knowledge')
    ctx.unregisterTool('read_document')
    ctx.unregisterTool('list_documents')
    ctx.unregisterTool('create_knowledge')
    ctx.unregisterTool('update_knowledge')
    ctx.unregisterTool('delete_knowledge')

    console.log('KnowledgePlugin uninstalled')
  }
  
  // ============================================================
  // 工具定义
  // ============================================================
  
  private createListKnowledgeTool(ctx: CasyContext): CasyTool {
    return defineTool<ListKnowledgeToolParams>({
      name: 'list_knowledge',
      policy: { write: false, level: 'L1' },
      description: '获取知识库列表，支持按职能分类筛选',
      category: 'knowledge',
      parameters: {
        type: 'object',
        properties: {
          filter: {
            type: 'object',
            properties: {
              category: { 
                type: 'string', 
                description: '职能分类：inspiration/method/reference/question/experience/log' 
              },
              search: { type: 'string', description: '搜索关键词' },
            },
          },
        },
      },
      execute: async (params) => {
        const result = await ctx.knowledge.list(params.filter || {})
        return result
      },
    })
  }
  
  private createSearchKnowledgeTool(ctx: CasyContext): CasyTool {
    return defineTool<{ query: string; limit?: number }>({
      name: 'search_knowledge',
      policy: { write: false, level: 'L1' },
      description: '搜索知识库（支持全文搜索和混合检索）。命中带 citation（knowledge:{id}），用 read_document 读正文。',
      category: 'knowledge',
      parameters: {
        type: 'object',
        properties: {
          query: { type: 'string', description: '搜索查询' },
          limit: { type: 'number', description: '返回数量限制，默认 10' },
        },
        required: ['query'],
      },
      execute: async (params) => {
        const limit = typeof params.limit === 'number' ? Math.max(1, Math.min(50, params.limit)) : 10
        const result = await ctx.knowledge.search(String(params.query || ''))
        if (!result.ok || !result.data) return result
        const rows = Array.isArray(result.data) ? result.data : []
        return {
          ok: true,
          data: rows.slice(0, limit).map((row: { id: string; title: string; content?: string | null; category?: string | null }) => ({
            id: row.id,
            title: row.title,
            category: row.category,
            snippet: (row.content || '').slice(0, 240),
            citation: `knowledge:${row.id}`,
            source: 'knowledge',
          })),
        }
      },
    })
  }

  private createReadDocumentTool(ctx: CasyContext): CasyTool {
    return defineTool<{ id: string; offset?: number; withBacklinks?: boolean }>({
      name: 'read_document',
      policy: { write: false, level: 'L1' },
      description: '读取知识/文书正文（分页 6000 字符）与关联；可选 backlinks 一圈。有 nextOffset 时继续读取。',
      category: 'knowledge',
      parameters: {
        type: 'object',
        properties: {
          id: { type: 'string', description: '知识 ID' },
          offset: { type: 'integer', description: '字符偏移，默认 0' },
          withBacklinks: { type: 'boolean', description: '是否附反链/相关笔记摘要' },
        },
        required: ['id'],
      },
      execute: async (params) => {
        const offset = Number(params.offset) || 0
        if (offset < 0) return { ok: false, error: 'offset 不能小于 0' }
        const result = await ctx.knowledge.getWithBlocks(String(params.id))
        if (!result.ok || !result.data) return result
        const { content, ...item } = result.data.item as { content?: string | null; [k: string]: unknown }
        const text = content || ''
        let backlinks: Array<{ id: string; title: string }> = []
        if (params.withBacklinks) {
          const g = await ctx.knowledge.graph(200)
          if (g.ok && g.data) {
            const nodes = g.data.nodes || []
            const edges = g.data.edges || []
            const titleOf = (id: string) => {
              const n = nodes.find((x: { id: string; name?: string }) => x.id === id) as { name?: string } | undefined
              return n?.name || id
            }
            backlinks = edges
              .filter((e: { source: string; target: string }) => e.source === String(params.id) || e.target === String(params.id))
              .map((e: { source: string; target: string }) => {
                const other = e.source === String(params.id) ? e.target : e.source
                return { id: other, title: titleOf(other) }
              })
              .slice(0, 12)
          }
        }
        return {
          ok: true,
          data: {
            item: { ...item, content: text.slice(offset, offset + 6000) },
            citation: `knowledge:${params.id}`,
            offset,
            totalCharacters: text.length,
            nextOffset: offset + 6000 < text.length ? offset + 6000 : null,
            backlinks,
          },
        }
      },
    })
  }

  private createListDocumentsTool(ctx: CasyContext): CasyTool {
    return defineTool<{ query?: string; limit?: number }>({
      name: 'list_documents',
      policy: { write: false, level: 'L1' },
      description: '列出知识笔记与文书草稿目录（不含正文）。正文用 read_document / get_draft。',
      category: 'knowledge',
      parameters: {
        type: 'object',
        properties: {
          query: { type: 'string', description: '标题关键词' },
          limit: { type: 'integer', description: '默认 30' },
        },
      },
      execute: async (params) => {
        const limit = typeof params.limit === 'number' ? Math.max(1, Math.min(100, params.limit)) : 30
        const q = String(params.query || '').trim().toLowerCase()
        const [notes, drafts] = await Promise.all([
          ctx.knowledge.list({}),
          ctx.docs.listDrafts().catch(() => ({ ok: false as const, data: [] as unknown[] })),
        ])
        const noteRows = ((notes.ok && notes.data) ? notes.data : []) as Array<Record<string, unknown>>
        const items = Array.isArray(noteRows) ? noteRows : []
        const draftRows = (((drafts as { ok: boolean; data?: unknown }).ok && (drafts as { data?: unknown[] }).data) || []) as Array<Record<string, unknown>>
        const knowledge = items
          .filter((r: Record<string, unknown>) => !q || String(r.title || '').toLowerCase().includes(q))
          .slice(0, limit)
          .map((r: Record<string, unknown>) => ({ kind: 'knowledge', id: r.id, title: r.title, category: r.category, citation: `knowledge:${r.id}` }))
        const docs = draftRows
          .filter((r: Record<string, unknown>) => !q || String(r.title || r.name || '').toLowerCase().includes(q))
          .slice(0, limit)
          .map((r: Record<string, unknown>) => ({ kind: 'draft', id: r.id, title: r.title || r.name, citation: `draft:${r.id}` }))
        return { ok: true, data: { knowledge, drafts: docs, counts: { knowledge: knowledge.length, drafts: docs.length } } }
      },
    })
  }

  private createCreateKnowledgeTool(ctx: CasyContext): CasyTool {
    return defineTool<Partial<CreateKnowledgeInput>>({
      name: 'create_knowledge',
      description: '创建知识条目',
      category: 'knowledge',
      parameters: {
        type: 'object',
        properties: {
          title: { type: 'string', description: '知识标题' },
          content: { type: 'string', description: '知识内容' },
          category: { 
            type: 'string', 
            description: '职能分类：inspiration/method/reference/question/experience/log' 
          },
          tags: { type: 'array', items: { type: 'string' }, description: '标签' },
        },
        required: ['title', 'content'],
      },
      execute: async (params) => {
        const result = await ctx.knowledge.create(params)
        return result
      },
    })
  }
  
  private createUpdateKnowledgeTool(ctx: CasyContext): CasyTool {
    return defineTool<{ id: string; data: KnowledgePatchInput }>({
      name: 'update_knowledge',
      description: '更新知识条目',
      category: 'knowledge',
      parameters: {
        type: 'object',
        properties: {
          id: { type: 'string', description: '知识ID' },
          data: { type: 'object', description: '更新数据' },
        },
        required: ['id', 'data'],
      },
      execute: async (params) => {
        const result = await ctx.knowledge.update(params.id, params.data)
        return result
      },
    })
  }
  
  private createDeleteKnowledgeTool(ctx: CasyContext): CasyTool {
    return defineTool<{ id: string }>({
      name: 'delete_knowledge',
      description: '删除知识条目',
      category: 'knowledge',
      parameters: {
        type: 'object',
        properties: {
          id: { type: 'string', description: '知识ID' },
        },
        required: ['id'],
      },
      // 需要 L2 确认（策略声明，由 executeTool 统一强制执行）
      policy: {
        level: 'L2',
        title: '确认删除知识',
        message: (p) => `确定要删除知识条目 ${String(p.id)} 吗？`,
      },
      execute: async (params) => {
        const result = await ctx.knowledge.remove(params.id)
        return result
      },
    })
  }
}
