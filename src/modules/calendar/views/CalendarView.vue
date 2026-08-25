<script setup>
import { ref, computed, onMounted } from 'vue'
import { ElMessage } from 'element-plus'
import { useRouter } from 'vue-router'
import { casyContext } from '../../../core/plugin/context'
import { bucketForDate } from '../../../shared/nlp/parseWhen'
import {
  ArrowLeft, ArrowRight, Calendar, Clock, Warning,
  Bell, Finished, Plus,
} from '@element-plus/icons-vue'

const router = useRouter()

// ============================================================
// 语义色（与 theme.css 第十二章语义色对齐）
// 红 #B4554F 硬性 · 蓝 #3E5C9A 计划 · 绿 #4C8067 完成
// 琥珀 #B0823A 到期 · 灰 #9BA2AF · 紫 #6C6A9C
// ============================================================
const COLORS = {
  hard: '#B4554F', // 红：硬性（开庭/口审）
  plan: '#3E5C9A', // 蓝：计划（弹性任务）
  done: '#4C8067', // 绿：完成
  due: '#B0823A',  // 琥珀：到期/期限
  gray: '#9BA2AF', // 灰：中性
  info: '#6C6A9C', // 紫：信息（二审等）
}

// ============================================================
// 状态
// ============================================================
const currentDate = ref(new Date())
const events = ref([])
const tasks = ref([])
const todayTasks = ref([])
const deadlineWarnings = ref([])
const loading = ref(false)
const selectedDay = ref(null)
const activeView = ref('month') // month/week/day/forecast

// 自然语言建日程（顶部输入条，对标 Fantastical 快速输入）
const captureInput = ref('')
const captureInputRef = ref(null)
const capturing = ref(false)

// ============================================================
// 常量
// ============================================================
const weekDays = ['一', '二', '三', '四', '五', '六', '日']
const WEEKDAY_MAP = { 一: 1, 二: 2, 三: 3, 四: 4, 五: 5, 六: 6, 日: 0, 天: 0 }
const CN_NUM = { 一: 1, 两: 2, 二: 2, 三: 3, 四: 4, 五: 5, 六: 6, 七: 7, 八: 8, 九: 9 }
const PERIOD_DEFAULT_TIME = { 凌晨: '06:00', 上午: '09:00', 中午: '12:00', 下午: '14:00', 晚上: '19:00' }

const viewOptions = [
  { key: 'day', label: '日视图' },
  { key: 'week', label: '周视图' },
  { key: 'month', label: '月视图' },
  { key: 'year', label: '年视图' },
  { key: 'forecast', label: '预测' },
]

// ============================================================
// 计算属性
// ============================================================
const currentMonth = computed(() => {
  const y = currentDate.value.getFullYear()
  const m = currentDate.value.getMonth()
  return { year: y, month: m }
})

const monthLabel = computed(() => {
  const { year, month } = currentMonth.value
  return `${year}年${month + 1}月`
})

/** 构建任一月份的 6×7 矩阵（month0 = 0-based；年视图与月视图共用） */
function buildMonthMatrix(year, month0) {
  const firstDay = new Date(year, month0, 1)
  const lastDay = new Date(year, month0 + 1, 0)

  // 周一起始
  let startWeekday = firstDay.getDay() - 1
  if (startWeekday < 0) startWeekday = 6

  const days = []

  // 上月末尾
  const prevLastDay = new Date(year, month0, 0)
  for (let i = startWeekday - 1; i >= 0; i--) {
    days.push({
      date: new Date(year, month0 - 1, prevLastDay.getDate() - i),
      isCurrentMonth: false,
    })
  }

  // 本月
  for (let d = 1; d <= lastDay.getDate(); d++) {
    days.push({
      date: new Date(year, month0, d),
      isCurrentMonth: true,
    })
  }

  // 下月开头
  const remaining = 42 - days.length
  for (let d = 1; d <= remaining; d++) {
    days.push({
      date: new Date(year, month0 + 1, d),
      isCurrentMonth: false,
    })
  }

  return days
}

const calendarDays = computed(() => {
  const { year, month } = currentMonth.value
  return buildMonthMatrix(year, month)
})

// ── 年视图（对标 Fantastical 年视图 + 负载热度）──
const yearLabel = computed(() => `${currentDate.value.getFullYear()} 年`)

const yearMatrices = computed(() => {
  const y = currentDate.value.getFullYear()
  return Array.from({ length: 12 }, (_, m) => buildMonthMatrix(y, m))
})

/** 当日负载 → 热度等级（日程+到期任务计数，贡献图式分档） */
function dayLoadLevel(date) {
  if (!date) return 0
  const n = eventsForDay(date).length + tasksForDay(date).length
  if (n === 0) return 0
  if (n === 1) return 1
  if (n <= 3) return 2
  if (n <= 6) return 3
  return 4
}

function jumpToMonth(month0) {
  currentDate.value = new Date(currentDate.value.getFullYear(), month0, 1)
  activeView.value = 'month'
  loadData()
}

function jumpToDay(cell) {
  if (!cell || !cell.date) return
  currentDate.value = new Date(cell.date)
  selectedDay.value = cell.date
  activeView.value = 'day'
  loadData()
}

// ============================================================
// 事件颜色编码（语义色）
// ============================================================

/**
 * 颜色规则：
 * - 开庭/口审: 红 #B4554F（硬性）
 * - 期限: 琥珀 #B0823A（到期）
 * - 二审: 紫 #6C6A9C
 * - 任务: 蓝 #3E5C9A（弹性/计划）
 * - 默认: 灰 #9BA2AF
 */
function getEventColor(event) {
  if (event.type === 'court' || event.type === 'hearing') {
    return COLORS.hard
  }
  if (event.type === 'appeal') {
    return COLORS.info
  }
  if (event.type?.startsWith('deadline')) {
    return COLORS.due
  }
  if (event.type === 'task') {
    return COLORS.plan
  }
  return COLORS.gray
}

function getEventBgColor(event) {
  const color = getEventColor(event)
  return color + '20' // 20% 透明度
}

/**
 * 获取事件类型图标
 */
function getEventIcon(type) {
  const icons = {
    hearing: Calendar,
    court: Bell,
    deadline: Warning,
    deadline_red: Warning,
    deadline_yellow: Warning,
    deadline_green: Warning,
    task: Finished,
  }
  return icons[type] || Calendar
}

function getDayStatus(date) {
  const dateStr = formatDate(date)
  const dayEvents = eventsForDay(date)
  const dayTasks = tasksForDay(date)

  // 检查是否有硬性日程
  const hasHardSchedule = dayEvents.some(e =>
    e.type === 'court' || e.type === 'hearing'
  )

  // 检查是否有逾期任务
  const hasOverdue = dayTasks.some(t => {
    const due = t.dueDate || t.deadline
    return due && due < new Date().toISOString().split('T')[0] && !t.completed
  })

  // 检查是否有即将到期任务
  const hasDueSoon = dayTasks.some(t => {
    const due = t.dueDate || t.deadline
    if (!due || t.completed) return false
    const diffDays = Math.ceil((new Date(due) - new Date()) / (1000 * 60 * 60 * 24))
    return diffDays >= 0 && diffDays <= 3
  })

  if (hasHardSchedule) return 'hard'
  if (hasOverdue) return 'overdue'
  if (hasDueSoon) return 'due-soon'
  return 'normal'
}

// ============================================================
// 数据加载（全部经 casyContext 服务，不再直调 tauriCallSafe）
// ============================================================
onMounted(() => {
  loadData()
})

async function loadData() {
  loading.value = true
  await Promise.all([
    loadEvents(),
    loadTasks(),
    loadTodayTasks(),
    loadDeadlineWarnings(),
  ])
  loading.value = false
}

async function loadEvents() {
  const { year, month } = currentMonth.value
  // 年视图：加载全年 12 个月（本地 SQLite 可承受）；
  // 其余视图：当前月 + 下月，保证 Forecast 未来窗口跨月不缺数据
  const months = []
  if (activeView.value === 'year') {
    for (let m = 0; m < 12; m++) months.push({ year, month: m + 1 })
  } else {
    const current = new Date(year, month, 1)
    for (let i = 0; i < 2; i++) {
      const d = new Date(current.getFullYear(), current.getMonth() + i, 1)
      months.push({ year: d.getFullYear(), month: d.getMonth() + 1 })
    }
  }
  const results = await Promise.all(months.map(m => casyContext.calendar.events(m.year, m.month)))
  const merged = []
  for (const r of results) {
    if (r.ok && Array.isArray(r.data)) {
      for (const e of r.data) {
        merged.push({
          ...e,
          // 归一化：后端投影 start_time → 周视图既有的 time 字段约定
          time: e.startTime || null,
          endTime: e.endTime || null,
          allDay: !!e.allDay,
        })
      }
    }
  }
  events.value = merged
}

async function loadTasks() {
  const result = await casyContext.tasks.list({ completed: false })
  if (result.ok && Array.isArray(result.data)) {
    tasks.value = result.data
  }
}

async function loadTodayTasks() {
  const result = await casyContext.tasks.list({ startBucket: 'today' })
  if (result.ok && Array.isArray(result.data)) {
    todayTasks.value = result.data
  }
}

async function loadDeadlineWarnings() {
  const result = await casyContext.calendar.deadlineWarnings()
  if (result.ok && Array.isArray(result.data)) {
    deadlineWarnings.value = result.data
  }
}

// ============================================================
// 导航（年视图步进 ±1 年，其余 ±1 月）
// ============================================================
function prevPeriod() {
  const d = new Date(currentDate.value)
  if (activeView.value === 'year') d.setFullYear(d.getFullYear() - 1)
  else d.setMonth(d.getMonth() - 1)
  currentDate.value = d
  loadData()
}

function nextPeriod() {
  const d = new Date(currentDate.value)
  if (activeView.value === 'year') d.setFullYear(d.getFullYear() + 1)
  else d.setMonth(d.getMonth() + 1)
  currentDate.value = d
  loadData()
}

function goToday() {
  currentDate.value = new Date()
  loadData()
}

// ============================================================
// 工具函数
// ============================================================
function isToday(date) {
  const today = new Date()
  return date.toDateString() === today.toDateString()
}

