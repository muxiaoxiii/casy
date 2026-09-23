<script setup lang="ts">
import { computed, ref } from 'vue'
import HolidayBadges from './HolidayBadges.vue'
import { toLocalISODate, todayLocalISO } from '../../../shared/utils/date'
import { yearTaskDays, heatLevel, type HeatmapTask } from '../yearHeatmap'
import type { HolidayCalendarEntry } from '../../../types/ipc'
const props = defineProps<{ year: number; tasks: HeatmapTask[]; holidays: HolidayCalendarEntry[]; loading?: boolean }>()
const emit = defineEmits<{ month: [date: string]; day: [date: string]; task: [task: HeatmapTask] }>()
const selected = ref('')
const taskDays = computed(() => yearTaskDays(props.year, props.tasks))
const holidayMap = computed(() => {
  const map = new Map<string, HolidayCalendarEntry[]>()
  for (const entry of props.holidays) map.set(entry.date, [...(map.get(entry.date) || []), entry])
  return map
})
const months = computed(() => Array.from({ length: 12 }, (_, month) => {
  const first = new Date(props.year, month, 1), count = new Date(props.year, month + 1, 0).getDate()
  const offset = (first.getDay() + 6) % 7
  const days = Array.from({ length: count }, (_, index) => {
    const key = toLocalISODate(new Date(props.year, month, index + 1)), tasks = taskDays.value.get(key) || [], holiday = holidayMap.value.get(key) || []
    return { key, number: index + 1, count: tasks.length, level: heatLevel(tasks.length), holiday, title: `${key} · ${tasks.length} 项任务${holiday.map(entry => ` · ${entry.source === 'personal' ? '个人' : '法定'}${entry.kind === 'holiday' ? '休' : '班'} ${entry.name}`).join('')}` }
  })
  const total = days.reduce((sum, day) => sum + day.count, 0)
  return { month, date: toLocalISODate(first), offset, days, total, busyDays: days.filter(day => day.count).length, density: Math.min(18, total / count * 4) }
}))
const selectedTasks = computed(() => taskDays.value.get(selected.value) || [])
</script>
<template>
  <section class="year-heatmap" aria-label="年度任务热力图" :aria-busy="loading">
    <div class="heatmap-intro"><div><h2>全年工作密度</h2><p>按每天已排期的任务数着色（含已完成），跨日任务在每天计一次；未排期任务不计入。</p></div><div class="heatmap-legend" aria-label="任务数量色阶"><span>少</span><span v-for="(label, level) in ['0 项', '1 项', '2—3 项', '4—6 项', '7 项及以上']" :key="level" class="heat-swatch" :data-level="level" :title="label" /><span>多</span></div></div>
    <div class="year-months">
      <section v-for="month in months" :key="month.month" class="mini-month" :style="{ '--month-density': `${month.density}%` }">
        <button class="mini-month-heading" @click="emit('month', month.date)"><strong>{{ month.month + 1 }} 月</strong><span>{{ month.busyDays }} 天有任务</span></button>
        <div class="mini-weekdays"><span v-for="day in ['一','二','三','四','五','六','日']" :key="day">{{ day }}</span></div>
        <div class="mini-days">
          <span v-for="space in month.offset" :key="`blank-${space}`" aria-hidden="true" />
          <button v-for="day in month.days" :key="day.key" class="heat-day" :data-level="day.level" :class="{ today: day.key === todayLocalISO(), selected: selected === day.key }" :title="day.title" :aria-label="day.title" :aria-pressed="selected === day.key" @click="selected = day.key">
            <span>{{ day.number }}</span><HolidayBadges class="heat-day-badges" :entries="day.holiday" compact />
          </button>
        </div>
      </section>
    </div>
    <section v-if="selected.startsWith(`${year}-`)" class="heatmap-detail" aria-live="polite">
      <header><strong>{{ selected }} · {{ selectedTasks.length }} 项任务</strong><el-button @click="emit('day', selected)">打开当日日历</el-button></header>
      <HolidayBadges :entries="holidayMap.get(selected) || []" />
      <button v-for="task in selectedTasks" :key="task.id" class="heat-task" @click="emit('task', task)"><span>{{ task.taskName || '未命名任务' }}</span><small>{{ task.completed ? '已完成' : '待完成' }}</small></button>
      <p v-if="!selectedTasks.length">当日没有已排期任务。</p>
    </section>
  </section>
