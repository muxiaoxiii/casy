/**
 * 收件箱插件
 * 
 * 将收件箱功能封装为插件
 */

import type { CasyPlugin, CasyContext, CasyTool } from '../plugin/types'

export class InboxPlugin implements CasyPlugin {
  name = 'inbox'
  version = '1.0.0'
  description = '收件箱模块'
  
  async install(ctx: CasyContext): Promise<void> {
    ctx.registerTool(this.createListInboxItemsTool(ctx))
    ctx.registerTool(this.createAddInboxItemTool(ctx))
    ctx.registerTool(this.createProcessInboxItemTool(ctx))
    ctx.registerTool(this.createFileInboxItemTool(ctx))
    ctx.registerTool(this.createDismissInboxItemTool(ctx))
    
    console.log('InboxPlugin installed')
  }
  
  async uninstall(ctx: CasyContext): Promise<void> {
    ctx.unregisterTool('list_inbox_items')
    ctx.unregisterTool('add_inbox_item')
    ctx.unregisterTool('process_inbox_item')
    ctx.unregisterTool('file_inbox_item')
    ctx.unregisterTool('dismiss_inbox_item')
    console.log('InboxPlugin uninstalled')
  }
  
  private createListInboxItemsTool(ctx: CasyContext): CasyTool {
    return {
      name: 'list_inbox_items',
      description: '获取收件箱列表（待处理/已归档/已忽略）',
      category: 'inbox',
      parameters: {
        type: 'object',
        properties: {
          status: { 
            type: 'string', 
            enum: ['pending', 'processed', 'filed', 'dismissed', 'all'],
            description: '筛选状态' 
          },
        },
      },
      execute: async (params) => {
        const { tauriCallSafe } = await import('../../core/tauriBridge')
        const result = await tauriCallSafe('list_inbox_items', { 
          status: params.status || 'all' 
        })
        return result
      },
    }
  }
  
  private createAddInboxItemTool(ctx: CasyContext): CasyTool {
    return {
      name: 'add_inbox_item',
      description: '添加收件箱条目（文本/文件/邮件）',
      category: 'inbox',
      parameters: {
        type: 'object',
        properties: {
          content: { type: 'string', description: '内容文本' },
          sourceType: { 
            type: 'string', 
            enum: ['text', 'file', 'email', 'clipboard'],
            description: '来源类型' 
          },
          sourcePath: { type: 'string', description: '文件路径（file类型）' },
        },
        required: ['content'],
      },
      execute: async (params) => {
        const { tauriCallSafe } = await import('../../core/tauriBridge')
        const result = await tauriCallSafe('add_inbox_item', {
          content_text: params.content,
          source_type: params.sourceType || 'text',
          source_path: params.sourcePath,
        })
        return result
      },
    }
  }
  
  private createProcessInboxItemTool(ctx: CasyContext): CasyTool {
    return {
      name: 'process_inbox_item',
      description: '处理收件箱条目（AI 分类 + 案件匹配）',
      category: 'inbox',
      parameters: {
        type: 'object',
        properties: {
          id: { type: 'string', description: '条目ID' },
        },
        required: ['id'],
      },
      execute: async (params) => {
        const { tauriCallSafe } = await import('../../core/tauriBridge')
        const result = await tauriCallSafe('process_inbox_item', { id: params.id })
        return result
      },
    }
  }
  
  private createFileInboxItemTool(ctx: CasyContext): CasyTool {
    return {
      name: 'file_inbox_item',
      description: '归档收件箱条目到案件',
      category: 'inbox',
      parameters: {
        type: 'object',
        properties: {
          id: { type: 'string', description: '条目ID' },
          caseId: { type: 'string', description: '目标案件ID' },
          category: { type: 'string', description: '文件分类' },
        },
        required: ['id', 'caseId'],
      },
      execute: async (params) => {
        const { tauriCallSafe } = await import('../../core/tauriBridge')
        const result = await tauriCallSafe('file_inbox_item', {
          id: params.id,
          case_id: params.caseId,
          category: params.category,
        })
        return result
      },
    }
  }
  
  private createDismissInboxItemTool(ctx: CasyContext): CasyTool {
    return {
      name: 'dismiss_inbox_item',
      description: '忽略收件箱条目',
      category: 'inbox',
      parameters: {
        type: 'object',
        properties: {
          id: { type: 'string', description: '条目ID' },
        },
        required: ['id'],
      },
      execute: async (params) => {
        const { tauriCallSafe } = await import('../../core/tauriBridge')
        const result = await tauriCallSafe('dismiss_inbox_item', { id: params.id })
        return result
      },
    }
  }
}