function formatDate(date) {
  const y = date.getFullYear()
  const m = String(date.getMonth() + 1).padStart(2, '0')
  const d = String(date.getDate()).padStart(2, '0')
  return `${y}-${m}-${d}`
}

function toDateStr(d) {
  return formatDate(d)
}

function todayStr() {
  return toDateStr(new Date())
}

function eventsForDay(date) {
  const dateStr = formatDate(date)
  return events.value.filter(e => e.date === dateStr)
}

function tasksForDay(date) {
  const dateStr = formatDate(date)
  return tasks.value.filter(t => {
    const due = t.dueDate || t.deadline
    const start = t.startDate
    return due === dateStr || start === dateStr
  })
}

function selectDay(day) {
  selectedDay.value = day
}

function getEventTypeLabel(type) {
  const labels = {
    hearing: '口审',
    court: '开庭',
    appeal: '二审',
    deadline: '期限',
    deadline_red: '紧急期限',
    deadline_yellow: '即将到期',
    deadline_green: '正常期限',
    task: '任务',
  }
  return labels[type] || type
}

// ============================================================
// 选中日期的详情
// ============================================================
const selectedDayEvents = computed(() => {
  if (!selectedDay.value) return []
  return eventsForDay(selectedDay.value.date)
})

const selectedDayTasks = computed(() => {
  if (!selectedDay.value) return []
  return tasksForDay(selectedDay.value.date)
})

const selectedDayStats = computed(() => {
  const events = selectedDayEvents.value
  const tasks = selectedDayTasks.value

  return {
    hardSchedule: events.filter(e => e.type === 'court' || e.type === 'hearing').length,
    deadlines: events.filter(e => e.type?.startsWith('deadline')).length,
    tasks: tasks.length,
    overdue: tasks.filter(t => {
      const due = t.dueDate || t.deadline
      return due && due < new Date().toISOString().split('T')[0] && !t.completed
    }).length,
  }
})

// ============================================================
// 自然语言建日程（对标 Fantastical 快速输入）
// 日期：今天/明天/后天 · 周X/下周X · X月X日 · MM-DD/MM/DD
// 时间：上午/下午/晚上 + X点/X点半/X点整 · HH:MM
// ============================================================

/** 中文数字 → 阿拉伯数字（支持 一~九、两、十、十一~十九、二十…） */
function parseCnNumber(s) {
  if (!s) return null
  if (/^\d+$/.test(s)) return parseInt(s, 10)
  if (s === '十') return 10
  if (s.includes('十')) {
    const [a, b] = s.split('十')
    return ((a ? CN_NUM[a] : 1) || 0) * 10 + (CN_NUM[b] || 0)
  }
  return CN_NUM[s] ?? null
}

/** 下午/晚上 12 小时制 → 24 小时制 */
function resolveHour(hour, period) {
  if ((period === '下午' || period === '晚上') && hour < 12) return hour + 12
  return hour
}

/** X月X日 / MM-DD：今年内已过则顺延到明年 */
function resolveMonthDay(m, day, today) {
  let d = new Date(today.getFullYear(), m - 1, day)
  if (d < today) d = new Date(today.getFullYear() + 1, m - 1, day)
  return toDateStr(d)
}

/**
 * 解析自然语言日程文本
 * @returns {{ title: string, dateStr: string|null, timeStr: string|null, timeLabel: string|null }}
 */
function parseCalendarText(raw) {
  let text = (raw || '').trim()
  if (!text) return null
  const now = new Date()
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate())

  let dateStr = null

  // ── 1) 日期：今天/明天/后天 ──
  const rel = text.match(/(今天|明天|后天)/)
  if (rel) {
    const offset = rel[1] === '今天' ? 0 : rel[1] === '明天' ? 1 : 2
    const d = new Date(today)
    d.setDate(d.getDate() + offset)
    dateStr = toDateStr(d)
    text = text.replace(rel[0], ' ')
  } else {
    // ── 2) 周X / 本周X / 下周X（本周或下周最近一次） ──
    const week = text.match(/(本|下)?周([一二三四五六日天])/)
    if (week) {
      const wd = WEEKDAY_MAP[week[2]]
      const delta = (wd - today.getDay() + 7) % 7
      const d = new Date(today)
      d.setDate(d.getDate() + delta + (week[1] === '下' ? 7 : 0))
      dateStr = toDateStr(d)
      text = text.replace(week[0], ' ')
    } else {
      // ── 3) X月X日 ──
      const mdCn = text.match(/(\d{1,2})月(\d{1,2})日/)
      if (mdCn) {
        dateStr = resolveMonthDay(parseInt(mdCn[1], 10), parseInt(mdCn[2], 10), today)
        text = text.replace(mdCn[0], ' ')
      } else {
        // ── 4) MM-DD / MM/DD ──
        const md = text.match(/(\d{1,2})[-/](\d{1,2})/)
        if (md) {
          dateStr = resolveMonthDay(parseInt(md[1], 10), parseInt(md[2], 10), today)
          text = text.replace(md[0], ' ')
        }
      }
    }
  }

  // ── 5) 时间 ──
  const periodMatch = text.match(/(上午|下午|中午|晚上|凌晨)/)
  const period = periodMatch ? periodMatch[1] : null
  let timeStr = null
  let timeLabel = null

  // HH:MM（可带 上午/下午/晚上 前缀）
  const hm = text.match(/(\d{1,2})[:：](\d{2})/)
  if (hm) {
    const hour = resolveHour(parseInt(hm[1], 10), period)
    timeStr = `${String(hour).padStart(2, '0')}:${hm[2]}`
    text = text.replace(hm[0], ' ')
    timeLabel = period ? `${period} ${timeStr}` : timeStr
  } else {
    // X点 / X点半 / X点整（支持中文数字）
    const cnHour = text.match(/([0-9一二两三四五六七八九十]{1,3})点(半|整)?/)
    if (cnHour) {
      const h = parseCnNumber(cnHour[1])
      if (h !== null && h >= 0 && h <= 24) {
        const minute = cnHour[2] === '半' ? 30 : 0
        const hour = resolveHour(h, period)
        timeStr = `${String(hour).padStart(2, '0')}:${String(minute).padStart(2, '0')}`
        text = text.replace(cnHour[0], ' ')
        timeLabel = period ? `${period} ${timeStr}` : timeStr
      }
    } else if (period) {
      // 仅时段：上午/下午/晚上 → 默认时刻
      timeStr = PERIOD_DEFAULT_TIME[period] || null
      timeLabel = timeStr ? `${period} ${timeStr}` : null
    }
  }

  if (period) text = text.replace(period, ' ')

  // 清理残留分隔符，得到标题
  const title = text.replace(/[，。,.、\s]+/g, ' ').trim() || (raw || '').trim()

  return { title, dateStr, timeStr, timeLabel }
}

/**
 * 回车创建日程。
 * 后端没有日历事件创建命令（get_calendar_events 只读）→ 降级为任务：
 * casyContext.tasks.create({ taskName, startDate, dueDate, startBucket: 'upcoming' })
 */
async function createFromNaturalLanguage() {
  const text = captureInput.value.trim()
  if (!text || capturing.value) return
  const parsed = parseCalendarText(text)
  const title = parsed ? parsed.title : text
  if (!title) {
    ElMessage.warning('请输入日程内容')
    return
  }

  const dateStr = (parsed && parsed.dateStr) || todayStr() // 未指定日期默认今天
  const startTime = (parsed && parsed.timeStr) || null
  const display = `${dateStr}${parsed && parsed.timeLabel ? ' ' + parsed.timeLabel : ''} · ${title}`

  capturing.value = true
  // D-7：日程是独立事实源——直接落 calendar_events，不再降级为任务
  const result = await casyContext.calendar.createEvent({
    title,
    eventDate: dateStr,
    startTime,
    allDay: startTime ? 0 : 1,
  })
  capturing.value = false

  if (result.ok) {
    ElMessage.success(`已创建日程：${display}`)
    captureInput.value = ''
    await loadData()
  } else {
    ElMessage.error(result.error || '创建失败')
  }
}

function onCaptureKeydown(e) {
  e.preventDefault()
  createFromNaturalLanguage()
}

/** 日视图议程勾选：乐观翻转 → IPC，失败回滚（M-GTD-1 手感一致） */
async function toggleAgendaTask(task) {
  const prev = task.completed
  task.completed = prev ? 0 : 1
  const result = await casyContext.tasks.toggle(task.id)
  if (!result.ok) {
    task.completed = prev
    ElMessage.error(result.error || '更新失败')
    return
  }
  // 完成后从未完成任务源消失，重拉保持口径一致
  await loadTasks()
}

// ============================================================
// Forecast 双栏（对标 Fantastical）
// 左栏：按日分组的日程/期限列表（events + deadlineWarnings）
// 右栏：当日"硬性日程 + 弹性任务"时间轴（tasks startBucket=today）
// ============================================================
const FORECAST_DAYS = 14

const forecastWindow = computed(() => {
  const today = new Date()
  const days = []
  for (let i = 0; i < FORECAST_DAYS; i++) {
    const d = new Date(today.getFullYear(), today.getMonth(), today.getDate() + i)
    days.push({
      date: d,
      dateStr: toDateStr(d),
      isToday: i === 0,
      label: i === 0 ? '今天' : i === 1 ? '明天' : `${d.getMonth() + 1}月${d.getDate()}日`,
      weekDay: weekDays[d.getDay() === 0 ? 6 : d.getDay() - 1],
    })
  }
  return days
})

/** 期限预警归一化（兼容后端 DeadlineResult 与 dashboard mock 两种形状） */
function normalizeWarning(w) {
  return {
    date: w.dueDate || w.date || w.deadline || '',
    title: w.ruleName || w.deadlineName || w.title || w.message || '期限预警',
    caseName: w.caseName || '',
    caseId: w.caseId || w.deadlineId || null,
    daysLeft: typeof w.daysLeft === 'number' ? w.daysLeft : null,
  }
}

