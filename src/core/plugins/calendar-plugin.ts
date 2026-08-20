/**
 * 日历插件
 * 
 * 将日历和期限功能封装为插件
 */

import type { CasyPlugin, CasyContext, CasyTool } from '../plugin/types'

export class CalendarPlugin implements CasyPlugin {
  name = 'calendar'
  version = '1.0.0'
  description = '日历与期限模块'
  
  async install(ctx: CasyContext): Promise<void> {
    ctx.registerTool(this.createGetCalendarEventsTool(ctx))
    ctx.registerTool(this.createGetDeadlineWarningsTool(ctx))
    ctx.registerTool(this.createGetDashboardStatsTool(ctx))
    
    console.log('CalendarPlugin installed')
  }
  
  async uninstall(ctx: CasyContext): Promise<void> {
    ctx.unregisterTool('get_calendar_events')
    ctx.unregisterTool('get_deadline_warnings')
    ctx.unregisterTool('get_dashboard_stats')
    console.log('CalendarPlugin uninstalled')
  }
  
  private createGetCalendarEventsTool(ctx: CasyContext): CasyTool {
    return {
      name: 'get_calendar_events',
      description: '获取日历事件（开庭、口审、期限、任务）',
      category: 'calendar',
      parameters: {
        type: 'object',
        properties: {
          year: { type: 'number', description: '年份' },
          month: { type: 'number', description: '月份' },
        },
      },
      execute: async (params) => {
        const result = await ctx.calendar.events(params.year, params.month)
        return result
      },
    }
  }
  
  private createGetDeadlineWarningsTool(ctx: CasyContext): CasyTool {
    return {
      name: 'get_deadline_warnings',
      description: '获取期限预警（红/黄/绿分级）',
      category: 'calendar',
      parameters: { type: 'object', properties: {} },
      execute: async () => {
        const result = await ctx.calendar.deadlineWarnings()
        return result
      },
    }
  }
  
  private createGetDashboardStatsTool(ctx: CasyContext): CasyTool {
    return {
      name: 'get_dashboard_stats',
      description: '获取仪表盘统计（活跃案件、期限预警、最近活动）',
      category: 'calendar',
      parameters: { type: 'object', properties: {} },
      execute: async () => {
        const result = await ctx.calendar.dashboardStats()
        return result
      },
    }
  }
}
