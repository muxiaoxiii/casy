<script setup lang="ts">
import { tauriCallSafe } from '../core/tauriBridge'
/**
 * GlobalSearch —— ⌘K 全局搜索面板（A1-6 · 对标 Linear/Raycast 命令面板）
 *
 * 交互契约（成熟方案既有惯例）：
 * - ⌘/Ctrl+K 全局唤起，Esc 关闭，点击遮罩关闭
 * - 输入即搜（200ms 防抖），结果分栏：任务 / 案件 / 知识
 * - ↑↓ 在扁平序上循环移动，Enter 跳转对应视图
 * - 所有数据经 casyContext 服务（双路径铁律），无直连 tauriCallSafe
 */
import { ref, shallowRef, watch, nextTick, computed, onBeforeUnmount } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import { Search, Finished, Folder, Reading } from '../shared/icons'
import { casyContext } from '../core/plugin/context'

const props = defineProps<{ modelValue: boolean }>()
const emit = defineEmits<{ (e: 'update:modelValue', v: boolean): void }>()

const router = useRouter()
const visible = ref(props.modelValue)
watch(() => props.modelValue, v => (visible.value = v))
watch(visible, v => emit('update:modelValue', v))

const query = ref('')
const inputRef = ref<HTMLInputElement | null>(null)
const panelRef = ref<HTMLElement | null>(null)
let returnFocus: HTMLElement | null = null
function keepPanelFocus(event: KeyboardEvent) {
  if (event.key !== 'Tab') return
  const elements = [...(panelRef.value?.querySelectorAll<HTMLElement>('input, button, [tabindex="0"]') || [])]
    .filter(element => element.tabIndex >= 0 && element.getClientRects().length)
  const first = elements[0]
  const last = elements[elements.length - 1]
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault()
    last?.focus()
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault()
    first?.focus()
  }
}
const listRef = ref<HTMLDivElement | null>(null)
const loading = ref(false)
const semanticLoading = ref(false)
const semanticWarning = ref('')
const mode = ref(localStorage.getItem('casy_global_search_mode') || 'keyword')

interface ResultItem {
  key: string
  group: '任务' | '案件' | '知识' | '项目' | '案卷'
  icon: typeof Finished
  title: string
  meta: string
  route: string
}

const results = shallowRef<ResultItem[]>([])
const activeIndex = ref(0)

const flatResults = computed(() => results.value)

let debounceTimer: ReturnType<typeof setTimeout> | null = null
let semanticTimer: ReturnType<typeof setTimeout> | null = null
let sequence = 0
const groupOrder = ['项目', '任务', '案件', '案卷', '知识']

function publish(items: ResultItem[]) {
  const activeKey = flatResults.value[activeIndex.value]?.key
  results.value = items.sort((a,b) => groupOrder.indexOf(a.group) - groupOrder.indexOf(b.group))
  activeIndex.value = Math.max(0, results.value.findIndex(item => item.key === activeKey))
}

