<script setup lang="ts">
import { computed, ref, watch, onBeforeUnmount } from 'vue'
import { casyContext } from '../../../core/plugin/context'
import { parseCalendarCapture, overlaps } from '../parseCalendarCapture'
import type { CalendarEvent } from '../../../types'

const props = defineProps<{ date: string; cases: Array<{ id: string; caseName?: string; caseNo?: string }> }>()
const emit = defineEmits<{ created: [date: string] }>()
const text = ref('')
const caseId = ref('')
const busy = ref(false)
const error = ref('')
const checking = ref(false)
const availabilityError = ref('')
const events = ref<CalendarEvent[]>([])
const parsed = computed(() => parseCalendarCapture(text.value, props.date))
let revision = 0
watch(() => text.value ? parsed.value.event.eventDate : '', async date => {
  const request = ++revision
  events.value = []; availabilityError.value = ''
  if (!date) return
  checking.value = true
  const [year, month] = date.split('-').map(Number)
  try {
    const result = await casyContext.calendar.events(year, month)
    if (request !== revision) return
    if (result.ok) events.value = (result.data || []).filter(e => e.date === date)
    else availabilityError.value = '无法核对已有日程，请在保存前核实排期'
  } finally { if (request === revision) checking.value = false }
})
onBeforeUnmount(() => { revision++ })
const conflicts = computed(() => {
  const event = parsed.value.event
  return event.startTime && event.endTime ? events.value.filter(e => !e.allDay && e.type !== 'task' && overlaps(event.startTime!, event.endTime!, e.startTime, e.endTime)) : []
})
async function submit(e?: KeyboardEvent) {
  if (e?.isComposing || busy.value || !text.value.trim() || parsed.value.error) return
  busy.value = true; error.value = ''
  try {
    const event = { ...parsed.value.event, caseId: caseId.value || null }
    const result = await casyContext.calendar.createEvent(event)
    if (!result.ok) { error.value = result.error || '创建失败'; return }
    text.value = ''; caseId.value = ''; emit('created', event.eventDate)
  } finally { busy.value = false }
}
</script>

<template>
  <div class="calendar-composer" @keydown.esc="!busy && (text = '')">
    <div class="composer-input ui-row"><span aria-hidden="true">＋</span><input v-model="text" :disabled="busy" aria-label="快捷创建日程" placeholder="明天下午3点 讨论证据 1小时 @会议室" @keydown.enter.prevent="submit" /><kbd>↵</kbd></div>
    <section v-if="text.trim()" class="composer-preview" aria-label="日程预览">
      <header class="ui-row ui-row--between row-gap-12"><span>即将安排</span><button type="button" :disabled="busy" @click="text = ''" aria-label="关闭日程预览">×</button></header>
      <h3>{{ parsed.event.title }}</h3>
      <div class="preview-time ui-row ui-row--between row-gap-12"><strong>{{ parsed.event.eventDate }}</strong><span>{{ parsed.event.startTime ? `${parsed.event.startTime} – ${parsed.event.endTime || '?'}` : '全天' }}</span></div>
      <p v-if="parsed.event.location">地点 · {{ parsed.event.location }}</p>
      <label class="ui-row row-gap-12">关联案件<select v-model="caseId" :disabled="busy"><option value="">独立日程</option><option v-for="c in cases" :key="c.id" :value="c.id">{{ c.caseName || c.caseNo }}</option></select></label>
      <p v-if="parsed.error || error" class="error" role="alert">{{ parsed.error || error }}</p>
      <p v-else-if="conflicts.length" class="conflict" role="status">与 {{ conflicts.map(e => e.title).join('、') }} 的时间重叠</p>
      <p v-else-if="checking || availabilityError" role="status">{{ checking ? '正在核对已有日程…' : availabilityError }}</p>
      <p v-else class="hint">{{ parsed.event.startTime ? '未发现已加载日程的时间冲突' : '在日期后输入时刻，例如 14:00–15:00' }}</p>
      <footer class="ui-row ui-row--between row-gap-12"><small>Enter 保存 · Esc 取消</small><button type="button" class="save-event" :disabled="busy || !!parsed.error" @click="submit()">{{ busy ? '保存中…' : conflicts.length ? '仍然安排' : '添加日程' }}</button></footer>
    </section>
  </div>
</template>

<style scoped>
.calendar-composer { position: relative; width: 340px; max-width: 100%; z-index: 20; }
.composer-input { padding: 9px 12px; border: 1px solid var(--c-border); border-radius: 8px; background: var(--c-bg-card); }
.composer-input:focus-within { border-color: var(--c-primary); box-shadow: 0 0 0 3px var(--c-primary-light); }
.composer-input > span { color: var(--c-primary); font-size: 19px; }
input { flex: 1; min-width: 0; border: 0; background: transparent; outline: 0; font: inherit; color: var(--c-text); font-size: 12px; }
kbd { color: var(--c-text-secondary); }
.composer-preview { position: absolute; top: calc(100% + 10px); right: 0; width: 380px; max-width: calc(100vw - 48px); padding: 22px; box-sizing: border-box; border: 1px solid var(--c-border-strong); background: var(--c-bg-card); color: var(--c-text); border-radius: 12px; box-shadow: 0 16px 56px #0002; }
.row-gap-12 { gap: 12px; }
header { font-size: 11px; color: var(--c-text-secondary); }
h3 { font-size: 19px; line-height: 1.5; margin: 10px 0 16px; }
.preview-time { background: var(--c-bg); padding: 12px; border-radius: 6px; font-size: 13px; }
p { font-size: 12px; line-height: 1.6; color: var(--c-text-secondary); }
label { margin: 16px 0; font-size: 12px; color: var(--c-text-secondary); }
select { flex: 1; min-width: 0; padding: 7px; border: 1px solid var(--c-border); border-radius: 6px; background: var(--c-bg-card); color: var(--c-text); }
button { border: 0; border-radius: 6px; cursor: pointer; color: inherit; background: transparent; font: inherit; }
header button { font-size: 22px; }
.save-event { background: var(--c-primary); color: var(--c-primary-contrast, white); padding: 9px 14px; font-size: 12px; }
button:disabled { opacity: .5; }
small { color: var(--c-text-secondary); font-size: 11px; }
.error { color: var(--c-danger); }.conflict { color: var(--c-warning); }
button:focus-visible, select:focus-visible { outline: 2px solid var(--c-primary); outline-offset: 2px; }

</style>
