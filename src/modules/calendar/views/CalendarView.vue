<script setup>
import { ref, computed, onMounted } from 'vue'
import { ElMessage } from 'element-plus'
import { useRouter } from 'vue-router'
import { casyContext } from '../../../core/plugin/context'
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

const calendarDays = computed(() => {
  const { year, month } = currentMonth.value
  const firstDay = new Date(year, month, 1)
  const lastDay = new Date(year, month + 1, 0)

  // 周一起始
  let startWeekday = firstDay.getDay() - 1
  if (startWeekday < 0) startWeekday = 6

  const days = []

  // 上月末尾
  const prevLastDay = new Date(year, month, 0)
  for (let i = startWeekday - 1; i >= 0; i--) {
    days.push({
      date: new Date(year, month - 1, prevLastDay.getDate() - i),
      isCurrentMonth: false,
    })
  }

  // 本月
  for (let d = 1; d <= lastDay.getDate(); d++) {
    days.push({
      date: new Date(year, month, d),
      isCurrentMonth: true,
    })
  }

  // 下月开头
  const remaining = 42 - days.length
  for (let d = 1; d <= remaining; d++) {
    days.push({
      date: new Date(year, month + 1, d),
      isCurrentMonth: false,
    })
  }

  return days
})

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
  // 加载当前月 + 下月，保证 Forecast 未来窗口跨月不缺数据
  const months = []
  const current = new Date(year, month, 1)
  for (let i = 0; i < 2; i++) {
    const d = new Date(current.getFullYear(), current.getMonth() + i, 1)
    months.push({ year: d.getFullYear(), month: d.getMonth() + 1 })
  }
  const results = await Promise.all(months.map(m => casyContext.calendar.events(m.year, m.month)))
  const merged = []
  for (const r of results) {
    if (r.ok && Array.isArray(r.data)) merged.push(...r.data)
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
// 导航
// ============================================================
function prevMonth() {
  const d = new Date(currentDate.value)
  d.setMonth(d.getMonth() - 1)
  currentDate.value = d
  loadData()
}

function nextMonth() {
  const d = new Date(currentDate.value)
  d.setMonth(d.getMonth() + 1)
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
  const taskName = parsed && parsed.timeStr ? `${parsed.timeStr} ${title}` : title
  const display = `${dateStr}${parsed && parsed.timeLabel ? ' ' + parsed.timeLabel : ''} · ${title}`

  capturing.value = true
  const result = await casyContext.tasks.create({
    taskName,
    startDate: dateStr,
    dueDate: dateStr,
    startBucket: 'upcoming',
    taskType: 'action',
  })
  capturing.value = false

  if (result.ok) {
    ElMessage.success(`已转为任务：${display}`)
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

// 今日时间轴（右栏）
const todayLabel = computed(() => {
  const d = new Date()
  return `${d.getMonth() + 1}月${d.getDate()}日 周${weekDays[d.getDay() === 0 ? 6 : d.getDay() - 1]}`
})

const todayHardItems = computed(() => {
  const ds = todayStr()
  const items = []
  for (const e of events.value) {
    if (e.date !== ds) continue
    const isHard = e.type === 'court' || e.type === 'hearing'
    const isDeadline = e.type?.startsWith('deadline')
    if (!isHard && !isDeadline) continue
    items.push({
      title: e.title,
      time: e.time || null,
      caseName: e.caseName || '',
      color: getEventColor(e),
      daysLeft: null,
    })
  }
  // 今日到期的期限预警
  for (const w of deadlineWarnings.value) {
    const n = normalizeWarning(w)
    if (n.date === ds) {
      items.push({ title: n.title, time: null, caseName: n.caseName, color: COLORS.due, daysLeft: n.daysLeft })
    }
  }
  items.sort((a, b) => {
    const ta = a.time || '99:99'
    const tb = b.time || '99:99'
    return ta < tb ? -1 : ta > tb ? 1 : 0
  })
  return items
})

const todayFlexTasks = computed(() => {
  return [...todayTasks.value].sort((a, b) => {
    if (!!a.completed !== !!b.completed) return a.completed ? 1 : -1
    return (a.todayIndex || 0) - (b.todayIndex || 0)
  })
})

/**
 * 周视图：获取某天某小时的事件
 */
const weekHours = Array.from({ length: 15 }, (_, i) => i + 7) // 7:00 - 21:00

function getWeekDay(dayIndex) {
  const d = new Date(currentDate.value)
  const currentDay = d.getDay() || 7
  const diff = dayIndex - currentDay
  d.setDate(d.getDate() + diff)
  return d
}

function getWeekDayHourEvents(date, hour) {
  const dateStr = formatDate(date)
  return events.value.filter(e => {
    if (e.date !== dateStr) return false
    if (!e.time) return false
    const eHour = parseInt(e.time.split(':')[0])
    return eHour === hour
  })
}

/**
 * 拖拽相关（改期 = casyContext.tasks.update）
 */
const draggedTask = ref(null)

function onDragStart(task, event) {
  draggedTask.value = task
  event.dataTransfer.effectAllowed = 'move'
  event.dataTransfer.setData('text/plain', task.id)
}

function onDragOver(dateStr, event) {
  event.preventDefault()
  event.dataTransfer.dropEffect = 'move'
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
        <el-button @click="prevMonth" :icon="ArrowLeft" circle />
        <el-button @click="goToday" size="small">今天</el-button>
        <span class="month-label">{{ monthLabel }}</span>
        <el-button @click="nextMonth" :icon="ArrowRight" circle />
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

            <!-- 任务 -->
            <div
              v-for="task in tasksForDay(day.date).slice(0, 2)"
              :key="task.id"
              class="event-badge task"
              :title="task.taskName"
            >
              <span class="event-text">{{ task.taskName }}</span>
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

    <!-- 周视图（时间块） -->
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

        <!-- 时间行 -->
        <div v-for="hour in weekHours" :key="hour" class="week-hour-row">
          <div class="week-time-label">{{ String(hour).padStart(2, '0') }}:00</div>
          <div v-for="day in 7" :key="day" class="week-cell" :class="{ today: isToday(getWeekDay(day)) }">
            <div
              v-for="ev in getWeekDayHourEvents(getWeekDay(day), hour)"
              :key="ev.id"
              class="week-event"
              :style="{ borderLeftColor: getEventColor(ev), background: getEventBgColor(ev) }"
            >
              <span class="week-event-title">{{ ev.title }}</span>
              <span class="week-event-time">{{ ev.time }}</span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- 日视图（硬性/弹性/成长时间块 · 设计哲学 §7） -->
    <div v-if="activeView === 'day'" class="day-container">
      <div class="day-grid">
        <!-- 左：时间轴 -->
        <div class="day-timeline">
          <div class="day-header">
            <span class="day-header-date">{{ formatDate(selectedDay?.date || new Date()) }}</span>
            <span class="day-header-weekday">周{{ weekDays[(selectedDay?.date || new Date()).getDay() === 0 ? 6 : (selectedDay?.date || new Date()).getDay() - 1] }}</span>
          </div>

          <!-- 硬性日程区 -->
          <div class="day-section">
            <div class="day-section-label hard">
              <el-icon><Bell /></el-icon> 硬性日程
            </div>
            <div
              v-for="event in (selectedDayEvents.length > 0 ? selectedDayEvents : eventsForDay(new Date())).filter(e => e.type === 'court' || e.type === 'hearing')"
              :key="event.id"
              class="day-slot hard"
            >
              <span class="slot-time">{{ event.time || '--:--' }}</span>
              <span class="slot-title">{{ event.title }}</span>
              <el-tag size="small" type="danger">硬性</el-tag>
            </div>
            <div v-if="(selectedDayEvents.length > 0 ? selectedDayEvents : eventsForDay(new Date())).filter(e => e.type === 'court' || e.type === 'hearing').length === 0" class="day-empty">
              无硬性日程
            </div>
          </div>

          <!-- 弹性任务区 -->
          <div class="day-section">
            <div class="day-section-label flex">
              <el-icon><Finished /></el-icon> 弹性任务
            </div>
            <div
              v-for="task in (selectedDayTasks.length > 0 ? selectedDayTasks : tasksForDay(new Date())).filter(t => !t.completed)"
              :key="task.id"
              class="day-slot flex"
              draggable="true"
              @dragstart="onDragStart(task, $event)"
            >
              <span class="slot-time">{{ task.estimatedMinutes ? `${task.estimatedMinutes}m` : '--' }}</span>
              <span class="slot-title">{{ task.taskName }}</span>
              <el-tag size="small" type="primary">弹性</el-tag>
            </div>
            <div v-if="(selectedDayTasks.length > 0 ? selectedDayTasks : tasksForDay(new Date())).filter(t => !t.completed).length === 0" class="day-empty">
              无弹性任务
            </div>
          </div>

          <!-- 提示 -->
          <div class="day-tip">
            时间分配遵循「先硬性 → 再弹性 → 最后成长」。拖拽任务到其他日期 = 改期。
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
              <el-checkbox :model-value="!!task.completed" />
              <span class="agenda-task-name">{{ task.taskName }}</span>
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

      <!-- 右栏：当日时间轴（硬性日程 + 弹性任务） -->
      <div class="forecast-right">
        <div class="forecast-timeline">
          <div class="forecast-section-header">
            <h3>今日时间轴</h3>
            <span class="forecast-today-date">{{ todayLabel }}</span>
          </div>

          <!-- 硬性日程 -->
          <div class="timeline-section">
            <div class="timeline-section-label hard">
              <el-icon><Bell /></el-icon> 硬性日程
              <span class="timeline-count">{{ todayHardItems.length }}</span>
            </div>
            <div
              v-for="(item, idx) in todayHardItems"
              :key="'h' + idx"
              class="timeline-slot hard"
            >
              <span class="timeline-time">{{ item.time || '--:--' }}</span>
              <span class="timeline-title">{{ item.title }}</span>
              <span v-if="item.caseName" class="timeline-case">{{ item.caseName }}</span>
            </div>
            <div v-if="todayHardItems.length === 0" class="timeline-empty">今日无开庭/口审/期限</div>
          </div>

          <!-- 弹性任务 -->
          <div class="timeline-section">
            <div class="timeline-section-label flex">
              <el-icon><Finished /></el-icon> 弹性任务
              <span class="timeline-count">{{ todayFlexTasks.length }}</span>
            </div>
            <div
              v-for="task in todayFlexTasks"
              :key="task.id"
              :class="['timeline-slot', 'flex', { done: task.completed }]"
              draggable="true"
              @dragstart="onDragStart(task, $event)"
              @click="router.push({ name: 'tasks', query: { edit: task.id } })"
            >
              <span class="timeline-time">{{ task.estimatedMinutes ? `${task.estimatedMinutes}m` : '--' }}</span>
              <span class="timeline-title">{{ task.taskName }}</span>
              <span v-if="task.caseName" class="timeline-case">{{ task.caseName }}</span>
            </div>
            <div v-if="todayFlexTasks.length === 0" class="timeline-empty">今日暂无弹性任务</div>
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
  box-shadow: 0 0 0 1px var(--c-primary, #3E5C9A) inset;
}

.capture-hint {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 6px;
  font-size: 11px;
  color: var(--c-text-secondary, #9BA2AF);
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
  color: #18181B;
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
  background: #E4E7ED;
  border: 1px solid #E4E7ED;
  border-radius: 8px;
  overflow: hidden;
}

.weekday-header {
  background: #F5F5F5;
  padding: 8px;
  text-align: center;
  font-size: 13px;
  font-weight: 500;
  color: #52525B;
}

.day-cell {
  background: #FFFFFF;
  padding: 6px;
  min-height: 100px;
  cursor: pointer;
  transition: background 0.15s;
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
  box-shadow: inset 0 0 0 2px #3E5C9A;
}

.day-cell.has-hard {
  border-top: 2px solid #B4554F;
}

.day-cell.has-overdue {
  border-top: 2px solid #B0823A;
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
  color: #18181B;
}

.day-cell.other-month .day-number {
  color: #A1A1AA;
}

.day-cell.today .day-number {
  color: #3E5C9A;
  font-weight: 600;
}

.day-indicator {
  display: flex;
  align-items: center;
}

.day-indicator.hard {
  color: #B4554F;
}

.day-indicator.overdue {
  color: #B0823A;
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
  color: #B4554F;
}

.event-badge.deadline {
  background: #F7F1E3;
  color: #B0823A;
}

.event-badge.task {
  background: #EDF1F8;
  color: #3E5C9A;
}

.event-text {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.event-more {
  font-size: 10px;
  color: #A1A1AA;
  text-align: center;
}

/* 选中日期面板 */
.selected-day-panel {
  width: 300px;
  background: #FFFFFF;
  border-radius: 8px;
  border: 1px solid #E4E7ED;
  overflow: hidden;
}

.panel-header {
  padding: 16px;
  background: #FAFAFA;
  border-bottom: 1px solid #E4E7ED;
}

.panel-header h3 {
  margin: 0 0 8px;
  font-size: 16px;
  font-weight: 600;
  color: #18181B;
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
  color: #B4554F;
}

.stat.deadline {
  background: #F7F1E3;
  color: #B0823A;
}

.stat.task {
  background: #EDF1F8;
  color: #3E5C9A;
}

.stat.overdue {
  background: #F6EDEC;
  color: #B4554F;
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
  color: #52525B;
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
  transition: background 0.15s;
}

.detail-event:hover,
.detail-task:hover {
  background: #F4F4F5;
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
  color: #18181B;
  margin-bottom: 2px;
}

.event-case,
.task-meta {
  display: block;
  font-size: 12px;
  color: #A1A1AA;
}

.empty-day {
  text-align: center;
  color: #A1A1AA;
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
  color: #52525B;
}

.legend-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
}

/* ============================================================
   周视图样式（时间块）
   ============================================================ */
.week-container {
  background: #FFFFFF;
  border: 1px solid #E4E7ED;
  border-radius: 8px;
  overflow: auto;
  max-height: calc(100vh - 200px);
}

.week-grid {
  min-width: 700px;
}

.week-header-row {
  display: grid;
  grid-template-columns: 56px repeat(7, 1fr);
  border-bottom: 1px solid #E4E7ED;
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
  border-left: 1px solid #EEF0F3;
}

.week-day-header.today {
  background: #EDF1F8;
}

.week-day-name {
  font-size: 11px;
  color: #9BA2AF;
  letter-spacing: .5px;
}

.week-day-number {
  font-size: 16px;
  font-weight: 600;
  color: #1F2430;
  margin-top: 2px;
}

.week-day-number.today-num {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  background: #3E5C9A;
  color: #fff;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.week-hour-row {
  display: grid;
  grid-template-columns: 56px repeat(7, 1fr);
  border-bottom: 1px solid #EEF0F3;
  min-height: 48px;
}

.week-time-label {
  font-size: 11px;
  color: #9BA2AF;
  font-family: var(--font-mono);
  text-align: right;
  padding: 4px 8px 0 0;
}

.week-cell {
  border-left: 1px solid #EEF0F3;
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
  color: #9BA2AF;
}

/* ============================================================
   日视图样式（硬性/弹性/成长时间块）
   ============================================================ */
.day-container {
  background: #FFFFFF;
  border: 1px solid #E4E7ED;
  border-radius: 8px;
  padding: 16px;
}

.day-grid {
  display: grid;
  grid-template-columns: 1fr 300px;
  gap: 16px;
}

.day-header {
  margin-bottom: 16px;
}

.day-header-date {
  font-size: 18px;
  font-weight: 700;
  color: #1F2430;
}

.day-header-weekday {
  font-size: 14px;
  color: #9BA2AF;
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
  border-bottom: 1px solid #EEF0F3;
}

.day-section-label.hard { color: #B4554F; }
.day-section-label.flex { color: #3E5C9A; }
.day-section-label.grow { color: #4C8067; }

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
  transition: all 0.15s;
}

.day-slot:hover {
  border-color: #CDD2DB;
}

.day-slot.hard {
  border-left: 3px solid #B4554F;
}

.day-slot.flex {
  border-left: 3px solid #3E5C9A;
}

.slot-time {
  font-family: var(--font-mono);
  font-size: 12px;
  color: #4B5160;
  width: 44px;
  flex-shrink: 0;
}

.slot-title {
  flex: 1;
  font-size: 13px;
  color: #1F2430;
}

.day-empty {
  text-align: center;
  padding: 16px;
  color: #9BA2AF;
  font-size: 12px;
}

.day-tip {
  font-size: 12px;
  color: #9BA2AF;
  padding: 8px 0;
  border-top: 1px dashed #E0E3E9;
  margin-top: 8px;
}

.day-agenda .card-header {
  font-size: 12px;
  font-weight: 700;
  color: #1F2430;
  padding-bottom: 10px;
  margin-bottom: 10px;
  border-bottom: 1px solid #EEF0F3;
}

.agenda-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 0;
  border-bottom: 1px solid #EEF0F3;
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
  color: #1F2430;
}

.agenda-time {
  display: block;
  font-size: 11px;
  color: #9BA2AF;
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
  color: #1F2430;
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
  border: 1px solid #E4E7ED;
  overflow: hidden;
}

.forecast-section-header {
  padding: 12px 16px;
  background: #FAFAFA;
  border-bottom: 1px solid #E4E7ED;
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.forecast-section-header h3 {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
  color: #18181B;
}

.forecast-hint {
  font-size: 11px;
  color: #9BA2AF;
}

.forecast-day-groups {
  max-height: calc(100vh - 300px);
  overflow-y: auto;
  padding: 4px 0;
}

.forecast-day-group {
  border-bottom: 1px solid #EEF0F3;
  padding: 8px 16px;
  transition: background 0.15s;
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
  color: #B4554F;
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
  color: #18181B;
  min-width: 44px;
}

.forecast-group-label.today {
  color: #3E5C9A;
}

.forecast-group-weekday {
  font-size: 11px;
  color: #9BA2AF;
}

.forecast-group-count {
  margin-left: auto;
  font-size: 11px;
  color: #A1A1AA;
}

.forecast-group-count.has-items {
  color: #3E5C9A;
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
  border-left-color: #B4554F;
  background: #F6EDEC;
}

.forecast-list-item.deadline {
  border-left-color: #B0823A;
  background: #F7F1E3;
}

.forecast-list-item.warning {
  border-left-color: #B0823A;
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
  color: #1F2430;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.forecast-list-meta {
  display: block;
  font-size: 11px;
  color: #9BA2AF;
  margin-top: 1px;
}

.forecast-group-empty {
  font-size: 11px;
  color: #A1A1AA;
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
  border: 1px solid #E4E7ED;
  overflow: hidden;
}

.forecast-today-date {
  font-size: 12px;
  color: #9BA2AF;
}

.timeline-section {
  padding: 10px 16px;
  border-bottom: 1px solid #EEF0F3;
}

.timeline-section-label {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 600;
  padding: 4px 0 8px;
  border-bottom: 1px solid #EEF0F3;
  margin-bottom: 8px;
}

.timeline-section-label.hard { color: #B4554F; }
.timeline-section-label.flex { color: #3E5C9A; }

.timeline-count {
  margin-left: auto;
  font-size: 11px;
  font-weight: 500;
  color: #9BA2AF;
  background: #F4F4F5;
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
  transition: border-color 0.15s;
}

.timeline-slot:hover {
  border-color: #CDD2DB;
}

.timeline-slot.hard {
  border-left: 3px solid #B4554F;
  background: #F6EDEC;
}

.timeline-slot.flex {
  border-left: 3px solid #3E5C9A;
  background: #EDF1F8;
}

.timeline-slot.flex.done {
  border-left-color: #4C8067;
  background: #EDF3EF;
  opacity: 0.75;
}

.timeline-slot.flex.done .timeline-title {
  color: #4C8067;
  text-decoration: line-through;
}

.timeline-time {
  font-family: var(--font-mono);
  font-size: 12px;
  color: #4B5160;
  width: 44px;
  flex-shrink: 0;
}

.timeline-title {
  flex: 1;
  min-width: 0;
  font-size: 13px;
  color: #1F2430;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.timeline-case {
  font-size: 11px;
  color: #9BA2AF;
  flex-shrink: 0;
  max-width: 100px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.timeline-empty {
  text-align: center;
  padding: 12px;
  color: #9BA2AF;
  font-size: 12px;
}

.forecast-tip {
  font-size: 11px;
  color: #9BA2AF;
  padding: 10px 16px;
  border-top: 1px dashed #E0E3E9;
}
</style>
