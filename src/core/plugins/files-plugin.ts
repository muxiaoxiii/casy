/**
 * 文件管理插件
 * 
 * 将文件管理功能封装为插件
 */

import type { CasyPlugin, CasyContext, CasyTool } from '../plugin/types'
import { defineTool } from '../plugin/defineTool'

export class FilesPlugin implements CasyPlugin {
  name = 'files'
  version = '1.0.0'
  description = '文件管理模块'
  
  async install(ctx: CasyContext): Promise<void> {
    ctx.registerTool(this.createListCaseFilesTool(ctx))
    ctx.registerTool(this.createAddCaseFileTool(ctx))
    ctx.registerTool(this.createDeleteCaseFileTool(ctx))
    
    console.log('FilesPlugin installed')
  }
  
  async uninstall(ctx: CasyContext): Promise<void> {
    ctx.unregisterTool('list_case_files')
    ctx.unregisterTool('add_case_file')
    ctx.unregisterTool('delete_case_file')
    console.log('FilesPlugin uninstalled')
  }
  
  private createListCaseFilesTool(ctx: CasyContext): CasyTool {
    return defineTool<{ caseId: string }>({
      name: 'list_case_files',
      description: '获取案件文件列表',
      category: 'files',
      parameters: {
        type: 'object',
        properties: {
          caseId: { type: 'string', description: '案件ID' },
        },
        required: ['caseId'],
      },
      execute: async (params) => {
        const result = await ctx.files.list(params.caseId)
        return result
      },
    })
  }
  
  private createAddCaseFileTool(ctx: CasyContext): CasyTool {
    return defineTool<{ caseId: string; filePath: string; category?: string }>({
      name: 'add_case_file',
      description: '上传文件到案件',
      category: 'files',
      parameters: {
        type: 'object',
        properties: {
          caseId: { type: 'string', description: '案件ID' },
          filePath: { type: 'string', description: '文件路径' },
          category: { type: 'string', description: '文件分类' },
        },
        required: ['caseId', 'filePath'],
      },
      execute: async (params) => {
        const result = await ctx.files.add(
          params.caseId,
          params.filePath,
          params.category
        )
        return result
      },
    })
  }
  
  private createDeleteCaseFileTool(ctx: CasyContext): CasyTool {
    return defineTool<{ caseId: string; fileId: string }>({
      name: 'delete_case_file',
      description: '删除案件文件',
      category: 'files',
      parameters: {
        type: 'object',
        properties: {
          caseId: { type: 'string', description: '案件ID' },
          fileId: { type: 'string', description: '文件ID' },
        },
        required: ['caseId', 'fileId'],
      },
      // 需要 L2 确认（策略声明，由 executeTool 统一强制执行）
      policy: {
        level: 'L2',
        title: '确认删除文件',
        message: (p) => `确定要删除文件 ${String(p.fileId)} 吗？`,
      },
      execute: async (params) => {
        const result = await ctx.files.remove(params.fileId)
        return result
      },
    })
  }
}
