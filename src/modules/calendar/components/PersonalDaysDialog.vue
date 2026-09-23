<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { casyContext } from '../../../core/plugin/context'
import type { HolidayCalendarEntry } from '../../../types/ipc'
import { availabilityTimeLabel, timeMinutes } from '../calendarDates'
import { parseLocalDate, toLocalISODate } from '../../../shared/utils/date'
const props = defineProps<{ modelValue: boolean; date: string }>()
const emit = defineEmits<{ 'update:modelValue': [value: boolean]; saved: [] }>()
const entries = ref<HolidayCalendarEntry[]>([]), date = ref(''), name = ref(''), busy = ref(false), loaded = ref(false), error = ref('')
const period = ref('all'), startTime = ref('09:00'), endTime = ref('12:00')
const selected = computed(() => entries.value.find(e => e.date === date.value))
function fill(entry?: HolidayCalendarEntry) {
  name.value = entry?.name || ''
  period.value = entry?.startTime ? 'custom' : 'all'
  startTime.value = entry?.startTime || '09:00'; endTime.value = entry?.endTime || '12:00'
  error.value = ''
}
watch(date, () => fill(selected.value))
watch(period, value => {
  if (value === 'morning') { startTime.value = '09:00'; endTime.value = '12:00' }
  if (value === 'afternoon') { startTime.value = '13:00'; endTime.value = '18:00' }
})
watch([startTime, endTime], ([start, end]) => {
  if (period.value === 'morning' && (start !== '09:00' || end !== '12:00')) period.value = 'custom'
  if (period.value === 'afternoon' && (start !== '13:00' || end !== '18:00')) period.value = 'custom'
})
watch(() => props.modelValue, async open => {
  if (!open) return
  loaded.value = false; busy.value = true; error.value = ''
  try {
    const result = await casyContext.settings.get()
    if (!result.ok) throw new Error(result.error || '加载失败')
    const saved = result.data?.personal_calendar_days
    entries.value = Array.isArray(saved) ? saved as unknown as HolidayCalendarEntry[] : []
    date.value = props.date; fill(entries.value.find(e => e.date === props.date)); loaded.value = true
  } catch (cause) { error.value = String(cause) }
  finally { busy.value = false }
})
async function persist(next: HolidayCalendarEntry[]) {
  if (busy.value || !loaded.value) return
  busy.value = true; error.value = ''
  try {
    const result = await casyContext.settings.save({ personal_calendar_days: next.map(entry => ({ date: entry.date, kind: entry.kind, name: entry.name, startTime: entry.startTime || null, endTime: entry.endTime || null })) })
    if (!result.ok) throw new Error(result.error || '保存失败')
    entries.value = next; emit('saved'); ElMessage.success('休息安排已保存')
  } catch (cause) { error.value = String(cause) }
  finally { busy.value = false }
}
function add() {
  const parsed = parseLocalDate(date.value)
  if (!parsed || toLocalISODate(parsed) !== date.value || date.value < '1900-01-01' || date.value > '2200-12-31') { error.value = '请选择有效日期（1900–2200 年）'; return }
  const start = timeMinutes(startTime.value), end = timeMinutes(endTime.value)
  if (period.value !== 'all' && (start === null || end === null || start >= end || start >= 1440)) { error.value = '请选择同一天内的开始与结束时间，结束须晚于开始'; return }
  const entry: HolidayCalendarEntry = { date: date.value, kind: 'holiday', name: name.value.trim(), startTime: period.value === 'all' ? null : startTime.value, endTime: period.value === 'all' ? null : endTime.value }
  void persist([...entries.value.filter(e => e.date !== date.value), entry].sort((a,b) => a.date.localeCompare(b.date)))
}
</script>
<template>
  <el-dialog :model-value="modelValue" title="添加休息日 / 时段" width="min(560px, 94vw)" :close-on-click-modal="!busy" :close-on-press-escape="!busy" :show-close="!busy" @update:model-value="!busy && emit('update:modelValue', $event)">
    <p>可以请全天、半天或指定时段。个人安排以虚线标记，法律期限仍按法定日历计算。已定时的工作按时段交叠提醒；没有具体时间的计划会提示核对。</p>
    <form class="rest-form" @submit.prevent="add">
      <label>日期<input v-model="date" type="date" min="1900-01-01" max="2200-12-31" :disabled="busy || !loaded" /></label>
      <label>休息范围<select v-model="period" :disabled="busy || !loaded"><option value="all">全天</option><option value="morning">上午 · 09:00–12:00</option><option value="afternoon">下午 · 13:00–18:00</option><option value="custom">自定义时段</option></select></label>
      <div v-if="period !== 'all'" class="time-range"><label>开始时间<input v-model="startTime" type="time" :disabled="busy || !loaded" /></label><label>结束时间<input v-model="endTime" type="time" :disabled="busy || !loaded || endTime === '24:00'" /><button v-if="endTime === '24:00'" type="button" @click="endTime='23:59'">结束于次日 00:00，点此修改</button></label></div>
      <label>备注（可选）<input v-model="name" maxlength="80" :disabled="busy || !loaded" /></label>
      <p v-if="selected">保存会替换 {{ date }} 已有的个人安排（{{ availabilityTimeLabel(selected) }}）。跨日期请分别添加。</p>
      <p v-if="error" role="alert" class="rest-error">{{ error }}</p>
      <el-button native-type="submit" type="primary" :loading="busy" :disabled="!loaded">保存休息安排</el-button>
    </form>
    <div class="personal-days"><div v-for="entry in entries" :key="entry.date"><span>{{ entry.date }} · {{ entry.kind === 'holiday' ? '自休' : '自班' }} · {{ availabilityTimeLabel(entry) }} {{ entry.name }}</span><div><el-button text :disabled="busy" @click="date=entry.date; fill(entry)">编辑</el-button><el-button text :disabled="busy" @click="persist(entries.filter(day => day.date !== entry.date))">移除</el-button></div></div><p v-if="loaded && !entries.length">尚未添加休息安排。</p></div>
  </el-dialog>
</template>
<style scoped>
p { color:var(--c-text-secondary); font-size:13px; line-height:1.7; }
.rest-form { display:flex; flex-direction:column; gap:12px; }.rest-form label { display:flex; flex-direction:column; gap:6px; font-size:13px; }.rest-form input,.rest-form select { font:inherit; color:var(--c-text); background:var(--c-bg-card); border:1px solid var(--c-border); border-radius:6px; padding:8px; min-width:0; }.time-range { display:grid; grid-template-columns:1fr 1fr; gap:12px; }.rest-error { color:var(--c-danger); }.personal-days { max-height:240px; overflow:auto; margin-top:18px; }.personal-days>div { display:flex; flex-wrap:wrap; justify-content:space-between; align-items:center; gap:8px; border-bottom:1px solid var(--c-border); padding-block:5px; }.personal-days>div>span { overflow-wrap:anywhere; flex:1; font-size:12px; }.personal-days>div>div { display:flex; }
</style>
