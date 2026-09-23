<script setup>
import HolidayBadges from '../components/HolidayBadges.vue'
import TimelineStream from '../components/TimelineStream.vue'
import PersonalDaysDialog from '../components/PersonalDaysDialog.vue'
import YearHeatmap from '../components/YearHeatmap.vue'
import CalendarComposer from '../components/CalendarComposer.vue'
import TimeGrid from '../components/TimeGrid.vue'
import { timeString } from '../parseCalendarCapture'
import { surroundingMonths, monthWorkingDays, eventDuration, isPlanningWorkday } from '../calendarDates'
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter, useRoute } from 'vue-router'
import { observeChanges } from '../../../core/observeChanges'
import { casyContext } from '../../../core/plugin/context'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  ArrowLeft,
  ArrowRight,
  Plus,
  Clock,
  Location,
  Search,
  Close,
  Rank,
  Check,
  Calendar as CalendarIcon,
  Timer,
  Bell,
  Edit,
  Delete,
  TrendCharts,
  Warning,
  Opportunity,
} from '../../../shared/icons'

const { t, locale } = useI18n()
const router = useRouter()
const route = useRoute()

// ============================================================
// 状态管理 (100% 连通真实 SQLite 数据库)
// ============================================================
function dateFromQuery(value) {
  if (typeof value !== 'string' || !/^\d{4}-\d{2}-\d{2}$/.test(value)) return new Date()
  const [year, month, day] = value.split('-').map(Number)
  const date = new Date(year, month - 1, day)
  return formatDate(date) === value ? date : new Date()
}
const currentDate = ref(dateFromQuery(route.query.date))
const events = ref([])
const tasks = ref([])
const allTasks = ref([])
const taskError = ref('')
const holidayError = ref('')
let holidayRequest = 0
const completedTaskIds = ref(new Set())
const todayTasks = ref([])
const cases = ref([])
const deadlineWarnings = ref([])
const holidayEntries = ref([])
const showPersonalDays = ref(false)
const loading = ref(false)
const eventError = ref('')
let eventRequest = 0

// 视图切换: 'timeline' | 'month' | 'week' | 'day' | 'forecast'
const activeView = ref(['year', 'timeline', 'month', 'week', 'day', 'forecast'].includes(route.query.view) ? route.query.view : 'month')

// Forecast 预测视图筛选: 'all' | 'risk_only' | 'free_only'
const forecastFilter = ref('all')

// 案件筛选（时间线视图）
const caseSearchQuery = ref('')
const selectedCaseIds = ref(new Set(['all']))

// 月视图点击当日日程弹窗
const showDayModal = ref(false)
const activeDaySummary = ref(null)

// 事项编辑详情弹窗
const showEditDialog = ref(false)
const editingItem = ref({
  id: '',
  type: 'task', // 'task' | 'event'
  title: '',
  startDate: '',
  dueDate: '',
  startTime: '',
  caseId: '',
  estimatedMinutes: 60,
  description: '',
  completed: 0,
})

// 右侧统一 Holding / Schedule Tank 状态
const tankFilter = ref('unscheduled') // 'unscheduled' | 'week' | 'multiday' | 'today'
const tankSearch = ref('')


// ============================================================
// 拖拽调度引擎 (Drag & Drop Engine - 真实持久化)
// ============================================================
const currentDraggedTask = ref(null)
const dragAction = ref('schedule') // 'schedule' | 'extend'
const dragOriginCellDate = ref(null)
const dragOverKey = ref(null)
const isOverTank = ref(false)

function onDragStart(e, task, action = 'schedule', cellDate = null) {
  currentDraggedTask.value = { ...task }
  dragAction.value = action
  dragOriginCellDate.value = cellDate ? formatDate(cellDate) : null

  if (e.dataTransfer) {
    e.dataTransfer.effectAllowed = 'move'
    e.dataTransfer.setData('application/json', JSON.stringify(task))
    e.dataTransfer.setData('text/plain', String(task.id))
  }
}

function onDragStartFromModal(e, item, cellDate) {
  onDragStart(e, item, 'schedule', cellDate)
  setTimeout(() => {
    showDayModal.value = false
  }, 50)
}

function onDragOver(e) {
  e.preventDefault()
  if (e.dataTransfer) {
    e.dataTransfer.dropEffect = 'move'
  }
}

function onDragEnd() {
  currentDraggedTask.value = null
  dragAction.value = 'schedule'
  dragOriginCellDate.value = null
  dragOverKey.value = null
  isOverTank.value = false
}