async function runSearch(q: string, request: number) {
  const text = q.trim()
  if (!text) {
    results.value = []
    return
  }
  loading.value = true
  const [tasksRes, casesRes, knRes, projRes, filesRes] = await Promise.all([
    casyContext.tasks.searchTasks(text),
    casyContext.cases.search(text),
    casyContext.knowledge.searchIndex(text, false),
    casyContext.projects.list(text),
    tauriCallSafe('global_search', {query:text}),
  ])
  if (request !== sequence || !visible.value) return
  loading.value = false

  const out: ResultItem[] = []

  if (tasksRes.ok && Array.isArray(tasksRes.data)) {
    for (const t of tasksRes.data as Array<Record<string, unknown>>) {
      out.push({
        key: 'task-' + String(t.id),
        group: '任务',
        icon: Finished,
        title: String(t.taskName ?? ''),
        meta: t.dueDate ? String(t.dueDate) : (t.completed ? '已完成' : '无日期'),
        route: '/tasks',
      })
    }
  }
  if (casesRes.ok && Array.isArray(casesRes.data)) {
    for (const c of casesRes.data as unknown as Array<Record<string, unknown>>) {
      out.push({
        key: 'case-' + String(c.id),
        group: '案件',
        icon: Folder,
        title: String(c.caseName ?? c.case_name ?? ''),
        meta: String(c.clientName ?? c.client_name ?? c.status ?? ''),
        route: '/cases/' + String(c.id),
      })
    }
  }
  if (knRes.ok && knRes.data) {
    for (const k of knRes.data.results) {
      out.push({
        key: 'kn-' + String(k.id),
        group: '知识',
        icon: Reading,
        title: String(k.title ?? ''),
        meta: k.content.replace(/\s+/g, ' ').slice(0,100),
        route: '/knowledge?select=' + encodeURIComponent(k.id),
      })
    }
  }

  if(filesRes.ok && filesRes.data) {
    for(const file of filesRes.data.filter(item=>item.itemType==='file'))out.push({key:'file-'+file.id,group:'案卷',icon:Folder,title:file.title,meta:(file.snippet || '').replace(/<[^>]+>/g,''),route:'/files/'+encodeURIComponent(file.caseId || '')+'?select='+encodeURIComponent(file.id)})
  }
  if (projRes.ok && Array.isArray(projRes.data)) {
    for (const p of projRes.data as Array<{ id: string; name: string; kind: string; description: string | null }>) {
      // legal 项目只是案件兼容镜像；案件结果已单独出现，避免搜索重复。
      if (p.kind !== 'personal') continue
      out.push({
        key: 'proj-' + p.id,
        group: '项目',
        icon: Folder,
        title: p.name,
        meta: '非案件项目',
        route: '/projects',
      })
    }
  }

  publish(out)
  if (!knRes.ok) semanticWarning.value = knRes.error || '知识检索失败'
  if (mode.value === 'hybrid') {
    semanticLoading.value = true
    semanticTimer = setTimeout(async () => {
      const response = await casyContext.knowledge.searchIndex(text, true)
      if (request !== sequence || !visible.value) return
      semanticLoading.value = false
      if (!response.ok || !response.data) {
        semanticWarning.value = response.error || '语义检索不可用，已保留关键词结果'
        return
      }
      semanticWarning.value = response.data.warning || ({not_configured:'语义检索尚未启用',not_indexed:'尚无当前模型的索引'}[response.data.semanticStatus] || '')
      const knowledge: ResultItem[] = response.data.results.map(k => ({
        key:'kn-'+k.id,group:'知识',icon:Reading,title:k.title,
        meta:(k.source === 'fts' ? '' : '语义 · ') + k.content.replace(/\s+/g,' ').slice(0,100),
        route:'/knowledge?select='+encodeURIComponent(k.id),
      }))
      publish([...out.filter(item => item.group !== '知识'), ...knowledge])
    }, 400)
  }
}

function scheduleSearch() {
  const request = ++sequence
  if (debounceTimer) clearTimeout(debounceTimer)
  if (semanticTimer) clearTimeout(semanticTimer)
  loading.value = false
  semanticLoading.value = false
  semanticWarning.value = ''
  results.value = []
  if (query.value.trim()) {
    loading.value = true
    debounceTimer = setTimeout(() => void runSearch(query.value, request), 200)
  }
}
watch(query, scheduleSearch)
watch(mode, value => { localStorage.setItem('casy_global_search_mode', value); scheduleSearch() })

watch(visible, v => {
  ++sequence
  if (debounceTimer) clearTimeout(debounceTimer)
  if (semanticTimer) clearTimeout(semanticTimer)
  loading.value = false
  semanticLoading.value = false
  semanticWarning.value = ''
  if (v) {
    returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null
    query.value = ''
    results.value = []
    activeIndex.value = 0
    void nextTick(() => inputRef.value?.focus())
  } else {
    void nextTick(() => returnFocus?.focus())
  }
})
onBeforeUnmount(() => { ++sequence; if (debounceTimer) clearTimeout(debounceTimer); if (semanticTimer) clearTimeout(semanticTimer) })

