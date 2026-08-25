<script setup lang="ts">
/**
 * AreaLineChart —— 双序列趋势（面积+折线，对标设计稿 TaskTrendChart）
 * hover 竖向参考线 + 数据点提示。
 */
import { computed, ref } from 'vue'

export interface TrendPoint {
  month: string
  created: number
  completed: number
}

const props = withDefaults(defineProps<{ data: TrendPoint[]; height?: number }>(), { height: 180 })

const W = 460
const PAD_L = 30
const PAD_R = 12
const PAD_T = 10
const PAD_B = 22

const hoverIdx = ref<number | null>(null)

const maxV = computed(() => Math.max(1, ...props.data.map(d => Math.max(d.created, d.completed))))
const innerW = computed(() => W - PAD_L - PAD_R)
const innerH = computed(() => props.height - PAD_T - PAD_B)

function xAt(i: number): number {
  const n = props.data.length
  if (n <= 1) return PAD_L + innerW.value / 2
  return PAD_L + (innerW.value * i) / (n - 1)
}
function yAt(v: number): number {
  return PAD_T + innerH.value - (v / maxV.value) * innerH.value
}

const linePath = computed(() =>
  props.data.map((d, i) => `${i === 0 ? 'M' : 'L'} ${xAt(i)} ${yAt(d.created)}`).join(' '),
)
const linePathDone = computed(() =>
  props.data.map((d, i) => `${i === 0 ? 'M' : 'L'} ${xAt(i)} ${yAt(d.completed)}`).join(' '),
)
const areaPath = computed(() => {
  if (!props.data.length) return ''
  const base = PAD_T + innerH.value
  return (
    linePath.value +
    ` L ${xAt(props.data.length - 1)} ${base} L ${xAt(0)} ${base} Z`
  )
})

function shortMonth(m: string): string {
  const parts = m.split('-')
  return parts.length === 2 ? `${Number(parts[1])}月` : m
}

function onMove(e: MouseEvent) {
  const rect = (e.currentTarget as SVGElement).getBoundingClientRect()
  const rel = ((e.clientX - rect.left) - PAD_L) / innerW.value
  const n = props.data.length
  if (n === 0) return
  hoverIdx.value = Math.min(n - 1, Math.max(0, Math.round(rel * (n - 1))))
}
</script>

<template>
  <div class="trend-wrap">
    <svg :width="W" :height="height" @mousemove="onMove" @mouseleave="hoverIdx = null">
      <!-- 横网格 -->
      <g v-for="gv in [0, 0.5, 1]" :key="gv">
        <line
          :x1="PAD_L" :x2="W - PAD_R"
          :y1="PAD_T + innerH * (1 - gv)" :y2="PAD_T + innerH * (1 - gv)"
          stroke="#EEF0F3" stroke-width="1"
        />
        <text :x="PAD_L - 6" :y="PAD_T + innerH * (1 - gv) + 3" text-anchor="end" class="tick">
          {{ Math.round(maxV * gv) }}
        </text>
      </g>
      <!-- 面积：创建量 -->
      <path :d="areaPath" fill="var(--c-primary)" opacity="0.08" />
      <!-- 折线 -->
      <path :d="linePath" fill="none" stroke="var(--c-primary)" stroke-width="2" />
      <path v-if="data.some(d => d.completed > 0)" :d="linePathDone" fill="none" stroke="var(--c-success)" stroke-width="2" stroke-dasharray="4 3" />
      <!-- 数据点 -->
      <circle v-for="(d, i) in data" :key="'p' + i" :cx="xAt(i)" :cy="yAt(d.created)" r="2.6" fill="var(--c-primary)" />
      <circle
        v-for="(d, i) in data"
        :key="'c' + i"
        :cx="xAt(i)" :cy="yAt(d.completed)" r="2.4" fill="var(--c-success)"
      />
      <!-- hover 参考线 -->
      <g v-if="hoverIdx !== null && data[hoverIdx]">
        <line
          :x1="xAt(hoverIdx)" :x2="xAt(hoverIdx)"
          :y1="PAD_T" :y2="PAD_T + innerH"
          stroke="var(--c-border-strong, #CCD0D8)" stroke-width="1" stroke-dasharray="3 3"
        />
        <rect
          :x="Math.min(xAt(hoverIdx) + 6, W - 118)" y="PAD_T"
          width="112" height="34" rx="5"
          fill="#fff" stroke="#E4E7EB"
        />
        <text class="tip-title" :x="Math.min(xAt(hoverIdx) + 6, W - 118) + 8" y="PAD_T + 14">
          {{ data[hoverIdx].month }}
        </text>
        <text class="tip-line c-new" :x="Math.min(xAt(hoverIdx) + 6, W - 118) + 8" y="PAD_T + 28">
          新建 {{ data[hoverIdx].created }} · 完成 {{ data[hoverIdx].completed }}
        </text>
      </g>
      <!-- X 轴标签 -->
      <text v-for="(d, i) in data" :key="'x' + i" :x="xAt(i)" :y="height - 6" text-anchor="middle" class="tick">
        {{ shortMonth(d.month) }}
      </text>
    </svg>
    <div class="legend">
      <span><i class="dot primary" />新建</span>
      <span><i class="dot success" />完成</span>
    </div>
  </div>
</template>

<style scoped>
.trend-wrap { position: relative; }
.tick {
  font-size: 9.5px;
  fill: var(--c-text-secondary);
  font-family: var(--font-mono);
}
.tip-title { font-size: 10px; fill: var(--c-text-secondary); }
.tip-line { font-size: 10px; }
.tip-line.c-new { fill: var(--c-text); }
.legend {
  display: flex; gap: 14px;
  font-size: var(--text-sm); color: var(--c-text-secondary);
  margin-top: 4px;
}
.legend .dot {
  display: inline-block; width: 8px; height: 8px;
  border-radius: 50%; margin-right: 4px;
}
.dot.primary { background: var(--c-primary); }
.dot.success { background: var(--c-success); }
</style>