const forecastGroups = computed(() => {
  const warnByDate = {}
  const overdue = []
  const todayDs = todayStr()
  for (const w of deadlineWarnings.value) {
    const n = normalizeWarning(w)
    if (!n.date) continue
    if (n.date < todayDs) {
      overdue.push({ kind: 'warning', ...n, color: COLORS.due, icon: Warning })
      continue
    }
    if (!warnByDate[n.date]) warnByDate[n.date] = []
    warnByDate[n.date].push({ kind: 'warning', ...n, color: COLORS.due, icon: Warning })
  }

  const rank = { hard: 0, deadline: 1, warning: 2, other: 3 }

  const groups = []
  if (overdue.length > 0) {
    groups.push({
      date: new Date(),
      dateStr: 'overdue',
      isToday: false,
      isOverdue: true,
      label: '已逾期',
      weekDay: '',
      items: overdue,
    })
  }

  for (const day of forecastWindow.value) {
    const items = []
    for (const e of events.value.filter(ev => ev.date === day.dateStr)) {
      const kind = e.type === 'court' || e.type === 'hearing'
        ? 'hard'
        : e.type?.startsWith('deadline')
          ? 'deadline'
          : 'other'
      items.push({
        kind,
        title: e.title,
        caseName: e.caseName || '',
        time: e.time || null,
        color: getEventColor(e),
        icon: kind === 'hard' ? Bell : kind === 'deadline' ? Warning : Calendar,
        daysLeft: null,
      })
    }
    for (const w of (warnByDate[day.dateStr] || [])) {
      items.push(w)
    }
    items.sort((a, b) => {
      const r = (rank[a.kind] ?? 4) - (rank[b.kind] ?? 4)
      if (r !== 0) return r
      const ta = a.time || '99:99'
      const tb = b.time || '99:99'
      return ta < tb ? -1 : ta > tb ? 1 : 0
    })
    groups.push({ ...day, items })
  }
  return groups
})

// ============================================================
// 今日时间分配网格（右栏 · 设计哲学 §7.2 时间块分区）
// 时间块：上午 06-12 / 下午 12-18 / 晚上 18-22 / 其他 22-06
// 无时间的弹性任务进"弹性"分区（单独一块，视觉与硬性区分）
// ============================================================
const todayLabel = computed(() => {
  const d = new Date()
  return `${d.getMonth() + 1}月${d.getDate()}日 周${weekDays[d.getDay() === 0 ? 6 : d.getDay() - 1]}`
})

const TIME_BLOCKS = [
  { key: 'morning', label: '上午', range: '06:00-12:00' },
  { key: 'afternoon', label: '下午', range: '12:00-18:00' },
  { key: 'evening', label: '晚上', range: '18:00-22:00' },
  { key: 'other', label: '其他', range: '22:00-06:00' },
]

/** 时间字符串 → 时间块；无时间或解析失败 → 弹性分区 */
function timeBlockFor(timeStr) {
  if (!timeStr) return 'flex'
  const m = String(timeStr).match(/(\d{1,2})[:：](\d{2})/)
  if (!m) return 'flex'
  const h = parseInt(m[1], 10)
  if (h >= 6 && h < 12) return 'morning'
  if (h >= 12 && h < 18) return 'afternoon'
  if (h >= 18 && h < 22) return 'evening'
  return 'other' // 22:00-06:00
}

/**
 * 解析自然语言建日程写入任务名的前缀时间（"14:00 会议" → 14:00），
 * 让带时间的弹性任务也能落进对应时间块；解析失败视为未定时。
 */
function extractTaskTime(name) {
  const m = String(name || '').match(/^(\d{1,2})[:：](\d{2})\s+(\S.*)$/)
  if (!m) return null
  const h = parseInt(m[1], 10)
  const min = parseInt(m[2], 10)
  if (h > 23 || min > 59) return null
  return {
    time: `${String(h).padStart(2, '0')}:${String(min).padStart(2, '0')}`,
    rest: m[3],
  }
}

/** 当日硬性日程（开庭/口审/期限 events + 今日到期的期限预警） */
const todayHardItems = computed(() => {
  const ds = todayStr()
  const items = []
  let idx = 0
  for (const e of events.value) {
    if (e.date !== ds) continue
    const isHard = e.type === 'court' || e.type === 'hearing'
    const isDeadline = e.type?.startsWith('deadline')
    if (!isHard && !isDeadline) continue
    items.push({
      uid: 'evt-' + (e.id || idx++),
      title: e.title,
      time: e.time || null,
      caseName: e.caseName || '',
      kind: 'hard',
      color: getEventColor(e),
      done: false,
      daysLeft: null,
      minutes: null,
      todayIndex: 0,
      task: null,
    })
  }
  for (const w of deadlineWarnings.value) {
    const n = normalizeWarning(w)
    if (n.date === ds) {
      items.push({
        uid: 'warn-' + (n.caseId || n.title),
        title: n.title,
        time: null,
        caseName: n.caseName,
        kind: 'hard',
        color: COLORS.due,
        done: false,
        daysLeft: n.daysLeft,
        minutes: null,
        todayIndex: 0,
        task: null,
      })
    }
  }
  return items
})

/** 当日弹性任务（startBucket=today；task 保留原引用供拖拽/点击） */
const todayFlexItems = computed(() => {
  return todayTasks.value.map(t => {
    const parsed = extractTaskTime(t.taskName)
    return {
      uid: 'task-' + t.id,
      title: parsed ? parsed.rest : t.taskName,
      time: parsed ? parsed.time : null,
      caseName: t.caseName || '',
      kind: 'flex',
      color: COLORS.plan,
      done: !!t.completed,
      daysLeft: null,
      minutes: t.estimatedMinutes || null,
      todayIndex: t.todayIndex || 0,
      task: t,
    }
  })
})

/** 时间块分区网格：4 个时段块 + 1 个弹性块，块内按时间排序 */
const todayBlocks = computed(() => {
  const blocks = [
    ...TIME_BLOCKS.map(b => ({ ...b, items: [] })),
    { key: 'flex', label: '弹性', range: '未定时', items: [] },
  ]

  for (const item of [...todayHardItems.value, ...todayFlexItems.value]) {
    const key = timeBlockFor(item.time)
    blocks.find(b => b.key === key)?.items.push(item)
  }

  for (const block of blocks) {
    block.items.sort((a, b) => {
      if (a.done !== b.done) return a.done ? 1 : -1 // 已完成排后
      if (block.key === 'flex') {
        // 弹性分区：硬性（期限）优先，其余按今日序号
        if (a.kind !== b.kind) return a.kind === 'hard' ? -1 : 1
        return (a.todayIndex || 0) - (b.todayIndex || 0)
      }
      // 时段块内按时间排序
      const ta = a.time || '99:99'
      const tb = b.time || '99:99'
      return ta < tb ? -1 : ta > tb ? 1 : 0
    })
  }
  return blocks
})

/** 硬性日程占据的时间块（仅时段块） */
const hardOccupiedBlocks = computed(() => {
  return TIME_BLOCKS
    .filter(b => todayBlocks.value.find(x => x.key === b.key)?.items.some(i => i.kind === 'hard'))
    .map(b => b.key)
})

/** 时间分配提示（§7.3）：硬性占 ≥2 个时段块 → 推荐空块 */
const allocationTip = computed(() => {
  const occupied = hardOccupiedBlocks.value
  if (occupied.length < 2) return null
  const free = TIME_BLOCKS.find(b => !occupied.includes(b.key))
  if (free) return `今日硬性日程密集，弹性任务建议安排在${free.label}`
  return '今日硬性日程密集，各时间块均有硬性安排，弹性任务建议择日再排'
})

/** 时间槽标签：优先时间，其次预计分钟/剩余天数 */
function itemTimeLabel(item) {
  if (item.time) return item.time
  if (item.minutes) return `${item.minutes}m`
  if (item.daysLeft !== null && item.daysLeft !== undefined) return `${item.daysLeft}天`
  return '--:--'
}

/**
 * 周视图：获取某天某小时的事件
 */
const weekHours = Array.from({ length: 15 }, (_, i) => i + 7) // 7:00 - 21:00

/** 当前时刻线：可见时段内的纵向偏移百分比（7:00-21:00 = 840 分钟） */
const nowLineTop = computed(() => {
  const now = new Date()
  const minutes = now.getHours() * 60 + now.getMinutes()
  const from = 7 * 60
  const span = 14 * 60
  if (minutes < from || minutes > from + span) return null
  return ((minutes - from) / span) * 100
})

/** 时刻线仅横贯今日列（Google Calendar 惯例）；top 相对 .week-body（恰为可见 840 分钟） */
const nowLineStyle = computed(() => {
  if (nowLineTop.value === null) return { display: 'none' }
  const d = new Date()
  const col = (d.getDay() || 7) - 1 // 周一 = 0
  return {
    top: `${nowLineTop.value}%`,
    left: `calc(56px + (100% - 56px) * ${col / 7})`,
    width: `calc((100% - 56px) / 7)`,
  }
})

function getWeekDay(dayIndex) {
  const d = new Date(currentDate.value)
  const currentDay = d.getDay() || 7
  const diff = dayIndex - currentDay
  d.setDate(d.getDate() + diff)
  return d
}

// ── 周视图时长定位（Google Calendar 式）──────────────────
const WEEK_START_MIN = 7 * 60   // 07:00
const WEEK_SPAN_MIN = 14 * 60   // 至 21:00

function toMin(t) {
  if (!t) return null
  const parts = String(t).split(':')
  const h = Number(parts[0])
  if (Number.isNaN(h)) return null
  return h * 60 + (Number(parts[1]) || 0)
}