// 日期格式化
function formatDate(date) {
  if (!date) return ''
  let d = date
  if (d && typeof d === 'object' && 'value' in d && d.value instanceof Date) {
    d = d.value
  } else if (!(d instanceof Date)) {
    d = new Date(d)
  }
  if (isNaN(d.getTime())) return ''
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${y}-${m}-${day}`
}

function isToday(date) {
  if (!date) return false
  const d = date instanceof Date ? date : new Date(date)
  return d.toDateString() === new Date().toDateString()
}

function getDaySpan(startStr, dueStr) {
  if (!startStr || !dueStr) return 1
  const d1 = new Date(startStr)
  const d2 = new Date(dueStr)
  if (isNaN(d1.getTime()) || isNaN(d2.getTime())) return 1
  const diffTime = d2.getTime() - d1.getTime()
  return Math.max(1, Math.ceil(diffTime / (1000 * 60 * 60 * 24)) + 1)
}

// Month/agenda drag changes the intended start date. Explicit extend remains a duration edit.
async function onDropOnDay(e, targetDate) {
  e.preventDefault()
  let task = currentDraggedTask.value
  if (!task && e.dataTransfer) { try { task = JSON.parse(e.dataTransfer.getData('application/json')) } catch { return } }
  if(task?.type==='event') {
    const event=events.value.find(e=>e.id===task.id && e.type==='event')
    if(!event)return
    const result=await casyContext.calendar.moveEvent(event.id,formatDate(targetDate),event.startTime || null)
    if(!result.ok)return ElMessage.error(result.error || '日程移动失败')
    await loadEvents();onDragEnd();ElMessage.success('已移动日程');return
  }
  if (!task?.id || !tasks.value.some(t => t.id === task.id)) return
  const date = formatDate(targetDate)
  const patch = dragAction.value === 'extend'
    ? { id: task.id, startDate: task.startDate || date, dueDate: date }
    : { id: task.id, startDate: date, startBucket: isToday(targetDate) ? 'today' : 'anytime' }
  const result = await casyContext.tasks.update(patch)
  if (!result.ok) return ElMessage.error(result.error || '排期失败')
  ElMessage.success(dragAction.value === 'extend' ? '已更新任务日期范围' : '已更新任务开始日期')
  await loadTasks()
  onDragEnd()
}

// 放置到 Holding Tank (取消排期)
async function onDropToHoldingTank(e) {
  e.preventDefault()
  let task = currentDraggedTask.value
  if (!task && e.dataTransfer) {
    try {
      task = JSON.parse(e.dataTransfer.getData('application/json'))
    } catch {}
  }
  if (!task || task.type==='event') return

  const res = await casyContext.tasks.update({
    id: task.id,
    startDate: null,
    startTime: null,
    startBucket: 'inbox',
  })
  if (!res.ok) return ElMessage.error(res.error || '移入未排期池失败')

  const existing = tasks.value.find(t => t.id === task.id)
  if (existing) {
    existing.startDate = null
    existing.startTime = null
    existing.startBucket = 'inbox'
  }

  ElMessage.success(`已将「${task.taskName}」移入未排期池`)
  await loadTasks()
  onDragEnd()
}

function openDayModal(cell) {
  activeDaySummary.value = cell
  showDayModal.value = true
}

// 双击打开事项详情编辑
function openEditDetail(item, type = 'task') {
  if (type === 'event' && item.type !== 'event') {
    if (item.type === 'task') {
      router.push({ name: 'tasks', query: { edit: item.id } })
    } else if (item.caseId) {
      router.push({ name: 'case-detail', params: { id: item.caseId }, query: {tab: item.type === 'hearing' ? 'hearings' : 'tracks'} })
    }
    return
  }
  if (type === 'task') {
    editingItem.value = {
      id: item.id,
      type: 'task',
      title: item.taskName || item.title || '',
      startDate: item.startDate || item.dueDate || (activeDaySummary.value ? formatDate(activeDaySummary.value.date) : ''),
      dueDate: item.dueDate || item.startDate || (activeDaySummary.value ? formatDate(activeDaySummary.value.date) : ''),
      startTime: item.startTime || '',
      caseId: item.caseId || '',
      estimatedMinutes: item.estimatedMinutes || 60,
      description: item.description || '',
      completed: item.completed || 0,
    }
  } else {
    editingItem.value = {
      id: item.id,
      type: 'event',
      title: item.title || '',
      startDate: item.date || item.eventDate || (activeDaySummary.value ? formatDate(activeDaySummary.value.date) : ''),
      dueDate: item.date || item.eventDate || (activeDaySummary.value ? formatDate(activeDaySummary.value.date) : ''),
      startTime: item.time || item.startTime || '',
      endTime: item.endTime || '',
      caseId: item.caseId || '',
      estimatedMinutes: 60,
      taskId: item.taskId || null,
      description: item.notes || '',
      completed: 0,
    }
  }
  showEditDialog.value = true
}

// 保存编辑 (真实写入数据库)
async function saveEditingItem() {
  const item = editingItem.value
  if (!item.title.trim()) {
    ElMessage.warning('请输入标题')
    return
  }

  if (item.type === 'task') {
    const res = !item.id
      ? await casyContext.tasks.create({
          taskName: item.title,
          startDate: item.startDate || null,
          dueDate: item.dueDate || null,
          startTime: item.startTime || null,
          caseId: item.caseId || null,
          estimatedMinutes: item.estimatedMinutes || 60,
          description: item.description || null,
        })
      : await casyContext.tasks.update({
          id: item.id,
          taskName: item.title,
          startDate: item.startDate || null,
          dueDate: item.dueDate || null,
          startTime: item.startTime || null,
          caseId: item.caseId || null,
          estimatedMinutes: item.estimatedMinutes || 60,
          description: item.description || null,
        })
    if (!res.ok) return ElMessage.error(res.error || '保存任务失败')
    ElMessage.success('已保存任务修改')
    await loadTasks()
    await loadTodayTasks()
  } else {
    const res = item.id
      ? await casyContext.calendar.updateEvent(item.id, {
          title: item.title,
          eventDate: item.dueDate || item.startDate,
          startTime: item.startTime || null,
          endTime: item.endTime || null,
          caseId: item.caseId || null,
          notes: item.description || null,
        })
      : await casyContext.calendar.createEvent({
          title: item.title,
          eventDate: item.dueDate || item.startDate,
          startTime: item.startTime || null,
          endTime: item.endTime || null,
          caseId: item.caseId || null,
          notes: item.description || null,
        })
    if (!res.ok) return ElMessage.error(res.error || '保存日程失败')
    ElMessage.success('已保存日程修改')
    await loadEvents()
  }

  showEditDialog.value = false
}

// 删除事项 (真实删除数据库记录)
async function deleteEditingItem() {
  const item = editingItem.value
  let confirmed = false
  try {
    await ElMessageBox.confirm(`确定删除「${item.title}」吗？`, '删除确认', {
      confirmButtonText: '确定删除',
      cancelButtonText: '取消',
      type: 'warning',
    })
    confirmed = true
  } catch { /* 用户取消：属预期 */ }
  if (!confirmed) return

  if (item.type === 'task') {
    if (item.id) {
      const res = await casyContext.tasks.remove(item.id)
      if (!res.ok) return ElMessage.error(res.error || '删除任务失败')
    }
    tasks.value = tasks.value.filter(t => t.id !== item.id)
    ElMessage.success('已删除任务')
    await loadTasks()
  } else if (item.type === 'event' && item.id) {
    const res = await casyContext.calendar.removeEvent(item.id)
    if (!res.ok) return ElMessage.error(res.error || '删除日程失败')
    events.value = events.value.filter(e => e.id !== item.id)
    ElMessage.success('已删除日程')
    await loadEvents()
  }
  showEditDialog.value = false
}

// ============================================================
// 真实数据视图计算 (Zero Mock Data)
// ============================================================
const viewOptions = computed(() => ['day', 'week', 'month', 'year', 'timeline', 'forecast'].map(key => ({ key, label: t(`calendar.${key}`) })))

const weekDaysEn = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun']
const weekDaysCn = ['一', '二', '三', '四', '五', '六', '日']
const weekHours = computed(() => {
  const hours = [...events.value.map(event => event.time), ...tasks.value.map(task => task.startTime)]
    .filter(Boolean).map(time => Number(time.split(':')[0])).filter(hour => Number.isInteger(hour) && hour >= 0 && hour < 24)
  const first = Math.min(8, ...hours)
  const last = Math.max(21, ...hours)
  return Array.from({ length: last - first + 1 }, (_, i) => i + first)
})

const currentMonthInfo = computed(() => {
  const y = currentDate.value.getFullYear()
  const m = currentDate.value.getMonth()
  const monthsEn = [
    'January', 'February', 'March', 'April', 'May', 'June',
    'July', 'August', 'September', 'October', 'November', 'December',
  ]
  return {
    year: y,
    month: m,
    monthNameEn: monthsEn[m],
    label: currentDate.value.toLocaleDateString(locale.value, { year: 'numeric', month: 'long' }),
    cnLabel: `${y}年${m + 1}月`,
  }
})

// 月度洞察统计 (真实聚合)
const monthInsights = computed(() => {
  const currentMonthStr = `${currentDate.value.getFullYear()}-${String(currentDate.value.getMonth() + 1).padStart(2, '0')}`
  const monthEvs = events.value.filter(e => e.date?.startsWith(currentMonthStr))
  const monthTs = tasks.value.filter(t => t.dueDate?.startsWith(currentMonthStr))

  const deadlinesCount = monthEvs.filter(e => e.type === 'court' || e.type === 'hearing' || e.type?.startsWith('deadline')).length
  const totalMinutes = monthTs.reduce((acc, t) => acc + (t.estimatedMinutes || 0), 0)
  const workloadHours = Math.round(totalMinutes / 60)
  const holidaysCount = holidayEntries.value.filter(e => e.date?.startsWith(currentMonthStr) && e.kind === 'holiday' && e.source !== 'personal').length

  return {
    deadlines: deadlinesCount,
    workload: `${workloadHours}h`,
    workdays: monthWorkingDays(currentDate.value, holidayEntries.value.filter(entry => entry.source !== 'personal')),
    holidays: holidaysCount,
  }
})

// 事件与任务匹配
function eventsForDay(date) {
  const ds = formatDate(date)
  const ids = new Set(tasksForDay(date).map(task => task.id))
  return events.value.filter(e => e.date === ds && !(e.type === 'task' && ids.has(e.id)))
}

function tasksForDay(date) {
  const ds = formatDate(date)
  return tasks.value.filter(t => {
    if (t.dueDate === ds || t.startDate === ds) return true
    if (t.startDate && t.dueDate && ds >= t.startDate && ds <= t.dueDate) return true
    return false
  })
}

function multiDayTasksForDay(date) {
  const ds = formatDate(date)
  return tasks.value.filter(t => {
    return t.startDate && t.dueDate && t.startDate !== t.dueDate && ds >= t.startDate && ds <= t.dueDate
  })
}

function distinctEventsForModal(date) {
  return eventsForDay(date)
}

function hasHardEventOnDay(date) {
  const dayEvs = eventsForDay(date)
  return dayEvs.some(e => e.type === 'court' || e.type === 'hearing' || e.type?.startsWith('deadline'))
}

function hasWaitingOnDay(date) {
  const dayEvs = eventsForDay(date)
  return dayEvs.some(e => e.type === 'waiting' || e.type === 'appeal' || e.type === 'discovery')
}

function hasPlanEventOnDay(date) {
  const dayTasks = tasksForDay(date)
  const dayEvs = eventsForDay(date)
  return dayTasks.length > 0 || dayEvs.some(e => e.type === 'task' || e.type === 'event')
}

// 案件过滤列表 (真实案件数据库)
const filteredCases = computed(() => {
  const q = caseSearchQuery.value.trim().toLowerCase()
  return cases.value.filter(c => !q || c.caseName?.toLowerCase().includes(q) || c.caseNo?.toLowerCase().includes(q))
})

// 时间线视图真实数据流 (按真实日期聚合真实事项)
const timelineStream = computed(() => {
  const map = new Map()

  // 1. 过滤案件
  const allowedCases = selectedCaseIds.value.has('all')
    ? null
    : selectedCaseIds.value

  // 2. 收集事件
  for (const ev of events.value) {
    if (ev.type === 'task' && tasks.value.some(t => t.id === ev.id && (t.dueDate || t.startDate) === ev.date)) continue
    if (allowedCases && ev.caseId && !allowedCases.has(ev.caseId)) continue
    const dateStr = ev.date
    if (!dateStr) continue
    if (!map.has(dateStr)) map.set(dateStr, [])
    map.get(dateStr).push({
      id: `${ev.type}:${ev.id}`,
      source: ev,
      sourceKind: 'event',
      time: ev.time || '全天',
      title: ev.title,
      caseName: ev.caseName || '律所事项',
      type: ev.type === 'court' || ev.type === 'hearing' ? 'court' : 'primary',
      tag1: ['court', 'hearing'].includes(ev.type) ? '庭审' : '日程',
      tag2: ['court', 'hearing'].includes(ev.type) ? '固定安排' : '已排期',
      tag2Type: ['court', 'hearing'].includes(ev.type) ? 'risk' : 'neutral',
      duration: eventDuration(ev.time, ev.endTime),
    })
  }

  // 3. 收集任务
  for (const t of tasks.value) {
    if (allowedCases && t.caseId && !allowedCases.has(t.caseId)) continue
    const dateStr = t.dueDate || t.startDate
    if (!dateStr) continue
    if (!map.has(dateStr)) map.set(dateStr, [])
    map.get(dateStr).push({
      id: `task:${t.id}`,
      source: t,
      sourceKind: 'task',
      time: t.startTime || '未排时',
      title: t.taskName,
      caseName: t.caseName || '常规待办',
      type: 'primary',
      tag1: '任务',
      tag2: '可调整',
      tag2Type: 'warning',
      duration: t.estimatedMinutes ? `${t.estimatedMinutes} 分钟` : '',
    })
  }

  // Include holiday-only days in the visible month, even when there are no tasks.
  for (const entry of holidayEntries.value) {
    if (entry.date.startsWith(formatDate(currentDate.value).slice(0, 7)) && !map.has(entry.date)) map.set(entry.date, [])
  }
  // 排序
  const sortedDates = Array.from(map.keys()).sort()
  return sortedDates.map(dateStr => {
    const d = dateFromQuery(dateStr)
    const items = map.get(dateStr)
    items.sort((a, b) => (a.time || '').localeCompare(b.time || ''))
    return {
      dateStr,
      dateLabel: `${d.getFullYear()}年${d.getMonth() + 1}月${d.getDate()}日`,
      weekday: `${isToday(d) ? '今天 · ' : ''}周${weekDaysCn[d.getDay() === 0 ? 6 : d.getDay() - 1]}`,
      holidays: holidaysOn(dateStr),
      items,
    }
  })
})

// 月视图 7x6 矩阵生成
const monthGridDays = computed(() => {
  const { year, month } = currentMonthInfo.value
  const firstDay = new Date(year, month, 1)
  const lastDay = new Date(year, month + 1, 0)

  let startWeekday = firstDay.getDay() - 1
  if (startWeekday < 0) startWeekday = 6

  const days = []
  const prevLastDay = new Date(year, month, 0)
  for (let i = startWeekday - 1; i >= 0; i--) {
    days.push({
      date: new Date(year, month - 1, prevLastDay.getDate() - i),
      isCurrentMonth: false,
    })
  }

  for (let d = 1; d <= lastDay.getDate(); d++) {
    days.push({
      date: new Date(year, month, d),
      isCurrentMonth: true,
    })
  }

  const remaining = 35 - days.length > 0 ? 35 - days.length : (42 - days.length)
  for (let d = 1; d <= remaining; d++) {
    days.push({
      date: new Date(year, month + 1, d),
      isCurrentMonth: false,
    })
  }

  return days.map(cell=>{
    const dayTasks=tasksForDay(cell.date),ids=new Set(dayTasks.map(t=>t.id)),ds=formatDate(cell.date)
    const dayEvents=events.value.filter(e=>e.date===ds && !(e.type==='task' && ids.has(e.id)))
    return {...cell,tasks:dayTasks,events:dayEvents,multiDayTasks:dayTasks.filter(t=>t.startDate && t.dueDate && t.startDate!==t.dueDate),hasHard:dayEvents.some(e=>e.type==='court'||e.type==='hearing'||e.type?.startsWith('deadline')),hasWaiting:dayEvents.some(e=>['waiting','appeal','discovery'].includes(e.type)),hasPlan:dayTasks.length>0||dayEvents.some(e=>['task','event'].includes(e.type))}
  })
})

// 周视图 7 天列数据
const weekColumns = computed(() => {
  const now = new Date(currentDate.value)
  const dayOfWeek = now.getDay() || 7
  const monday = new Date(now)
  monday.setDate(monday.getDate() - dayOfWeek + 1)

  const list = []
  for (let i = 0; i < 7; i++) {
    const d = new Date(monday)
    d.setDate(d.getDate() + i)
    list.push({
      date: d,
      dateStr: formatDate(d),
      dayNum: d.getDate(),
      weekDayEn: weekDaysEn[i],
      weekDayCn: weekDaysCn[i],
      isToday: isToday(d),
      hasHard: hasHardEventOnDay(d),
      events: eventsForDay(d),
      tasks: tasksForDay(d),
      multiDayTasks: multiDayTasksForDay(d),
    })
  }
  return list
})

// 预测视图 14 天数据
const forecast14Days = computed(() => {
  const base = new Date(currentDate.value)
  const list = []

  for (let i = 0; i < 14; i++) {
    const d = new Date(base)
    d.setDate(d.getDate() + i)
    const dateStr = formatDate(d)
    const dayEvs = eventsForDay(d)
    const dayTasks = tasksForDay(d)
    const dayMultis = multiDayTasksForDay(d)

    const totalMinutes = dayTasks.reduce((acc, t) => acc + (t.estimatedMinutes || 0), 0)
    const totalHours = Math.round((totalMinutes / 60) * 10) / 10

    const hasCourt = dayEvs.some(e => e.type === 'court' || e.type === 'hearing')
    const hasDeadline = dayEvs.some(e => e.type?.startsWith('deadline') || e.type === 'appeal') || deadlineWarnings.value.some(w => w.deadlineDate === dateStr)

    const available = isPlanningWorkday(d, holidaysOn(dateStr))
    let riskLevel = available ? 'free' : 'rest'
    let riskTag = available ? '排期充裕 · 专注窗口' : '休息安排 · 留意个人计划'

    if (hasCourt || hasDeadline) {
      riskLevel = 'risk'
      riskTag = hasCourt ? '🔴 法庭庭审日 (强时间锁定)' : '🔴 法定诉讼期限截止日'
    } else if (totalHours >= 4 || dayTasks.length >= 3) {
      riskLevel = 'busy'
      riskTag = `🟡 高密度工作日 (${totalHours}h)`
    }

    list.push({
      index: i + 1,
      date: d,
      dateStr,
      monthDayStr: `${d.getMonth() + 1}月${d.getDate()}日`,
      weekdayCn: weekDaysCn[d.getDay() === 0 ? 6 : d.getDay() - 1],
      isToday: isToday(d),
      dayLabel: i === 0 ? '今日' : i === 1 ? '明日' : i === 2 ? '后天' : `${i}天后`,
      events: dayEvs,
      tasks: dayTasks,
      multiDayTasks: dayMultis,
      totalHours,
      riskLevel,
      riskTag,
    })
  }

  if (forecastFilter.value === 'risk_only') {
    return list.filter(d => d.riskLevel === 'risk')
  } else if (forecastFilter.value === 'free_only') {
    return list.filter(d => d.riskLevel === 'free')
  }

  return list
})

const forecastOverviewStats = computed(() => {
  const base = new Date(currentDate.value)
  let riskDays = 0
  let totalHours = 0
  let freeDays = 0

  for (let i = 0; i < 14; i++) {
    const d = new Date(base)
    d.setDate(d.getDate() + i)
    const dayEvs = eventsForDay(d)
    const dayTasks = tasksForDay(d)

    const hasCourt = dayEvs.some(e => e.type === 'court' || e.type === 'hearing' || e.type?.startsWith('deadline'))
    const mins = dayTasks.reduce((acc, t) => acc + (t.estimatedMinutes || 60), 0)
    totalHours += mins

    if (hasCourt) {
      riskDays++
    } else if (mins <= 120 && isPlanningWorkday(d, holidaysOn(d))) {
      freeDays++
    }
  }

  return {
    riskDays,
    workloadHours: Math.round(totalHours / 60),
    freeDays,
  }
})

// 右侧统一 Holding Tank 真实任务池
const tankTasks = computed(() => {
  const q = tankSearch.value.trim().toLowerCase()
  let list = []

  if (tankFilter.value === 'unscheduled') {
    list = tasks.value.filter(t => !t.completed && !t.startDate && !t.startTime && !events.value.some(e => e.taskId === t.id && e.date >= formatDate(currentDate.value)))
  } else if (tankFilter.value === 'week') {
    const now = new Date(currentDate.value)
    const dayOfWeek = now.getDay() || 7
    const monday = new Date(now)
    monday.setDate(monday.getDate() - dayOfWeek + 1)
    const sunday = new Date(monday)
    sunday.setDate(sunday.getDate() + 6)
    const monStr = formatDate(monday)
    const sunStr = formatDate(sunday)

    list = tasks.value.filter(t => t.dueDate && t.dueDate >= monStr && t.dueDate <= sunStr && !t.completed)
  } else if (tankFilter.value === 'multiday') {
    list = tasks.value.filter(t => t.startDate && t.dueDate && t.startDate !== t.dueDate && !t.completed)
  } else if (tankFilter.value === 'today') {
    const ds = formatDate(currentDate.value)
    list = tasks.value.filter(t => (t.dueDate === ds || t.startDate === ds) && !t.completed)
  }

  if (q) {
    list = list.filter(t => t.taskName?.toLowerCase().includes(q) || t.caseName?.toLowerCase().includes(q))
  }

  return list
})

// ============================================================
// 数据加载 (真实 IPC 接口)
// ============================================================
onMounted(async () => {
  await loadData()
})
onUnmounted(observeChanges(casyContext, ['task', 'calendar', 'case', 'inbox', 'holiday'], loadData))

async function loadData() {
  loading.value = true
  await Promise.all([
    loadEvents(),
    loadTasks(),
    loadTodayTasks(),
    loadCases(),
    loadDeadlineWarnings(),
    loadHolidays(),
  ])
  loading.value = false
}

async function loadEvents() {
  const request = ++eventRequest
  eventError.value = ''
  const months = activeView.value === 'year' ? Array.from({ length: 12 }, (_, month) => ({ year: currentDate.value.getFullYear(), month: month + 1 })) : surroundingMonths(currentDate.value)
  const [results, independent] = await Promise.all([
    casyContext.calendar.events(months[0].year, months[0].month, months.length).then(result => [result]),
    casyContext.calendar.listEvents(formatDate(new Date(months[0].year, months[0].month - 1, 1)), formatDate(new Date(months[months.length - 1].year, months[months.length - 1].month, 0))),
  ])
  if (request !== eventRequest) return
  const failed = results.find(result => !result.ok || !Array.isArray(result.data))
  if (!failed) {
    events.value = results.flatMap(result => result.data).map(e => ({
      ...e,
      ...(e.type === 'event' ? (independent.data || []).find(row => row.id === e.id) : {}),
      time: e.startTime || null,
      endTime: e.endTime || null,
      allDay: !!e.allDay,
    }))
  } else {
    eventError.value = failed.error || '日程加载失败'
  }
}

async function loadTasks() {
  const result = await casyContext.tasks.list({})
  taskError.value = result.ok && Array.isArray(result.data) ? '' : result.error || '任务加载失败'
  if (result.ok && Array.isArray(result.data)) {
    allTasks.value = result.data
    completedTaskIds.value = new Set(result.data.filter(task => task.completed).map(task => task.id))
    tasks.value = result.data.filter(task => !task.completed)
  }
}

async function loadTodayTasks() {
  const result = await casyContext.tasks.list({ startBucket: 'today' })
  if (result.ok && Array.isArray(result.data)) {
    todayTasks.value = result.data
  }
}

async function loadCases() {
  const items = []
  for (let page = 1; ; page++) {
    const result = await casyContext.cases.list({ page, perPage: 200 })
    if (!result.ok || !Array.isArray(result.data?.items)) return
    items.push(...result.data.items)
    if (!result.data.items.length || items.length >= result.data.total) break
  }
  cases.value = items
}

async function loadDeadlineWarnings() {
  const result = await casyContext.calendar.deadlineWarnings()
  if (result.ok && Array.isArray(result.data)) {
    deadlineWarnings.value = result.data
  }
}

async function loadHolidays() {
  const request = ++holidayRequest
  const year = currentDate.value.getFullYear()
  // Include adjacent years for weeks/month grids and forecasts crossing New Year.
  const results = await Promise.all([year - 1, year, year + 1].map(value => casyContext.calendar.holidays(value)))
  if (request !== holidayRequest) return
  const failure = results.find(result => !result.ok || !Array.isArray(result.data?.entries))
  holidayError.value = failure ? failure.error || '节假日加载失败' : ''
  if (!failure) holidayEntries.value = results.flatMap(result => result.data.entries)
}

function prevPeriod() {
  const d = new Date(currentDate.value)
  if (activeView.value === 'year') {
    d.setMonth(0, 1)
    d.setFullYear(d.getFullYear() + -1)
  } else if (activeView.value === 'day') {
    d.setDate(d.getDate() - 1)
  } else if (activeView.value === 'week') {
    d.setDate(d.getDate() - 7)
  } else if (activeView.value === 'forecast') {
    d.setDate(d.getDate() - 14)
  } else {
    d.setDate(1)
    d.setMonth(d.getMonth() - 1)
  }
  currentDate.value = d
  loadData()
}

function nextPeriod() {
  const d = new Date(currentDate.value)
  if (activeView.value === 'year') {
    d.setMonth(0, 1)
    d.setFullYear(d.getFullYear() + 1)
  } else if (activeView.value === 'day') {
    d.setDate(d.getDate() + 1)
  } else if (activeView.value === 'week') {
    d.setDate(d.getDate() + 7)
  } else if (activeView.value === 'forecast') {
    d.setDate(d.getDate() + 14)
  } else {
    d.setDate(1)
    d.setMonth(d.getMonth() + 1)
  }
  currentDate.value = d
  loadData()
}

function goToday() {
  currentDate.value = new Date()
  loadData()
}

async function toggleTask(task) {
  const newDone = !task.completed
  const res = await casyContext.tasks.update({ id: task.id, completed: newDone ? 1 : 0 })
  if (!res.ok) return ElMessage.error(res.error || '操作失败')
  await loadTasks()
  await loadTodayTasks()
}

async function onCalendarCreated(date) {
  currentDate.value = dateFromQuery(date)
  await loadData()
}
watch(() => [route.query.date, route.query.view], ([date, view]) => {
  if (date) currentDate.value = dateFromQuery(date)
  if (['year', 'timeline', 'month', 'week', 'day', 'forecast'].includes(view)) activeView.value = view
  void loadData()
})
watch(activeView, () => { void loadEvents() })
const holidayByDate = computed(() => {
  const map = new Map()
  for (const entry of holidayEntries.value) map.set(entry.date, [...(map.get(entry.date) || []), entry])
  return map
})
function holidaysOn(date) { return holidayByDate.value.get(typeof date === 'string' ? date : formatDate(date)) || [] }
function openYearDate(date, view) {
  currentDate.value = dateFromQuery(date)
  activeView.value = view
  void loadData()
}
const jumpDate = computed({ get: () => formatDate(currentDate.value), set: value => { currentDate.value = dateFromQuery(value); void loadData() } })
function timedItems(date) {
  const dayEvents = eventsForDay(date).filter(e => e.time && e.type !== 'task')
  return [
    ...dayEvents.map(e => ({ id: e.id, title: e.title, startTime: e.time, endTime: e.endTime, color: e.color, kind: 'event', completed: !!e.taskId && completedTaskIds.value.has(e.taskId) })),
    ...tasksForDay(date).filter(t => t.startTime && !dayEvents.some(e => e.taskId === t.id)).map(t => ({ id: t.id, title: t.taskName, startTime: t.startTime, endTime: null, kind: 'task' })),
  ]
}
const timedDays = computed(() => (activeView.value === 'week' ? weekColumns.value.map(c => c.date) : [currentDate.value]).map(date => ({ date: formatDate(date), items: timedItems(date) })))
function openTimedItem(item, date) {
  const source = item.kind === 'task' ? tasks.value.find(t => t.id === item.id) : eventsForDay(date).find(e => e.id === item.id)
  if (source) openEditDetail(source, item.kind)
}
const schedulingTasks = new Set()
async function scheduleTaskBlock(event, date, hour) {
  let task = currentDraggedTask.value
  if (!task && event.dataTransfer) {
    try { task = JSON.parse(event.dataTransfer.getData('application/json')) } catch { return }
  }
  if (!task?.id || !tasks.value.some(t => t.id === task.id) || schedulingTasks.has(task.id)) return
  const duration = Math.max(15, Number(task.estimatedMinutes) || 60)
  if (hour * 60 + duration >= 1440) return ElMessage.warning('时间块跨越午夜，请选择更早的开始时间')
  schedulingTasks.add(task.id)
  try {
    const result = await casyContext.calendar.createEvent({ title: task.taskName, eventDate: date, startTime: timeString(hour * 60), endTime: timeString(hour * 60 + duration), allDay: false, taskId: task.id, caseId: task.caseId || null })
    if (!result.ok) return ElMessage.error(result.error || '排期失败')
    ElMessage.success('已为任务安排时间块')
    await loadEvents()
  } finally { schedulingTasks.delete(task.id); onDragEnd() }
}
</script>

<template>
  <div class="stitch-calendar-workspace">
    <div v-if="eventError" class="calendar-data-error" role="alert">日程加载失败 <el-button text @click="loadEvents">重试</el-button></div>
    <div v-if="holidayError || taskError" class="calendar-data-error" role="alert">{{ holidayError || taskError }} <el-button text @click="loadData">重试</el-button></div>
    <!-- ═══ 1. 顶部 Header 栏 ═══ -->
    <div class="calendar-top-header">
      <div class="header-titles">
        <div class="month-title-row">
          <h1 class="month-display-title">
            {{ activeView === 'year' ? `${currentDate.getFullYear()} 年` : activeView === 'day' ? formatDate(currentDate) : activeView === 'forecast' ? `未来 14 天诉讼与排期预测` : currentMonthInfo.label }}
          </h1>
          <div class="month-nav-btns">
            <button class="nav-arrow-btn" @click="prevPeriod" :title="t('calendar.previous')" :aria-label="t('calendar.previous')">
              <el-icon :size="16"><ArrowLeft /></el-icon>
            </button>
            <button class="nav-today-pill" @click="goToday">{{ t('calendar.today') }}</button>
            <button class="nav-arrow-btn" @click="nextPeriod" :title="t('calendar.next')" :aria-label="t('calendar.next')">
              <el-icon :size="16"><ArrowRight /></el-icon>
            </button>
          </div>
        </div>
      </div>

      <div class="header-right-actions">
        <!-- 快速输入条 -->
        <CalendarComposer :date="formatDate(currentDate)" :cases="cases" @created="onCalendarCreated" />

        <!-- 视图切换胶囊 -->
        <div class="view-switch-pill">
          <button
            v-for="vo in viewOptions"
            :key="vo.key"
            class="switch-btn"
            :class="{ active: activeView === vo.key }"
            @click="activeView = vo.key"
          >
            {{ vo.label }}
          </button>
        </div>
      </div>
    </div>

    <div class="calendar-holiday-legend"><span>实色：法定休 / 班</span><span>虚线：个人自休 / 自班（可与法定安排并存）</span><el-button @click="showPersonalDays = true">添加休息日</el-button></div>
    <PersonalDaysDialog v-model="showPersonalDays" :date="formatDate(currentDate)" @saved="loadHolidays" />

    <!-- ═══ 2. 时间线视图 (Global Timeline · 真实数据库流) ═══ -->
    <div v-if="activeView === 'timeline'" class="timeline-global-layout">
      <aside class="timeline-case-filter-aside">
        <h2 class="filter-headline-title">按案件筛选</h2>
        <div class="case-search-wrapper">
          <el-icon class="case-search-icon" :size="16"><Search /></el-icon>
          <input v-model="caseSearchQuery" placeholder="搜索案件…" class="case-search-input" />
        </div>
        <div class="case-checkbox-list">
          <label class="case-checkbox-item all-cases">
            <input type="checkbox" :checked="selectedCaseIds.has('all')" class="case-native-checkbox" @change="selectedCaseIds.clear(); if ($event.target.checked) selectedCaseIds.add('all')" />
            <span class="case-checkbox-name font-bold">全部案件</span>
          </label>
          <label
            v-for="c in filteredCases"
            :key="c.id"
            class="case-checkbox-item"
          >
            <input type="checkbox" :checked="selectedCaseIds.has(c.id)" class="case-native-checkbox" @change="selectedCaseIds.delete('all'); selectedCaseIds.has(c.id) ? selectedCaseIds.delete(c.id) : selectedCaseIds.add(c.id)" />
            <span class="case-checkbox-name">{{ c.caseName || c.caseNo }}</span>
          </label>
        </div>
      </aside>

      <main class="timeline-main-stream">
        <div class="timeline-stream-top">
          <div>
            <h1 class="stream-main-heading">{{ t('calendar.globalTimeline') }}</h1>
            <p class="stream-sub-caption">看清案件安排，留出推进工作的时间。</p>
          </div>
        </div>
        <TimelineStream :groups="timelineStream" @open="item => openEditDetail(item.source, item.sourceKind)" />
      </main>
    </div>

    <!-- ═══ 3. 主工作区：月视图 / 周视图 / 日视图 / 预测视图 统一联动 Holding Tank ═══ -->
    <YearHeatmap v-else-if="activeView === 'year'" :year="currentDate.getFullYear()" :tasks="allTasks" :holidays="holidayEntries" :loading="loading" @month="openYearDate($event, 'month')" @day="openYearDate($event, 'day')" @task="openEditDetail($event, 'task')" />
    <div v-else class="calendar-unified-workspace-grid">
      <!-- ── A. 左侧主视图区域 ── -->
      <div class="calendar-main-stage">
        <!-- 1. 月视图 (Month View) -->
        <div v-if="activeView === 'month'" class="month-full-card">
          <!-- 上方一排月度洞察小卡片 -->
          <div class="month-top-stats-strip">
            <div class="m-stat-pill">
              <span class="m-stat-lbl">{{ t('calendar.deadlines') }}</span>
              <strong class="m-stat-val text-risk">{{ monthInsights.deadlines }}</strong>
            </div>
            <div class="m-stat-pill">
              <span class="m-stat-lbl">{{ t('calendar.workload') }}</span>
              <strong class="m-stat-val text-primary">{{ monthInsights.workload }}</strong>
            </div>
            <div class="m-stat-pill">
              <span class="m-stat-lbl">{{ t('calendar.workdays') }}</span>
              <strong class="m-stat-val">{{ monthInsights.workdays }}</strong>
            </div>
            <div class="m-stat-pill">
              <span class="m-stat-lbl">{{ t('calendar.holidays') }}</span>
              <strong class="m-stat-val text-warning">{{ monthInsights.holidays }}</strong>
            </div>
          </div>

          <!-- 星期头 -->
          <div class="month-days-of-week-row">
            <div v-for="d in (locale === 'en-US' ? weekDaysEn : weekDaysCn.map(day => '周' + day))" :key="d" class="dow-cell">{{ d }}</div>
          </div>

          <!-- 紧凑日历矩阵 -->
          <div class="month-dates-matrix-grid">
            <div
              v-for="(cell, cIdx) in monthGridDays"
              :key="cIdx"
              class="month-matrix-day-cell"
              :class="{
                'is-outside': !cell.isCurrentMonth,
                'is-risk-day': cell.hasHard && cell.isCurrentMonth,
                'is-drag-target': dragOverKey === formatDate(cell.date),
              }"
              @dragover="onDragOver"
              @dragenter="dragOverKey = formatDate(cell.date)"
              @dragleave="dragOverKey = null"
              @drop="onDropOnDay($event, cell.date)"
              @click="openDayModal(cell)"
            >
              <div class="cell-header-flex">
                <div class="cell-dots-indicator-group">
                  <span v-if="cell.hasHard" class="dot-indicator dot-risk" title="Hard Deadline" />
                  <span v-if="cell.hasWaiting" class="dot-indicator dot-warning" title="Waiting on Client" />
                  <span v-if="cell.hasPlan" class="dot-indicator dot-primary" title="Flexible Task" />
                </div>
                <HolidayBadges :entries="holidaysOn(cell.date)" compact />
                <span class="cell-number-badge" :class="{ 'today-pill': isToday(cell.date) }">
                  {{ cell.date.getDate() }}
                </span>
              </div>

              <!-- 当日事项流 -->
              <div class="cell-items-preview">
                <div v-for="event in cell.events.filter(e=>e.type==='event')" :key="'event-'+event.id" class="cell-task-capsule" draggable="true" @dragstart.stop="onDragStart($event,event)" @dragend="onDragEnd" @click.stop="openEditDetail(event,'event')"><span class="capsule-title">{{ event.startTime || '全天' }} · {{ event.title }}</span></div>
                <!-- 跨天条带 -->
                <div
                  v-for="mt in cell.multiDayTasks"
                  :key="'mt-' + mt.id"
                  class="cell-multiday-ribbon"
                  :class="{
                    'is-start': formatDate(cell.date) === mt.startDate,
                    'is-end': formatDate(cell.date) === mt.dueDate,
                    'is-middle': formatDate(cell.date) > mt.startDate && formatDate(cell.date) < mt.dueDate,
                  }"
                  draggable="true"
                  @dragstart.stop="onDragStart($event, mt, 'schedule', cell.date)"
                  @dragend="onDragEnd"
                  @click.stop="openDayModal(cell)"
                  @dblclick.stop="openEditDetail(mt, 'task')"
                  :title="`${mt.taskName} (${mt.startDate} ~ ${mt.dueDate}) · 双击编辑`"
                >
                  <span v-if="formatDate(cell.date) === mt.startDate" class="ribbon-text">
                    ▶ {{ mt.taskName }}
                  </span>
                  <span v-else-if="formatDate(cell.date) === mt.dueDate" class="ribbon-text">
                    🏁 结束
                  </span>
                  <span v-else class="ribbon-cont-line" />

                  <!-- 右边缘拉伸把手 -->
                  <div
                    class="ribbon-extend-handle"
                    title="按住向后拖动到其他日期可延长跨天"
                    draggable="true"
                    @dragstart.stop="onDragStart($event, mt, 'extend', cell.date)"
                    @dragend="onDragEnd"
                  />
                </div>

                <!-- 单日任务胶囊 -->
                <div
                  v-for="t in cell.tasks.filter(t => !t.startDate || !t.dueDate || t.startDate === t.dueDate).slice(0, 2)"
                  :key="t.id"
                  class="cell-task-capsule"
                  draggable="true"
                  @dragstart.stop="onDragStart($event, t, 'schedule', cell.date)"
                  @dragend="onDragEnd"
                  @click.stop="openDayModal(cell)"
                  @dblclick.stop="openEditDetail(t, 'task')"
                  title="单击查看当日，双击编辑任务"
                >
                  <span class="capsule-dot" />
                  <span class="capsule-title">{{ t.taskName }}</span>

                  <!-- 右边缘拉伸把手 -->
                  <div
                    class="capsule-extend-handle"
                    title="按住向后拖到其他日期可延长跨天"
                    draggable="true"
                    @dragstart.stop="onDragStart($event, t, 'extend', cell.date)"
                    @dragend="onDragEnd"
                  />
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- 2. 周视图 (Week View) -->
        <div v-else-if="activeView === 'week'" class="week-full-card">
          <div class="week-cols-header-row">
            <div class="week-gutter-head" />
            <div
              v-for="col in weekColumns"
              :key="col.dateStr"
              class="week-col-header-cell"
              :class="{ 'is-today-col': col.isToday }"
            >
              <span class="week-col-name">周{{ col.weekDayCn }}</span>
              <span class="week-col-date-pill" :class="{ active: col.isToday }">{{ col.dayNum }}</span>
              <HolidayBadges :entries="holidaysOn(col.date)" compact />
            </div>
          </div>

          <div class="week-allday-ribbon-bar">
            <div class="allday-label-col">全天/跨天</div>
            <div class="allday-grid-cols">
              <div
                v-for="col in weekColumns"
                :key="'ad-' + col.dateStr"
                class="allday-col-drop-slot"
                @dragover="onDragOver"
                @drop="onDropOnDay($event, col.date)"
              >
                <button v-for="ev in eventsForDay(col.date).filter(e => !e.time)" :key="ev.type + ev.id" type="button" :title="`${ev.title} · ${ev.caseName || ''}`" class="calendar-event-item" @click="openEditDetail(ev, 'event')">
                  <strong>{{ ev.title }}</strong><span>{{ ev.caseName }}</span>
                </button>
                <div
                  v-for="mt in col.multiDayTasks"
                  :key="'w-mt-' + mt.id"
                  class="allday-multiday-pill"
                  @dblclick="openEditDetail(mt, 'task')"
                >
                  {{ mt.taskName }}
                </div>
              </div>
            </div>
          </div>

          <TimeGrid :days="timedDays" @open="openTimedItem" @drop-task="scheduleTaskBlock" />
        </div>

        <!-- 3. 日视图 (Day View) -->
        <div v-else-if="activeView === 'day'" class="day-full-card">
          <div class="day-schedule-header">
            <div>
              <h2 class="day-schedule-heading">{{ formatDate(currentDate) }}</h2><HolidayBadges :entries="holidaysOn(currentDate)" />
              <span class="day-schedule-sub">
                {{ isToday(currentDate) ? '今日时间表' : '单日日程' }} · {{ tasksForDay(currentDate).length + eventsForDay(currentDate).length }} 项安排
              </span>
            </div>
            <span class="day-view-mode-tag">时间安排</span>
          </div>

          <div v-if="multiDayTasksForDay(currentDate).length" class="day-multiday-active-banner">
            <div class="dma-title-row">
              <el-icon :size="15"><Timer /></el-icon>
              <strong>跨天进行中任务</strong>
            </div>
            <div class="dma-cards-list">
              <div
                v-for="mt in multiDayTasksForDay(currentDate)"
                :key="'dma-' + mt.id"
                class="dma-item-card"
                @dblclick="openEditDetail(mt, 'task')"
              >
                <span class="dma-badge">共 {{ getDaySpan(mt.startDate, mt.dueDate) }} 天</span>
                <strong class="dma-name">{{ mt.taskName }}</strong>
                <span class="dma-span">({{ mt.startDate }} ~ {{ mt.dueDate }})</span>
                <button class="dma-check-btn" :class="{ checked: mt.completed }" @click.stop="toggleTask(mt)">
                  <el-icon v-if="mt.completed" :size="12"><Check /></el-icon>
                </button>
              </div>
            </div>
          </div>

          <div v-if="eventsForDay(currentDate).some(e => !e.time) || tasksForDay(currentDate).some(t => !t.startTime)" class="day-unscheduled">
            <h3>全天 / 未指定时间</h3>
            <button v-for="ev in eventsForDay(currentDate).filter(e => !e.time)" :key="ev.type + ev.id" type="button" :title="`${ev.title} · ${ev.caseName || ''}`" class="calendar-event-item" @click="openEditDetail(ev, 'event')">
              <strong>{{ ev.title }}</strong><span>{{ ev.caseName }}</span>
            </button>
            <button v-for="task in tasksForDay(currentDate).filter(t => !t.startTime && !eventsForDay(currentDate).some(e => e.type === 'task' && e.id === t.id))" :key="task.id" type="button" class="calendar-event-item" @click="openEditDetail(task, 'task')">
              <strong>{{ task.taskName }}</strong><span>{{ task.caseName }}</span>
            </button>
          </div>
          <TimeGrid :days="timedDays" @open="openTimedItem" @drop-task="scheduleTaskBlock" />
        </div>

        <!-- 4. 预测视图 (Forecast: 完整 14 天诉讼与负荷预测工作台) -->
        <div v-else-if="activeView === 'forecast'" class="forecast-full-card">
          <!-- 顶部 14 天宏观指标面板 -->
          <div class="forecast-overview-header">
            <div>
              <h2 class="forecast-heading">未来 14 天诉讼与负荷全景预测</h2>
              <p class="forecast-sub">
                覆盖 {{ formatDate(currentDate) }} 起 14 天全量庭审日程、上诉/举证期限、预估工时与深度起草窗口。
              </p>
            </div>

            <!-- 3 宫格统计 -->
            <div class="forecast-stats-strip">
              <div class="f-stat-card">
                <span class="f-stat-label">诉讼/开庭日</span>
                <strong class="f-stat-number text-risk">{{ forecastOverviewStats.riskDays }} 天</strong>
              </div>
              <div class="f-stat-card">
                <span class="f-stat-label">预估总工时</span>
                <strong class="f-stat-number text-primary">{{ forecastOverviewStats.workloadHours }}h</strong>
              </div>
              <div class="f-stat-card">
                <span class="f-stat-label">黄金专注窗口</span>
                <strong class="f-stat-number text-success">{{ forecastOverviewStats.freeDays }} 天</strong>
              </div>
            </div>
          </div>

          <!-- 快速筛选 Tab -->
          <div class="forecast-filter-bar">
            <div class="f-filter-tabs">
              <button class="f-filter-tab" :class="{ active: forecastFilter === 'all' }" @click="forecastFilter = 'all'">全部 14 天</button>
              <button class="f-filter-tab" :class="{ active: forecastFilter === 'risk_only' }" @click="forecastFilter = 'risk_only'">仅看开庭与期限日</button>
              <button class="f-filter-tab" :class="{ active: forecastFilter === 'free_only' }" @click="forecastFilter = 'free_only'">仅看专注空闲窗口</button>
            </div>
            <span class="f-total-hint">已展示 {{ forecast14Days.length }} 天预测</span>
          </div>

          <!-- 完整 14 天预测数据流 (支持拖拽落位改期) -->
          <div class="forecast-days-stream">
            <div
              v-for="day in forecast14Days"
              :key="day.dateStr"
              class="forecast-day-row"
              :class="{
                'is-risk-day': day.riskLevel === 'risk',
                'is-busy-day': day.riskLevel === 'busy',
                'is-today': day.isToday,
                'is-drag-target': dragOverKey === day.dateStr,
              }"
              @dragover="onDragOver"
              @dragenter="dragOverKey = day.dateStr"
              @dragleave="dragOverKey = null"
              @drop="onDropOnDay($event, day.date)"
            >
              <!-- 日期与定位 -->
              <div class="f-date-col">
                <span class="f-day-badge" :class="day.riskLevel">{{ day.dayLabel }}</span>
                <strong class="f-day-date">{{ day.monthDayStr }}</strong>
                <small class="f-day-weekday">周{{ day.weekdayCn }}</small><HolidayBadges :entries="holidaysOn(day.dateStr)" />
              </div>

              <!-- 中间：当日事项概览与预警标签 -->
              <div class="f-events-col">
                <div class="f-risk-tag-row">
                  <span class="f-risk-pill" :class="day.riskLevel">{{ day.riskTag }}</span>
                  <span v-if="day.totalHours > 0" class="f-hours-pill">工时: {{ day.totalHours }}h</span>
                </div>

                <!-- 事项列表 -->
                <div class="f-items-stack">
                  <!-- 开庭事件 -->
                  <div
                    v-for="ev in day.events"
                    :key="ev.id"
                    class="f-event-chip court"
                    @dblclick="openEditDetail(ev, 'event')"
                  >
                    <el-icon :size="12"><Location /></el-icon>
                    <strong>{{ ev.time || '全天' }}</strong>
                    <span>{{ ev.title }}</span>
                  </div>

                  <!-- 跨天任务 -->
                  <div
                    v-for="mt in day.multiDayTasks"
                    :key="'fmt-' + mt.id"
                    class="f-event-chip multiday"
                    @dblclick="openEditDetail(mt, 'task')"
                  >
                    <el-icon :size="12"><Timer /></el-icon>
                    <span>{{ mt.taskName }}</span>
                    <small>({{ mt.startDate.slice(5) }}~{{ mt.dueDate.slice(5) }})</small>
                  </div>

                  <!-- 单日待办 -->
                  <div
                    v-for="t in day.tasks.filter(t => !t.startDate || !t.dueDate || t.startDate === t.dueDate)"
                    :key="'ft-' + t.id"
                    class="f-event-chip task"
                    @dblclick="openEditDetail(t, 'task')"
                  >
                    <span class="f-task-dot" />
                    <span :class="{ struck: t.completed }">{{ t.taskName }}</span>
                  </div>

                  <!-- 无排期 -->
                  <div v-if="!day.events.length && !day.tasks.length && !day.multiDayTasks.length" class="f-empty-slot">
                    <span>{{ holidaysOn(day.dateStr).some(entry => entry.kind === 'holiday') ? '当日有休息安排，请结合个人计划排期' : '暂无已排期事项 · 支持将右侧任务拖入落位' }}</span>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- ── B. 右侧统一 Holding Tank 真实任务池 ── -->
      <aside
        class="calendar-unified-holding-tank"
        :class="{ 'is-drag-target': isOverTank }"
        @dragover="onDragOver"
        @dragenter="isOverTank = true"
        @dragleave="isOverTank = false"
        @drop="onDropToHoldingTank($event)"
      >
        <div class="agenda-mini">
          <label>跳转日期 <input v-model="jumpDate" type="date" /></label>
          <HolidayBadges :entries="holidaysOn(currentDate)" /><div class="agenda-mini-title"><strong>{{ formatDate(currentDate) }}</strong><button type="button" @click="activeView = 'day'">展开当日 ↗</button></div>
          <p v-if="!eventsForDay(currentDate).length">当天暂无日程</p>
          <button v-for="event in eventsForDay(currentDate)" :key="event.type + event.id" class="agenda-mini-item" type="button" @click="openEditDetail(event, 'event')"><time>{{ event.time || '全天' }}</time><span>{{ event.title }}</span></button>
        </div>
        <div class="tank-header-card">
          <div class="tank-title-row">
            <div>
              <h3 class="tank-title">{{ t('calendar.taskPool') }}</h3>
              <p class="tank-sub-desc">
                {{ isOverTank ? '松开鼠标移回未排期池' : '拖入时段，为任务留出专注时间' }}
              </p>
            </div>
            <span class="tank-badge">{{ tankTasks.length }} 项</span>
          </div>

          <div class="tank-filter-pills">
            <button class="tank-tab-btn" :class="{ active: tankFilter === 'unscheduled' }" @click="tankFilter = 'unscheduled'">未安排</button>
            <button class="tank-tab-btn" :class="{ active: tankFilter === 'week' }" @click="tankFilter = 'week'">本周</button>
            <button class="tank-tab-btn" :class="{ active: tankFilter === 'multiday' }" @click="tankFilter = 'multiday'">跨天</button>
            <button class="tank-tab-btn" :class="{ active: tankFilter === 'today' }" @click="tankFilter = 'today'">今日</button>
          </div>

          <div class="tank-search-box">
            <el-icon class="tank-search-icon" :size="14"><Search /></el-icon>
            <input v-model="tankSearch" placeholder="搜索待办任务..." class="tank-search-real" />
          </div>
        </div>

        <div class="holding-tasks-scroll-list">
          <div
            v-for="task in tankTasks"
            :key="task.id"
            class="tank-task-card"
            draggable="true"
            @dragstart="onDragStart($event, task, 'schedule')"
            @dragend="onDragEnd"
            @dblclick="openEditDetail(task, 'task')"
          >
            <div class="tank-task-main">
              <div class="tank-task-header">
                <el-icon class="tank-drag-handle" :size="16"><Rank /></el-icon>
                <strong class="tank-task-name" :class="{ struck: task.completed }">{{ task.taskName }}</strong>
              </div>
              <div class="tank-task-meta">
                <span v-if="task.caseName" class="meta-case-tag">{{ task.caseName }}</span>
                <span v-if="task.startDate && task.dueDate && task.startDate !== task.dueDate" class="meta-multiday-badge">
                  跨 {{ getDaySpan(task.startDate, task.dueDate) }} 天 ({{ task.startDate.slice(5) }} ~ {{ task.dueDate.slice(5) }})
                </span>
                <span v-else-if="task.dueDate" class="meta-due-date">截止: {{ task.dueDate }}</span>
              </div>
            </div>

            <div class="tank-task-right">
              <span class="tank-est-pill">{{ task.estimatedMinutes || 60 }}m</span>
              <button
                class="tank-check-box"
                :class="{ checked: task.completed }"
                @click.stop="toggleTask(task)"
                title="标记完成"
              >
                <el-icon v-if="task.completed" :size="12"><Check /></el-icon>
              </button>
            </div>
          </div>

          <div v-if="!tankTasks.length" class="tank-empty-box">
            <span>暂无此类待办事项</span>
          </div>
        </div>
      </aside>
    </div>

    <!-- ═══ 4. 单日日程明细弹窗 ═══ -->
    <el-dialog
      v-model="showDayModal"
      :title="activeDaySummary ? `${formatDate(activeDaySummary.date)} 日程明细` : '日程明细'"
      width="540px"
      destroy-on-close
    >
      <div v-if="activeDaySummary" class="day-modal-content">
        <!-- 1. 跨天进行中任务 -->
        <div v-if="multiDayTasksForDay(activeDaySummary.date).length" class="modal-sec-box">
          <div class="sec-title-flex">
            <span class="sec-label-caps">跨天进行中专项 (Multi-day Sprints)</span>
            <small class="sec-hint-txt">支持拖拽 / 双击编辑</small>
          </div>
          <div class="modal-tasks-list">
            <div
              v-for="mt in multiDayTasksForDay(activeDaySummary.date)"
              :key="'m-mt-' + mt.id"
              class="modal-multiday-item"
              draggable="true"
              @dragstart="onDragStartFromModal($event, mt, activeDaySummary.date)"
              @dragend="onDragEnd"
              @dblclick="openEditDetail(mt, 'task')"
              title="按住拖拽可直接改期，双击编辑详情"
            >
              <div class="m-left">
                <el-icon class="m-drag-icon"><Rank /></el-icon>
                <span class="m-badge">共 {{ getDaySpan(mt.startDate, mt.dueDate) }} 天</span>
                <strong>{{ mt.taskName }}</strong>
                <small>({{ mt.startDate }} ~ {{ mt.dueDate }})</small>
              </div>
              <button class="m-check-btn" :class="{ checked: mt.completed }" @click.stop="toggleTask(mt)">
                <el-icon v-if="mt.completed" :size="12"><Check /></el-icon>
              </button>
            </div>
          </div>
        </div>

        <!-- 2. 客观排期与开庭事项 -->
        <div v-if="distinctEventsForModal(activeDaySummary.date).length" class="modal-sec-box">
          <div class="sec-title-flex">
            <span class="sec-label-caps">法庭开庭与客观排期 (Court & Fixed Events)</span>
            <small class="sec-hint-txt">双击编辑</small>
          </div>
          <div class="modal-tasks-list">
            <div
              v-for="ev in distinctEventsForModal(activeDaySummary.date)"
              :key="ev.id"
              class="modal-event-item"
              :class="ev.type === 'court' || ev.type === 'hearing' ? 'is-court' : 'is-normal'"
              @dblclick="openEditDetail(ev, 'event')"
              title="双击编辑排期详情"
            >
              <div class="m-ev-left">
                <span class="m-ev-time">{{ ev.time || '全天' }}</span>
                <strong>{{ ev.title }}</strong>
                <span v-if="ev.location" class="m-ev-loc">· {{ ev.location }}</span>
              </div>
            </div>
          </div>
        </div>

        <!-- 3. 当日待办清单 -->
        <div class="modal-sec-box">
          <div class="sec-title-flex">
            <span class="sec-label-caps">当日待办清单 (Action Items · 可打勾完成)</span>
            <small class="sec-hint-txt">支持拖出改期 / 双击编辑</small>
          </div>
          <div class="modal-tasks-list">
            <div
              v-for="t in tasksForDay(activeDaySummary.date).filter(t => !t.startDate || !t.dueDate || t.startDate === t.dueDate)"
              :key="'m-t-' + t.id"
              class="modal-task-item"
              draggable="true"
              @dragstart="onDragStartFromModal($event, t, activeDaySummary.date)"
              @dragend="onDragEnd"
              @dblclick="openEditDetail(t, 'task')"
              title="按住拖拽可直接改期到其他天，双击编辑详情"
            >
              <div class="m-t-left">
                <el-icon class="m-drag-icon"><Rank /></el-icon>
                <span v-if="t.startTime" class="m-t-time">{{ t.startTime }}</span>
                <strong :class="{ struck: t.completed }">{{ t.taskName }}</strong>
                <span v-if="t.caseName" class="m-t-case">{{ t.caseName }}</span>
              </div>
              <button class="m-check-btn" :class="{ checked: t.completed }" @click.stop="toggleTask(t)" title="完成任务">
                <el-icon v-if="t.completed" :size="12"><Check /></el-icon>
              </button>
            </div>
            <div v-if="!tasksForDay(activeDaySummary.date).filter(t => !t.startDate || !t.dueDate || t.startDate === t.dueDate).length" class="modal-empty-hint">暂无当日待办事项</div>
          </div>
        </div>
      </div>

      <template #footer>
        <div class="dialog-footer">
          <button class="btn-primary-action" @click="showDayModal = false">确定</button>
        </div>
      </template>
    </el-dialog>

    <!-- ═══ 5. 事项/任务详情编辑弹窗 (真实增删改) ═══ -->
    <el-dialog
      v-model="showEditDialog"
      :title="editingItem.type === 'task' ? '编辑任务详情' : '编辑日程排期'"
      width="500px"
      destroy-on-close
    >
      <el-button v-if="editingItem.type === 'event' && editingItem.taskId" text @click="router.push({ path: '/tasks', query: { edit: editingItem.taskId } })">打开关联任务 ↗</el-button>
      <div class="edit-modal-body">
        <div class="edit-form-item">
          <label>标题 / 名称</label>
          <input v-model="editingItem.title" class="edit-input" placeholder="输入名称..." />
        </div>

        <div class="edit-form-row">
          <div class="edit-form-item">
            <label>开始日期</label>
            <input v-model="editingItem.startDate" type="date" class="edit-input" />
          </div>
          <div class="edit-form-item">
            <label>截止日期</label>
            <input v-model="editingItem.dueDate" type="date" class="edit-input" />
          </div>
        </div>

        <div class="edit-form-row">
          <div class="edit-form-item">
            <label>开始时间</label>
            <input v-model="editingItem.startTime" type="time" class="edit-input" />
          </div>
          <div v-if="editingItem.type === 'task'" class="edit-form-item">
            <label>预计工时 (分钟)</label>
            <input v-model.number="editingItem.estimatedMinutes" type="number" min="0" step="15" class="edit-input" />
          </div>
          <div v-else class="edit-form-item">
            <label>结束时间</label>
            <input v-model="editingItem.endTime" type="time" class="edit-input" />
          </div>
        </div>

        <div class="edit-form-item">
          <label>关联案件</label>
          <select v-model="editingItem.caseId" class="edit-select">
            <option value="">（无关联案件）</option>
            <option v-for="c in cases" :key="c.id" :value="c.id">
              {{ c.caseName || c.caseNo }}
            </option>
          </select>
        </div>

        <div class="edit-form-item">
          <label>备注 / 说明</label>
          <textarea v-model="editingItem.description" rows="3" class="edit-textarea" placeholder="填写事项补充说明或庭室地点..." />
        </div>
      </div>

      <template #footer>
        <div class="edit-dialog-footer">
          <button v-if="editingItem.id" class="btn-delete" @click="deleteEditingItem">
            <el-icon :size="14"><Delete /></el-icon>
            <span>删除</span>
          </button>
          <div class="right-btns">
            <button class="btn-cancel" @click="showEditDialog = false">取消</button>
            <button class="btn-primary-action" @click="saveEditingItem">保存修改</button>
          </div>
        </div>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.calendar-holiday-legend { display: flex; gap: 14px; flex-wrap: wrap; align-items: center; color: var(--c-text-secondary); font-size: 12px; }

.calendar-data-error { display: flex; align-items: center; gap: 8px; padding: 12px; color: var(--c-danger); }
.calendar-event-item { display: flex; flex-wrap: wrap; align-items: center; gap: 6px 12px; width: 100%; min-width: 0; padding: 10px 12px; border: 0; border-left: 3px solid var(--c-danger); border-radius: 4px; background: var(--c-bg-hover); color: var(--c-text); text-align: left; font: inherit; cursor: pointer; margin-block: 4px; }
.calendar-event-item strong, .calendar-event-item span { overflow-wrap: anywhere; min-width: 0; }
.calendar-event-item span { font-size: 12px; color: var(--c-text-secondary); }
.calendar-event-item:hover { background: var(--c-bg-page); }
.day-unscheduled { padding: 16px; border-bottom: 1px solid var(--c-border); }
.day-unscheduled h3 { font-size: 13px; margin: 0 0 8px; }
/* ═══════════════════════════════════════════════════════════
   Stitch Unified Calendar Layout (v4.0_9 & v4.1_3)
   ═══════════════════════════════════════════════════════════ */
.stitch-calendar-workspace {
  container-type: inline-size;
  max-width: 1440px;
  margin: 0 auto;
  padding: 16px 24px 32px;
  display: flex;
  flex-direction: column;
  gap: 16px;
  color: var(--c-text);
  font-family: var(--font-family);
  min-height: calc(100vh - 80px);
}

/* ── 顶部 Header ─────────────────────────────────────────── */
.calendar-top-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  flex-wrap: wrap;
}

.month-title-row {
  display: flex;
  align-items: center;
  gap: 16px;
}

.month-display-title {
  font-size: 22px;
  font-weight: 700;
  color: var(--c-text-heading);
  letter-spacing: -0.3px;
  margin: 0;
}

.month-nav-btns {
  display: flex;
  align-items: center;
  gap: 2px;
  background: var(--c-bg-subtle);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  padding: 2px 4px;
}

.nav-arrow-btn {
  width: 26px;
  height: 26px;
  display: grid;
  place-items: center;
  border: none;
  background: transparent;
  color: var(--c-text-regular);
  cursor: pointer;
  border-radius: 4px;
}

.nav-arrow-btn:hover { background: var(--c-bg-hover); color: var(--c-text); }

.nav-today-pill {
  border: none;
  background: transparent;
  padding: 3px 8px;
  font-size: 11.5px;
  font-weight: 600;
  color: var(--c-text-heading);
  cursor: pointer;
  border-radius: 4px;
}

.nav-today-pill:hover { background: var(--c-bg-hover); }

.header-right-actions {
  display: flex;
  align-items: center;
  gap: 12px;
}

.natural-input-box {
  display: flex;
  align-items: center;
  gap: 8px;
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  padding: 0 12px;
  height: 34px;
  width: 280px;
  box-shadow: var(--shadow-sm);
}

.natural-input-box:focus-within {
  border-color: var(--c-primary);
}

.input-icon { color: var(--slate-gray-light); }

.natural-real-input {
  flex: 1;
  border: none;
  background: transparent;
  outline: none;
  font-size: 12px;
  color: var(--c-text);
}

.view-switch-pill {
  display: flex;
  background: var(--c-bg-subtle);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  padding: 2px;
  gap: 2px;
}

.switch-btn {
  padding: 4px 12px;
  border: none;
  background: transparent;
  color: var(--c-text-secondary);
  font-size: 11.5px;
  font-weight: 500;
  cursor: pointer;
  border-radius: 5px;
}

.switch-btn.active {
  background: var(--c-bg-card);
  color: var(--c-text);
  box-shadow: var(--shadow-sm);
  font-weight: 600;
}

/* ═══════════════════════════════════════════════════════════
   主工作区双栏统一布局 (Left: 主日历/周/日视图, Right: Holding Tank)
   ═══════════════════════════════════════════════════════════ */
.calendar-unified-workspace-grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 320px;
  gap: 20px;
  align-items: stretch;
  flex: 1;
}

.calendar-main-stage {
  min-width: 0;
  display: flex;
  flex-direction: column;
}

/* ── 1. Month View (上方一排统计药丸 + 紧凑月历网格) ── */
.month-full-card {
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-xl);
  box-shadow: var(--shadow-sm);
  overflow: hidden;
  display: flex;
  flex-direction: column;
  height: 100%;
}

.month-top-stats-strip {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 10px;
  padding: 10px 14px;
  background: var(--c-bg-subtle);
  border-bottom: 1px solid var(--c-border);
}

.m-stat-pill {
  display: flex;
  align-items: center;
  justify-content: space-between;
  background: var(--c-bg-card);
  padding: 6px 12px;
  border-radius: var(--c-radius-lg);
  border: 1px solid var(--c-border);
}

.m-stat-lbl {
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 700;
  color: var(--slate-gray-light);
  text-transform: uppercase;
}

.m-stat-val {
  font-size: 14px;
  font-weight: 700;
  color: var(--c-text-heading);
}

.m-stat-val.text-risk { color: var(--status-risk); }
.m-stat-val.text-primary { color: var(--c-primary); }
.m-stat-val.text-warning { color: var(--status-warning); }

.month-days-of-week-row {
  display: grid;
  grid-template-columns: repeat(7, minmax(0, 1fr));
  background: var(--c-bg-page);
  border-bottom: 1px solid var(--c-border);
}

.dow-cell {
  padding: 6px 10px;
  font-family: var(--font-mono);
  font-size: 10.5px;
  font-weight: 700;
  color: var(--slate-gray-light);
  text-transform: uppercase;
  letter-spacing: 0.8px;
  text-align: right;
}

.month-dates-matrix-grid {
  display: grid;
  grid-template-columns: repeat(7, minmax(0, 1fr));
  grid-auto-rows: minmax(78px, 1fr);
  background: var(--c-border);
  gap: 1px;
  flex: 1;
}

.month-matrix-day-cell {
  background: var(--c-bg-card);
  padding: 5px 6px;
  display: flex;
  flex-direction: column;
  cursor: pointer;
  transition: all var(--motion-fast);
  position: relative;
}

.month-matrix-day-cell:hover { background: var(--c-bg-hover); }

.month-matrix-day-cell.is-outside {
  background: var(--c-bg-page);
  opacity: 0.4;
}

.month-matrix-day-cell.is-drag-target {
  background: var(--c-primary-light) !important;
  box-shadow: inset 0 0 0 2px var(--c-primary);
}

.cell-header-flex {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  width: 100%;
  pointer-events: none;
}

.cell-dots-indicator-group {
  display: flex;
  gap: 3px;
  margin-top: 2px;
}

.dot-indicator {
  width: 5px;
  height: 5px;
  border-radius: 50%;
}

.dot-risk { background: var(--status-risk); }
.dot-warning { background: var(--status-warning); }
.dot-primary { background: var(--c-primary); }

.cell-number-badge {
  font-family: var(--font-mono);
  font-size: 11.5px;
  color: var(--c-text);
}

.cell-number-badge.today-pill {
  background: var(--c-primary);
  color: var(--c-primary-contrast);
  border-radius: 50%;
  width: 18px;
  height: 18px;
  display: grid;
  place-items: center;
  font-weight: 700;
  box-shadow: var(--shadow-sm);
  margin: -1px -1px 0 0;
  font-size: 10.5px;
}

.cell-items-preview {
  display: flex;
  flex-direction: column;
  gap: 2.5px;
  margin-top: 3px;
}

/* 跨天连续条带 + 右端把手 */
.cell-multiday-ribbon {
  display: flex;
  align-items: center;
  height: 16px;
  padding: 0 4px;
  background: var(--c-primary-light);
  border-top: 1.5px solid var(--c-primary);
  border-bottom: 1.5px solid var(--c-primary);
  font-size: 9px;
  font-weight: 600;
  color: var(--c-primary);
  cursor: grab;
  position: relative;
  overflow: hidden;
  white-space: nowrap;
}

.cell-multiday-ribbon.is-start {
  border-left: 3px solid var(--c-primary);
  border-top-left-radius: 3px;
  border-bottom-left-radius: 3px;
}

.cell-multiday-ribbon.is-end {
  border-right: 3px solid var(--c-primary);
  border-top-right-radius: 3px;
  border-bottom-right-radius: 3px;
}

.ribbon-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ribbon-cont-line {
  display: block;
  width: 100%;
  height: 2px;
  background: color-mix(in srgb, var(--c-primary) 40%, transparent);
}

.ribbon-extend-handle,
.capsule-extend-handle {
  position: absolute;
  right: 0;
  top: 0;
  bottom: 0;
  width: 6px;
  cursor: e-resize;
  background: transparent;
}

.ribbon-extend-handle:hover,
.capsule-extend-handle:hover {
  background: var(--c-primary);
}

.cell-task-capsule {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 1.5px 4px;
  border-radius: 3px;
  background: var(--c-bg-subtle);
  font-size: 9.5px;
  cursor: grab;
  position: relative;
  overflow: hidden;
}

.cell-task-capsule:hover { background: var(--c-primary-light); }

.capsule-dot {
  width: 3.5px;
  height: 3.5px;
  border-radius: 50%;
  background: var(--c-primary);
  flex-shrink: 0;
}

.capsule-title {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  color: var(--c-text);
}

/* ── 2. Week View ─────────────────────────────────────────── */
.week-full-card {
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-xl);
  box-shadow: var(--shadow-sm);
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.week-cols-header-row {
  display: grid;
  grid-template-columns: 48px repeat(7, minmax(0, 1fr));
  background: var(--c-bg-subtle);
  border-bottom: 1px solid var(--c-border);
}

.week-gutter-head {
  border-right: 1px solid var(--c-border);
}

.week-col-header-cell {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 8px 0;
  gap: 2px;
  border-right: 1px solid var(--c-border-light);
}

.week-col-header-cell .holiday-badges { min-height: 18px; justify-content: center; }
.allday-grid-cols { max-height: 180px; overflow-y: auto; }
.allday-col-drop-slot .calendar-event-item { display: block; padding: 6px; }
.allday-col-drop-slot .calendar-event-item strong,
.allday-col-drop-slot .calendar-event-item span { display: block; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.week-col-header-cell.is-today-col {
  background: var(--c-primary-light);
}

.week-col-name {
  font-family: var(--font-mono);
  font-size: 10.5px;
  font-weight: 700;
  color: var(--slate-gray-light);
}

.week-col-date-pill {
  font-family: var(--font-mono);
  font-size: 12px;
  font-weight: 700;
  padding: 1px 6px;
  border-radius: 4px;
  color: var(--c-text);
}

.week-col-date-pill.active {
  background: var(--c-primary);
  color: var(--c-primary-contrast);
}

.week-allday-ribbon-bar {
  display: grid;
  grid-template-columns: 48px minmax(0, 1fr);
  border-bottom: 1px solid var(--c-border);
  background: var(--c-bg-page);
  min-height: 32px;
}

.allday-label-col {
  font-size: 9.5px;
  font-weight: 700;
  color: var(--slate-gray-light);
  display: grid;
  place-items: center;
  border-right: 1px solid var(--c-border);
}

.allday-grid-cols {
  display: grid;
  grid-template-columns: repeat(7, minmax(0, 1fr));
}

.allday-col-drop-slot {
  min-width: 0;
  padding: 3px 4px;
  border-right: 1px solid var(--c-border-light);
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.allday-court-pill {
  font-size: 9.5px;
  font-weight: 700;
  padding: 1px 4px;
  border-radius: 3px;
  background: var(--status-risk);
  color: var(--c-primary-contrast);
}

.allday-multiday-pill {
  font-size: 9.5px;
  font-weight: 600;
  padding: 1px 4px;
  border-radius: 3px;
  background: var(--c-primary-light);
  color: var(--c-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  cursor: pointer;
}

.week-timegrid-main-body {
  display: flex;
  flex-direction: column;
  max-height: 580px;
  overflow-y: auto;
}

.week-hour-grid-row {
  display: grid;
  grid-template-columns: 48px minmax(0, 1fr);
  min-height: 44px;
  border-bottom: 1px solid var(--c-border-light);
}

.week-time-gutter {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--slate-gray-light);
  text-align: right;
  padding-right: 8px;
  border-right: 1px solid var(--c-border);
  transform: translateY(-6px);
}

.week-hour-7cols {
  display: grid;
  grid-template-columns: repeat(7, minmax(0, 1fr));
}

.week-slot-day-cell {
  border-right: 1px solid var(--c-border-light);
  padding: 2px 4px;
  display: flex;
  flex-direction: column;
  gap: 2px;
  transition: background var(--motion-fast);
}

.week-slot-day-cell:hover {
  background: var(--c-bg-hover);
}

.week-slot-day-cell.is-today-slot {
  background: color-mix(in srgb, var(--c-primary) 3%, transparent);
}

.week-cell-task-block {
  padding: 2px 5px;
  background: var(--c-bg-subtle);
  border-left: 2.5px solid var(--c-primary);
  border-radius: 3px;
  font-size: 11px;
  cursor: pointer;
}

.w-task-title {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  color: var(--c-text);
}

/* ── 3. Day View ─────────────────────────────────────────── */
.day-full-card {
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-xl);
  padding: 20px;
  box-shadow: var(--shadow-sm);
  display: flex;
  flex-direction: column;
}

.day-schedule-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--c-border);
  margin-bottom: 12px;
}

.day-schedule-heading {
  font-size: 18px;
  font-weight: 700;
  color: var(--c-text-heading);
  margin: 0;
}

.day-schedule-sub {
  font-size: 11.5px;
  color: var(--slate-gray-light);
  margin-top: 2px;
  display: block;
}

.day-view-mode-tag {
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 700;
  color: var(--c-primary);
  background: var(--c-primary-light);
  padding: 2px 6px;
  border-radius: 4px;
}

.day-multiday-active-banner {
  background: var(--c-primary-light);
  border: 1px solid color-mix(in srgb, var(--c-primary) 30%, transparent);
  border-radius: var(--c-radius-lg);
  padding: 10px 14px;
  margin-bottom: 16px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.dma-title-row {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11.5px;
  color: var(--c-primary);
}

.dma-cards-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.dma-item-card {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  min-width: 0;
  align-items: center;
  justify-content: space-between;
  background: var(--c-bg-card);
  padding: 6px 10px;
  border-radius: var(--c-radius);
  border: 1px solid var(--c-border);
  font-size: 12px;
  cursor: pointer;
}

.dma-name { min-width: 0; flex: 1 1 180px; overflow-wrap: anywhere; }
.dma-badge {
  font-family: var(--font-mono);
  font-size: 9px;
  font-weight: 700;
  padding: 1px 4px;
  background: var(--c-primary-light);
  color: var(--c-primary);
  border-radius: 3px;
  margin-right: 6px;
}

.dma-span {
  font-family: var(--font-mono);
  font-size: 10.5px;
  color: var(--slate-gray-light);
  margin-left: 6px;
}

.dma-check-btn,
.slot-check-btn {
  flex-shrink: 0;
  width: 16px;
  height: 16px;
  border-radius: 4px;
  border: 1.5px solid var(--c-border-strong);
  background: transparent;
  cursor: pointer;
  display: grid;
  place-items: center;
  color: var(--c-primary-contrast);
}

.dma-check-btn.checked,
.slot-check-btn.checked {
  background: var(--c-primary);
  border-color: var(--c-primary);
}

.day-hours-drop-stream {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.day-hour-drop-row {
  display: grid;
  grid-template-columns: 50px 1fr;
  gap: 12px;
  align-items: flex-start;
  border-radius: var(--c-radius);
  padding: 2px;
}

.day-hour-drop-row.is-drag-over {
  background: var(--c-primary-light);
}

.hour-time-label {
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 600;
  color: var(--slate-gray-light);
  padding-top: 8px;
  text-align: right;
}

.hour-slot-drop-area {
  min-height: 44px;
  border-radius: var(--c-radius);
  border: 1px dashed var(--c-border);
  background: var(--c-bg-page);
  padding: 6px 10px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  justify-content: center;
}

.day-slot-item-card {
  padding: 6px 10px;
  border-radius: var(--c-radius);
  display: flex;
  align-items: center;
  justify-content: space-between;
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-left: 3px solid var(--c-primary);
  cursor: pointer;
}

.slot-badge-caps {
  font-family: var(--font-mono);
  font-size: 8.5px;
  font-weight: 700;
  color: var(--c-primary);
  margin-right: 6px;
}

.slot-item-title {
  font-size: 12.5px;
  color: var(--c-text-heading);
  flex: 1;
}

.slot-item-title.struck {
  text-decoration: line-through;
  color: var(--slate-gray-light);
}

.hour-empty-slot-placeholder {
  font-size: 11px;
  color: var(--slate-gray-light);
  opacity: 0.7;
}

/* ── 4. Forecast View ─────────────────────────────────────── */
.forecast-full-card {
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-xl);
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.forecast-overview-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 16px;
  padding-bottom: 16px;
  border-bottom: 1px solid var(--c-border);
}

.forecast-heading {
  font-size: 20px;
  font-weight: 700;
  color: var(--c-text-heading);
  margin: 0;
}

.forecast-sub {
  font-size: 12px;
  color: var(--slate-gray-light);
  margin: 4px 0 0;
}

.forecast-stats-strip {
  display: flex;
  gap: 12px;
}

.f-stat-card {
  padding: 8px 14px;
  background: var(--c-bg-page);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.f-stat-label {
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 700;
  color: var(--slate-gray-light);
  text-transform: uppercase;
}

.f-stat-number {
  font-size: 16px;
  font-weight: 700;
}

.f-stat-number.text-risk { color: var(--status-risk); }
.f-stat-number.text-primary { color: var(--c-primary); }
.f-stat-number.text-success { color: var(--status-success); }

.forecast-filter-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.f-filter-tabs {
  display: flex;
  background: var(--c-bg-subtle);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  padding: 2px;
  gap: 2px;
}

.f-filter-tab {
  padding: 4px 12px;
  border: none;
  background: transparent;
  font-size: 11px;
  font-weight: 600;
  color: var(--c-text-secondary);
  border-radius: 5px;
  cursor: pointer;
}

.f-filter-tab.active {
  background: var(--c-bg-card);
  color: var(--c-text);
  box-shadow: var(--shadow-sm);
}

.f-total-hint {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--slate-gray-light);
}

.forecast-days-stream {
  display: flex;
  flex-direction: column;
  gap: 10px;
  max-height: calc(100vh - 270px);
  overflow-y: auto;
  padding-right: 4px;
}

.forecast-day-row {
  display: grid;
  grid-template-columns: 110px 1fr;
  gap: 16px;
  padding: 12px 16px;
  border-radius: var(--c-radius-lg);
  border: 1px solid var(--c-border);
  background: var(--c-bg-page);
  transition: all var(--motion-fast);
}

.forecast-day-row:hover {
  background: var(--c-bg-hover);
  border-color: var(--c-border-strong);
}

.forecast-day-row.is-risk-day {
  background: var(--bg-risk-weak);
  border-left: 4px solid var(--status-risk);
}

.forecast-day-row.is-busy-day {
  border-left: 4px solid var(--status-warning);
}

.forecast-day-row.is-today {
  box-shadow: inset 0 0 0 1px var(--c-primary);
}

.forecast-day-row.is-drag-target {
  background: var(--c-primary-light) !important;
  border: 2px dashed var(--c-primary);
}

.f-date-col {
  display: flex;
  flex-direction: column;
  gap: 2px;
  border-right: 1px solid var(--c-border);
  padding-right: 12px;
}

.f-day-badge {
  font-family: var(--font-mono);
  font-size: 9.5px;
  font-weight: 700;
  padding: 1px 6px;
  border-radius: 3px;
  display: inline-block;
  width: fit-content;
}

.f-day-badge.risk {
  background: var(--status-risk);
  color: var(--c-primary-contrast);
}

.f-day-badge.busy {
  background: var(--status-warning);
  color: var(--c-primary-contrast);
}

.f-day-badge.free {
  background: var(--c-bg-subtle);
  color: var(--c-text-regular);
}

.f-day-date {
  font-size: 14px;
  font-weight: 700;
  color: var(--c-text-heading);
  margin-top: 2px;
}

.f-day-weekday {
  font-size: 11px;
  color: var(--slate-gray-light);
}

.f-events-col {
  min-width: 0;
  overflow-wrap: anywhere;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.f-risk-tag-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.f-risk-pill {
  font-size: 11px;
  font-weight: 600;
  color: var(--c-text-heading);
}

.f-risk-pill.risk { color: var(--status-risk); }
.f-risk-pill.busy { color: var(--status-warning); }

.f-hours-pill {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--slate-gray-light);
  background: var(--c-bg-card);
  padding: 1px 6px;
  border-radius: 3px;
  border: 1px solid var(--c-border);
}

.f-items-stack {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.f-event-chip {
  display: flex;
  flex-wrap: wrap;
  max-width: 100%;
  min-width: 0;
  align-items: center;
  gap: 6px;
  padding: 3px 8px;
  border-radius: 4px;
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  font-size: 11px;
  cursor: pointer;
  transition: all var(--motion-fast);
}

.f-event-chip > span:not(.f-task-dot) { min-width: 0; overflow-wrap: anywhere; }
.f-event-chip small, .f-event-chip strong, .f-event-chip .el-icon { flex-shrink: 0; }
.f-event-chip small { white-space: nowrap; }
.f-risk-tag-row, .forecast-stats-strip, .f-filter-tabs { flex-wrap: wrap; }
.f-event-chip:hover {
  transform: translateY(-1px);
  box-shadow: var(--shadow-sm);
}

.f-event-chip.court {
  background: var(--bg-risk-weak);
  border-color: var(--status-risk);
  color: var(--status-risk);
  font-weight: 600;
}

.f-event-chip.multiday {
  background: var(--c-primary-light);
  border-color: var(--c-primary);
  color: var(--c-primary);
  font-weight: 600;
}

.f-event-chip.task {
  color: var(--c-text);
}

.f-task-dot {
  width: 4px;
  height: 4px;
  border-radius: 50%;
  background: var(--c-primary);
}

.f-empty-slot {
  font-size: 11.5px;
  color: var(--slate-gray-light);
  opacity: 0.7;
  padding: 4px 0;
}

/* ── B. 右侧统一 Holding Tank 任务池 ─────────────────────── */
.calendar-unified-holding-tank {
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-xl);
  padding: 16px;
  box-shadow: var(--shadow-sm);
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: 100%;
}

.calendar-unified-holding-tank.is-drag-target {
  border: 2px dashed var(--c-primary);
  background: var(--c-primary-light);
}

.tank-title-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  margin-bottom: 10px;
}

.tank-title {
  font-size: 15px;
  font-weight: 700;
  color: var(--c-text-heading);
  margin: 0;
}

.tank-sub-desc {
  font-size: 11px;
  color: var(--slate-gray-light);
  margin: 2px 0 0;
}

.tank-badge {
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 700;
  color: var(--c-primary);
  background: var(--c-primary-light);
  padding: 2px 6px;
  border-radius: 4px;
}

.tank-filter-pills {
  display: flex;
  background: var(--c-bg-subtle);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius);
  padding: 2px;
  gap: 2px;
  margin-bottom: 8px;
}

.tank-tab-btn {
  flex: 1;
  padding: 4px 0;
  border: none;
  background: transparent;
  font-size: 10.5px;
  font-weight: 600;
  color: var(--c-text-secondary);
  border-radius: 4px;
  cursor: pointer;
}

.tank-tab-btn.active {
  background: var(--c-bg-card);
  color: var(--c-text);
  box-shadow: var(--shadow-sm);
}

.tank-search-box { position: relative; }

.tank-search-icon {
  position: absolute;
  left: 8px;
  top: 50%;
  transform: translateY(-50%);
  color: var(--slate-gray-light);
}

.tank-search-real {
  width: 100%;
  background: var(--c-bg-page);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius);
  padding: 5px 8px 5px 26px;
  font-size: 11.5px;
  outline: none;
  color: var(--c-text);
}

.holding-tasks-scroll-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  overflow-y: auto;
  flex: 1;
  max-height: calc(100vh - 240px);
}

.tank-task-card {
  padding: 10px 12px;
  border-radius: var(--c-radius-lg);
  border: 1px solid var(--c-border);
  background: var(--c-bg-page);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  cursor: grab;
  transition: all var(--motion-fast);
}

.tank-task-card:hover {
  background: var(--c-bg-hover);
  border-color: var(--c-primary);
  box-shadow: var(--shadow-sm);
}

.tank-task-main {
  display: flex;
  flex-direction: column;
  gap: 3px;
  flex: 1;
  min-width: 0;
}

.tank-task-header {
  display: flex;
  align-items: center;
  gap: 5px;
}

.tank-drag-handle { color: var(--slate-gray-light); flex-shrink: 0; }

.tank-task-name {
  font-size: 12px;
  color: var(--c-text-heading);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.tank-task-name.struck {
  text-decoration: line-through;
  color: var(--slate-gray-light);
}

.tank-task-meta {
  display: flex;
  align-items: center;
  gap: 6px;
  padding-left: 20px;
}

.meta-case-tag { font-size: 10.5px; color: var(--c-primary); }

.meta-multiday-badge {
  font-family: var(--font-mono);
  font-size: 9px;
  font-weight: 700;
  color: var(--status-discovery);
  background: var(--c-bg-subtle);
  padding: 1px 4px;
  border-radius: 2px;
}

.meta-due-date {
  font-family: var(--font-mono);
  font-size: 9.5px;
  color: var(--slate-gray-light);
}

.tank-task-right {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}

.tank-est-pill {
  font-family: var(--font-mono);
  font-size: 9.5px;
  padding: 1px 5px;
  border-radius: 3px;
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  color: var(--slate-gray-light);
}

.tank-check-box {
  width: 16px;
  height: 16px;
  border-radius: 4px;
  border: 1.5px solid var(--c-border-strong);
  background: transparent;
  cursor: pointer;
  display: grid;
  place-items: center;
  color: var(--c-primary-contrast);
}

.tank-check-box.checked {
  background: var(--c-primary);
  border-color: var(--c-primary);
}

.tank-empty-box {
  padding: 24px 0;
  text-align: center;
  font-size: 12px;
  color: var(--slate-gray-light);
}

/* ═══════════════════════════════════════════════════════════
   Timeline Read-only View Styles
   ═══════════════════════════════════════════════════════════ */
.timeline-global-layout {
  display: flex;
  width: 100%;
  min-height: calc(100vh - 160px);
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-xl);
  overflow: hidden;
}

.timeline-case-filter-aside {
  flex: 0 0 220px;
  min-width: 0;
  border-right: 1px solid var(--c-border);
  padding: 20px;
}

.filter-headline-title { font-size: 14px; font-weight: 700; margin: 0 0 16px; }

.case-search-wrapper { position: relative; margin-bottom: 16px; }

.case-search-icon { position: absolute; left: 8px; top: 50%; transform: translateY(-50%); color: var(--slate-gray-light); }

.case-search-input {
  width: 100%;
  background: var(--c-bg-page);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius);
  padding: 6px 10px 6px 28px;
  font-size: 12px;
  color: var(--c-text);
  outline: none;
}

.case-checkbox-list { display: flex; flex-direction: column; gap: 8px; }

.case-checkbox-item { display: flex; align-items: center; gap: 8px; cursor: pointer; }

.case-native-checkbox { accent-color: var(--c-primary); }

.case-checkbox-name { font-size: 12.5px; min-width: 0; overflow-wrap: anywhere; }

.timeline-main-stream { flex: 1; min-width: 0; background: var(--c-bg-page); }

.timeline-stream-top {
  padding: 20px 28px;
  border-bottom: 1px solid var(--c-border);
  background: var(--c-bg-card);
}

.stream-main-heading { font-size: 20px; font-weight: 700; margin: 0; }

.stream-sub-caption { font-size: 12px; color: var(--slate-gray-light); margin: 2px 0 0; }

/* ═══════════════════════════════════════════════════════════
   4. 单日日程明细弹窗样式 (Day Modal)
   ═══════════════════════════════════════════════════════════ */
.day-modal-content {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.modal-sec-box {
  background: var(--c-bg-page);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  padding: 14px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.sec-title-flex {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.sec-label-caps {
  font-family: var(--font-mono);
  font-size: 10.5px;
  font-weight: 700;
  color: var(--slate-gray-light);
  text-transform: uppercase;
}

.sec-hint-txt {
  font-size: 11px;
  color: var(--slate-gray-light);
}

.modal-tasks-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.modal-multiday-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  background: var(--c-primary-light);
  border-left: 3.5px solid var(--c-primary);
  border-radius: var(--c-radius);
  font-size: 12.5px;
  cursor: grab;
  transition: all var(--motion-fast);
}

.modal-multiday-item:hover {
  box-shadow: var(--shadow-sm);
  filter: brightness(0.98);
}

.m-left {
  display: flex;
  align-items: center;
  gap: 6px;
}

.m-drag-icon {
  color: var(--slate-gray-light);
  flex-shrink: 0;
}

.m-badge {
  font-family: var(--font-mono);
  font-size: 9.5px;
  font-weight: 700;
  color: var(--c-primary);
  background: var(--c-bg-card);
  padding: 1px 4px;
  border-radius: 2px;
}

.modal-event-item {
  padding: 8px 12px;
  border-radius: var(--c-radius);
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  font-size: 12.5px;
  cursor: pointer;
}

.modal-event-item.is-court {
  background: var(--bg-risk-weak);
  border-left: 3.5px solid var(--status-risk);
}

.modal-event-item.is-normal {
  border-left: 3.5px solid var(--c-primary);
}

.m-ev-left {
  display: flex;
  align-items: center;
  gap: 8px;
}

.m-ev-time {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--slate-gray-light);
}

.m-ev-loc {
  font-size: 11.5px;
  color: var(--slate-gray-light);
}

.modal-task-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-left: 3.5px solid var(--c-primary);
  border-radius: var(--c-radius);
  font-size: 12.5px;
  cursor: grab;
  transition: all var(--motion-fast);
}

.modal-task-item:hover {
  box-shadow: var(--shadow-sm);
  background: var(--c-bg-hover);
}

.m-t-left {
  display: flex;
  align-items: center;
  gap: 8px;
}

.m-t-time {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--slate-gray-light);
}

.m-t-case {
  font-size: 11px;
  color: var(--c-primary);
}

.m-check-btn {
  width: 16px;
  height: 16px;
  border-radius: 4px;
  border: 1.5px solid var(--c-border-strong);
  background: transparent;
  cursor: pointer;
  display: grid;
  place-items: center;
  color: var(--c-primary-contrast);
}

.m-check-btn.checked {
  background: var(--c-primary);
  border-color: var(--c-primary);
}

.modal-empty-hint {
  font-size: 11.5px;
  color: var(--slate-gray-light);
  padding: 4px 0;
}

.dialog-footer {
  display: flex;
  justify-content: flex-end;
}

.btn-primary-action {
  padding: 6px 16px;
  border-radius: var(--c-radius-lg);
  border: none;
  background: var(--c-primary);
  color: var(--c-primary-contrast);
  font-weight: 600;
  cursor: pointer;
}

/* ═══════════════════════════════════════════════════════════
   5. 编辑详情弹窗样式
   ═══════════════════════════════════════════════════════════ */
.edit-modal-body {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.edit-form-item {
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex: 1;
}

.edit-form-item label {
  font-size: 12px;
  font-weight: 600;
  color: var(--c-text-heading);
}

.edit-form-row {
  display: flex;
  gap: 14px;
}

.edit-input,
.edit-select,
.edit-textarea {
  width: 100%;
  padding: 8px 10px;
  border-radius: var(--c-radius);
  border: 1px solid var(--c-border);
  background: var(--c-bg-page);
  color: var(--c-text);
  font-size: 12.5px;
  outline: none;
}

.edit-input:focus,
.edit-select:focus,
.edit-textarea:focus {
  border-color: var(--c-primary);
}

.edit-dialog-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
}

.btn-delete {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 6px 12px;
  border-radius: var(--c-radius-lg);
  border: 1px solid var(--status-risk);
  background: var(--bg-risk-weak);
  color: var(--status-risk);
  font-size: 12px;
  cursor: pointer;
}

.btn-delete:hover {
  background: var(--status-risk);
  color: var(--c-primary-contrast);
}

.right-btns {
  display: flex;
  gap: 10px;
}

.btn-cancel {
  padding: 6px 14px;
  border-radius: var(--c-radius-lg);
  border: 1px solid var(--c-border);
  background: var(--c-bg-subtle);
  color: var(--c-text);
  font-size: 12px;
  cursor: pointer;
}

@media (max-width: 1024px) {
  .calendar-unified-workspace-grid { grid-template-columns: 1fr; }
  .calendar-unified-holding-tank { width: 100%; }
}
@container (max-width: 1000px) {
  .calendar-unified-workspace-grid { grid-template-columns: minmax(0, 1fr); }
}
@container (max-width: 760px) {
  .calendar-unified-workspace-grid { grid-template-columns: minmax(0, 1fr); }
  .timeline-global-layout { flex-direction: column; }
  .timeline-case-filter-aside { flex-basis: auto; width: auto; border-right: 0; border-bottom: 1px solid var(--c-border); }
  .case-checkbox-list { max-height: 140px; overflow: auto; }
  .cell-header-flex { flex-wrap: wrap; gap: 3px; }
  .forecast-day-row { grid-template-columns: 90px minmax(0, 1fr); gap: 10px; }
  .forecast-overview-header, .forecast-filter-bar, .day-schedule-header { flex-wrap: wrap; gap: 12px; }
  .header-right-actions { width: 100%; min-width: 0; flex-wrap: wrap; gap: 12px; }
  .natural-input-box { width: 100%; min-width: 0; }
  .view-switch-pill { width: 100%; justify-content: space-between; }
  .switch-btn { white-space: nowrap; flex: 1; padding: 8px; }
  .month-title-row { flex-wrap: wrap; gap: 12px; }
  .month-top-stats-strip { gap: 6px; padding: 10px; }
  .m-stat-pill { flex-direction: column; gap: 4px; min-width: 0; padding: 8px 4px; }
  .m-stat-lbl { white-space: nowrap; font-size: 11px; }
  .month-matrix-day-cell { min-height: 76px; }
}
</style>

<style scoped>
.stitch-calendar-workspace { padding: 28px 24px; gap: 24px; }
.calendar-top-header { padding-bottom: 4px; }
.month-display-title { font-size: 28px; letter-spacing: -.035em; font-weight: 600; }
.calendar-unified-holding-tank { background: var(--c-bg-card); box-shadow: none; border-radius: 10px; }
.agenda-mini { padding: 18px; border-bottom: 1px solid var(--c-border); }
.agenda-mini label { display: flex; align-items: center; gap: 10px; color: var(--c-text-secondary); font-size: 11px; }
.agenda-mini input { min-width: 0; flex: 1; font: inherit; color: var(--c-text); padding: 6px; background: var(--c-bg); border: 1px solid var(--c-border); border-radius: 5px; }
.agenda-mini-title { display: flex; align-items: center; justify-content: space-between; margin: 18px 0 12px; font-size: 13px; }
.agenda-mini button { background: transparent; border: 0; color: var(--c-text); font: inherit; cursor: pointer; border-radius: 5px; }
.agenda-mini-title button { font-size: 11px; color: var(--c-primary); }
.agenda-mini .agenda-mini-item { width: 100%; padding: 9px 2px; display: flex; align-items: baseline; gap: 12px; text-align: left; font-size: 12px; }
.agenda-mini-item time { flex: 0 0 38px; color: var(--c-text-secondary); font-size: 10px; }
.agenda-mini-item span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.agenda-mini .agenda-mini-item:hover { background: var(--c-bg-hover); }
.agenda-mini p { font-size: 12px; color: var(--c-text-secondary); }
.agenda-mini button:focus-visible { outline: 2px solid var(--c-primary); }
.tank-sub-desc { line-height: 1.7; }
@media (max-width: 650px) { .stitch-calendar-workspace { padding: 20px 12px; } .header-right-actions { width: 100%; flex-wrap: wrap; } .month-display-title { font-size: 24px; } }
</style>
