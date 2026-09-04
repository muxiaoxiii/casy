/**
 * 任务行展示辅助（M-UI-0 · U-3 核心表面自绘起步）
 *
 * 从 TasksView.vue 抽出的纯函数——只依赖入参、可单测；
 * TaskRow 组件与视图层共用，保证行内展示与抽屉/其他引用一致。
 */

import {
  daysUntil as daysUntilLocal,
  parseLocalDate,
  todayLocalISO,
  addDaysLocalISO,
} from '../../../shared/utils/date'

/**
 * 截止日距今天的天数（负数=已逾期）；无效日期返回 null
 *
 * 实现已下沉到 `shared/utils/date.ts`（审查 P1-2），此处保留同名导出
 * 以免改动全部调用点。原实现存在时区 bug：'YYYY-MM-DD' 被按 UTC 午夜解析，
 * 与本地零点相减在东八区把"今天"算成 +1 天。
 */
export function daysUntil(deadline: string | null | undefined): number | null {
  return daysUntilLocal(deadline)
}

export function isOverdue(deadline: string | null | undefined): boolean {
  const days = daysUntil(deadline)
  return days !== null && days < 0
}

const TASK_TYPE_LABELS: Record<string, string> = {
  action: '行动',
  waiting: '等待',
  delegated: '委派',
  someday: '某天',
}

export function getTaskTypeLabel(type: string | null | undefined): string {
  return TASK_TYPE_LABELS[type ?? ''] ?? String(type ?? '')
}

const TASK_TYPE_COLORS: Record<string, string> = {
  action: '#409EFF',
  waiting: '#E6A23C',
  delegated: '#909399',
  someday: '#909399',
}

export function getTaskTypeColor(type: string | null | undefined): string {
  return TASK_TYPE_COLORS[type ?? ''] ?? '#909399'
}

/** 紧凑日期文案：今天/明天/中文短日期 */
export function formatDate(dateStr: string | null | undefined): string {
  if (!dateStr) return ''
  // 时区（审查 P1-2）：原本 `new Date(dateStr)` 按 UTC 解析、
  // `toISOString().split('T')[0]` 按 UTC 取日期，东八区凌晨 8 小时窗口内
  // "今天"会偏早一天。改为两侧一致地按本地日期比较。
  const date = parseLocalDate(dateStr) ?? new Date(dateStr)
  if (Number.isNaN(date.getTime())) return ''
  const today = todayLocalISO()
  if (dateStr === today) return '今天'
  if (dateStr === addDaysLocalISO(today, 1)) return '明天'
  return date.toLocaleDateString('zh-CN', { month: 'short', day: 'numeric' })
}

/** 等待天数：followUpDate 距今天数（用于"等待超时"提示） */
export function getWaitingDays(task: { followUpDate?: string | null }): number {
  if (!task.followUpDate) return 0
  // 时区（审查 P1-2）：原本 `new Date(followUpDate)` 按 UTC 解析、
  // 减本地当下时刻再 Math.ceil，差值含小时分量，"今天到期"会随时刻跳变。
  const followUp = parseLocalDate(task.followUpDate)
  if (!followUp) return 0
  const today = parseLocalDate(todayLocalISO())!
  return Math.round((today.getTime() - followUp.getTime()) / 86400000)
}