/** 七日列数据：allday chips + timed 绝对定位块（重叠聚类 → 贪心分列） */
const weekColumns = computed(() => {
  return Array.from({ length: 7 }, (_, di) => {
    const date = getWeekDay(di + 1)
    const dateStr = formatDate(date)

    const allday = []
    const timed = []
    for (const e of events.value.filter(x => x.date === dateStr)) {
      if (!e.time) {
        allday.push({ ev: { ...e, isTask: false } })
      } else {
        timed.push({ ...e, isTask: false })
      }
    }
    // 有具体时刻的任务并入时间块（§7：日历+待办合一）
    for (const t of tasksForDay(date)) {
      if (t.completed || !t.dueTime) continue
      timed.push({
        ...t,
        id: 'task-' + t.id,
        title: t.taskName,
        time: t.dueTime,
        endTime: null,
        isTask: true,
      })
    }

    const positioned = timed.map(e => {
      let s = toMin(e.time) ?? WEEK_START_MIN
      let en = toMin(e.endTime) ?? s + 60
      s = Math.max(WEEK_START_MIN, Math.min(s, WEEK_START_MIN + WEEK_SPAN_MIN))
      en = Math.max(s + 45, Math.min(en, WEEK_START_MIN + WEEK_SPAN_MIN))
      return { ev: e, start: s, end: en }
    })

    positioned.sort((a, b) => a.start - b.start || a.end - b.end)
    const clusters = []
    let cur = null
    for (const p of positioned) {
      if (cur && p.start < cur.end) {
        cur.items.push(p)
        cur.end = Math.max(cur.end, p.end)
      } else {
        cur = { items: [p], end: p.end }
        clusters.push(cur)
      }
    }
    for (const cl of clusters) {
      const colEnds = []
      for (const p of cl.items) {
        let ci = colEnds.findIndex(end => end <= p.start)
        if (ci === -1) {
          colEnds.push(p.end)
          ci = colEnds.length - 1
        } else {
          colEnds[ci] = p.end
        }
        p.col = ci
      }
      const cols = colEnds.length
      for (const p of cl.items) {
        p.style = {
          top: `${((p.start - WEEK_START_MIN) / WEEK_SPAN_MIN) * 100}%`,
          height: `max(${((p.end - p.start) / WEEK_SPAN_MIN) * 100}%, 26px)`,
          left: `calc(${(p.col / cols) * 100}% + 2px)`,
          width: `calc(${100 / cols}% - 4px)`,
        }
        p.label = `${Math.floor(p.start / 60)}:${String(p.start % 60).padStart(2, '0')}${p.ev.endTime ? ' - ' + p.ev.endTime : ''}`
      }
    }

    return { allday, timed: positioned }
  })
})

/** 投放换算：timed 层按 offsetY 反解小时；allday 层 hour=null */
function onDropToSlot(e, dayIndex, kind) {
  let hour = null
  if (kind === 'timed') {
    const rect = e.currentTarget.getBoundingClientRect()
    const ratio = (e.clientY - rect.top) / rect.height
    hour = Math.min(21, Math.max(7, Math.floor(WEEK_START_MIN / 60 + ratio * 14)))
  }
  onDropToTime(formatDate(getWeekDay(dayIndex)), hour)
}

// ── 日视图时间轴（Sunsama 式单日排期）──────────────────
const selectedDateStr = computed(() => formatDate(selectedDay.value?.date || new Date()))
const isTodaySelected = computed(() => isToday(selectedDay.value?.date || new Date()))

/** 当日时间块：有时刻的日程 + 未完成任务(dueTime)，统一绝对定位 */
const dayTimedItems = computed(() => {
  const d = selectedDay.value?.date || new Date()
  const evs = eventsForDay(d)
    .filter(e => e.time)
    .map(e => ({ ...e, isTask: false }))
  const tks = tasksForDay(d)
    .filter(t => !t.completed && t.dueTime)
    .map(t => ({
      id: 'task-' + t.id,
      title: t.taskName,
      time: t.dueTime,
      endTime: null,
      isTask: true,
    }))

  return [...evs, ...tks].map(e => {
    let s = toMin(e.time) ?? WEEK_START_MIN
    let en = toMin(e.endTime) ?? s + 60
    s = Math.max(WEEK_START_MIN, Math.min(s, WEEK_START_MIN + WEEK_SPAN_MIN))
    en = Math.max(s + 40, Math.min(en, WEEK_START_MIN + WEEK_SPAN_MIN))
    return {
      ev: e,
      style: {
        top: `${((s - WEEK_START_MIN) / WEEK_SPAN_MIN) * 100}%`,
        height: `max(${((en - s) / WEEK_SPAN_MIN) * 100}%, 24px)`,
        left: '4px',
        right: '6px',
      },
      time: `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`,
    }
  })
})

/** 拖入日时间轴：按 offsetY 反解整点，落库 dueDate+dueTime */
async function onDropToDayHour(e) {
  if (!draggedTask.value) return
  const rect = e.currentTarget.getBoundingClientRect()
  const ratio = (e.clientY - rect.top) / rect.height
  const minutes = WEEK_START_MIN + Math.floor((ratio * WEEK_SPAN_MIN) / 60) * 60
  const hhmm = `${String(Math.floor(minutes / 60)).padStart(2, '0')}:00`
  const dateStr = selectedDateStr.value

  const task = draggedTask.value
  const result = await casyContext.tasks.update({
    id: task.id,
    dueDate: dateStr,
    deadline: dateStr,
    dueTime: hhmm,
  })
  if (result.ok) {
    ElMessage.success(`已排期到 ${dateStr} ${hhmm}`)
    await loadTasks()
  }
  draggedTask.value = null
}

function getWeekDayHourEvents(date, hour) {
  const dateStr = formatDate(date)
  const evs = events.value.filter(e => {
    if (e.date !== dateStr) return false
    if (!e.time) return false
    const eHour = parseInt(e.time.split(':')[0])
    return eHour === hour
  })
  // 综合显示：同时列出该时刻的有具体时间点的任务（设计哲学 §7：日历+待办合一）
  const dayTasks = tasksForDay(date).filter(t => {
    if (!t.dueTime) return false
    const tHour = parseInt(t.dueTime.split(':')[0])
    return tHour === hour
  })
  return [...evs, ...dayTasks.map(t => ({
    id: 'task-' + t.id,
    isTask: true,
    task: t,
    title: t.taskName,
    time: t.dueTime || '',
    date: dateStr,
  }))]
}

/**
 * 拖拽相关（改期 = casyContext.tasks.update）
 */
const draggedTask = ref(null)
const draggedEvent = ref(null)

/** 独立日程拖拽（仅 type='event'；庭审/期限是案件域投影不可移动） */
function onEventDragStart(ev, e) {
  if (ev.type !== 'event') return
  draggedEvent.value = ev
  e.dataTransfer.effectAllowed = 'move'
  e.dataTransfer.setData('text/plain', String(ev.id))
}

/** 月视图投放路由：任务走 tasks.update，独立日程走 calendar.moveEvent */
async function onMonthDrop(e, day) {
  const dateStr = day && day.date ? formatDate(day.date) : ''
  if (!/^\d{4}-\d{2}-\d{2}$/.test(dateStr)) return

  if (draggedTask.value) {
    await onDrop(dateStr, e)
    return
  }
  if (draggedEvent.value) {
    const ev = draggedEvent.value
    draggedEvent.value = null
    if (ev.date === dateStr) return
    const result = await casyContext.calendar.moveEvent(ev.id, dateStr, ev.time ?? null)
    if (result.ok) {
      ElMessage.success(`日程已移动到 ${dateStr}`)
      await loadEvents()
    } else {
      ElMessage.error(result.error || '移动失败')
    }
  }
}

function onDragStart(task, event) {
  draggedTask.value = task
  event.dataTransfer.effectAllowed = 'move'
  event.dataTransfer.setData('text/plain', task.id)
}

function onDragOver(dateStr, event) {
  event.preventDefault()
  event.dataTransfer.dropEffect = 'move'
}

// 拖拽到时间点（设计哲学 §7.3：拖到时间轴某时刻 = 改日期 + 时间）
async function onDropToTime(dateStr, hour) {
  if (!draggedTask.value) return
  const task = draggedTask.value
  // 全天投放（hour=null）：保留原时刻或默认 09:00
  const timeStr = hour == null
    ? (task.dueTime || '09:00')
    : String(hour).padStart(2, '0') + ':00'
  if (task.dueDate === dateStr && (task.dueTime || '00:00').slice(0, 2) === timeStr.slice(0, 2)) return
  const result = await casyContext.tasks.update({
    id: task.id,
    dueDate: dateStr,
    deadline: dateStr,
    dueTime: timeStr,
  })
  if (result.ok) {
    ElMessage.success('已改期到 ' + dateStr + ' ' + timeStr)
    await Promise.all([loadTasks(), loadTodayTasks()])
  }
  draggedTask.value = null
}

async function onDrop(dateStr, event) {
  event.preventDefault()
  if (!draggedTask.value) return
  // 仅接受 YYYY-MM-DD 目标（'overdue' 分组不是日期，忽略）
  if (!/^\d{4}-\d{2}-\d{2}$/.test(dateStr)) return

  const task = draggedTask.value
  const oldDate = task.dueDate || task.deadline

  if (oldDate === dateStr) return

  // 更新任务日期（data 内带 id，对齐后端 update_task 只收 data）
  const result = await casyContext.tasks.update({
    id: task.id,
    dueDate: dateStr,
    deadline: dateStr,
  })

  if (result.ok) {
    ElMessage.success(`已改期到 ${dateStr}`)
    await Promise.all([loadTasks(), loadTodayTasks()])
  }

  draggedTask.value = null
}
</script>


