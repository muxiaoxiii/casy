<script setup lang="ts">
/**
 * GanttTimeline —— 近期日程甘特条（对标设计稿近期庭审时间线）
 * 每行 = 一个事件；横条位置按日期在窗口内比例定位，今天线高亮。
 */
import { computed } from 'vue'

export interface GanttItem {
  id: string
  title: string
  date: string // YYYY-MM-DD
  caseName?: string
  caseId?: string
  daysLeft: number
}

const props = withDefaults(defineProps<{ items: GanttItem[]; windowDays?: number }>(), { windowDays: 30 })
const emit = defineEmits<{ (e: 'select', item: GanttItem): void }>()

const today = new Date()

const rows = computed(() =>
  [...props.items]
    .sort((a, b) => a.date.localeCompare(b.date))
    .map(it => {
      const dayIdx = Math.max(0, Math.min(props.windowDays, it.daysLeft))
      return {
        ...it,
        leftPct: (dayIdx / props.windowDays) * 100,
        urgency: it.daysLeft <= 3 ? 'high' : it.daysLeft <= 10 ? 'mid' : 'low',
      }
    }),
)

const axisTicks = computed(() =>
  [0, Math.floor(props.windowDays / 2), props.windowDays].map(d => {
    const dt = new Date(today.getTime() + d * 86400000)
    return { d, label: `${dt.getMonth() + 1}/${dt.getDate()}` }
  }),
)
</script>

<template>
  <div class="gantt">
    <div class="gt-axis">
      <span v-for="t in axisTicks" :key="t.d" class="gt-tick" :style="{ left: (t.d / windowDays) * 100 + '%' }">
        {{ t.label }}
      </span>
    </div>
    <ul class="gt-rows">
      <li v-for="r in rows" :key="r.id">
        <button type="button" class="gt-row" @click="emit('select', r)" :aria-label="`${r.title}，${r.caseName || ''}，${r.date}`">
        <span class="gt-title" :title="`${r.title} · ${r.caseName}`">{{ r.title }}</span>
        <div class="gt-lane">
          <span
            class="gt-bar"
            :class="'u-' + r.urgency"
            :style="{ left: r.leftPct + '%' }"
            :title="`${r.title} · ${r.caseName} · ${r.date}`"
          />
          <span class="gt-today" />
        </div>
        <span class="gt-days" :class="'d-' + r.urgency">
          {{ r.daysLeft === 0 ? '今天' : `${r.daysLeft}天` }}
        </span>
        </button>
      </li>
    </ul>
    <div v-if="!items.length" class="gt-empty">未来 {{ windowDays }} 天没有庭审安排</div>
  </div>
</template>

<style scoped>
.gantt { display: flex; flex-direction: column; gap: 2px; }
.gt-axis {
  margin-left: calc(var(--gt-label-width) + 8px);
  margin-right: 48px;
  position: relative; height: 16px;
  font-size: var(--text-sm); color: var(--c-text-secondary);
  font-family: var(--font-mono);
}
.gt-tick { position: absolute; transform: translateX(-50%); }
.gt-tick:first-child { transform: none; }
.gt-tick:last-child { transform: translateX(-100%); }
.gt-rows { list-style: none; margin: 0; padding: 0; }
.gt-row {
  display: flex; align-items: center; gap: 8px;
  min-height: 40px; width: 100%; border: 0; padding: 0; background: transparent; font: inherit; text-align: left; cursor: pointer;
}
.gt-row:hover { background: var(--c-bg-hover); }
.gantt { --gt-label-width: 120px; }
.gt-title {
  width: var(--gt-label-width); flex-shrink: 0;
  font-size: var(--text-base); color: var(--c-text-regular);
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
}
.gt-lane {
  flex: 1; position: relative; height: 12px;
  background:
    linear-gradient(90deg, transparent calc(0% - .5px), var(--gray-100) 0) left/100% 100%;
  border-radius: 3px;
  background-color: var(--gray-50);
}
.gt-bar {
  position: absolute; top: 2px;
  width: 14px; height: 8px; border-radius: 4px;
  transform: translateX(-50%);
  transition: transform var(--motion-fast) var(--ease-out);
}
.gt-bar:hover { transform: translateX(-50%) scale(1.6); }
.gt-bar.u-high { background: var(--c-danger); }
.gt-bar.u-mid { background: var(--c-warning); }
.gt-bar.u-low { background: var(--c-info); }
.gt-today {
  position: absolute; left: 0; top: -3px; bottom: -3px;
  width: 1px; background: var(--c-border-strong, #CCD0D8);
}
.gt-days {
  width: 40px; text-align: right; flex-shrink: 0;
  font-size: var(--text-sm); font-family: var(--font-mono);
}
.gt-days.d-high { color: var(--c-danger); font-weight: 600; }
.gt-empty {
  padding: 18px 0; text-align: center;
  font-size: var(--text-base); color: var(--c-text-secondary);
}
@media (max-width: 600px) { .gantt { --gt-label-width: 88px; } }
</style>
