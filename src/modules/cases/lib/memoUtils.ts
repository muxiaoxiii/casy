/**
 * 办案备忘纯工具（从 CaseListView.vue 拆分 · 低风险重构）
 *
 * 只包含无副作用纯函数，供 useCaseMemos composable 复用，并可直接单测。
 * 不触碰 IPC 类型 / CommandMap。
 */
import { todayLocalISO } from '../../../shared/utils/date'

/** 备忘记录项 */
export interface MemoItem {
  id: string
  title: string
  content: string
  date: string
  time?: string
  tags?: string[]
  createdAt?: string
  [key: string]: unknown
}

/** 从案件 notes 字段解析备忘列表（优先级：合法 JSON 数组 > 单条字符串兜底） */
export function parseMemosFromNotes(notes: string | null | undefined): MemoItem[] {
  if (!notes) return []
  try {
    const parsed = JSON.parse(notes)
    return Array.isArray(parsed)
      ? (parsed as MemoItem[])
      : [{ id: '1', title: '办案备忘', content: notes, date: '2026-08-28' }]
  } catch {
    return [{ id: '1', title: '办案备忘', content: notes, date: '2026-08-28', tags: ['办案随笔'] }]
  }
}

/** 构造一条备忘记录（日期用本地时区 todayLocalISO，时间取当前 HH:MM） */
export function buildMemoRecord(input: {
  title?: string
  content: string
  tags?: string[]
  caseName?: string
}): MemoItem {
  const now = new Date()
  const dateStr = todayLocalISO()
  const timeStr = `${String(now.getHours()).padStart(2, '0')}:${String(now.getMinutes()).padStart(2, '0')}`
  return {
    id: Date.now().toString(),
    title: input.title || `办案备忘 · ${dateStr}`,
    content: input.content,
    date: dateStr,
    time: timeStr,
    tags: [...(input.tags || []), `#${(input.caseName || '').slice(0, 8) || '案件'}`],
  }
}

/** 备忘 NLP 智能提炼结果 */
export interface MemoExtraction {
  detection: 'event' | 'task' | 'none'
  /** 文本中识别到的日期（若有），已归一为 YYYY-MM-DD */
  eventDate?: string
  eventTitle?: string
  eventType?: string
  taskName?: string
}

/**
 * 简易 NLP 抽取：从备忘正文识别开庭/口审 → 事件，截止/到期/提交 → 待办。
 * 若检出日期则一并返回归一化的 eventDate。
 */
export function extractMemoIntent(text: string, fallbackTitle?: string): MemoExtraction {
  if (!text) return { detection: 'none' }
  const dateMatch =
    text.match(/\d{4}[-/年]\d{1,2}[-/月]\d{1,2}/) || text.match(/\d{1,2}月\d{1,2}日/)
  const eventDate = dateMatch
    ? dateMatch[0].replace('年', '-').replace('月', '-').replace('日', '')
    : undefined

  if (text.includes('开庭') || text.includes('口审')) {
    return { detection: 'event', eventDate, eventTitle: '法庭开庭审理', eventType: 'court' }
  }
  if (text.includes('截止') || text.includes('到期') || text.includes('提交')) {
    return { detection: 'task', eventDate, taskName: fallbackTitle || text.slice(0, 30) }
  }
  return { detection: 'none', eventDate }
}
