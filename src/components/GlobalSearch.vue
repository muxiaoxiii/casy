<script setup lang="ts">
/**
 * GlobalSearch —— ⌘K 全局搜索面板（A1-6 · 对标 Linear/Raycast 命令面板）
 *
 * 交互契约（成熟方案既有惯例）：
 * - ⌘/Ctrl+K 全局唤起，Esc 关闭，点击遮罩关闭
 * - 输入即搜（200ms 防抖），结果分栏：任务 / 案件 / 知识
 * - ↑↓ 在扁平序上循环移动，Enter 跳转对应视图
 * - 所有数据经 casyContext 服务（双路径铁律），无直连 tauriCallSafe
 */
import { ref, watch, nextTick, computed } from 'vue'
import { useRouter } from 'vue-router'
import { ElMessage } from 'element-plus'
import { Search, Finished, Folder, Reading, Briefcase } from '@element-plus/icons-vue'
import { casyContext } from '../core/plugin/context'

const props = defineProps<{ modelValue: boolean }>()
const emit = defineEmits<{ (e: 'update:modelValue', v: boolean): void }>()

const router = useRouter()
const visible = ref(props.modelValue)
watch(() => props.modelValue, v => (visible.value = v))
watch(visible, v => emit('update:modelValue', v))

const query = ref('')
const inputRef = ref<HTMLInputElement | null>(null)
const listRef = ref<HTMLDivElement | null>(null)
const loading = ref(false)

interface ResultItem {
  key: string
  group: '任务' | '案件' | '知识' | '项目'
  icon: typeof Finished
  title: string
  meta: string
  route: string
}

const results = ref<ResultItem[]>([])
const activeIndex = ref(0)

const flatResults = computed(() => results.value)

let debounceTimer: ReturnType<typeof setTimeout> | null = null

async function runSearch(q: string) {
  const text = q.trim()
  if (!text) {
    results.value = []
    return
  }
  loading.value = true
  const [tasksRes, casesRes, knRes, projRes] = await Promise.all([
    casyContext.tasks.searchTasks(text),
    casyContext.cases.search(text),
    casyContext.knowledge.search(text),
    casyContext.projects.list(text),
  ])
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
  if (knRes.ok && Array.isArray(knRes.data)) {
    for (const k of knRes.data as Array<Record<string, unknown>>) {
      out.push({
        key: 'kn-' + String(k.id),
        group: '知识',
        icon: Reading,
        title: String(k.title ?? ''),
        meta: String(k.category ?? k.updatedAt ?? ''),
        route: '/knowledge',
      })
    }
  }

  if (projRes.ok && Array.isArray(projRes.data)) {
    for (const p of projRes.data as Array<{ id: string; name: string; kind: string; description: string | null }>) {
      out.push({
        key: 'proj-' + p.id,
        group: '项目',
        icon: Folder,
        title: p.name,
        meta: p.kind === 'legal' ? '法律项目' : '个人项目',
        route: '/projects',
      })
    }
  }

  results.value = out
  activeIndex.value = 0
}

watch(query, q => {
  if (debounceTimer) clearTimeout(debounceTimer)
  debounceTimer = setTimeout(() => void runSearch(q), 200)
})

watch(visible, v => {
  if (v) {
    query.value = ''
    results.value = []
    activeIndex.value = 0
    void nextTick(() => inputRef.value?.focus())
  }
})

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
        <div class="cmdk-panel" role="dialog" aria-label="全局搜索">
          <div class="cmdk-input-row">
            <el-icon class="cmdk-search-icon"><Search /></el-icon>
            <input
              ref="inputRef"
              v-model="query"
              class="cmdk-input"
              placeholder="搜索任务、案件、知识…"
              spellcheck="false"
              @keydown="onKeydown"
            />
            <span class="cmdk-esc">Esc</span>
          </div>

          <div ref="listRef" class="cmdk-list">
            <template v-if="flatResults.length">
              <template
                v-for="(group, gi) in ['项目', '任务', '案件', '知识']"
                :key="group"
              >
                <div
                  v-if="flatResults.some(r => r.group === group)"
                  class="cmdk-group-label"
                >
                  {{ ['项目', '任务', '案件', '知识'][gi] }}
                </div>
                <template v-for="r in flatResults.filter(x => x.group === group)" :key="r.key">
                  <div
                    class="cmdk-item"
                    :data-active="flatResults[activeIndex]?.key === r.key"
                    @mouseenter="activeIndex = flatResults.findIndex(x => x.key === r.key)"
                    @click="choose(r)"
                  >
                    <el-icon class="cmdk-item-icon"><component :is="r.icon" /></el-icon>
                    <span class="cmdk-item-title">{{ r.title }}</span>
                    <span class="cmdk-item-meta">{{ r.meta }}</span>
                    <span v-if="flatResults[activeIndex]?.key === r.key" class="cmdk-enter-hint">↵</span>
                  </div>
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
  background: #ffffff;
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
  border-bottom: 1px solid #EEF0F3;
}
.cmdk-search-icon {
  color: #9BA2AF;
  font-size: 16px;
}
.cmdk-input {
  flex: 1;
  border: none;
  outline: none;
  font-size: 15px;
  color: #18181B;
  background: transparent;
}
.cmdk-input::placeholder { color: #C8CCD4; }
.cmdk-esc {
  font-size: 10px;
  color: #9BA2AF;
  border: 1px solid #E4E7ED;
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
  color: #9BA2AF;
  padding: 8px 10px 4px;
  font-weight: 600;
}
.cmdk-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border-radius: 6px;
  cursor: pointer;
}
.cmdk-item[data-active='true'] {
  background: #F0F4FA;
}
.cmdk-item-icon { color: #6B7280; flex-shrink: 0; }
.cmdk-item-title {
  flex: 1;
  min-width: 0;
  font-size: 13.5px;
  color: #27272A;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.cmdk-item-meta {
  font-size: 11px;
  color: #9BA2AF;
  flex-shrink: 0;
}
.cmdk-enter-hint { color: #9BA2AF; font-size: 13px; }

.cmdk-empty {
  padding: 28px 12px;
  text-align: center;
  font-size: 13px;
  color: #A1A1AA;
}

.cmdk-footer {
  display: flex;
  gap: 14px;
  align-items: center;
  padding: 8px 14px;
  border-top: 1px solid #EEF0F3;
  font-size: 11px;
  color: #9BA2AF;
}
.cmdk-footer kbd {
  font-family: var(--font-mono);
  font-size: 10px;
  border: 1px solid #E4E7ED;
  border-bottom-width: 2px;
  border-radius: 3px;
  padding: 0 4px;
  margin-right: 2px;
  background: #FAFAFB;
}
.cmdk-footer-brand {
  margin-left: auto;
  font-weight: 600;
  color: #C8CCD4;
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
