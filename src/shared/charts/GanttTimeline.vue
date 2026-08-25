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
  daysLeft: number
}

const props = withDefaults(defineProps<{ items: GanttItem[]; windowDays?: number }>(), { windowDays: 30 })

const today = new Date()
const todayStr = `${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, '0')}-${String(today.getDate()).padStart(2, '0')}`

const rows = computed(() =>
  [...props.items]
    .sort((a, b) => a.date.localeCompare(b.date))
    .map(it => {
      const d = new Date(it.date)
      const dayIdx = Math.max(
        0,
        Math.min(props.windowDays - 1, Math.round((d.getTime() - today.getTime()) / 86400000)),
      )
      return {
        ...it,
        leftPct: (dayIdx / props.windowDays) * 100,
        urgency: it.daysLeft <= 3 ? 'high' : it.daysLeft <= 10 ? 'mid' : 'low',
      }
    }),
)

const axisTicks = computed(() =>
  [0, Math.floor(props.windowDays / 2), props.windowDays - 1].map(d => {
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
      <li v-for="r in rows" :key="r.id" class="gt-row">
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
      </li>
    </ul>
    <div v-if="!items.length" class="gt-empty">未来 {{ windowDays }} 天没有庭审安排</div>
  </div>
</template>

<style scoped>
.gantt { display: flex; flex-direction: column; gap: 2px; }
.gt-axis {
  position: relative; height: 16px;
  font-size: var(--text-sm); color: var(--c-text-secondary);
  font-family: var(--font-mono);
}
.gt-tick { position: absolute; transform: translateX(-50%); }
.gt-rows { list-style: none; margin: 0; padding: 0; }
.gt-row {
  display: flex; align-items: center; gap: 8px;
  height: 30px;
}
.gt-title {
  width: 150px; flex-shrink: 0;
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
</style>
