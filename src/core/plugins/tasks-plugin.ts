/**
 * 任务管理插件
 * 
 * 将任务管理功能封装为插件
 */

import type { CasyPlugin, CasyContext, CasyTool } from '../plugin/types'

export class TasksPlugin implements CasyPlugin {
  name = 'tasks'
  version = '1.0.0'
  description = '任务管理模块'
  
  async install(ctx: CasyContext): Promise<void> {
    // 注册工具
    ctx.registerTool(this.createListTasksTool(ctx))
    ctx.registerTool(this.createCreateTaskTool(ctx))
    ctx.registerTool(this.createToggleTaskTool(ctx))
    ctx.registerTool(this.createUpdateTaskTool(ctx))
    ctx.registerTool(this.createDeleteTaskTool(ctx))
    
    console.log('TasksPlugin installed')
  }
  
  async uninstall(ctx: CasyContext): Promise<void> {
    ctx.unregisterTool('list_tasks')
    ctx.unregisterTool('create_task')
    ctx.unregisterTool('toggle_task')
    ctx.unregisterTool('update_task')
    ctx.unregisterTool('delete_task')
    
    console.log('TasksPlugin uninstalled')
  }
  
  // ============================================================
  // 工具定义
  // ============================================================
  
  private createListTasksTool(ctx: CasyContext): CasyTool {
    return {
      name: 'list_tasks',
      description: '获取任务列表，支持按案件、类型、状态筛选',
      category: 'tasks',
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
        const result = await ctx.tasks.list(params.filter || {})
        return result
      },
    }
  }
  
  private createCreateTaskTool(ctx: CasyContext): CasyTool {
    return {
      name: 'create_task',
      description: '创建新任务',
      category: 'tasks',
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
        const result = await ctx.tasks.create(params)
        
        // 触发事件
        if (result.ok) {
          ctx.emit('task:created', { id: result.data?.id, ...params })
        }
        
        return result
      },
    }
  }
  
  private createToggleTaskTool(ctx: CasyContext): CasyTool {
    return {
      name: 'toggle_task',
      description: '切换任务完成状态',
      category: 'tasks',
      parameters: {
        type: 'object',
        properties: {
          id: { type: 'string', description: '任务ID' },
        },
        required: ['id'],
      },
      execute: async (params) => {
        const result = await ctx.tasks.toggle(params.id)
        
        // 触发事件
        if (result.ok) {
          ctx.emit('task:completed', { id: params.id })
        }
        
        return result
      },
    }
  }
  
  private createUpdateTaskTool(ctx: CasyContext): CasyTool {
    return {
      name: 'update_task',
      description: '更新任务信息',
      category: 'tasks',
      parameters: {
        type: 'object',
        properties: {
          id: { type: 'string', description: '任务ID' },
          data: { type: 'object', description: '更新数据' },
        },
        required: ['id', 'data'],
      },
      execute: async (params) => {
        const result = await ctx.tasks.update({ ...params.data, id: params.id })
        return result
      },
    }
  }
  
  private createDeleteTaskTool(ctx: CasyContext): CasyTool {
    return {
      name: 'delete_task',
      description: '删除任务',
      category: 'tasks',
      parameters: {
        type: 'object',
        properties: {
          id: { type: 'string', description: '任务ID' },
        },
        required: ['id'],
      },
      execute: async (params) => {
        const result = await ctx.tasks.remove(params.id)
        return result
      },
    }
  }
}
