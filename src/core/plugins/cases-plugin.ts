/**
 * 案件管理插件
 * 
 * 将案件管理功能封装为插件
 */

import type { CasyPlugin, CasyContext, CasyTool } from '../plugin/types'

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
    return {
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
        const { tauriCallSafe } = await import('../../core/tauriBridge')
        const result = await tauriCallSafe('list_cases', { filter: params.filter || {} })
        return result
      },
    }
  }
  
  private createGetCaseTool(ctx: CasyContext): CasyTool {
    return {
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
        const { tauriCallSafe } = await import('../../core/tauriBridge')
        const result = await tauriCallSafe('get_case', { id: params.id })
        return result
      },
    }
  }
  
  private createCreateCaseTool(ctx: CasyContext): CasyTool {
    return {
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
        const { tauriCallSafe } = await import('../../core/tauriBridge')
        const result = await tauriCallSafe('create_case', { data: params })
        
        // 触发事件
        if (result.ok) {
          ctx.emit('case:created', { id: result.data?.id, ...params })
        }
        
        return result
      },
    }
  }
  
  private createUpdateCaseTool(ctx: CasyContext): CasyTool {
    return {
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
        const { tauriCallSafe } = await import('../../core/tauriBridge')
        const result = await tauriCallSafe('update_case', { id: params.id, data: params.data })
        
        // 触发事件
        if (result.ok) {
          ctx.emit('case:updated', { id: params.id, ...params.data })
        }
        
        return result
      },
    }
  }
  
  private createDeleteCaseTool(ctx: CasyContext): CasyTool {
    return {
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
      execute: async (params) => {
        // 需要 L3 确认
        const confirmLevel = ctx.calculateEffectiveLevel({
          isExternalWrite: true,
        })
        
        if (confirmLevel === 'L3') {
          const confirmed = await ctx.requestConfirm({
            level: 'L3',
            title: '确认删除案件',
            message: `确定要删除案件 ${params.id} 吗？此操作不可撤销。`,
            onConfirm: async () => {},
          })
          
          if (!confirmed) {
            return { ok: false, error: '用户取消操作' }
          }
        }
        
        const { tauriCallSafe } = await import('../../core/tauriBridge')
        const result = await tauriCallSafe('delete_case', { id: params.id })
        
        // 触发事件
        if (result.ok) {
          ctx.emit('case:deleted', { id: params.id })
        }
        
        return result
      },
    }
  }
  
  private createSearchCasesTool(ctx: CasyContext): CasyTool {
    return {
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
        const { tauriCallSafe } = await import('../../core/tauriBridge')
        const result = await tauriCallSafe('search_cases', { query: params.keyword })
        return result
      },
    }
  }
}
