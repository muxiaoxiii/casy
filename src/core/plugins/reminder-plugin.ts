/**
 * 提醒插件
 * 
 * 将提醒功能封装为插件
 */

import type { CasyPlugin, CasyContext, CasyTool } from '../plugin/types'

export class ReminderPlugin implements CasyPlugin {
  name = 'reminder'
  version = '1.0.0'
  description = '提醒模块'
  
  async install(ctx: CasyContext): Promise<void> {
    ctx.registerTool(this.createListReminderRulesTool(ctx))
    ctx.registerTool(this.createCreateReminderRuleTool(ctx))
    ctx.registerTool(this.createGetReminderLogTool(ctx))
    ctx.registerTool(this.createStartReminderEngineTool(ctx))
    
    console.log('ReminderPlugin installed')
  }
  
  async uninstall(ctx: CasyContext): Promise<void> {
    ctx.unregisterTool('list_reminder_rules')
    ctx.unregisterTool('create_reminder_rule')
    ctx.unregisterTool('get_reminder_log')
    ctx.unregisterTool('start_reminder_engine')
    console.log('ReminderPlugin uninstalled')
  }
  
  private createListReminderRulesTool(ctx: CasyContext): CasyTool {
    return {
      name: 'list_reminder_rules',
      description: '获取提醒规则列表',
      category: 'reminder',
      parameters: { type: 'object', properties: {} },
      execute: async () => {
        const result = await ctx.reminder.rules()
        return result
      },
    }
  }
  
  private createCreateReminderRuleTool(ctx: CasyContext): CasyTool {
    return {
      name: 'create_reminder_rule',
      description: '创建提醒规则',
      category: 'reminder',
      parameters: {
        type: 'object',
        properties: {
          name: { type: 'string', description: '规则名称' },
          triggerType: { 
            type: 'string',
            enum: ['deadline_before', 'deadline_on', 'deadline_after', 'hearing_before', 'task_due', 'task_overdue'],
            description: '触发类型'
          },
          triggerValue: { type: 'number', description: '触发值（天数）' },
          channels: { type: 'string', description: '通知渠道（JSON数组）' },
        },
        required: ['name', 'triggerType'],
      },
      execute: async (params) => {
        const result = await ctx.reminder.createRule({
          name: params.name,
          triggerType: params.triggerType,
          triggerValue: params.triggerValue,
          channels: params.channels || '["local"]',
        })
        return result
      },
    }
  }
  
  private createGetReminderLogTool(ctx: CasyContext): CasyTool {
    return {
      name: 'get_reminder_log',
      description: '获取提醒日志（R1-R4 分级）',
      category: 'reminder',
      parameters: {
        type: 'object',
        properties: {
          limit: { type: 'number', description: '返回数量' },
        },
      },
      execute: async (params) => {
        const result = await ctx.reminder.log(params.limit)
        return result
      },
    }
  }
  
  private createStartReminderEngineTool(ctx: CasyContext): CasyTool {
    return {
      name: 'start_reminder_engine',
      description: '启动提醒引擎',
      category: 'reminder',
      parameters: {
        type: 'object',
        properties: {
          intervalSeconds: { type: 'number', description: '检查间隔（秒）' },
        },
      },
      execute: async (params) => {
        const result = await ctx.reminder.startEngine(params.intervalSeconds)
        return result
      },
    }
  }
}
