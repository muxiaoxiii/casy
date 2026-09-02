/**
 * 案件管理插件
 * 
 * 将案件管理功能封装为插件，严格声明 V5 策略
 */

import type { CasyPlugin, CasyContext, CasyTool } from '../plugin/types'
import { defineTool } from '../plugin/defineTool'

export class CasesPlugin implements CasyPlugin {
  name = 'cases'
  version = '1.0.0'
  description = '案件管理模块'
  
  async install(ctx: CasyContext): Promise<void> {
    ctx.registerTool(this.createListCasesTool(ctx))
    ctx.registerTool(this.createGetCaseTool(ctx))
    ctx.registerTool(this.createCreateCaseTool(ctx))
    ctx.registerTool(this.createUpdateCaseTool(ctx))
    ctx.registerTool(this.createDeleteCaseTool(ctx))
    ctx.registerTool(this.createSearchCasesTool(ctx))
  }
  
  async uninstall(ctx: CasyContext): Promise<void> {
    ctx.unregisterTool('list_cases')
    ctx.unregisterTool('get_case')
    ctx.unregisterTool('create_case')
    ctx.unregisterTool('update_case')
    ctx.unregisterTool('delete_case')
    ctx.unregisterTool('search_cases')
  }
  
  // ============================================================
  // 工具定义
  // ============================================================
  
  private createListCasesTool(ctx: CasyContext): CasyTool {
    return defineTool<{ filter?: Record<string, unknown> }>({
      name: 'list_cases',
      description: '获取案件列表，支持按轨道、状态、客户筛选',
      category: 'cases',
      policy: {
        write: false,
        level: 'L1',
      },
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
        return ctx.cases.list(params.filter || {})
      },
    })
  }
  
  private createGetCaseTool(ctx: CasyContext): CasyTool {
    return defineTool<{ id: string }>({
      name: 'get_case',
      description: '获取单个案件详情',
      category: 'cases',
      policy: {
        write: false,
        level: 'L1',
      },
      parameters: {
        type: 'object',
        properties: {
          id: { type: 'string', description: '案件ID' },
        },
        required: ['id'],
      },
      execute: async (params) => {
        return ctx.cases.get(params.id)
      },
    })
  }
  
  private createCreateCaseTool(ctx: CasyContext): CasyTool {
    return defineTool<Record<string, unknown>>({
      name: 'create_case',
      description: '创建新案件',
      category: 'cases',
      policy: {
        write: true,
        level: 'L2',
        title: 'AI 创建案件确认',
        message: (p) => `确定由 AI 创建案件「${p.caseName || '未命名案件'}」吗？`,
      },
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
        return ctx.cases.create(params)
      },
    })
  }
  
  private createUpdateCaseTool(ctx: CasyContext): CasyTool {
    return defineTool<{ id: string; data: Record<string, unknown> }>({
      name: 'update_case',
      description: '更新案件信息',
      category: 'cases',
      policy: {
        write: true,
        level: 'L2',
        title: 'AI 更新案件确认',
        message: (p) => `确定由 AI 修改案件 #${p.id} 吗？`,
      },
      parameters: {
        type: 'object',
        properties: {
          id: { type: 'string', description: '案件ID' },
          data: { type: 'object', description: '更新数据' },
        },
        required: ['id', 'data'],
      },
      execute: async (params) => {
        // P0-2：透传提案批准后的 origin/proposalToken（随 data 进入服务端网关校验）
        const p = params as { id: string; data: Record<string, unknown>; origin?: string; proposalToken?: string }
        return ctx.cases.update(p.id, p.data, { origin: p.origin, proposalToken: p.proposalToken })
      },
    })
  }
  
  private createDeleteCaseTool(ctx: CasyContext): CasyTool {
    return defineTool<{ id: string }>({
      name: 'delete_case',
      description: '删除案件',
      category: 'cases',
      policy: {
        write: true,
        level: 'L3',
        title: 'AI 删除案件二次确认',
        message: (p) => `⚠️ 危险操作：确定由 AI 删除案件 #${p.id} 吗？此操作不可逆。`,
      },
      parameters: {
        type: 'object',
        properties: {
          id: { type: 'string', description: '案件ID' },
        },
        required: ['id'],
      },
      execute: async (params) => {
        const p = params as { id: string; origin?: string; proposalToken?: string }
        return ctx.cases.remove(p.id, { origin: p.origin, proposalToken: p.proposalToken })
      },
    })
  }
  
  private createSearchCasesTool(ctx: CasyContext): CasyTool {
    return defineTool<{ keyword: string }>({
      name: 'search_cases',
      description: '搜索案件',
      category: 'cases',
      policy: {
        write: false,
        level: 'L1',
      },
      parameters: {
        type: 'object',
        properties: {
          keyword: { type: 'string', description: '搜索关键词' },
        },
        required: ['keyword'],
      },
      execute: async (params) => {
        return ctx.cases.search(params.keyword)
      },
    })
  }
}
