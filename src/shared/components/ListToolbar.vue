<script setup lang="ts">
/**
 * ListToolbar —— 共享列表工具栏（X1）
 *
 * 对标律智荟 OA 的"列表三件套"：快捷搜索 + 可收起高级搜索 + 自定义显示字段。
 * 任务、卷宗、案件各自长出了一套相似但互不一致的工具栏；本组件收敛共同部分，
 * 模块特有的部分走 `#actions` 与 `#advanced` 插槽。
 *
 * 约定：
 * - 搜索输入 200ms 防抖后经 `update:query` 出去，避免每敲一个字符就打后端；
 * - 排序变化即时生效（`update:sort`）；
 * - 高级筛选默认收起，展开状态记忆在 `advancedKey` 对应的 localStorage；
 * - 列显隐由调用方传 `columns`（`{key,label,visible}`）并监听 `update:columns`，
 *   组件本身不持有列状态——各模块的列集合不同。
 */
import { ref, computed, watch } from 'vue'
import { Search, Rank, Filter, Grid, Close } from '../icons'

const props = withDefaults(defineProps<{
  /** 搜索关键词（v-model） */
  query?: string
  /** 当前排序 key（v-model） */
  sort?: string
  /** 排序选项：[{ key, label }] */
  sortOptions?: Array<{ key: string; label: string }>
  /** 搜索占位文案 */
  searchPlaceholder?: string
  /** 列定义（传了才显示"列"按钮） */
  columns?: Array<{ key: string; label: string; visible: boolean }>
  /** 高级筛选 localStorage 记忆键（不传则不记忆） */
  advancedKey?: string
  /** 结果总数提示（如 "128 项"） */
  totalLabel?: string
}>(), {
  query: '',
  sort: '',
  sortOptions: () => [],
  searchPlaceholder: '搜索…',
  columns: () => [],
  advancedKey: '',
  totalLabel: '',
})

const emit = defineEmits<{
  (e: 'update:query', value: string): void
  (e: 'update:sort', value: string): void
  (e: 'update:columns', value: Array<{ key: string; label: string; visible: boolean }>): void
  (e: 'search', value: string): void
  (e: 'clear'): void
}>()

const localQuery = ref(props.query)
const advancedOpen = ref(false)
watch(() => props.query, value => { if (value !== localQuery.value) localQuery.value = value })

let debounceTimer: ReturnType<typeof setTimeout> | undefined
function onQueryInput(value: string) {
  localQuery.value = value
  if (debounceTimer) clearTimeout(debounceTimer)
  debounceTimer = setTimeout(() => {
    emit('update:query', value)
    emit('search', value)
  }, 200)
}

if (props.advancedKey) {
  try {
    advancedOpen.value = localStorage.getItem(`casy_list_advanced_${props.advancedKey}`) === 'open'
  } catch { /* 隐私模式：忽略 */ }
}
function toggleAdvanced() {
  advancedOpen.value = !advancedOpen.value
  if (!props.advancedKey) return
  try { localStorage.setItem(`casy_list_advanced_${props.advancedKey}`, advancedOpen.value ? 'open' : 'closed') } catch { /* 忽略 */ }
}

const visibleCount = computed(() => props.columns.filter((column) => column.visible).length)
function toggleColumn(key: string) {
  emit('update:columns', props.columns.map((column) => column.key === key ? { ...column, visible: !column.visible } : column))
}
function resetColumns() {
  emit('update:columns', props.columns.map((column) => ({ ...column, visible: true })))
}
</script>

<template>
  <div class="list-toolbar ui-row ui-row--wrap">
    <div class="toolbar-search">
      <el-icon :size="14"><Search /></el-icon>
      <input
        :value="localQuery"
        type="text"
        class="search-input"
        :placeholder="searchPlaceholder"
        aria-label="搜索列表"
        @input="onQueryInput(($event.target as HTMLInputElement).value)"
      />
      <el-icon v-if="localQuery" :size="12" class="clear-icon" @click="localQuery = ''; onQueryInput('')"><Close /></el-icon>
    </div>

    <el-select
      v-if="sortOptions.length"
      :model-value="sort"
      size="small"
      class="toolbar-sort"
      aria-label="排序方式"
      @update:model-value="emit('update:sort', String($event))"
    >
      <template #prefix><el-icon :size="13"><Rank /></el-icon></template>
      <el-option v-for="option in sortOptions" :key="option.key" :value="option.key" :label="option.label" />
    </el-select>

    <el-button v-if="$slots.advanced" size="small" :type="advancedOpen ? 'primary' : 'default'" @click="toggleAdvanced">
      <el-icon :size="13"><Filter /></el-icon>
      筛选<template v-if="advancedOpen"> ▲</template><template v-else> ▼</template>
    </el-button>

    <el-dropdown v-if="columns.length" trigger="click" placement="bottom-end">
      <el-button size="small">
        <el-icon :size="13"><Grid /></el-icon>
        列 {{ visibleCount }}/{{ columns.length }}
      </el-button>
      <template #dropdown>
        <el-dropdown-menu>
          <el-dropdown-item v-for="column in columns" :key="column.key" @click.prevent="toggleColumn(column.key)">
            <span :class="{ 'column-off': !column.visible }">{{ column.visible ? '✓' : '○' }} {{ column.label }}</span>
          </el-dropdown-item>
          <el-dropdown-item divided @click.prevent="resetColumns">全部显示</el-dropdown-item>
        </el-dropdown-menu>
      </template>
    </el-dropdown>

    <span v-if="totalLabel" class="toolbar-total">{{ totalLabel }}</span>

    <div class="toolbar-actions">
      <slot name="actions" />
    </div>

    <div v-if="$slots.advanced && advancedOpen" class="toolbar-advanced">
      <slot name="advanced" />
    </div>
  </div>
</template>

<style scoped>
.list-toolbar { gap: 8px; align-items: center; }
.toolbar-search { display: inline-flex; align-items: center; gap: 6px; height: 32px; padding: 0 10px; border: 1px solid var(--c-border); border-radius: 6px; background: var(--c-bg-card); color: var(--c-text-secondary); min-width: 200px; }
.toolbar-search:focus-within { border-color: var(--c-primary); }
.search-input { flex: 1; min-width: 0; border: 0; outline: 0; background: transparent; color: var(--c-text); font-size: 12px; }
.clear-icon { cursor: pointer; }
.clear-icon:hover { color: var(--c-text); }
.toolbar-sort { width: 150px; }
.toolbar-total { font-size: 12px; color: var(--c-text-secondary); }
.toolbar-actions { display: inline-flex; align-items: center; gap: 6px; margin-left: auto; }
.toolbar-advanced { flex-basis: 100%; display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding: 10px; border: 1px solid var(--c-border-light); border-radius: 6px; background: var(--c-bg-subtle); }
.column-off { color: var(--c-text-secondary); }
</style>
