/**
 * 案件管理插件
 * 
 * 将案件管理功能封装为插件
 */

import type { CasyPlugin, CasyContext, CasyTool } from '../plugin/types'
import { defineTool } from '../plugin/defineTool'

export class CasesPlugin implements CasyPlugin {
  name = 'cases'
  version = '1.0.0'
  description = '案件管理模块'
  
  async install(ctx: CasyContext): Promise<void> {
    // 注册工具
    ctx.registerTool(this.createListCasesTool(ctx))
    ctx.registerTool(this.createGetCaseTool(ctx))
    ctx.registerTool(this.createCreateCaseTool(ctx))
    ctx.registerTool(this.createUpdateCaseTool(ctx))
    ctx.registerTool(this.createDeleteCaseTool(ctx))
    ctx.registerTool(this.createSearchCasesTool(ctx))
    
    console.log('CasesPlugin installed')
  }
  
  async uninstall(ctx: CasyContext): Promise<void> {
    ctx.unregisterTool('list_cases')
    ctx.unregisterTool('get_case')
    ctx.unregisterTool('create_case')
    ctx.unregisterTool('update_case')
    ctx.unregisterTool('delete_case')
    ctx.unregisterTool('search_cases')
    
    console.log('CasesPlugin uninstalled')
  }
  
  // ============================================================
  // 工具定义
  // ============================================================
  
  private createListCasesTool(ctx: CasyContext): CasyTool {
    return defineTool<{ filter?: Record<string, unknown> }>({
      name: 'list_cases',
      description: '获取案件列表，支持按轨道、状态、客户筛选',
      category: 'cases',
      parameters: {
        type: 'object',
        properties: {
          filter: {
            type: 'object',
            properties: {
              track: { type: 'string', description: '案件轨道' },
              status: { type: 'string', description: '案件状态' },
              clientId: { type: 'string', description: '客户ID' },
              search: { type: 'string', description: '搜索关键词' },
            },
          },
        },
      },
      execute: async (params) => {
        // 调用 Tauri 命令
        const result = await ctx.cases.list(params.filter || {})
        return result
      },
    })
  }
  
  private createGetCaseTool(ctx: CasyContext): CasyTool {
    return defineTool<{ id: string }>({
      name: 'get_case',
      description: '获取单个案件详情',
      category: 'cases',
      parameters: {
        type: 'object',
        properties: {
          id: { type: 'string', description: '案件ID' },
        },
        required: ['id'],
      },
      execute: async (params) => {
        const result = await ctx.cases.get(params.id)
        return result
      },
    })
  }
  
  private createCreateCaseTool(ctx: CasyContext): CasyTool {
    return defineTool<Record<string, unknown>>({
      name: 'create_case',
      description: '创建新案件',
      category: 'cases',
      parameters: {
        type: 'object',
        properties: {
          caseName: { type: 'string', description: '案件名称' },
          clientName: { type: 'string', description: '客户名称' },
          track: { type: 'string', description: '案件轨道' },
          caseNo: { type: 'string', description: '案号' },
          court: { type: 'string', description: '法院' },
        },
        required: ['caseName', 'clientName'],
      },
      execute: async (params) => {
        // 领域事件由 cases service 层统一发出（K-3①：人与 AI 同一事件流）
        return ctx.cases.create(params)
      },
    })
  }
  
  private createUpdateCaseTool(ctx: CasyContext): CasyTool {
    return defineTool<{ id: string; data: Record<string, unknown> }>({
      name: 'update_case',
      description: '更新案件信息',
      category: 'cases',
      parameters: {
        type: 'object',
        properties: {
          id: { type: 'string', description: '案件ID' },
          data: { type: 'object', description: '更新数据' },
        },
        required: ['id', 'data'],
      },
      execute: async (params) => {
        // 领域事件由 cases service 层统一发出（K-3①）
        return ctx.cases.update(params.id, params.data)
      },
    })
  }
  
  private createDeleteCaseTool(ctx: CasyContext): CasyTool {
    return defineTool<{ id: string }>({
      name: 'delete_case',
      description: '删除案件',
      category: 'cases',
      parameters: {
        type: 'object',
        properties: {
          id: { type: 'string', description: '案件ID' },
        },
        required: ['id'],
      },
      // 需要 L3 确认（策略声明，由 executeTool 统一强制执行）
      policy: {
        write: true,
        level: 'L3',
        title: '确认删除案件',
        message: (p) => `确定要删除案件 ${String(p.id)} 吗？此操作不可撤销。`,
      },
      execute: async (params) => {
        // 领域事件由 cases service 层统一发出（K-3①）
        return ctx.cases.remove(params.id)
      },
    })
  }
  
  private createSearchCasesTool(ctx: CasyContext): CasyTool {
    return defineTool<{ keyword: string }>({
      name: 'search_cases',
      description: '搜索案件',
      category: 'cases',
      parameters: {
        type: 'object',
        properties: {
          keyword: { type: 'string', description: '搜索关键词' },
        },
        required: ['keyword'],
      },
      execute: async (params) => {
        const result = await ctx.cases.search(params.keyword)
        return result
      },
    })
  }
}
