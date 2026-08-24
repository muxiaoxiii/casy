/**
 * 任务行展示辅助（M-UI-0 · U-3 核心表面自绘起步）
 *
 * 从 TasksView.vue 抽出的纯函数——只依赖入参、可单测；
 * TaskRow 组件与视图层共用，保证行内展示与抽屉/其他引用一致。
 */

/** 截止日距今天的天数（负数=已逾期）；无效日期返回 null */
export function daysUntil(deadline: string | null | undefined): number | null {
  if (!deadline) return null
  const d = new Date(deadline)
  if (Number.isNaN(d.getTime())) return null
  const today = new Date()
  today.setHours(0, 0, 0, 0)
  return Math.ceil((d.getTime() - today.getTime()) / (1000 * 60 * 60 * 24))
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
  const date = new Date(dateStr)
  if (Number.isNaN(date.getTime())) return ''
  const today = new Date()
  const iso = (d: Date) => d.toISOString().split('T')[0]
  if (dateStr === iso(today)) return '今天'
  const tomorrow = new Date(today)
  tomorrow.setDate(tomorrow.getDate() + 1)
  if (dateStr === iso(tomorrow)) return '明天'
  return date.toLocaleDateString('zh-CN', { month: 'short', day: 'numeric' })
}

/** 等待天数：followUpDate 距今天数（用于"等待超时"提示） */
export function getWaitingDays(task: { followUpDate?: string | null }): number {
  if (!task.followUpDate) return 0
  const today = new Date()
  const followUp = new Date(task.followUpDate)
  if (Number.isNaN(followUp.getTime())) return 0
  return Math.ceil((today.getTime() - followUp.getTime()) / (1000 * 60 * 60 * 24))
}
