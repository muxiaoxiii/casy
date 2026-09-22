<script setup lang="ts">
import { ref, computed, watch, onBeforeUnmount } from 'vue'
import { useRouter } from 'vue-router'
import { casyContext } from '../../core/plugin/context'
import { observeChanges } from '../../core/observeChanges'
import type { WorkspaceService } from '../../core/services/workspace'

const props = defineProps<{ caseId: string }>()
const router = useRouter()
const data = ref<Awaited<ReturnType<WorkspaceService['caseContext']>>['data']>()
const error = ref('')
const loading = ref(false)
const active = ref('knowledge')
let revision = 0
function localDate(d: Date) { return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}` }
async function load() {
  const request = ++revision
  if (!props.caseId) { data.value = undefined; return }
  loading.value = true
  const start = new Date(), end = new Date()
  end.setDate(end.getDate() + 30)
  try {
    const result = await casyContext.workspace.caseContext(props.caseId, localDate(start), localDate(end))
    if (request !== revision) return
    data.value = result.ok ? result.data : undefined
    error.value = result.error || ''
  } catch (e) { if (request === revision) error.value = String(e) }
  finally { if (request === revision) loading.value = false }
}
watch(() => props.caseId, () => { data.value = undefined; void load() }, { immediate: true })
const stop = observeChanges(casyContext, ['task', 'case', 'calendar', 'knowledge', 'doc', 'file'], load)
onBeforeUnmount(() => { revision++; stop() })
const groups = computed(() => [
  { key: 'knowledge', label: '笔记', items: data.value?.knowledge.map(n => ({ id: n.id, title: n.title, detail: n.category, to: { path: '/knowledge', query: { select: n.id } } })) || [] },
  { key: 'docs', label: '文书', items: data.value?.docs.map(d => ({ id: d.id, title: d.title, detail: `v${d.version}`, to: { path: '/docs', query: { select: d.id } } })) || [] },
  { key: 'tasks', label: '任务', items: data.value?.tasks.filter(t => !t.completed).map(t => ({ id: t.id, title: t.taskName, detail: t.dueDate || '未设期限', to: { path: '/tasks', query: { edit: t.id } } })) || [] },
  { key: 'calendar', label: '日程', items: data.value?.calendar.map(e => ({ id: e.id, title: e.title, detail: `${e.eventDate} ${e.startTime || '全天'}`, to: { path: '/calendar', query: { date: e.eventDate, view: 'day' } } })) || [] },
])
const current = computed(() => groups.value.find(g => g.key === active.value)!)
</script>

<template>
  <section class="related-work" aria-label="关联工作">
    <header><span>关联工作</span><button type="button" @click="router.push(`/cases/${caseId}`)">打开案件 ↗</button></header>
    <nav aria-label="关联资料分类"><button v-for="g in groups" :key="g.key" type="button" :aria-pressed="active === g.key" @click="active = g.key">{{ g.label }} <small>{{ g.items.length }}</small></button></nav>
    <p v-if="error" role="alert">{{ error }} <button type="button" @click="load">重试</button></p>
    <p v-else-if="loading && !data" role="status">正在读取关联资料…</p>
    <template v-else>
      <p v-if="data?.errors.length" role="status">部分来源未能读取：{{ data.errors.map(e => e.source).join('、') }} <button type="button" @click="load">重试</button></p>
      <p v-if="!current.items.length">{{ active === 'calendar' ? '未来 30 天暂无关联日程' : '暂无关联' + current.label }}</p>
      <div v-else class="related-items"><button v-for="item in current.items" :key="item.id" type="button" @click="router.push(item.to)"><span>{{ item.title }}</span><small>{{ item.detail }}</small><span aria-hidden="true">↗</span></button></div>
    </template>
  </section>
</template>

<style scoped>
.related-work { border-top: 1px solid var(--c-border); padding-top: 18px; margin-top: 20px; color: var(--c-text); font-size: 13px; }
header { display: flex; align-items: center; justify-content: space-between; font-weight: 600; margin-bottom: 12px; }
button { font: inherit; color: inherit; cursor: pointer; border: 0; background: transparent; border-radius: 5px; }
button:focus-visible { outline: 2px solid var(--c-primary); outline-offset: 2px; }
header button { color: var(--c-primary); font-size: 12px; }
nav { display: flex; gap: 4px; }
nav button { padding: 6px 10px; }
nav button[aria-pressed=true] { background: var(--c-primary-light); color: var(--c-primary); }
small, p { color: var(--c-text-secondary); font-size: 12px; }
.related-items { max-height: 220px; overflow: auto; margin-top: 8px; }
.related-items button { width: 100%; display: flex; align-items: center; gap: 10px; text-align: left; padding: 10px 6px; }
.related-items button:hover { background: var(--c-bg-hover); }
.related-items button > span:first-child { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
