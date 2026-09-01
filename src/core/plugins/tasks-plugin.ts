/**
 * 任务管理插件
 * 
 * 将任务管理功能封装为插件，声明严格的 V5 权限策略 (A0-A3)
 */

import type { CasyPlugin, CasyContext, CasyTool } from '../plugin/types'
import { defineTool } from '../plugin/defineTool'

export class TasksPlugin implements CasyPlugin {
  name = 'tasks'
  version = '1.0.0'
  description = '任务管理模块'
  
  async install(ctx: CasyContext): Promise<void> {
    ctx.registerTool(this.createListTasksTool(ctx))
    ctx.registerTool(this.createCreateTaskTool(ctx))
    ctx.registerTool(this.createToggleTaskTool(ctx))
    ctx.registerTool(this.createUpdateTaskTool(ctx))
    ctx.registerTool(this.createDeleteTaskTool(ctx))
  }
  
  async uninstall(ctx: CasyContext): Promise<void> {
    ctx.unregisterTool('list_tasks')
    ctx.unregisterTool('create_task')
    ctx.unregisterTool('toggle_task')
    ctx.unregisterTool('update_task')
    ctx.unregisterTool('delete_task')
  }
  
  // ============================================================
  // 工具定义
  // ============================================================
  
  private createListTasksTool(ctx: CasyContext): CasyTool {
    return defineTool<{ filter?: Record<string, unknown> }>({
      name: 'list_tasks',
      description: '获取任务列表，支持按案件、类型、状态筛选',
      category: 'tasks',
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
              caseId: { type: 'string', description: '案件ID' },
              completed: { type: 'boolean', description: '是否完成' },
              taskType: { type: 'string', description: '任务类型' },
              startBucket: { type: 'string', description: '时间桶' },
              areaId: { type: 'string', description: '领域ID' },
            },
          },
        },
      },
      execute: async (params) => {
        return ctx.tasks.list(params.filter || {})
      },
    })
  }
  
  private createCreateTaskTool(ctx: CasyContext): CasyTool {
    return defineTool<Record<string, unknown>>({
      name: 'create_task',
      description: '创建新任务',
      category: 'tasks',
      policy: {
        write: true,
        level: 'L2',
        title: 'AI 创建任务确认',
        message: (p) => `确定由 AI 创建任务「${p.taskName || '未命名任务'}」吗？`,
      },
      parameters: {
        type: 'object',
        properties: {
          taskName: { type: 'string', description: '任务名称' },
          caseId: { type: 'string', description: '关联案件ID' },
          taskType: { type: 'string', description: '任务类型：action/waiting/delegated/someday' },
          startDate: { type: 'string', description: '开始日期' },
          dueDate: { type: 'string', description: '截止日期' },
          priority: { type: 'string', description: '优先级' },
          context: { type: 'string', description: '上下文标签' },
        },
        required: ['taskName'],
      },
      execute: async (params) => {
        return ctx.tasks.create(params)
      },
    })
  }
  
  private createToggleTaskTool(ctx: CasyContext): CasyTool {
    return defineTool<{ id: string }>({
      name: 'toggle_task',
      description: '切换任务完成状态',
      category: 'tasks',
      policy: {
        write: true,
        level: 'L2',
        title: 'AI 变更任务状态确认',
        message: (p) => `确定由 AI 切换任务 #${p.id} 的完成状态吗？`,
      },
      parameters: {
        type: 'object',
        properties: {
          id: { type: 'string', description: '任务ID' },
        },
        required: ['id'],
      },
      execute: async (params) => {
        return ctx.tasks.toggle(params.id)
      },
    })
  }

  private createUpdateTaskTool(ctx: CasyContext): CasyTool {
    return defineTool<{ id: string; data: Record<string, unknown> }>({
      name: 'update_task',
      description: '更新任务信息',
      category: 'tasks',
      policy: {
        write: true,
        level: 'L2',
        title: 'AI 更新任务确认',
        message: (p) => `确定由 AI 修改任务 #${p.id} 的字段吗？`,
      },
      parameters: {
        type: 'object',
        properties: {
          id: { type: 'string', description: '任务ID' },
          data: { type: 'object', description: '更新数据' },
        },
        required: ['id', 'data'],
      },
      execute: async (params) => {
        return ctx.tasks.update({
          ...params.data,
          id: params.id,
        })
      },
    })
  }
  
  private createDeleteTaskTool(ctx: CasyContext): CasyTool {
    return defineTool<{ id: string }>({
      name: 'delete_task',
      description: '删除任务',
      category: 'tasks',
      policy: {
        write: true,
        level: 'L3',
        title: 'AI 删除任务二次确认',
        message: (p) => `⚠️ 危险操作：确定由 AI 删除任务 #${p.id} 吗？此操作不可逆。`,
      },
      parameters: {
        type: 'object',
        properties: {
          id: { type: 'string', description: '任务ID' },
        },
        required: ['id'],
      },
      execute: async (params) => {
        return ctx.tasks.remove(params.id)
      },
    })
  }
}
