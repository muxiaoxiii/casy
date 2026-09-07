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
    ctx.registerTool(this.createCreateKnowledgeTool(ctx))
    ctx.registerTool(this.createUpdateKnowledgeTool(ctx))
    ctx.registerTool(this.createDeleteKnowledgeTool(ctx))
    
    console.log('KnowledgePlugin installed')
  }
  
  async uninstall(ctx: CasyContext): Promise<void> {
    ctx.unregisterTool('list_knowledge')
    ctx.unregisterTool('search_knowledge')
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
      description: '搜索知识库（支持全文搜索和混合检索）',
      category: 'knowledge',
      parameters: {
        type: 'object',
        properties: {
          query: { type: 'string', description: '搜索查询' },
          limit: { type: 'number', description: '返回数量限制' },
        },
        required: ['query'],
      },
      execute: async (params) => {
        const result = await ctx.knowledge.search(params.query)
        return result
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
