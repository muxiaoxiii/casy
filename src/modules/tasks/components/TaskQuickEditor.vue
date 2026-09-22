<script setup lang="ts">
import { ref } from 'vue'
import { ElMessage } from 'element-plus'
import { useRouter } from 'vue-router'
import { casyContext } from '../../../core/plugin/context'
import type { UpdateTaskPayload } from '../../../types/ipc'
import { minutes } from '../../calendar/parseCalendarCapture'
import type { Task } from '../../../types'
import RelatedWork from '../../../shared/components/RelatedWork.vue'

const props = defineProps<{ task: Task; cases: Array<{ id: string; caseName?: string | null; caseNo?: string | null }> }>()
const emit = defineEmits<{ close: []; saved: []; advanced: [] }>()
const router = useRouter()
const title = ref(props.task.taskName)
const notes = ref(props.task.description || '')
const dueDate = ref(props.task.dueDate || '')
const caseId = ref(props.task.caseId || '')
const bucket = ref(props.task.startBucket || 'inbox')
const busy = ref(false)
const scheduling = ref(false)
const now = new Date()
const day = ref(`${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`)
const time = ref('09:00')
const duration = ref(props.task.estimatedMinutes || 60)
const error = ref('')
const form = () => ({ taskName: title.value.trim(), description: notes.value, dueDate: dueDate.value || null, caseId: caseId.value || null, startBucket: bucket.value })
let baseline = form()
const snapshot = () => JSON.stringify(form())
let savedSnapshot = snapshot()

async function save() {
  if (busy.value || !title.value.trim()) return false
  busy.value = true
  try {
    const current = form()
    // Send only edited fields, so background changes to other task fields survive.
    const changed = Object.fromEntries(Object.entries(current).filter(([key, value]) => value !== baseline[key as keyof typeof baseline]))
    if (!Object.keys(changed).length) return true
    const patch: UpdateTaskPayload = { id: props.task.id, ...changed }
    const result = await casyContext.tasks.update(patch)
    if (!result.ok) { error.value = result.error || '保存失败'; return false }
    baseline = current; savedSnapshot = snapshot(); error.value = ''; emit('saved'); return true
  } finally { busy.value = false }
}
async function saveAndClose() { if (await save()) emit('close') }
async function schedule() {
  if (busy.value || !day.value || !time.value) return
  const start = minutes(time.value)
  const end = (start ?? 0) + Number(duration.value)
  if (start === null || !Number.isInteger(duration.value) || end >= 1440 || duration.value <= 0) { error.value = '请将时间块安排在同一天内，时长需大于 0'; return }
  if (!(await save())) return
  busy.value = true
  try {
    const result = await casyContext.calendar.createEvent({
      title: title.value.trim(), eventDate: day.value, startTime: time.value,
      endTime: `${String(Math.floor(end / 60)).padStart(2, '0')}:${String(end % 60).padStart(2, '0')}`,
      allDay: false, caseId: caseId.value || null, taskId: props.task.id,
    })
    if (!result.ok) { error.value = result.error || '排期失败，任务修改已保存'; return }
    ElMessage.success('已添加关联时间块')
    emit('close')
    await router.push({ path: '/calendar', query: { date: day.value, view: 'day' } })
  } finally { busy.value = false }
}
defineExpose({ saveIfDirty: () => snapshot() === savedSnapshot ? Promise.resolve(true) : save() })
</script>