<template>
  <div class="calendar-page">
    <!-- 自然语言建日程（对标 Fantastical 快速输入） -->
    <div class="capture-bar">
      <el-input
        ref="captureInputRef"
        v-model="captureInput"
        placeholder="自然语言建日程：如「周五下午3点和张三开会」「明天 14:00 提交材料」"
        clearable
        :disabled="capturing"
        @keydown.enter="onCaptureKeydown"
      >
        <template #prefix>
          <el-icon><Plus /></el-icon>
        </template>
      </el-input>
      <div class="capture-hint">
        <span>回车创建</span>
        <span class="capture-hint-divider">·</span>
        <span>支持 今天/明天/周X/X月X日/MM-DD + 上午/下午/X点/HH:MM</span>
      </div>
    </div>

    <!-- 工具栏 -->
    <div class="calendar-toolbar">
      <div class="toolbar-left">
        <el-button @click="prevPeriod" :icon="ArrowLeft" circle />
        <el-button @click="goToday" size="small">今天</el-button>
        <span class="month-label">{{ activeView === 'year' ? yearLabel : monthLabel }}</span>
        <el-button @click="nextPeriod" :icon="ArrowRight" circle />
      </div>

      <div class="toolbar-right">
        <el-radio-group v-model="activeView" size="small">
          <el-radio-button
            v-for="view in viewOptions"
            :key="view.key"
            :value="view.key"
          >
            {{ view.label }}
          </el-radio-button>
        </el-radio-group>
      </div>
    </div>

    <!-- 月视图 -->
    <div v-if="activeView === 'month'" class="calendar-container">
      <div class="calendar-grid">
        <!-- 星期头 -->
        <div v-for="day in weekDays" :key="day" class="weekday-header">{{ day }}</div>

        <!-- 日期格子 -->
        <div
          v-for="(day, idx) in calendarDays"
          :key="idx"
          :class="['day-cell', {
            'other-month': !day.isCurrentMonth,
            'today': isToday(day.date),
            'selected': selectedDay && day.date.toDateString() === selectedDay.date.toDateString(),
            'has-hard': getDayStatus(day.date) === 'hard',
            'has-overdue': getDayStatus(day.date) === 'overdue',
            'has-due-soon': getDayStatus(day.date) === 'due-soon',
          }]"
          @click="selectDay(day)"
          @dragover.prevent="onDragOver(formatDate(day.date), $event)"
          @drop.prevent="onMonthDrop($event, day)"
        >
          <div class="day-header">
            <span class="day-number">{{ day.date.getDate() }}</span>
            <span v-if="getDayStatus(day.date) === 'hard'" class="day-indicator hard">
              <el-icon :size="12"><Bell /></el-icon>
            </span>
            <span v-else-if="getDayStatus(day.date) === 'overdue'" class="day-indicator overdue">
              <el-icon :size="12"><Warning /></el-icon>
            </span>
          </div>

          <div class="day-events">
            <!-- 硬性日程 -->
            <div
              v-for="event in eventsForDay(day.date).filter(e => e.type === 'court' || e.type === 'hearing').slice(0, 2)"
              :key="event.id"
              class="event-badge hard"
              :title="event.title"
            >
              <span class="event-text">{{ event.title }}</span>
            </div>

            <!-- 期限 -->
            <div
              v-for="event in eventsForDay(day.date).filter(e => e.type?.startsWith('deadline')).slice(0, 1)"
              :key="event.id"
              class="event-badge deadline"
              :style="{ backgroundColor: getEventBgColor(event), color: getEventColor(event) }"
              :title="event.title"
            >
              <span class="event-text">{{ event.title }}</span>
            </div>

            <!-- 独立日程（D-7）：可拖拽改期 -->
            <div
              v-for="event in eventsForDay(day.date).filter(e => e.type === 'event').slice(0, 2)"
              :key="'ev-' + event.id"
              class="event-badge event"
              :style="{ backgroundColor: getEventBgColor(event), color: getEventColor(event) }"
              :title="event.title + '（拖拽可改期）'"
              draggable="true"
              @dragstart="onEventDragStart(event, $event)"
            >
              <span class="event-text">{{ event.time ? event.time + ' ' : '' }}{{ event.title }}</span>
            </div>

            <!-- 任务（可拖拽改期） -->
            <div
              v-for="task in tasksForDay(day.date).slice(0, 2)"
              :key="task.id"
              class="event-badge task"
              :title="task.taskName + '（拖拽可改期）'"
              draggable="true"
              @dragstart="onDragStart(task, $event)"
            >
              <span class="event-text">{{ task.dueTime ? task.dueTime + ' ' : '' }}{{ task.taskName }}</span>
            </div>

            <!-- 更多提示 -->
            <div
              v-if="eventsForDay(day.date).length + tasksForDay(day.date).length > 4"
              class="event-more"
            >
              +{{ eventsForDay(day.date).length + tasksForDay(day.date).length - 4 }}
            </div>
          </div>
        </div>
      </div>

      <!-- 选中日期详情 -->
      <div v-if="selectedDay" class="selected-day-panel">
        <div class="panel-header">
          <h3>{{ formatDate(selectedDay.date) }}</h3>
          <div class="panel-stats">
            <span v-if="selectedDayStats.hardSchedule > 0" class="stat hard">
              <el-icon><Bell /></el-icon>
              {{ selectedDayStats.hardSchedule }} 硬性日程
            </span>
            <span v-if="selectedDayStats.deadlines > 0" class="stat deadline">
              <el-icon><Clock /></el-icon>
              {{ selectedDayStats.deadlines }} 期限
            </span>
            <span v-if="selectedDayStats.tasks > 0" class="stat task">
              <el-icon><Calendar /></el-icon>
              {{ selectedDayStats.tasks }} 任务
            </span>
            <span v-if="selectedDayStats.overdue > 0" class="stat overdue">
              <el-icon><Warning /></el-icon>
              {{ selectedDayStats.overdue }} 逾期
            </span>
          </div>
        </div>

        <div class="panel-content">
          <!-- 硬性日程 -->
          <div v-if="selectedDayEvents.filter(e => e.type === 'court' || e.type === 'hearing').length > 0">
            <h4>硬性日程</h4>
            <div
              v-for="event in selectedDayEvents.filter(e => e.type === 'court' || e.type === 'hearing')"
              :key="event.id"
              class="detail-event hard"
            >
              <el-icon :color="getEventColor(event)"><Bell /></el-icon>
              <div class="event-info">
                <span class="event-title">{{ event.title }}</span>
                <span class="event-case" v-if="event.caseName">{{ event.caseName }}</span>
              </div>
            </div>
          </div>

          <!-- 期限 -->
          <div v-if="selectedDayEvents.filter(e => e.type?.startsWith('deadline')).length > 0">
            <h4>期限</h4>
            <div
              v-for="event in selectedDayEvents.filter(e => e.type?.startsWith('deadline'))"
              :key="event.id"
              class="detail-event deadline"
            >
              <el-icon :color="getEventColor(event)"><Warning /></el-icon>
              <div class="event-info">
                <span class="event-title">{{ event.title }}</span>
                <span class="event-case" v-if="event.caseName">{{ event.caseName }}</span>
              </div>
            </div>
          </div>

          <!-- 任务 -->
          <div v-if="selectedDayTasks.length > 0">
            <h4>任务</h4>
            <div
              v-for="task in selectedDayTasks"
              :key="task.id"
              class="detail-task"
              draggable="true"
              @dragstart="onDragStart(task, $event)"
              @click="router.push({ name: 'tasks', query: { edit: task.id } })"
            >
              <el-icon :color="COLORS.plan"><Finished /></el-icon>
              <div class="task-info">
                <span class="task-name">{{ task.taskName }}</span>
                <span class="task-meta">
                  <span v-if="task.caseName">{{ task.caseName }}</span>
                  <span v-if="task.estimatedMinutes">{{ task.estimatedMinutes }}分钟</span>
                </span>
              </div>
            </div>
          </div>

          <!-- 空状态 -->
          <div v-if="selectedDayEvents.length === 0 && selectedDayTasks.length === 0" class="empty-day">
            当日无事件
          </div>
        </div>
      </div>
    </div>

    <!-- 年视图（对标 Fantastical 年视图：12 迷你月 + 负载热度，点击下钻） -->
    <div v-if="activeView === 'year'" class="year-container">
      <div class="year-grid">
        <div v-for="(matrix, mi) in yearMatrices" :key="mi" class="year-month">
          <div class="year-month-title" @click="jumpToMonth(mi)">{{ mi + 1 }}月</div>
          <div class="year-weekdays">
            <span v-for="d in weekDays" :key="d">{{ d }}</span>
          </div>
          <div class="year-days">
            <span
              v-for="(cell, ci) in matrix"
              :key="ci"
              class="year-day"
              :class="[`yl-${dayLoadLevel(cell.date)}`, {
                outside: !cell.isCurrentMonth,
                today: isToday(cell.date),
              }]"
              :title="`${cell.date.getMonth() + 1}月${cell.date.getDate()}日 · ${eventsForDay(cell.date).length + tasksForDay(cell.date).length} 项`"
              @click="jumpToDay(cell)"
            >{{ cell.date.getDate() }}</span>
          </div>
        </div>
      </div>
      <div class="year-legend">
        <span>负载：</span>
        <span class="year-day yl-0">·</span><span class="year-day yl-1">1</span><span class="year-day yl-2">2-3</span><span class="year-day yl-3">4-6</span><span class="year-day yl-4">7+</span>
        <em>点击日期进入日视图 · 点击月份标题进入月视图</em>
      </div>
    </div>

    <!-- 周视图（时间块 · Google Calendar 式时长定位） -->
    <div v-if="activeView === 'week'" class="week-container">
      <div class="week-grid">
        <!-- 时间轴 + 星期头 -->
        <div class="week-header-row">
          <div class="week-time-gutter" />
          <div v-for="day in 7" :key="day" class="week-day-header" :class="{ today: isToday(getWeekDay(day)) }">
            <div class="week-day-name">周{{ weekDays[day - 1] }}</div>
            <div class="week-day-number" :class="{ 'today-num': isToday(getWeekDay(day)) }">
              {{ getWeekDay(day).getDate() }}
            </div>
          </div>
        </div>

        <!-- 全天 / 无固定时刻行 -->
        <div class="week-allday-row">
          <div class="week-time-label">全天</div>
          <div
            v-for="day in 7"
            :key="'a' + day"
            class="week-allday-cell"
            :class="{ today: isToday(getWeekDay(day)) }"
          >
            <div
              v-for="p in weekColumns[day - 1].allday"
              :key="p.ev.id"
              class="week-event allday-chip"
              :style="{ borderLeftColor: getEventColor(p.ev), background: getEventBgColor(p.ev) }"
            >
              <span class="week-event-title">{{ p.ev.title }}</span>
            </div>
            <div
              class="week-dropzone"
              @dragover.prevent
              @drop.prevent="onDropToSlot($event, day, null)"
            />
          </div>
        </div>

        <!-- 时段主体：背景小时线 + 绝对定位事件层（时长渲染 + 重叠分列） -->
        <div class="week-body">
          <div v-for="hour in weekHours" :key="hour" class="week-hour-row">
            <div class="week-time-label">{{ String(hour).padStart(2, '0') }}:00</div>
            <div
              v-for="day in 7"
              :key="day"
              class="week-cell-bg"
              :class="{ today: isToday(getWeekDay(day)) }"
            />
          </div>

          <div
            v-for="day in 7"
            :key="'c' + day"
            class="week-day-layer"
            :style="{
              left: `calc(56px + (100% - 56px) * ${(day - 1) / 7})`,
              width: `calc((100% - 56px) / 7)`,
            }"
            @dragover.prevent
            @drop.prevent="onDropToSlot($event, day, 'timed')"
          >
            <div
              v-for="p in weekColumns[day - 1].timed"
              :key="p.ev.id"
              class="week-event-abs"
              :class="{ 'week-event-task': p.ev.isTask }"
              :style="{
                ...p.style,
                borderLeftColor: p.ev.isTask ? '#3E5C9A' : getEventColor(p.ev),
                background: p.ev.isTask ? '#EFF4FC' : getEventBgColor(p.ev),
              }"
            >
              <span class="we-title">{{ p.ev.title }}</span>
              <span class="we-time">{{ p.label }}</span>
            </div>
          </div>

          <!-- 当前时刻线（相对时段主体，仅今日列） -->
          <div v-if="nowLineStyle" class="now-line" :style="nowLineStyle" />
        </div>
      </div>
    </div>

    <!-- 日视图（硬性/弹性/成长时间块 · 设计哲学 §7） -->
    <div v-if="activeView === 'day'" class="day-container">
      <div class="day-grid">
        <!-- 左：小时时间轴（Sunsama 式 · §7 硬性红/弹性蓝语义着色） -->
        <div class="day-timeline">
          <div class="day-header">
            <span class="day-header-date">{{ formatDate(selectedDay?.date || new Date()) }}</span>
            <span class="day-header-weekday">周{{ weekDays[(selectedDay?.date || new Date()).getDay() === 0 ? 6 : (selectedDay?.date || new Date()).getDay() - 1] }}</span>
          </div>

          <div
            class="day-hour-body"
            @dragover.prevent
            @drop.prevent="onDropToDayHour($event)"
          >
            <div v-for="hour in weekHours" :key="hour" class="week-hour-row">
              <span class="week-time-label">{{ String(hour).padStart(2, '0') }}:00</span>
              <div class="day-cell-bg" />
            </div>
            <div class="day-event-layer">
              <div
                v-for="p in dayTimedItems"
                :key="p.ev.id"
                class="week-event-abs"
                :class="{ 'week-event-task': p.ev.isTask }"
                :style="{
                  ...p.style,
                  borderLeftColor: p.ev.isTask ? '#3E5C9A' : getEventColor(p.ev),
                  background: p.ev.isTask ? '#EFF4FC' : getEventBgColor(p.ev),
                }"
              >
                <span class="we-title">{{ p.ev.title }}</span>
                <span class="we-time">{{ p.time }}</span>
              </div>
            </div>
            <div
              v-if="isTodaySelected && nowLineStyle"
              class="now-line"
              :style="{ top: `${nowLineTop}%`, left: '44px', right: '6px', width: 'auto' }"
            />
          </div>

          <!-- 提示 -->
          <div class="day-tip">
            右侧「弹性任务」可拖到左侧时间轴排期；硬性日程（庭审/期限）以红色块呈现，不可移动。
          </div>
        </div>

        <!-- 右：当日议程 -->
        <div class="day-agenda">
          <div class="card">
            <div class="card-header">当日议程</div>
            <div
              v-for="event in (selectedDayEvents.length > 0 ? selectedDayEvents : eventsForDay(new Date()))"
              :key="event.id"
              class="agenda-item"
            >
              <span class="agenda-dot" :style="{ background: getEventColor(event) }"></span>
              <div class="agenda-info">
                <span class="agenda-title">{{ event.title }}</span>
                <span class="agenda-time">{{ event.time }} · {{ getEventTypeLabel(event.type) }}</span>
              </div>
            </div>
            <div v-if="(selectedDayEvents.length > 0 ? selectedDayEvents : eventsForDay(new Date())).length === 0" class="day-empty">
              当日无事件
            </div>
          </div>

          <div class="card" style="margin-top: 14px;">
            <div class="card-header">当日到期任务</div>
            <div
              v-for="task in (selectedDayTasks.length > 0 ? selectedDayTasks : tasksForDay(new Date()))"
              :key="task.id"
              class="agenda-task"
            >
              <el-checkbox
                :model-value="!!task.completed"
                @change="toggleAgendaTask(task)"
              />
              <span class="agenda-task-name" :class="{ done: !!task.completed }">{{ task.taskName }}</span>
            </div>
            <div v-if="(selectedDayTasks.length > 0 ? selectedDayTasks : tasksForDay(new Date())).length === 0" class="day-empty">
              无到期任务
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Forecast 双栏（对标 Fantastical） -->
    <div v-if="activeView === 'forecast'" class="forecast-container">
      <!-- 左栏：按日分组的日程/期限列表 -->
      <div class="forecast-left">
        <div class="forecast-section-header">
          <h3>未来日程与期限</h3>
          <span class="forecast-hint">拖拽任务到日期可改期</span>
        </div>
        <div class="forecast-day-groups">
          <div
            v-for="group in forecastGroups"
            :key="group.dateStr"
            :class="['forecast-day-group', { 'is-today': group.isToday, 'is-overdue': group.isOverdue }]"
            @dragover="onDragOver(group.dateStr, $event)"
            @drop="onDrop(group.dateStr, $event)"
          >
            <div class="forecast-group-header">
              <span class="forecast-group-label" :class="{ 'today': group.isToday, 'overdue': group.isOverdue }">{{ group.label }}</span>
              <span class="forecast-group-weekday" v-if="group.weekDay">周{{ group.weekDay }}</span>
              <span class="forecast-group-count" :class="{ 'has-items': group.items.length > 0 }">
                {{ group.items.length > 0 ? `${group.items.length} 项` : '无安排' }}
              </span>
            </div>
            <div class="forecast-group-items">
              <div
                v-for="(item, idx) in group.items"
                :key="idx"
                class="forecast-list-item"
                :class="item.kind"
              >
                <el-icon :size="14" :color="item.color"><component :is="item.icon" /></el-icon>
                <div class="forecast-list-info">
                  <span class="forecast-list-title">{{ item.title }}</span>
                  <span class="forecast-list-meta">
                    <template v-if="item.time">{{ item.time }}</template>
                    <template v-if="item.caseName"><template v-if="item.time"> · </template>{{ item.caseName }}</template>
                    <template v-if="item.daysLeft !== null && item.daysLeft !== undefined"><template v-if="item.time || item.caseName"> · </template>{{ item.daysLeft }} 天</template>
                  </span>
                </div>
              </div>
            </div>
            <div v-if="group.items.length === 0" class="forecast-group-empty">无安排</div>
          </div>
        </div>
      </div>

      <!-- 右栏：当日时间分配网格（§7.2 时间块分区，硬性/弹性视觉分区） -->
      <div class="forecast-right">
        <div class="forecast-timeline">
          <div class="forecast-section-header">
            <h3>今日时间分配</h3>
            <span class="forecast-today-date">{{ todayLabel }}</span>
          </div>

          <!-- 时间分配提示（§7.3）：硬性占满多个时间块 → 推荐空块 -->
          <div v-if="allocationTip" class="timeline-tip">
            {{ allocationTip }}
          </div>

          <!-- 时间块分区网格（§7.2）：上午/下午/晚上/其他 + 弹性 -->
          <div
            v-for="block in todayBlocks"
            :key="block.key"
            :class="['timeline-section', { 'flex-section': block.key === 'flex' }]"
          >
            <div class="timeline-section-label" :class="block.key">
              <span class="block-name">{{ block.label }}</span>
              <span class="block-range">{{ block.range }}</span>
              <span class="timeline-count" :class="{ 'has-items': block.items.length > 0 }">
                {{ block.items.length }}
              </span>
            </div>
            <div
              v-for="item in block.items"
              :key="item.uid"
              :class="['timeline-slot', item.kind, { done: item.done }]"
              :draggable="!!item.task"
              @dragstart="item.task && onDragStart(item.task, $event)"
              @click="item.task && router.push({ name: 'tasks', query: { edit: item.task.id } })"
            >
              <span class="timeline-time">{{ itemTimeLabel(item) }}</span>
              <span class="timeline-title">{{ item.title }}</span>
              <span v-if="item.caseName" class="timeline-case">{{ item.caseName }}</span>
            </div>
            <div v-if="block.items.length === 0" class="timeline-empty">
              {{ block.key === 'flex' ? '今日暂无弹性任务' : '本块无安排' }}
            </div>
          </div>

          <div class="forecast-tip">
            先硬性 → 再弹性。拖拽任务到左栏日期 = 改期。
          </div>
        </div>
      </div>
    </div>

    <!-- 图例 -->
    <div class="calendar-legend">
      <span class="legend-item">
        <span class="legend-dot" style="background: #B4554F" />
        硬性（开庭/口审）
      </span>
      <span class="legend-item">
        <span class="legend-dot" style="background: #B0823A" />
        期限
      </span>
      <span class="legend-item">
        <span class="legend-dot" style="background: #3E5C9A" />
        弹性任务
      </span>
      <span class="legend-item">
        <span class="legend-dot" style="background: #4C8067" />
        已完成
      </span>
      <span class="legend-item">
        <span class="legend-dot" style="background: #6C6A9C" />
        二审
      </span>
    </div>
  </div>