function move(delta: number) {
  const n = flatResults.value.length
  if (!n) return
  activeIndex.value = (activeIndex.value + delta + n) % n
  void nextTick(() => {
    const el = listRef.value?.querySelector('[data-active="true"]')
    el?.scrollIntoView({ block: 'nearest' })
  })
}

function choose(item?: ResultItem) {
  const target = item ?? flatResults.value[activeIndex.value]
  if (!target) return
  if (!target.route) {
    ElMessage.info('「项目」独立视图在 A1-1 阶段二提供，当前可在任务页按案件/领域筛选')
    return
  }
  visible.value = false
  void router.push(target.route).catch(() => {
    ElMessage.warning('跳转失败：' + target.route)
  })
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'ArrowDown') {
    e.preventDefault()
    move(1)
  } else if (e.key === 'ArrowUp') {
    e.preventDefault()
    move(-1)
  } else if (e.key === 'Enter') {
    e.preventDefault()
    choose()
  } else if (e.key === 'Escape') {
    visible.value = false
  }
}
</script>

<template>
  <Teleport to="body">
    <Transition name="cmdk-fade">
      <div v-if="visible" class="cmdk-overlay" @mousedown.self="visible = false">
        <div ref="panelRef" class="cmdk-panel" @keydown="keepPanelFocus" role="dialog" aria-modal="true" aria-label="全局搜索" @keydown.esc="visible = false">
          <div class="cmdk-input-row">
            <el-icon class="cmdk-search-icon"><Search /></el-icon>
            <input
              ref="inputRef"
              v-model="query"
              class="cmdk-input"
              placeholder="搜索任务、案件、知识…"
              aria-label="全局检索问题"
              maxlength="500"
              spellcheck="false"
              @keydown="onKeydown"
            />
            <span class="cmdk-esc">Esc</span>
          </div>

          <div class="cmdk-modes">
            <el-radio-group v-model="mode" size="small" aria-label="全局检索方式">
              <el-radio-button value="keyword">关键词</el-radio-button>
              <el-radio-button value="hybrid">混合检索</el-radio-button>
            </el-radio-group>
            <span v-if="semanticLoading" role="status">语义检索中</span>
          </div>
          <div v-if="semanticWarning" class="cmdk-warning" role="status">{{ semanticWarning }}</div>

          <div ref="listRef" class="cmdk-list">
            <template v-if="flatResults.length">
              <template
                v-for="(group, gi) in ['项目', '任务', '案件', '案卷', '知识']"
                :key="group"
              >
                <div
                  v-if="flatResults.some(r => r.group === group)"
                  class="cmdk-group-label"
                >
                  {{ ['项目', '任务', '案件', '案卷', '知识'][gi] }}
                </div>
                <template v-for="r in flatResults.filter(x => x.group === group)" :key="r.key">
                  <button
                    type="button"
                    class="cmdk-item"
                    :data-active="flatResults[activeIndex]?.key === r.key"
                    @mouseenter="activeIndex = flatResults.findIndex(x => x.key === r.key)"
                    @click="choose(r)"
                  >
                    <el-icon class="cmdk-item-icon"><component :is="r.icon" /></el-icon>
                    <span class="cmdk-item-body"><span class="cmdk-item-title">{{ r.title }}</span><span class="cmdk-item-meta">{{ r.meta }}</span></span>
                    <span v-if="flatResults[activeIndex]?.key === r.key" class="cmdk-enter-hint">↵</span>
                  </button>
                </template>
              </template>
            </template>
            <div v-else-if="!loading" class="cmdk-empty">
              {{ query.trim() ? '没有匹配结果' : '输入以搜索任务、案件、知识…' }}
            </div>
            <div v-if="loading && !flatResults.length" class="cmdk-empty">搜索中…</div>
          </div>

          <div class="cmdk-footer">
            <span><kbd>↑</kbd><kbd>↓</kbd> 选择</span>
            <span><kbd>↵</kbd> 打开</span>
            <span class="cmdk-footer-brand">Casy</span>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.cmdk-overlay {
  position: fixed;
  inset: 0;
  background: rgba(15, 15, 20, 0.42);
  backdrop-filter: blur(2px);
  z-index: 3000;
  display: flex;
  justify-content: center;
  align-items: flex-start;
  padding-top: 14vh;
}

