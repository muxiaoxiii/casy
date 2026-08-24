/**
 * 统一自然语言日期/时间解析（M-GTD-1 · A0-5）
 *
 * 全应用唯一的 When 解析器：快速捕获条（任务视图）、全局快捷捕获（App.vue）、
 * 日历 NL 建日程共用。此前三处各有一套实现且互不一致（dueTime 恒 null、
 * startBucket 写入非法值 'upcoming' 违反 schema CHECK 约束导致带日期捕获静默失败）。
 *
 * 语法（前缀匹配，识别后从标题剥离）：
 *   日期：今天 / 明天 / 后天 / 大后天 / 周X / 下周X / N天后 / N周后 / MM-DD 或 MM/DD
 *   时间：HH:MM / HH：MM / X点[半|整] / (上午|早上|下午|晚上)X点半
 *
 * 返回 { taskName, date, time }：
 *   date = YYYY-MM-DD | null；time = HH:MM(24h) | null；taskName 为剥离后的剩余文本。
 */

export interface ParsedWhen {
  taskName: string
  date: string | null
  time: string | null
}

const WEEKDAY_MAP: Record<string, number> = { 一: 1, 二: 2, 三: 3, 四: 4, 五: 5, 六: 6, 日: 0, 天: 0 }

function toDateStr(d: Date): string {
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${y}-${m}-${day}`
}

function startOfToday(): Date {
  const n = new Date()
  return new Date(n.getFullYear(), n.getMonth(), n.getDate())
}

/** 中文数字（一~十/两）转数值；超出范围返回 null */
function cnNum(s: string): number | null {
  if (/^\d+$/.test(s)) return parseInt(s, 10)
  const map: Record<string, number> = {
    一: 1, 两: 2, 二: 2, 三: 3, 四: 4, 五: 5, 六: 6, 七: 7, 八: 8, 九: 9, 十: 10,
  }
  if (s.length === 1) return map[s] ?? null
  if (s.length === 2 && s[1] === '十') {
    const tens = map[s[0]]
    return tens ? tens * 10 : null
  }
  if (s.length === 3 && s[1] === '十') {
    const tens = map[s[0]]
    const ones = map[s[2]]
    return tens && ones !== undefined ? tens * 10 + ones : null
  }
  return null
}

/** "X点[半|整]" → "HH:MM"；period 为 上午/下午/晚上 时做 12h→24h 换算 */
function parseCnHour(hourPart: string, minutePart: string | undefined, period: string | undefined): string | null {
  const h = cnNum(hourPart)
  if (h === null || h < 0 || h > 23) return null
  let hour24 = h
  const isAfternoon = period === '下午' || period === '晚上'
  const isMorning = period === '上午' || period === '早上'
  if (isAfternoon && h < 12) hour24 = h + 12
  // "上午12点" 视为 0 点的口语习惯不在此处理，按字面 12 点处理
  if (!isAfternoon && !isMorning && h === 12 && period === undefined) hour24 = 12
  const minute = minutePart === '半' ? 30 : 0
  return `${String(hour24).padStart(2, '0')}:${String(minute).padStart(2, '0')}`
}

/**
 * 解析文本前缀中的日期与时间词。
 * 未匹配任何内容时返回原文本与 null date/time。
 */
export function parseWhen(raw: string): ParsedWhen {
  let text = raw.trim()
  let date: string | null = null
  let time: string | null = null
  const today = startOfToday()

  /* ── 1. 时间词优先于日期词出现时也能各自命中；两者独立剥离 ── */

  // 1a) 带时段前缀：下午3点半 / 晚上8点 / 早上9点
  let m = text.match(/^(上午|早上|下午|晚上)\s*([0-9一二两三四五六七八九十]{1,3})点(半|整)?\s*/)
  if (m) {
    const t = parseCnHour(m[2], m[3], m[1])
    if (t) {
      time = t
      text = text.slice(m[0].length).trim()
    }
  }

  // 1b) 纯数字时间：HH:MM / HH：MM
  if (!time) {
    m = text.match(/^([01]?\d|2[0-3])[:：]([0-5]\d)\s*/)
    if (m) {
      time = `${m[1].padStart(2, '0')}:${m[2]}`
      text = text.slice(m[0].length).trim()
    }
  }

  // 1c) X点/X点半/X点整（无时段前缀）
  if (!time) {
    m = text.match(/^([0-9一二两三四五六七八九十]{1,3})点(半|整)?\s*/)
    if (m) {
      const t = parseCnHour(m[1], m[2], undefined)
      if (t) {
        time = t
        text = text.slice(m[0].length).trim()
      }
    }
  }

  /* ── 2. 日期词 ── */

  // 2a) 今天/明天/后天/大后天
  m = text.match(/^(今天|明天|后天|大后天)/)
  if (m) {
    const offset = m[1] === '今天' ? 0 : m[1] === '明天' ? 1 : m[1] === '后天' ? 2 : 3
    const d = new Date(today)
    d.setDate(d.getDate() + offset)
    date = toDateStr(d)
    text = text.slice(m[1].length).trim()
  } else {
    // 2b) 周X / 下周X（本周取最近一次已过也顺延下周；下周必 +7）
    m = text.match(/^(下?)周([一二三四五六日天])/)
    if (m) {
      const wd = WEEKDAY_MAP[m[2]]
      let delta = (wd - today.getDay() + 7) % 7
      if (delta === 0) delta = 7 // "周三"在周三当天说 → 指下周三
      if (m[1] === '下') delta += 7
      const d = new Date(today)
      d.setDate(d.getDate() + delta)
      date = toDateStr(d)
      text = text.slice(m[0].length).trim()
    } else {
      // 2c) N天后 / N周后
      m = text.match(/^([0-9一二两三四五六七八九十]{1,3})\s*(天|周)后\s*/)
      if (m) {
        const n = cnNum(m[1])
        if (n !== null) {
          const d = new Date(today)
          d.setDate(d.getDate() + (m[2] === '周' ? n * 7 : n))
          date = toDateStr(d)
          text = text.slice(m[0].length).trim()
        }
      } else {
        // 2d) MM-DD / MM/DD（今年内已过则顺延到明年）
        m = text.match(/^(\d{1,2})[-/](\d{1,2})\s*/)
        if (m) {
          const mo = parseInt(m[1], 10)
          const day = parseInt(m[2], 10)
          if (mo >= 1 && mo <= 12 && day >= 1 && day <= 31) {
            let d = new Date(today.getFullYear(), mo - 1, day)
            if (d < today) d = new Date(today.getFullYear() + 1, mo - 1, day)
            date = toDateStr(d)
            text = text.slice(m[0].length).trim()
          }
        }
      }
    }
  }

  /* ── 3. 兜底：剥离了词但剩空 → 还原原文 ── */
  const taskName = (text || raw).trim()

  return { taskName, date, time }
}

/**
 * 由解析出的 date 推导合法的时间桶（schema CHECK：inbox|anytime|someday|today）。
 * 无日期 → inbox（待厘清）；是今天 → today；未来某天 → anytime（"计划中"透视由日期字段推导）。
 */
export function bucketForDate(date: string | null): 'inbox' | 'today' | 'anytime' {
  if (!date) return 'inbox'
  return date === toDateStr(startOfToday()) ? 'today' : 'anytime'
}