</template>
<style scoped>
.year-heatmap { --heat-0: var(--c-bg-subtle); --heat-1: color-mix(in srgb, var(--c-primary) 18%, var(--c-bg-card)); --heat-2: color-mix(in srgb, var(--c-primary) 36%, var(--c-bg-card)); --heat-3: color-mix(in srgb, var(--c-primary) 60%, var(--c-bg-card)); --heat-4: var(--c-primary); }
.heatmap-intro, .heatmap-detail header { display: flex; align-items: center; justify-content: space-between; gap: 16px; flex-wrap: wrap; }
h2 { font-size: 17px; margin: 0; }
p { color: var(--c-text-secondary); line-height: 1.6; font-size: 13px; }
.heatmap-legend { display: flex; align-items: center; gap: 5px; font-size: 12px; color: var(--c-text-secondary); }
.heat-swatch { width: 15px; height: 15px; border-radius: 3px; }
.year-months { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 235px), 1fr)); gap: 16px; margin-top: 12px; }
.mini-month { border: 1px solid var(--c-border); border-radius: 12px; padding: 14px; background: color-mix(in srgb, var(--c-primary) var(--month-density), var(--c-bg-card)); }
.mini-month-heading { border: 0; background: none; color: var(--c-text-heading); display: flex; align-items: baseline; justify-content: space-between; width: 100%; padding: 0 0 12px; cursor: pointer; }
.mini-month-heading strong { font-size: 16px; }.mini-month-heading span { color: var(--c-text-secondary); font-size: 11px; }
.mini-weekdays, .mini-days { display: grid; grid-template-columns: repeat(7, 1fr); gap: 4px; }
.mini-weekdays { color: var(--c-text-secondary); font-size: 11px; text-align: center; padding-bottom: 7px; }
.heat-day { border: 1px solid transparent; border-radius: 4px; min-width: 0; aspect-ratio: 1; min-height: 36px; position: relative; display: grid; place-content: center; color: var(--c-text); font-size: 12px; cursor: pointer; padding: 0 0 10px; }
[data-level="0"] { background: var(--heat-0); } [data-level="1"] { background: var(--heat-1); } [data-level="2"] { background: var(--heat-2); } [data-level="3"] { background: var(--heat-3); } [data-level="4"] { background: var(--heat-4); color: var(--c-primary-contrast); }
.heat-day:hover, .heat-day.selected { outline: 2px solid var(--c-primary); outline-offset: 1px; }
.heat-day.today { border-color: var(--c-text); font-weight: 800; }
.heat-day-badges { position: absolute; bottom: 1px; left: 0; right: 0; gap: 1px; justify-content: center; flex-wrap: nowrap; }.heat-day-badges :deep(.holiday-badge) { font-size: 7px; padding: 0; line-height: 1; }
.heat-day small { position: absolute; font-size: 8px; bottom: 0; right: 1px; line-height: 1; padding: 1px; border-radius: 2px; color: var(--c-warning); background: var(--c-warning-light); }.heat-day small.workday { color: var(--c-info); background: var(--c-info-light); }
.heatmap-detail { margin-top: 20px; border: 1px solid var(--c-border); background: var(--c-bg-card); border-radius: 12px; padding: 16px; }
.heat-task { display: flex; justify-content: space-between; gap: 12px; border: 0; border-bottom: 1px solid var(--c-border-light); width: 100%; padding: 12px 0; background: transparent; color: var(--c-text); text-align: left; cursor: pointer; }.heat-task > span { min-width: 0; overflow-wrap: anywhere; }.heat-task small { color: var(--c-text-secondary); flex-shrink: 0; }
@container(min-width: 1100px) { .year-months { grid-template-columns: repeat(4, 1fr); } }
@container(max-width: 520px) { .year-months { grid-template-columns: 1fr; } }
</style>