</template>


<style scoped>
/* 周视图任务（综合显示：日历+待办合一） */
.week-event-task {
  cursor: pointer;
}
.week-event-task .week-event-title {
  font-weight: 500;
}
/* 时间轴放置区（拖任务到时间点） */
.week-dropzone {
  position: absolute;
  inset: 0;
  z-index: 2;
}
.week-cell {
  position: relative;
}
.calendar-page {
  max-width: 1200px;
  margin: 0 auto;
  padding: 20px;
}

/* ============================================================
   自然语言建日程条（对标 Fantastical 快速输入）
   ============================================================ */
.capture-bar {
  margin-bottom: 14px;
}

.capture-bar .el-input__wrapper {
  border-radius: var(--c-radius-lg, 8px);
  box-shadow: 0 0 0 1px var(--c-border, #E0E3E9) inset;
  background: #FFFFFF;
}

.capture-bar .el-input__wrapper.is-focus {
  box-shadow: 0 0 0 1px var(--c-primary, var(--c-primary)) inset;
}

.capture-hint {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 6px;
  font-size: 11px;
  color: var(--c-text-secondary, var(--c-text-secondary));
}

.capture-hint-divider {
  color: var(--c-border, #E0E3E9);
}

/* 工具栏 */
.calendar-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
}

.toolbar-left {
  display: flex;
  align-items: center;
  gap: 12px;
}

.month-label {
  font-size: 18px;
  font-weight: 600;
  min-width: 120px;
  text-align: center;
  color: var(--c-text);
}

/* 日历网格 */
.calendar-container {
  display: flex;
  gap: 20px;
}

.calendar-grid {
  flex: 1;
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  gap: 1px;
  background: var(--c-border);
  border: 1px solid var(--c-border);
  border-radius: 8px;
  overflow: hidden;
}

.weekday-header {
  background: #F5F5F5;
  padding: 8px;
  text-align: center;
  font-size: 13px;
  font-weight: 500;
  color: var(--c-text-regular);
}

.day-cell {
  background: #FFFFFF;
  padding: 6px;
  min-height: 100px;
  cursor: pointer;
  transition: background var(--motion-fast);
}

.day-cell:hover {
  background: #FAFAFA;
}

.day-cell.other-month {
  background: #F9F9F9;
  opacity: 0.6;
}

.day-cell.today {
  background: #EDF1F8;
}

.day-cell.selected {
  background: #C3CFE3;
  box-shadow: inset 0 0 0 2px var(--c-primary);
}

.day-cell.has-hard {
  border-top: 2px solid var(--c-danger);
}

.day-cell.has-overdue {
  border-top: 2px solid var(--c-warning);
}

.day-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 4px;
}

