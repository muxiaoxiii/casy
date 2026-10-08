<script setup>
import { computed } from 'vue'
import EmptyState from '../../../shared/components/EmptyState.vue'

const props = defineProps({
  timeline: { type: Array, default: () => [] },
  loading: { type: Boolean, default: false },
})

const emit = defineEmits(['addLog', 'deleteLog'])

/** 按 YYYY-MM 分组的时间线 */
const groupedTimeline = computed(() => {
  const groups = new Map()
  for (const event of props.timeline) {
    const dateStr = event.eventDate || ''
    const ym = dateStr.slice(0, 7) // "YYYY-MM"
    if (!ym) continue
    if (!groups.has(ym)) groups.set(ym, [])
    groups.get(ym).push(event)
  }
  return Array.from(groups.entries()).map(([month, events]) => ({ month, events }))
})

function formatMonthLabel(ym) {
  const [y, m] = ym.split('-')
  return `${y}年${parseInt(m)}月`
}
</script>

<template>
  <el-card>
    <template #header>
      <div class="ui-row ui-row--between card-header-row">
        <strong>时间线</strong>
        <el-button size="small" text @click="emit('addLog')">添加事件</el-button>
      </div>
    </template>
    <div v-if="loading" class="timeline-loading">加载中...</div>
    <div v-else-if="!timeline.length" class="timeline-empty">
      <EmptyState type="custom" title="还没有事件记录" action-text="添加第一条日志" @action="emit('addLog')" />
    </div>
    <div v-else class="ui-col timeline-list">
      <template v-for="group in groupedTimeline" :key="group.month">
        <div class="ui-row timeline-month-header">
          <span class="month-label">{{ formatMonthLabel(group.month) }}</span>
          <span class="month-divider" />
        </div>
        <div v-for="event in group.events" :key="event.id" class="timeline-item">
          <div class="timeline-marker" :style="{ color: event.color }">{{ event.icon }}</div>
          <div class="timeline-content">
            <div class="ui-row timeline-header">
              <span class="timeline-date">{{ event.eventDate }}</span>
              <span class="timeline-title">{{ event.title }}</span>
              <el-button v-if="event.sourceTable==='case_logs'" size="small" text type="danger" @click="emit('deleteLog', event.sourceId)">×</el-button>
            </div>
            <div v-if="event.detail" class="timeline-detail">{{ event.detail }}</div>
          </div>
        </div>
      </template>
    </div>
  </el-card>
</template>

<style scoped>
.timeline-empty {
  padding: 20px 0;
}

.timeline-loading {
  text-align: center;
  padding: 20px;
  color: #666;
}

.timeline-list {
  gap: 8px;
}

.timeline-item {
  display: flex;
  gap: 10px;
  padding: 8px;
  border-radius: 6px;
  transition: background var(--motion-base);
}

.timeline-item:hover {
  background: var(--gray-50);
}

.timeline-marker {
  font-size: 16px;
  flex-shrink: 0;
  width: 24px;
  text-align: center;
}

.timeline-content {
  flex: 1;
  min-width: 0;
}

.timeline-date {
  font-size: 12px;
  color: #999;
  flex-shrink: 0;
}

.timeline-title {
  flex: 1;
  font-size: 14px;
}

.timeline-detail {
  font-size: 13px;
  color: #666;
  margin-top: 4px;
  white-space: pre-wrap;
}

.timeline-month-header {
  padding: 12px 0 6px;
}

.timeline-month-header:first-child {
  padding-top: 0;
}

.month-label {
  font-size: 13px;
  font-weight: 600;
  color: #409eff;
  white-space: nowrap;
}

.month-divider {
  flex: 1;
  height: 1px;
  background: var(--c-border);
}
</style>
