<script setup>
import { ref, computed, onMounted } from 'vue'
import { tauriCallSafe } from '../../../core/tauriBridge.js'

const currentDate = ref(new Date())
const events = ref([])
const loading = ref(false)
const selectedDay = ref(null)

const weekDays = ['一', '二', '三', '四', '五', '六', '日']

const currentMonth = computed(() => {
  const y = currentDate.value.getFullYear()
  const m = currentDate.value.getMonth()
  return { year: y, month: m }
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

const monthLabel = computed(() => {
  const { year, month } = currentMonth.value
  return `${year}年${month + 1}月`
})

onMounted(() => {
  loadEvents()
})

async function loadEvents() {
  loading.value = true
  const { year, month } = currentMonth.value
  const result = await tauriCallSafe('get_calendar_events', {
    year,
    month: month + 1,
  })
  if (result.ok) {
    events.value = result.data || []
  }
  loading.value = false
}

function prevMonth() {
  const d = new Date(currentDate.value)
  d.setMonth(d.getMonth() - 1)
  currentDate.value = d
  loadEvents()
}

function nextMonth() {
  const d = new Date(currentDate.value)
  d.setMonth(d.getMonth() + 1)
  currentDate.value = d
  loadEvents()
}

function goToday() {
  currentDate.value = new Date()
  loadEvents()
}

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
  return events.value.filter((e) => e.date === dateStr)
}

function eventTypeColor(type) {
  const colors = {
    hearing: '#3b82f6',       // 蓝=口审
    court: '#ef4444',         // 红=开庭
    appeal: '#eab308',        // 黄=二审
    deadline: '#f97316',      // 橙=期限
    deadline_red: '#ef4444',
    deadline_yellow: '#eab308',
    deadline_green: '#22c55e',
    task: '#8b5cf6',          // 紫=任务
  }
  return colors[type] || '#9ca3af'
}

function selectDay(day) {
  selectedDay.value = day
}
</script>

<template>
  <div class="calendar-page">
    <div class="calendar-toolbar">
      <el-button @click="prevMonth" text>◀</el-button>
      <el-button @click="goToday" size="small">今天</el-button>
      <span class="month-label">{{ monthLabel }}</span>
      <el-button @click="nextMonth" text>▶</el-button>
    </div>

    <div class="calendar-grid">
      <div v-for="day in weekDays" :key="day" class="weekday-header">{{ day }}</div>
      <div
        v-for="(day, idx) in calendarDays"
        :key="idx"
        class="day-cell"
        :class="{
          'other-month': !day.isCurrentMonth,
          today: isToday(day.date),
          selected: selectedDay && day.date.toDateString() === selectedDay.date.toDateString(),
        }"
        @click="selectDay(day)"
      >
        <div class="day-number">{{ day.date.getDate() }}</div>
        <div class="day-events">
          <div
            v-for="event in eventsForDay(day.date).slice(0, 3)"
            :key="event.id"
            class="event-badge"
            :style="{ background: eventTypeColor(event.type) }"
            :title="event.title"
          >
            <span class="event-badge-text">{{ event.title }}</span>
          </div>
          <div v-if="eventsForDay(day.date).length > 3" class="event-more">
            +{{ eventsForDay(day.date).length - 3 }}
          </div>
        </div>
      </div>
    </div>

    <!-- 选中日期的事件列表 -->
    <div v-if="selectedDay" class="selected-day-events">
      <h4>{{ formatDate(selectedDay.date) }} 的事件</h4>
      <div v-if="eventsForDay(selectedDay.date).length">
        <div
          v-for="event in eventsForDay(selectedDay.date)"
          :key="event.id"
          class="event-item"
        >
          <span class="event-color" :style="{ background: eventTypeColor(event.type) }" />
          <span class="event-title">{{ event.title }}</span>
          <span v-if="event.caseName" class="event-case">{{ event.caseName }}</span>
        </div>
      </div>
      <div v-else class="no-events">当日无事件</div>
    </div>

    <!-- 图例 -->
    <div class="calendar-legend">
      <span class="legend-item"><span class="legend-dot" style="background: #3b82f6" /> 口审</span>
      <span class="legend-item"><span class="legend-dot" style="background: #ef4444" /> 开庭</span>
      <span class="legend-item"><span class="legend-dot" style="background: #eab308" /> 二审</span>
      <span class="legend-item"><span class="legend-dot" style="background: #f97316" /> 期限</span>
      <span class="legend-item"><span class="legend-dot" style="background: #8b5cf6" /> 任务</span>
    </div>
  </div>
</template>

<style scoped>
.calendar-page {
  max-width: 900px;
  margin: 0 auto;
}

.calendar-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
}

.month-label {
  font-size: 18px;
  font-weight: 500;
  min-width: 120px;
  text-align: center;
}

.calendar-grid {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  gap: 1px;
  background: #e0e0e0;
  border: 1px solid #e0e0e0;
  border-radius: 8px;
  overflow: hidden;
}

.weekday-header {
  background: #f5f5f5;
  padding: 8px;
  text-align: center;
  font-size: 13px;
  font-weight: 500;
  color: #666;
}

.day-cell {
  background: white;
  padding: 4px 6px;
  min-height: 90px;
  cursor: pointer;
}

.day-cell:hover {
  background: #f0f7ff;
}

.day-cell.other-month {
  background: #fafafa;
  color: #ccc;
}

.day-cell.today {
  background: #ecf5ff;
}

.day-cell.selected {
  outline: 2px solid #409eff;
  outline-offset: -2px;
}

.day-number {
  font-size: 14px;
  font-weight: 500;
  margin-bottom: 4px;
}

.day-events {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.event-badge {
  padding: 1px 4px;
  border-radius: 3px;
  font-size: 11px;
  color: white;
  line-height: 1.4;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.event-badge-text {
  font-size: 11px;
}

.event-more {
  font-size: 10px;
  color: #999;
  text-align: center;
}

.selected-day-events {
  margin-top: 16px;
  padding: 12px;
  background: #f5f7fa;
  border-radius: 8px;
}

.selected-day-events h4 {
  margin: 0 0 8px 0;
}

.event-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 0;
}

.event-color {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  flex-shrink: 0;
}

.event-title {
  font-weight: 500;
}

.event-case {
  color: #666;
  font-size: 13px;
}

.no-events {
  color: #999;
  font-size: 13px;
}

.calendar-legend {
  margin-top: 16px;
  display: flex;
  gap: 16px;
  flex-wrap: wrap;
}

.legend-item {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 13px;
  color: #666;
}

.legend-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
}
</style>