.day-number {
  font-size: 13px;
  font-weight: 500;
  color: var(--c-text);
}

.day-cell.other-month .day-number {
  color: var(--c-text-secondary);
}

.day-cell.today .day-number {
  color: var(--c-primary);
  font-weight: 600;
}

.day-indicator {
  display: flex;
  align-items: center;
}

.day-indicator.hard {
  color: var(--c-danger);
}

.day-indicator.overdue {
  color: var(--c-warning);
}

.day-events {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.event-badge {
  padding: 2px 4px;
  border-radius: 3px;
  font-size: 11px;
  line-height: 1.3;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.event-badge.hard {
  background: #F6EDEC;
  color: var(--c-danger);
}

.event-badge.deadline {
  background: #F7F1E3;
  color: var(--c-warning);
}

.event-badge.task {
  background: #EDF1F8;
  color: var(--c-primary);
}

.event-text {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.event-more {
  font-size: 10px;
  color: var(--c-text-secondary);
  text-align: center;
}

/* 选中日期面板 */
.selected-day-panel {
  width: 300px;
  background: #FFFFFF;
  border-radius: 8px;
  border: 1px solid var(--c-border);
  overflow: hidden;
}

.panel-header {
  padding: 16px;
  background: #FAFAFA;
  border-bottom: 1px solid var(--c-border);
}

.panel-header h3 {
  margin: 0 0 8px;
  font-size: 16px;
  font-weight: 600;
  color: var(--c-text);
}

.panel-stats {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.stat {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  padding: 2px 8px;
  border-radius: 4px;
}

.stat.hard {
  background: #F6EDEC;
  color: var(--c-danger);
}

.stat.deadline {
  background: #F7F1E3;
  color: var(--c-warning);
}

.stat.task {
  background: #EDF1F8;
  color: var(--c-primary);
}

.stat.overdue {
  background: #F6EDEC;
  color: var(--c-danger);
}

.panel-content {
  padding: 16px;
  max-height: 400px;
  overflow-y: auto;
}

.panel-content h4 {
  margin: 0 0 8px;
  font-size: 13px;
  font-weight: 600;
  color: var(--c-text-regular);
}

.detail-event,
.detail-task {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 8px;
  border-radius: 6px;
  margin-bottom: 8px;
  cursor: pointer;
  transition: background var(--motion-fast);
}

.detail-event:hover,
.detail-task:hover {
  background: var(--gray-50);
}

.event-info,
.task-info {
  flex: 1;
  min-width: 0;
}

.event-title,
.task-name {
  display: block;
  font-size: 13px;
  font-weight: 500;
  color: var(--c-text);
  margin-bottom: 2px;
}

.event-case,
.task-meta {
  display: block;
  font-size: 12px;
  color: var(--c-text-secondary);
}

.empty-day {
  text-align: center;
  color: var(--c-text-secondary);
  font-size: 13px;
  padding: 20px;
}

/* 图例 */
.calendar-legend {
  display: flex;
  flex-wrap: wrap;
  gap: 16px;
  margin-top: 20px;
  padding: 12px;
  background: #FAFAFA;
  border-radius: 8px;
}

.legend-item {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--c-text-regular);
}

.legend-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
}

/* ============================================================
   年视图样式（12 迷你月 + 负载热度 · 对标 Fantastical Year）
   ============================================================ */
.year-container {
  background: #FFFFFF;
  border: 1px solid var(--c-border);
  border-radius: 8px;
  padding: 20px;
}
.year-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 20px 16px;
}
@media (max-width: 1100px) {
  .year-grid { grid-template-columns: repeat(2, 1fr); }
}
.year-month {
  min-width: 0;
}
.year-month-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--c-text);
  margin-bottom: 6px;
  cursor: pointer;
  width: fit-content;
  padding: 1px 6px;
  border-radius: 4px;
  transition: background var(--motion-fast) var(--ease-out);
}
.year-month-title:hover {
  background: var(--c-bg-hover, var(--c-bg-hover));
  color: var(--c-primary, var(--c-primary));
}
.year-weekdays {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  gap: 2px 0;
  margin-bottom: 3px;
}
.year-weekdays span {
  text-align: center;
  font-size: 10px;
  color: var(--c-text-secondary);
}
.year-days {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  gap: 2px 0;
}
.year-day {
  aspect-ratio: 1 / 1;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 11px;
  line-height: 1;
  border-radius: 4px;
  color: var(--c-text-regular);
  cursor: pointer;
  transition:
    transform var(--motion-fast) var(--ease-out),
    box-shadow var(--motion-fast) var(--ease-out);
}
.year-day:hover {
  transform: scale(1.18);
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.15);
  z-index: 1;
  position: relative;
}
/* 负载热度：绿→琥珀渐进（与任务语义色一致），硬性日程日由 title 提示 */
.year-day.yl-0 { color: var(--gray-300); background: transparent; }
.year-day.outside { opacity: 0.35; }
.year-day.yl-1 { background: #DCE8DF; }
.year-day.yl-2 { background: #B9D4C0; }
.year-day.yl-3 { background: #8FBDA0; color: #FFFFFF; }
.year-day.yl-4 { background: var(--c-success); color: #FFFFFF; font-weight: 600; }
.year-day.today {
  box-shadow: inset 0 0 0 2px var(--c-primary, var(--c-primary));
  font-weight: 700;
}
.year-legend {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 16px;
  font-size: 12px;
  color: var(--c-text-secondary);
}
.year-legend .year-day {
  aspect-ratio: auto;
  width: 22px;
  height: 18px;
  cursor: default;
}
.year-legend .year-day:hover { transform: none; box-shadow: none; }
.year-legend em {
  margin-left: 10px;
  font-style: normal;
  color: var(--gray-300);
}

/* ============================================================
   周视图样式（时间块）
   ============================================================ */
.week-container {
  background: #FFFFFF;
  border: 1px solid var(--c-border);
  border-radius: 8px;
  overflow: auto;
  max-height: calc(100vh - 200px);
}

.week-grid {
  min-width: 700px;
  position: relative;
}

/* 当前时刻线（Google Calendar/Sunsama 惯例）：位置由 nowLineStyle 内联计算 */
.now-line {
  position: absolute;
  height: 2px;
  background: #E5484D;
  z-index: 3;
  pointer-events: none;
}
.now-line::before {
  content: '';
  position: absolute;
  left: -5px;
  top: -3px;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #E5484D;
}

.week-header-row {
  display: grid;
  grid-template-columns: 56px repeat(7, 1fr);
  border-bottom: 1px solid var(--c-border);
  position: sticky;
  top: 0;
  background: #FFFFFF;
  z-index: 2;
}

.week-time-gutter {
  background: #FAFAFA;
}

.week-day-header {
  padding: 8px 0;
  text-align: center;
  border-left: 1px solid var(--c-border-light);
}

.week-day-header.today {
  background: #EDF1F8;
}

.week-day-name {
  font-size: 11px;
  color: var(--c-text-secondary);
  letter-spacing: .5px;
}

.week-day-number {
  font-size: 16px;
  font-weight: 600;
  color: var(--c-text);
  margin-top: 2px;
}

.week-day-number.today-num {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  background: var(--c-primary);
  color: #fff;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.week-hour-row {
  display: grid;
  grid-template-columns: 56px repeat(7, 1fr);
  border-bottom: 1px solid var(--c-border-light);
  min-height: 48px;
}

.week-time-label {
  font-size: 11px;
  color: var(--c-text-secondary);
  font-family: var(--font-mono);
  text-align: right;
  padding: 4px 8px 0 0;
}

.week-cell {
  border-left: 1px solid var(--c-border-light);
  padding: 2px 3px;
  position: relative;
}

.week-cell.today {
  background: rgba(62, 92, 154, 0.03);
}

.week-event {
  font-size: 11px;
  padding: 2px 6px;
  border-radius: 3px;
  border-left: 2px solid;
  margin-bottom: 2px;
  cursor: pointer;
  overflow: hidden;
}

.week-event-title {
  display: block;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  font-weight: 500;
}

.week-event-time {
  font-size: 10px;
  color: var(--c-text-secondary);
}

/* ── 周视图 v2：时长定位层（Google Calendar 式）── */
.week-allday-row {
  display: grid;
  grid-template-columns: 56px repeat(7, 1fr);
  border-bottom: 1px solid var(--c-border);
  min-height: 30px;
}
.week-allday-cell {
  border-left: 1px solid var(--c-border-light);
  padding: 2px 3px;
  position: relative;
  min-height: 30px;
}
.week-allday-cell.today { background: rgba(62, 92, 154, 0.03); }
.allday-chip {
  display: block;
}

.week-body {
  position: relative;
}
/* 背景小时行：纯线条，不承载内容 */
.week-body .week-hour-row {
  min-height: 48px;
}
.week-cell-bg {
  border-left: 1px solid var(--c-border-light);
}
.week-cell-bg.today { background: rgba(62, 92, 154, 0.03); }

/* 每日定位层：覆盖时段主体的一列，自身接收拖放 */
.week-day-layer {
  position: absolute;
  top: 0;
  bottom: 0;
  pointer-events: auto;
}
.week-event-abs {
  position: absolute;
  overflow: hidden;
  font-size: 11px;
  padding: 3px 6px;
  border-radius: 4px;
  border-left: 3px solid;
  cursor: pointer;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06);
  transition: box-shadow var(--motion-fast) var(--ease-out);
  z-index: 1;
}
.week-event-abs:hover {
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.14);
  z-index: 2;
}
.we-title {
  display: block;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  font-weight: 500;
  color: var(--c-text);
}
.we-time {
  display: block;
  font-size: 10px;
  color: var(--gray-500);
}

/* ============================================================
   日视图样式（硬性/弹性/成长时间块）
   ============================================================ */
.day-container {
  background: #FFFFFF;
  border: 1px solid var(--c-border);
  border-radius: 8px;
  padding: 16px;
}

.day-grid {
  display: grid;
  grid-template-columns: 1fr 300px;
  gap: 16px;
}

/* ── 日视图时间轴（复用 week-hour-row / week-time-label / now-line）── */
.day-hour-body {
  position: relative;
  background: #fff;
  border: 1px solid var(--c-border);
  border-radius: 8px;
  overflow: hidden;
}
/* 单日列：覆盖 week-hour-row 的 7 列模板 */
.day-hour-body .week-hour-row {
  grid-template-columns: 44px 1fr;
}
.day-cell-bg {
  border-left: 1px solid var(--c-border-lighter);
}
.day-event-layer {
  position: absolute;
  inset: 0;
}

.day-header {
  margin-bottom: 16px;
}

.day-header-date {
  font-size: 18px;
  font-weight: 700;
  color: var(--c-text);
}

.day-header-weekday {
  font-size: 14px;
  color: var(--c-text-secondary);
  margin-left: 8px;
}

.day-section {
  margin-bottom: 16px;
}

.day-section-label {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 600;
  padding: 6px 0;
  margin-bottom: 8px;
  border-bottom: 1px solid var(--c-border-light);
}

.day-section-label.hard { color: var(--c-danger); }
.day-section-label.flex { color: var(--c-primary); }
.day-section-label.grow { color: var(--c-success); }

.day-slot {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  border-radius: 6px;
  margin-bottom: 6px;
  border: 1px solid #E0E3E9;
  background: #FFFFFF;
  cursor: pointer;
  transition: all var(--motion-fast) var(--ease-out);
}

.day-slot:hover {
  border-color: #CDD2DB;
}

.day-slot.hard {
  border-left: 3px solid var(--c-danger);
}

.day-slot.flex {
  border-left: 3px solid var(--c-primary);
}

.slot-time {
  font-family: var(--font-mono);
  font-size: 12px;
  color: var(--c-text-regular);
  width: 44px;
  flex-shrink: 0;
}

.slot-title {
  flex: 1;
  font-size: 13px;
  color: var(--c-text);
}

.day-empty {
  text-align: center;
  padding: 16px;
  color: var(--c-text-secondary);
  font-size: 12px;
}

.day-tip {
  font-size: 12px;
  color: var(--c-text-secondary);
  padding: 8px 0;
  border-top: 1px dashed #E0E3E9;
  margin-top: 8px;
}

.day-agenda .card-header {
  font-size: 12px;
  font-weight: 700;
  color: var(--c-text);
  padding-bottom: 10px;
  margin-bottom: 10px;
  border-bottom: 1px solid var(--c-border-light);
}

.agenda-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 0;
  border-bottom: 1px solid var(--c-border-light);
}

.agenda-item:last-child { border-bottom: none; }

.agenda-dot {
  width: 4px;
  height: 28px;
  border-radius: 2px;
  flex-shrink: 0;
}

.agenda-info {
  flex: 1;
}

.agenda-title {
  display: block;
  font-size: 13px;
  font-weight: 500;
  color: var(--c-text);
}

.agenda-time {
  display: block;
  font-size: 11px;
  color: var(--c-text-secondary);
  margin-top: 1px;
}

.agenda-task {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 0;
}

.agenda-task-name {
  font-size: 13px;
  color: var(--c-text);
}

/* ============================================================
   Forecast 双栏（对标 Fantastical）
   ============================================================ */
.forecast-container {
  display: flex;
  gap: 20px;
  align-items: flex-start;
}

/* 左栏：按日分组列表 */
.forecast-left {
  flex: 1;
  min-width: 0;
  background: #FFFFFF;
  border-radius: 8px;
  border: 1px solid var(--c-border);
  overflow: hidden;
}

.forecast-section-header {
  padding: 12px 16px;
  background: #FAFAFA;
  border-bottom: 1px solid var(--c-border);
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.forecast-section-header h3 {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
  color: var(--c-text);
}

.forecast-hint {
  font-size: 11px;
  color: var(--c-text-secondary);
}

.forecast-day-groups {
  max-height: calc(100vh - 300px);
  overflow-y: auto;
  padding: 4px 0;
}

.forecast-day-group {
  border-bottom: 1px solid var(--c-border-light);
  padding: 8px 16px;
  transition: background var(--motion-fast);
}

.forecast-day-group:hover {
  background: #FAFAFA;
}

.forecast-day-group.is-today {
  background: #EDF1F8;
}

.forecast-day-group.is-overdue {
  background: #F6EDEC;
}

.forecast-group-label.overdue {
  color: var(--c-danger);
}

.forecast-group-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 4px;
}

.forecast-group-label {
  font-size: 13px;
  font-weight: 600;
  color: var(--c-text);
  min-width: 44px;
}

.forecast-group-label.today {
  color: var(--c-primary);
}

.forecast-group-weekday {
  font-size: 11px;
  color: var(--c-text-secondary);
}

.forecast-group-count {
  margin-left: auto;
  font-size: 11px;
  color: var(--c-text-secondary);
}

.forecast-group-count.has-items {
  color: var(--c-primary);
  font-weight: 500;
}

.forecast-group-items {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.forecast-list-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border-radius: 6px;
  border-left: 3px solid transparent;
  background: #FAFAFA;
}

.forecast-list-item.hard {
  border-left-color: var(--c-danger);
  background: #F6EDEC;
}

.forecast-list-item.deadline {
  border-left-color: var(--c-warning);
  background: #F7F1E3;
}

.forecast-list-item.warning {
  border-left-color: var(--c-warning);
  background: #F7F1E3;
}

.forecast-list-info {
  flex: 1;
  min-width: 0;
}

.forecast-list-title {
  display: block;
  font-size: 12px;
  font-weight: 500;
  color: var(--c-text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.forecast-list-meta {
  display: block;
  font-size: 11px;
  color: var(--c-text-secondary);
  margin-top: 1px;
}

.forecast-group-empty {
  font-size: 11px;
  color: var(--c-text-secondary);
  padding: 2px 8px;
}

/* 右栏：今日时间轴 */
.forecast-right {
  width: 380px;
  flex-shrink: 0;
}

.forecast-timeline {
  background: #FFFFFF;
  border-radius: 8px;
  border: 1px solid var(--c-border);
  overflow: hidden;
}

.forecast-today-date {
  font-size: 12px;
  color: var(--c-text-secondary);
}

.timeline-section {
  padding: 10px 16px;
  border-bottom: 1px solid var(--c-border-light);
}

.timeline-section-label {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 600;
  padding: 4px 0 8px;
  border-bottom: 1px solid var(--c-border-light);
  margin-bottom: 8px;
}

.timeline-section-label.hard { color: var(--c-danger); }
.timeline-section-label.flex { color: var(--c-primary); }

/* 时间块分区（§7.2）：块范围 / 计数 / 弹性分区底色 */
.timeline-section-label .block-range {
  font-size: 10px;
  font-weight: 400;
  color: var(--c-text-secondary);
  font-family: var(--font-mono, monospace);
}

.timeline-section.flex-section {
  background: #FBFBFD;
}

.timeline-count.has-items {
  color: var(--c-primary);
  background: #EDF1F8;
}

/* 时间分配提示（§7.3） */
.timeline-tip {
  margin: 10px 16px 0;
  padding: 8px 12px;
  border-radius: 6px;
  font-size: 12px;
  line-height: 1.5;
  color: var(--c-warning);
  background: #F7F1E3;
  border-left: 3px solid var(--c-warning);
}

.timeline-count {
  margin-left: auto;
  font-size: 11px;
  font-weight: 500;
  color: var(--c-text-secondary);
  background: var(--gray-50);
  border-radius: 999px;
  padding: 0 8px;
  line-height: 16px;
}

.timeline-slot {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border-radius: 6px;
  margin-bottom: 6px;
  border: 1px solid #E0E3E9;
  background: #FFFFFF;
  cursor: pointer;
  transition: border-color var(--motion-fast);
}

.timeline-slot:hover {
  border-color: #CDD2DB;
}

.timeline-slot.hard {
  border-left: 3px solid var(--c-danger);
  background: #F6EDEC;
}

.timeline-slot.flex {
  border-left: 3px solid var(--c-primary);
  background: #EDF1F8;
}

.timeline-slot.flex.done {
  border-left-color: var(--c-success);
  background: #EDF3EF;
  opacity: 0.75;
}

.timeline-slot.flex.done .timeline-title {
  color: var(--c-success);
  text-decoration: line-through;
}

.timeline-time {
  font-family: var(--font-mono);
  font-size: 12px;
  color: var(--c-text-regular);
  width: 44px;
  flex-shrink: 0;
}

.timeline-title {
  flex: 1;
  min-width: 0;
  font-size: 13px;
  color: var(--c-text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.timeline-case {
  font-size: 11px;
  color: var(--c-text-secondary);
  flex-shrink: 0;
  max-width: 100px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.timeline-empty {
  text-align: center;
  padding: 12px;
  color: var(--c-text-secondary);
  font-size: 12px;
}

.forecast-tip {
  font-size: 11px;
  color: var(--c-text-secondary);
  padding: 10px 16px;
  border-top: 1px dashed #E0E3E9;
}
</style>