.cmdk-panel {
  width: min(560px, calc(100vw - 48px));
  background: var(--c-bg-elevated);
  border: 1px solid var(--c-border);
  border-radius: 12px;
  box-shadow:
    0 24px 64px rgba(0, 0, 0, 0.24),
    0 2px 8px rgba(0, 0, 0, 0.08);
  overflow: hidden;
  display: flex;
  flex-direction: column;
  max-height: 68vh;
}

.cmdk-input-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 14px 16px;
  border-bottom: 1px solid var(--c-border-light);
}
.cmdk-modes { display:flex; gap:12px; align-items:center; padding:8px 16px; border-bottom:1px solid var(--c-border-light); font-size:12px; color:var(--c-text-secondary); }
.cmdk-warning { padding:8px 16px; font-size:12px; color:var(--c-text-secondary); overflow-wrap:anywhere; }
.cmdk-search-icon {
  color: var(--c-text-secondary);
  font-size: 16px;
}
.cmdk-input {
  min-width: 0;
  flex: 1;
  border: none;
  outline: none;
  font-size: 15px;
  color: var(--c-text);
  background: transparent;
}
.cmdk-input::placeholder { color: var(--c-text-placeholder); }
.cmdk-esc {
  font-size: 10px;
  color: var(--c-text-secondary);
  border: 1px solid var(--c-border);
  border-radius: 4px;
  padding: 1px 5px;
}

.cmdk-list {
  overflow-y: auto;
  padding: 6px;
  flex: 1;
}
.cmdk-group-label {
  font-size: 11px;
  color: var(--c-text-secondary);
  padding: 8px 10px 4px;
  font-weight: 600;
}
.cmdk-item {
  width: 100%;
  border: 0;
  text-align: left;
  background: transparent;
  font: inherit;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border-radius: 6px;
  cursor: pointer;
}
.cmdk-item[data-active='true'] {
  background: var(--c-primary-light);
}
.cmdk-item-icon { color: var(--gray-500); flex-shrink: 0; }
.cmdk-item-body { flex:1; min-width:0; display:flex; flex-direction:column; gap:4px; }
.cmdk-item-title {
  flex: 1;
  min-width: 0;
  font-size: 13.5px;
  color: var(--c-text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.cmdk-item-meta {
  font-size: 11px;
  color: var(--c-text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.cmdk-enter-hint { color: var(--c-text-secondary); font-size: 13px; }

.cmdk-empty {
  padding: 28px 12px;
  text-align: center;
  font-size: 13px;
  color: var(--c-text-secondary);
}

.cmdk-footer {
  display: flex;
  gap: 14px;
  align-items: center;
  padding: 8px 14px;
  border-top: 1px solid var(--c-border-light);
  font-size: 11px;
  color: var(--c-text-secondary);
}
.cmdk-footer kbd {
  font-family: var(--font-mono);
  font-size: 10px;
  border: 1px solid var(--c-border);
  border-bottom-width: 2px;
  border-radius: 3px;
  padding: 0 4px;
  margin-right: 2px;
  background: var(--gray-50);
}
.cmdk-footer-brand {
  margin-left: auto;
  font-weight: 600;
  color: var(--c-text-placeholder);
}

/* 出入场（M-UI-0 Motion Tokens） */
.cmdk-fade-enter-active,
.cmdk-fade-leave-active {
  transition: opacity var(--motion-fast) var(--ease-out);
}
.cmdk-fade-enter-active .cmdk-panel {
  transition:
    transform var(--motion-base) var(--ease-spring),
    opacity var(--motion-fast) ease-out;
}
.cmdk-fade-enter-from,
.cmdk-fade-leave-to {
  opacity: 0;
}
.cmdk-fade-enter-from .cmdk-panel {
  transform: translateY(-8px) scale(0.98);
  opacity: 0;
}

@media (prefers-reduced-motion: reduce) {
  .cmdk-fade-enter-active .cmdk-panel { transition: none; }
}
</style>
