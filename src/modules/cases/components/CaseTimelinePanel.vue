<script setup>
import { computed } from 'vue'

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
      <div class="card-header-row">
        <strong>时间线</strong>
        <el-button size="small" text @click="emit('addLog')">添加事件</el-button>
      </div>
    </template>
    <div v-if="loading" class="timeline-loading">加载中...</div>
    <div v-else-if="!timeline.length" class="timeline-empty">
      <el-empty description="还没有事件记录" :image-size="60">
        <el-button size="small" @click="emit('addLog')">添加第一条日志</el-button>
      </el-empty>
    </div>
    <div v-else class="timeline-list">
      <template v-for="group in groupedTimeline" :key="group.month">
        <div class="timeline-month-header">
          <span class="month-label">{{ formatMonthLabel(group.month) }}</span>
          <span class="month-divider" />
        </div>
        <div v-for="event in group.events" :key="event.id" class="timeline-item">
          <div class="timeline-marker" :style="{ color: event.color }">{{ event.icon }}</div>
          <div class="timeline-content">
            <div class="timeline-header">
              <span class="timeline-date">{{ event.eventDate }}</span>
              <span class="timeline-title">{{ event.title }}</span>
              <el-button size="small" text type="danger" @click="emit('deleteLog', event.id)">×</el-button>
            </div>
            <div v-if="event.detail" class="timeline-detail">{{ event.detail }}</div>
          </div>
        </div>
      </template>
    </div>
  </el-card>
</template>

<style scoped>
.card-header-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.timeline-empty {
  padding: 20px 0;
}

.timeline-loading {
  text-align: center;
  padding: 20px;
  color: #666;
}

.timeline-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.timeline-item {
  display: flex;
  gap: 10px;
  padding: 8px;
  border-radius: 6px;
  transition: background 0.2s;
}

.timeline-item:hover {
  background: #f5f7fa;
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

.timeline-header {
  display: flex;
  align-items: center;
  gap: 8px;
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
  display: flex;
  align-items: center;
  gap: 8px;
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
  background: #e4e7ed;
}
</style>
