<script setup lang="ts">
import EmptyState from '../components/EmptyState.vue'
import SkeletonCard from '../components/SkeletonCard.vue'
import DegradedBanner from '../components/DegradedBanner.vue'
withDefaults(defineProps<{
  loading: boolean
  error?: string
  count: number
  filtered?: boolean
  emptyTitle?: string
  emptyDescription?: string
  actionText?: string
}>(), { error: '', filtered: false, emptyTitle: '还没有数据', emptyDescription: '', actionText: '' })
const emit = defineEmits<{ retry: []; clear: []; create: [] }>()
</script>
<template>
  <section class="ui-data-state" :aria-busy="loading">
    <DegradedBanner v-if="error" title="数据读取失败" :reason="error" :dismissible="false"
      :alternative="count ? '保留上次加载的数据，可能不是最新状态' : ''" @retry="emit('retry')" />
    <div v-if="loading && !count" role="status" aria-live="polite" aria-label="正在加载数据" class="ui-data-state__loading">
      <SkeletonCard v-for="row in 3" :key="row" :rows="2" />
    </div>
    <template v-else-if="count">
      <p v-if="loading" class="ui-data-state__refresh" role="status">正在刷新，保留当前内容…</p>
      <slot />
    </template>
    <EmptyState v-else-if="!error" type="custom" :title="filtered ? '没有匹配结果' : emptyTitle"
      :description="filtered ? '尝试清除筛选，查看全部内容' : emptyDescription"
      :action-text="filtered ? '清除筛选' : actionText" :hide-action="!filtered && !actionText"
      @action="filtered ? emit('clear') : emit('create')" />
  </section>
</template>
<style scoped>
.ui-data-state { min-width: 0; }
.ui-data-state__loading { display: grid; gap: 12px; }
.ui-data-state__refresh { color: var(--c-text-secondary); font-size: 12px; margin: 0 0 12px; }
</style>
