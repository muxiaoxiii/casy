<template>
  <div class="knowledge-sidebar">
    <div class="ks-header">
      <div class="ks-title">
        <el-icon><Collection /></el-icon>
        <span>知识 & 案卷</span>
      </div>
      <el-button link @click="$emit('close')">
        <el-icon><Close /></el-icon>
      </el-button>
    </div>

    <div class="ks-search-box">
      <el-input
        v-model="searchQuery"
        placeholder="搜索全文..."
        clearable
        @input="handleSearch"
      >
        <template #prefix>
          <el-icon><Search /></el-icon>
        </template>
      </el-input>
    </div>

    <div class="ks-results" v-loading="loading">
      <div
        v-for="item in safeResults"
        :key="item.id"
        class="ks-result-item"
        draggable="true"
        @dragstart="onDragStart($event, item)"
        @click="previewItem(item)"
      >
        <div class="ks-item-header ui-row ui-row--between">
          <el-tag size="small" :type="item.item_type === 'file' ? 'success' : 'primary'">
            {{ item.item_type === 'file' ? '案卷' : '知识' }}
          </el-tag>
          <span class="ks-item-category">{{ item.category }}</span>
        </div>
        <div class="ks-item-title">{{ item.title }}</div>
        <div class="ks-item-snippet" v-html="item.safeSnippet"></div>
      </div>
      
      <EmptyState v-if="!loading && results.length === 0" type="custom" compact hide-action title="输入关键词开始检索" />
    </div>
  </div>
</template>

<script setup>
import { ref, computed } from 'vue'
import { Collection, Close, Search } from '../../../shared/icons'
import EmptyState from '../../../shared/components/EmptyState.vue'
import { casyContext } from '../../../core/plugin/context'
// 审查 P0-3：SQLite FTS5 的 snippet() 只插入 <b> 高亮、不转义原文，渲染前必须消毒
import { sanitizeInlineHtml } from '../../../shared/markdown/mdBridge'

const emit = defineEmits(['close', 'insert'])

const searchQuery = ref('')
const results = ref([])
const loading = ref(false)
let debounceTimer = null

// 审查 P2-1：搜索结果 snippet 来自 SQLite FTS5 的 snippet()（只插 <b>、不转义原文），
// 渲染前必须消毒（sanitizeInlineHtml）。原写法在模板 v-for 内每次渲染都调用一次，
// 属 O(结果项 × 渲染次数)。此处预先映射为 { ...item, safeSnippet }，仅在 results 变化时计算一次。
const safeResults = computed(() =>
  (results.value || []).map(item => ({
    ...item,
    safeSnippet: sanitizeInlineHtml(item.snippet ?? ''),
  }))
)

function handleSearch() {
  if (debounceTimer) clearTimeout(debounceTimer)
  debounceTimer = setTimeout(async () => {
    if (!searchQuery.value.trim()) {
      results.value = []
      return
    }
    loading.value = true
    const res = await casyContext.knowledge.globalSearch(searchQuery.value)
    loading.value = false
    if (res.ok) {
      results.value = res.data || []
    }
  }, 300)
}

function onDragStart(event, item) {
  // 设置拖拽数据，包含特殊标记以便编辑器识别
  event.dataTransfer.setData('application/x-casy-reference', JSON.stringify({
    type: item.item_type,
    id: item.id,
    title: item.title,
    category: item.category
  }))
  event.dataTransfer.effectAllowed = 'copy'
}

function previewItem(item) {
  // 这里可以触发事件在外部展开全文预览
  emit('preview', item)
}
</script>

<style scoped>
.knowledge-sidebar {
  width: 320px;
  height: 100%;
  background: var(--el-bg-color-overlay);
  border-left: 1px solid var(--el-border-color-light);
  display: flex;
  flex-direction: column;
  box-shadow: -2px 0 8px rgba(0, 0, 0, 0.05);
}
.ks-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
  border-bottom: 1px solid var(--el-border-color-light);
}
.ks-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-weight: 600;
  color: var(--el-text-color-primary);
}
.ks-search-box {
  padding: 12px;
  border-bottom: 1px solid var(--el-border-color-light);
}
.ks-results {
  flex: 1;
  overflow-y: auto;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.ks-result-item {
  background: var(--el-bg-color);
  border: 1px solid var(--el-border-color);
  border-radius: 6px;
  padding: 10px;
  cursor: grab;
  transition: all 0.2s;
}
.ks-result-item:active {
  cursor: grabbing;
}
.ks-result-item:hover {
  border-color: var(--el-color-primary-light-5);
  transform: translateY(-1px);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.05);
}
.ks-item-header {
  margin-bottom: 6px;
}
.ks-item-category {
  font-size: 12px;
  color: var(--el-text-color-secondary);
}
.ks-item-title {
  font-weight: 500;
  font-size: 14px;
  margin-bottom: 4px;
  color: var(--el-text-color-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.ks-item-snippet {
  font-size: 12px;
  color: var(--el-text-color-regular);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.ks-item-snippet :deep(b) {
  color: var(--el-color-primary);
  background: var(--el-color-primary-light-9);
  padding: 0 2px;
  border-radius: 2px;
}
</style>
