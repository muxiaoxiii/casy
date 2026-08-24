/**
 * 今日简报 Skill（K-4：技能层首个真实实现，走通 注册→executeSkill 链路）
 *
 * 只读聚合：今日待办 + 当日日程 + 期限预警数量，供 AI 注入当日上下文。
 * 无业务副作用、无 schema 变更——技能层的存在性证明与模板参考。
 */
import { casyContext } from '../plugin/context'
import type { CasySkill } from '../plugin/types'

export function createTodayBriefingSkill(): CasySkill {
  return {
    name: 'today_briefing',
    description: '获取今日概览：未完成任务、当日日程与期限预警（只读聚合）',
    execute: async () => {
      const [tasks, events, warnings] = await Promise.all([
        casyContext.tasks.list({ completed: false }),
        casyContext.calendar.events(),
        casyContext.calendar.deadlineWarnings(),
      ])

      const taskItems = Array.isArray(tasks.data) ? tasks.data : []
      const eventItems = Array.isArray(events.data) ? events.data : []
      const warningItems = Array.isArray(warnings.data) ? warnings.data : []

      return [
        '今日概览:',
        '- 未完成任务 ' + taskItems.length + ' 项',
        '- 日程事件 ' + eventItems.length + ' 项',
        '- 期限预警 ' + warningItems.length + ' 项' +
          (warningItems.length > 0 ? '（优先查看日历红/黄区）' : ''),
      ].join('\n')
    },
  }
}
