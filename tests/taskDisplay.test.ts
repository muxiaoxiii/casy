import { describe, expect, it } from 'vitest'
import {
  daysUntil, isOverdue, formatDate, getWaitingDays,
  getTaskTypeLabel, getTaskTypeColor,
} from '../src/modules/tasks/utils/taskDisplay'

describe('taskDisplay · 日期逻辑', () => {
  const localDate = (date: Date) => {
    const year = date.getFullYear()
    const month = String(date.getMonth() + 1).padStart(2, '0')
    const day = String(date.getDate()).padStart(2, '0')
    return `${year}-${month}-${day}`
  }

  it('daysUntil: 今天=0，昨天=-1，明天=1', () => {
    const d = (offset: number) => {
      const t = new Date()
      t.setDate(t.getDate() + offset)
      return localDate(t)
    }
    expect(daysUntil(d(0))).toBe(0)
    expect(daysUntil(d(1))).toBe(1)
    expect(daysUntil(d(-1))).toBe(-1)
  })

  it('daysUntil: 空值与非法日期', () => {
    expect(daysUntil(null)).toBeNull()
    expect(daysUntil('')).toBeNull()
    expect(daysUntil('not-a-date')).toBeNull()
  })

  it('isOverdue: 仅过去且非空为真', () => {
    const past = new Date()
    past.setDate(past.getDate() - 2)
    expect(isOverdue(localDate(past))).toBe(true)
    expect(isOverdue(null)).toBe(false)
  })

  it('formatDate: 今天/明天/短日期', () => {
    const today = new Date()
    const tomorrow = new Date()
    tomorrow.setDate(tomorrow.getDate() + 1)
    // 用本地日期串而非 toISOString()（UTC），避免凌晨窗口下测试输入偏一天
    expect(formatDate(localDate(today))).toBe('今天')
    expect(formatDate(localDate(tomorrow))).toBe('明天')
  })
})

describe('taskDisplay · 任务类型映射', () => {
  it('类型标签与颜色有默认兜底', () => {
    expect(getTaskTypeLabel('action')).toBe('行动')
    expect(getTaskTypeLabel('waiting')).toBe('等待')
    expect(getTaskTypeLabel('unknown_x')).toBe('unknown_x')
    expect(getTaskTypeColor('waiting')).toMatch(/^#/)
    expect(getTaskTypeColor('')).toBe('#909399')
  })
})

describe('taskDisplay · 等待天数', () => {
  it('无 followUpDate 为 0', () => {
    expect(getWaitingDays({})).toBe(0)
    expect(getWaitingDays({ followUpDate: null })).toBe(0)
  })
})
