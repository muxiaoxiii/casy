<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { tauriCallSafe } from '../../../core/tauriBridge'
import { useRouter } from 'vue-router'
import {
  ArrowLeft, ArrowRight, Calendar, Clock, Warning,
  Bell, Folder, Timer, Finished
} from '@element-plus/icons-vue'

const router = useRouter()

// ============================================================
// 状态
// ============================================================
const currentDate = ref(new Date())
const events = ref([])
const tasks = ref([])
const loading = ref(false)
const selectedDay = ref(null)
const activeView = ref('month') // month/week/forecast

// ============================================================
// 常量
// ============================================================
const weekDays = ['一', '二', '三', '四', '五', '六', '日']

const viewOptions = [
  { key: 'month', label: '月视图' },
  { key: 'week', label: '周视图' },
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
// 事件颜色编码
// ============================================================

/**
 * 根据事件类型返回颜色
 *
 * 颜色规则：
 * - 口审: 蓝色 #409EFF
 * - 开庭: 红色 #F56C6C
 * - 二审: 黄色 #E6A23C
 * - 期限: 橙色 #E6A23C
 * - 任务: 紫色 #8B5CF6
 */
function getEventColor(event) {
  // 口审
  if (event.type === 'hearing') {
    return '#409EFF' // 蓝色
  }
  // 开庭
  if (event.type === 'court') {
    return '#F56C6C' // 红色
  }
  // 二审
  if (event.type === 'appeal') {
    return '#E6A23C' // 黄色
  }
  // 期限
  if (event.type?.startsWith('deadline')) {
    return '#E6A23C' // 橙色
  }
  // 任务
  if (event.type === 'task') {
    return '#8B5CF6' // 紫色
  }
  // 默认
  return '#909399' // 灰色
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
// 数据加载
// ============================================================
onMounted(() => {
  loadData()
})

async function loadData() {
  loading.value = true
  await Promise.all([
    loadEvents(),
    loadTasks(),
  ])
  loading.value = false
}

async function loadEvents() {
  const { year, month } = currentMonth.value
  const result = await tauriCallSafe('get_calendar_events', {
    year,
    month: month + 1,
  })
  if (result.ok) {
    events.value = result.data || []
  }
}

async function loadTasks() {
  const result = await tauriCallSafe('list_tasks', {
    filter: { completed: false }
  })
  if (result.ok) {
    tasks.value = result.data || []
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
// Forecast 视图相关
// ============================================================

/**
 * 获取日期的事件总数（用于紧凑月视图显示数量）
 */
function getEventCount(date) {
  return eventsForDay(date).length + tasksForDay(date).length
}

/**
 * 未来7天预测数据
 */
const forecastDays = computed(() => {
  const today = new Date()
  const days = []

  for (let i = 0; i < 7; i++) {
    const date = new Date(today)
    date.setDate(today.getDate() + i)

    const dayEvents = eventsForDay(date)
    const dayTasks = tasksForDay(date)

    // 关键事件摘要
    const keyEvents = []
    const hardEvents = dayEvents.filter(e => e.type === 'court' || e.type === 'hearing')
    const deadlineEvents = dayEvents.filter(e => e.type?.startsWith('deadline'))

    if (hardEvents.length > 0) {
      keyEvents.push(...hardEvents.slice(0, 2).map(e => ({
        title: e.title,
        type: 'hard',
        color: getEventColor(e),
      })))
    }
    if (deadlineEvents.length > 0) {
      keyEvents.push(...deadlineEvents.slice(0, 1).map(e => ({
        title: e.title,
        type: 'deadline',
        color: getEventColor(e),
      })))
    }
    if (dayTasks.length > 0) {
      keyEvents.push(...dayTasks.slice(0, 2).map(t => ({
        title: t.taskName,
        type: 'task',
        color: '#8B5CF6',
      })))
    }

    days.push({
      date,
      dateStr: formatDate(date),
      dayLabel: i === 0 ? '今天' : i === 1 ? '明天' : `${date.getMonth() + 1}/${date.getDate()}`,
      weekDay: ['日', '一', '二', '三', '四', '五', '六'][date.getDay()],
      eventCount: dayEvents.length + dayTasks.length,
      hardCount: hardEvents.length,
      deadlineCount: deadlineEvents.length,
      taskCount: dayTasks.length,
      keyEvents: keyEvents.slice(0, 3),
      hasHardSchedule: hardEvents.length > 0,
    })
  }

  return days
})

/**
 * 拖拽相关
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

  const task = draggedTask.value
  const oldDate = task.dueDate || task.deadline

  if (oldDate === dateStr) return

  // 更新任务日期
  const result = await tauriCallSafe('update_task', {
    id: task.id,
    updates: { dueDate: dateStr, deadline: dateStr }
  })

  if (result.ok) {
    await loadTasks()
  }

  draggedTask.value = null
}
</script>

<template>
  <div class="calendar-page">
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
              <el-icon color="#8B5CF6"><Finished /></el-icon>
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

    <!-- Forecast 视图 -->
    <div v-if="activeView === 'forecast'" class="forecast-container">
      <!-- 左栏：紧凑月视图 -->
      <div class="forecast-left">
        <div class="compact-calendar">
          <!-- 星期头 -->
          <div v-for="day in weekDays" :key="day" class="compact-weekday">{{ day }}</div>

          <!-- 日期格子 -->
          <div
            v-for="(day, idx) in calendarDays"
            :key="idx"
            :class="['compact-day', {
              'other-month': !day.isCurrentMonth,
              'today': isToday(day.date),
              'selected': selectedDay && day.date.toDateString() === selectedDay.date.toDateString(),
              'has-events': getEventCount(day.date) > 0,
              'has-hard': getDayStatus(day.date) === 'hard',
            }]"
            @click="selectDay(day)"
            @dragover="onDragOver(formatDate(day.date), $event)"
            @drop="onDrop(formatDate(day.date), $event)"
          >
            <span class="compact-day-number">{{ day.date.getDate() }}</span>
            <span v-if="getEventCount(day.date) > 0" class="compact-event-count" :class="{ 'has-hard': getDayStatus(day.date) === 'hard' }">
              {{ getEventCount(day.date) }}
            </span>
          </div>
        </div>
      </div>

      <!-- 右栏：详情 + 预测 -->
      <div class="forecast-right">
        <!-- 上半部分：选中日期详情 -->
        <div class="forecast-detail">
          <div class="forecast-section-header">
            <h3>{{ selectedDay ? formatDate(selectedDay.date) : '选择日期查看详情' }}</h3>
            <div v-if="selectedDay" class="forecast-stats">
              <span v-if="selectedDayStats.hardSchedule > 0" class="stat hard">
                <el-icon><Bell /></el-icon> {{ selectedDayStats.hardSchedule }}
              </span>
              <span v-if="selectedDayStats.deadlines > 0" class="stat deadline">
                <el-icon><Warning /></el-icon> {{ selectedDayStats.deadlines }}
              </span>
              <span v-if="selectedDayStats.tasks > 0" class="stat task">
                <el-icon><Finished /></el-icon> {{ selectedDayStats.tasks }}
              </span>
            </div>
          </div>

          <div class="forecast-detail-content" v-if="selectedDay">
            <!-- 硬性日程 -->
            <div
              v-for="event in selectedDayEvents.filter(e => e.type === 'court' || e.type === 'hearing')"
              :key="event.id"
              class="forecast-event-item"
              :style="{ borderLeftColor: getEventColor(event) }"
            >
              <el-icon :color="getEventColor(event)"><Bell /></el-icon>
              <div class="forecast-event-info">
                <span class="forecast-event-title">{{ event.title }}</span>
                <span class="forecast-event-case" v-if="event.caseName">{{ event.caseName }}</span>
              </div>
              <span class="forecast-event-type">开庭</span>
            </div>

            <!-- 期限 -->
            <div
              v-for="event in selectedDayEvents.filter(e => e.type?.startsWith('deadline'))"
              :key="event.id"
              class="forecast-event-item"
              :style="{ borderLeftColor: getEventColor(event) }"
            >
              <el-icon :color="getEventColor(event)"><Warning /></el-icon>
              <div class="forecast-event-info">
                <span class="forecast-event-title">{{ event.title }}</span>
                <span class="forecast-event-case" v-if="event.caseName">{{ event.caseName }}</span>
              </div>
              <span class="forecast-event-type">期限</span>
            </div>

            <!-- 任务 -->
            <div
              v-for="task in selectedDayTasks"
              :key="task.id"
              class="forecast-event-item task"
              :style="{ borderLeftColor: '#8B5CF6' }"
              draggable="true"
              @dragstart="onDragStart(task, $event)"
              @click="router.push({ name: 'tasks', query: { edit: task.id } })"
            >
              <el-icon color="#8B5CF6"><Finished /></el-icon>
              <div class="forecast-event-info">
                <span class="forecast-event-title">{{ task.taskName }}</span>
                <span class="forecast-event-case" v-if="task.caseName">{{ task.caseName }}</span>
              </div>
              <span class="forecast-event-type">任务</span>
            </div>

            <!-- 空状态 -->
            <div v-if="selectedDayEvents.length === 0 && selectedDayTasks.length === 0" class="forecast-empty">
              当日无安排
            </div>
          </div>

          <div v-else class="forecast-empty">
            <el-icon :size="48" color="#D4D4D8"><Calendar /></el-icon>
            <p>点击左侧日期查看详情</p>
          </div>
        </div>

        <!-- 下半部分：未来7天预测 -->
        <div class="forecast-prediction">
          <div class="forecast-section-header">
            <h3>未来 7 天预测</h3>
          </div>

          <div class="forecast-days">
            <div
              v-for="day in forecastDays"
              :key="day.dateStr"
              :class="['forecast-day-item', { 'has-hard': day.hasHardSchedule }]"
              @click="selectDay({ date: day.date, isCurrentMonth: true })"
            >
              <div class="forecast-day-header">
                <span class="forecast-day-label">{{ day.dayLabel }}</span>
                <span class="forecast-day-weekday">周{{ day.weekDay }}</span>
                <span class="forecast-day-count" :class="{ 'has-events': day.eventCount > 0 }">
                  {{ day.eventCount > 0 ? `${day.eventCount} 项` : '无安排' }}
                </span>
              </div>

              <div v-if="day.keyEvents.length > 0" class="forecast-day-events">
                <div
                  v-for="(event, eIdx) in day.keyEvents"
                  :key="eIdx"
                  class="forecast-day-event"
                  :style="{ color: event.color }"
                >
                  <span class="forecast-day-event-dot" :style="{ background: event.color }"></span>
                  {{ event.title }}
                </div>
              </div>

              <div v-if="day.hardCount > 0 || day.deadlineCount > 0" class="forecast-day-badges">
                <span v-if="day.hardCount > 0" class="forecast-badge hard">
                  <el-icon :size="12"><Bell /></el-icon> {{ day.hardCount }}
                </span>
                <span v-if="day.deadlineCount > 0" class="forecast-badge deadline">
                  <el-icon :size="12"><Warning /></el-icon> {{ day.deadlineCount }}
                </span>
                <span v-if="day.taskCount > 0" class="forecast-badge task">
                  <el-icon :size="12"><Finished /></el-icon> {{ day.taskCount }}
                </span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- 图例 -->
    <div class="calendar-legend">
      <span class="legend-item">
        <span class="legend-dot" style="background: #409EFF" />
        口审
      </span>
      <span class="legend-item">
        <span class="legend-dot" style="background: #F56C6C" />
        开庭
      </span>
      <span class="legend-item">
        <span class="legend-dot" style="background: #E6A23C" />
        二审/期限
      </span>
      <span class="legend-item">
        <span class="legend-dot" style="background: #8B5CF6" />
        任务
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
  background: #EFF6FF;
}

.day-cell.selected {
  background: #DBEAFE;
  box-shadow: inset 0 0 0 2px #2563EB;
}

.day-cell.has-hard {
  border-top: 2px solid #EF4444;
}

.day-cell.has-overdue {
  border-top: 2px solid #F59E0B;
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
  color: #2563EB;
  font-weight: 600;
}

.day-indicator {
  display: flex;
  align-items: center;
}

.day-indicator.hard {
  color: #EF4444;
}

.day-indicator.overdue {
  color: #F59E0B;
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
  background: #FEF2F2;
  color: #F56C6C;
}

.event-badge.deadline {
  background: #FDF6EC;
  color: #E6A23C;
}

.event-badge.task {
  background: #EDE9FE;
  color: #8B5CF6;
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
  background: #FEF2F2;
  color: #F56C6C;
}

.stat.deadline {
  background: #FDF6EC;
  color: #E6A23C;
}

.stat.task {
  background: #EDE9FE;
  color: #8B5CF6;
}

.stat.overdue {
  background: #FEF2F2;
  color: #F56C6C;
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
   Forecast 视图样式
   ============================================================ */
.forecast-container {
  display: flex;
  gap: 20px;
  min-height: 600px;
}

.forecast-left {
  width: 60%;
  background: #FFFFFF;
  border-radius: 8px;
  border: 1px solid #E4E7ED;
  padding: 16px;
}

.forecast-right {
  width: 40%;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

/* 紧凑月视图 */
.compact-calendar {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  gap: 4px;
}

.compact-weekday {
  text-align: center;
  font-size: 12px;
  font-weight: 500;
  color: #71717A;
  padding: 8px 0;
}

.compact-day {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 8px 4px;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.15s;
  min-height: 48px;
}

.compact-day:hover {
  background: #F4F4F5;
}

.compact-day.other-month {
  opacity: 0.4;
}

.compact-day.today {
  background: #EFF6FF;
}

.compact-day.selected {
  background: #DBEAFE;
  box-shadow: inset 0 0 0 2px #2563EB;
}

.compact-day.has-events {
  background: #FAFAFA;
}

.compact-day.has-hard {
  border-top: 2px solid #F56C6C;
}

.compact-day-number {
  font-size: 14px;
  font-weight: 500;
  color: #18181B;
}

.compact-day.today .compact-day-number {
  color: #2563EB;
  font-weight: 600;
}

.compact-event-count {
  font-size: 10px;
  color: #71717A;
  margin-top: 2px;
}

.compact-event-count.has-hard {
  color: #F56C6C;
  font-weight: 600;
}

/* Forecast 右栏 */
.forecast-detail,
.forecast-prediction {
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

.forecast-stats {
  display: flex;
  gap: 8px;
}

.forecast-detail-content {
  padding: 12px;
  max-height: 280px;
  overflow-y: auto;
}

/* Forecast 事件项 */
.forecast-event-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px;
  border-radius: 6px;
  margin-bottom: 8px;
  border-left: 3px solid transparent;
  background: #FAFAFA;
  cursor: pointer;
  transition: background 0.15s;
}

.forecast-event-item:hover {
  background: #F4F4F5;
}

.forecast-event-item.task {
  cursor: grab;
}

.forecast-event-item.task:active {
  cursor: grabbing;
}

.forecast-event-info {
  flex: 1;
  min-width: 0;
}

.forecast-event-title {
  display: block;
  font-size: 13px;
  font-weight: 500;
  color: #18181B;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.forecast-event-case {
  display: block;
  font-size: 11px;
  color: #A1A1AA;
  margin-top: 2px;
}

.forecast-event-type {
  font-size: 11px;
  color: #71717A;
  padding: 2px 6px;
  background: #F4F4F5;
  border-radius: 4px;
}

.forecast-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 32px 16px;
  color: #A1A1AA;
  font-size: 13px;
}

.forecast-empty p {
  margin: 8px 0 0;
}

/* 未来7天预测 */
.forecast-prediction {
  flex: 1;
}

.forecast-days {
  padding: 8px;
}

.forecast-day-item {
  padding: 10px 12px;
  border-radius: 6px;
  margin-bottom: 4px;
  cursor: pointer;
  transition: background 0.15s;
}

.forecast-day-item:hover {
  background: #F4F4F5;
}

.forecast-day-item.has-hard {
  background: #FEF2F2;
}

.forecast-day-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 4px;
}

.forecast-day-label {
  font-size: 13px;
  font-weight: 600;
  color: #18181B;
  min-width: 40px;
}

.forecast-day-weekday {
  font-size: 12px;
  color: #71717A;
}

.forecast-day-count {
  margin-left: auto;
  font-size: 12px;
  color: #A1A1AA;
}

.forecast-day-count.has-events {
  color: #2563EB;
  font-weight: 500;
}

.forecast-day-events {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin-top: 4px;
}

.forecast-day-event {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.forecast-day-event-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex-shrink: 0;
}

.forecast-day-badges {
  display: flex;
  gap: 6px;
  margin-top: 6px;
}

.forecast-badge {
  display: flex;
  align-items: center;
  gap: 3px;
  font-size: 11px;
  padding: 2px 6px;
  border-radius: 4px;
}

.forecast-badge.hard {
  background: #FEE2E2;
  color: #F56C6C;
}

.forecast-badge.deadline {
  background: #FEF3C7;
  color: #E6A23C;
}

.forecast-badge.task {
  background: #EDE9FE;
  color: #8B5CF6;
}
</style>