<template>
  <section class="task-quick-editor" aria-label="编辑任务" @keydown.meta.enter.prevent="saveAndClose" @keydown.ctrl.enter.prevent="saveAndClose" @keydown.esc.stop="!busy && emit('close')">
    <input v-model="title" aria-label="任务名称" class="quick-title" placeholder="下一步要做什么？" :disabled="busy" />
    <textarea v-model="notes" aria-label="任务备注" placeholder="补充思路、准备材料或下一步…" rows="3" :disabled="busy" />
    <div class="quick-fields">
      <label>何时开始<select v-model="bucket" :disabled="busy"><option value="inbox">收件箱</option><option value="today">今天</option><option value="anytime">随时</option><option value="someday">将来</option></select></label>
      <label>截止日期<input v-model="dueDate" type="date" :disabled="busy" /></label>
      <label>关联案件<select v-model="caseId" :disabled="busy"><option value="">独立任务</option><option v-for="c in cases" :key="c.id" :value="c.id">{{ c.caseName || c.caseNo }}</option></select></label>
    </div>
    <div v-if="scheduling" class="schedule-fields">
      <p>为这项任务留出一段时间，截止日期单独管理。</p>
      <label>排期日期<input v-model="day" type="date" :disabled="busy" /></label>
      <label>开始<input v-model="time" type="time" :disabled="busy" /></label>
      <label>分钟<input v-model.number="duration" type="number" min="5" max="720" step="15" :disabled="busy" /></label>
      <button type="button" class="primary" :disabled="busy || !title.trim()" @click="schedule">安排并打开日历</button>
    </div>
    <p v-if="error" role="alert" class="error">{{ error }}</p>
    <footer>
      <button type="button" :disabled="busy" :aria-expanded="scheduling" @click="scheduling = !scheduling">◷ 安排时间</button>
      <button type="button" :disabled="busy" @click="async () => { if (await save()) emit('advanced') }">更多选项</button>
      <span />
      <button type="button" :disabled="busy" @click="emit('close')">取消</button>
      <button type="button" class="primary" :disabled="busy || !title.trim()" @click="saveAndClose">{{ busy ? '保存中…' : '完成' }}</button>
    </footer>
    <button v-if="task.knowledgeId" type="button" class="task-source-link" @click="router.push({ path: '/knowledge', query: { select: task.knowledgeId } })">↗ 返回来源笔记</button>
    <RelatedWork v-if="caseId" :case-id="caseId" />
  </section>
</template>

<style scoped>
.task-quick-editor { padding: 22px 24px; margin: 0 8px 16px 32px; background: var(--c-bg-card); border: 1px solid var(--c-border-strong); border-radius: 12px; box-shadow: 0 8px 24px #00000009; }
input, textarea, select, button { font: inherit; color: var(--c-text); }
input, select { min-width: 0; width: 100%; border: 1px solid var(--c-border); background: var(--c-bg-card); border-radius: 6px; padding: 7px 8px; box-sizing: border-box; }
.quick-title { border: 0; font-size: 18px; font-weight: 600; padding: 4px 0; }
textarea { width: 100%; box-sizing: border-box; resize: vertical; margin: 10px 0 16px; line-height: 1.8; border: 0; background: transparent; }
.quick-fields { display: grid; grid-template-columns: 1fr 1fr 1.4fr; gap: 12px; }
label { display: flex; flex-direction: column; gap: 6px; font-size: 11px; color: var(--c-text-secondary); }
label input, label select { font-size: 13px; }
footer { display: flex; align-items: center; gap: 8px; margin-top: 20px; }
footer span { flex: 1; }
button { border: 0; background: transparent; padding: 7px 10px; font-size: 12px; border-radius: 6px; cursor: pointer; }
button:hover { background: var(--c-bg-hover); }
button.primary { background: var(--c-primary); color: var(--c-text-on-primary, white); }
button:disabled { opacity: .5; cursor: wait; }
input:focus-visible, textarea:focus-visible, select:focus-visible, button:focus-visible { outline: 2px solid var(--c-primary); outline-offset: 3px; }
.schedule-fields { margin-top: 16px; display: grid; grid-template-columns: 1.5fr 1fr 1fr; gap: 10px; padding: 14px; background: var(--c-bg-hover); border-radius: 8px; }
.schedule-fields p { grid-column: 1 / -1; font-size: 12px; color: var(--c-text-secondary); margin: 0 0 6px; }
.schedule-fields button { grid-column: 1 / -1; justify-self: end; }
.error { color: var(--c-danger); font-size: 12px; }
@media (max-width: 640px) { .task-quick-editor { margin-left: 0; padding: 16px; } .quick-fields { grid-template-columns: 1fr; } footer { flex-wrap: wrap; } }
</style>
