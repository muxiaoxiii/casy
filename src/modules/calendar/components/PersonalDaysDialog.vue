<script setup lang="ts">
import { ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import { casyContext } from '../../../core/plugin/context'
import type { HolidayCalendarEntry } from '../../../types/ipc'
const props = defineProps<{ modelValue: boolean; date: string }>()
const emit = defineEmits<{ 'update:modelValue': [value: boolean]; saved: [] }>()
const entries = ref<HolidayCalendarEntry[]>([]), date = ref(''), kind = ref('holiday'), name = ref(''), busy = ref(false), loaded = ref(false)
watch(() => props.modelValue, async open => {
  if (!open) return
  loaded.value = false; busy.value = true; date.value = props.date; name.value = ''
  try {
    const result = await casyContext.settings.get()
    if (!result.ok) throw new Error(result.error || '加载失败')
    const saved = result.data?.personal_calendar_days
    entries.value = Array.isArray(saved) ? saved as unknown as HolidayCalendarEntry[] : []
    loaded.value = true
  } catch (error) { ElMessage.error(String(error)) }
  finally { busy.value = false }
})
async function persist(next: HolidayCalendarEntry[]) {
  if (busy.value || !loaded.value) return
  busy.value = true
  try {
    const result = await casyContext.settings.save({ personal_calendar_days: next.map(entry => ({ date: entry.date, kind: entry.kind, name: entry.name })) })
    if (!result.ok) throw new Error(result.error || '保存失败')
    entries.value = next; emit('saved'); ElMessage.success('个人调休已保存')
  } catch (error) { ElMessage.error(String(error)) }
  finally { busy.value = false }
}
function add() {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(date.value)) return ElMessage.warning('请选择日期')
  void persist([...entries.value.filter(entry => entry.date !== date.value), { date: date.value, kind: kind.value, name: name.value.trim() }].sort((a, b) => a.date.localeCompare(b.date)))
}
</script>
<template>
  <el-dialog :model-value="modelValue" title="个人调休" width="560px" :close-on-click-modal="!busy" :show-close="!busy" @update:model-value="!busy && emit('update:modelValue', $event)">
    <p>设置自己的休息日或上班日，以虚线「自休 / 自班」标记；与法定安排同时展示，不改变法律期限的法定工作日计算。</p>
    <el-form label-position="top">
      <el-form-item label="日期"><el-date-picker v-model="date" type="date" value-format="YYYY-MM-DD" :disabled="busy || !loaded" /></el-form-item>
      <el-form-item label="安排"><el-radio-group v-model="kind" :disabled="busy || !loaded"><el-radio-button value="holiday">个人休息</el-radio-button><el-radio-button value="workday">个人上班</el-radio-button></el-radio-group></el-form-item>
      <el-form-item label="备注（可选）"><el-input v-model="name" maxlength="80" :disabled="busy || !loaded" /></el-form-item>
      <el-button type="primary" :loading="busy" :disabled="!loaded" @click="add">保存此日期</el-button>
    </el-form>
    <div class="personal-days"><div v-for="entry in entries" :key="entry.date"><span>{{ entry.date }} · {{ entry.kind === 'holiday' ? '自休' : '自班' }} {{ entry.name }}</span><el-button text :disabled="busy" @click="persist(entries.filter(day => day.date !== entry.date))">移除</el-button></div><p v-if="loaded && !entries.length">尚未设置个人调休。</p></div>
  </el-dialog>
</template>
<style scoped>
p { color: var(--c-text-secondary); font-size: 13px; line-height: 1.7; }
.personal-days { max-height: 240px; overflow: auto; margin-top: 18px; }.personal-days > div { display: flex; justify-content: space-between; align-items: center; gap: 12px; border-bottom: 1px solid var(--c-border); padding-block: 5px; }
</style>
