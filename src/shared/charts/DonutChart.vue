<script setup lang="ts">
/**
 * DonutChart —— 环形分布图（手绘 SVG · 零依赖 · 对标设计稿 CaseStatusChart）
 *
 * - 分段悬停外扩 + 中心总数标签
 * - 点击分段触发下钻（可选）
 */
import { computed, ref } from 'vue'

export interface DonutDatum {
  label: string
  value: number
  color: string
}

const props = withDefaults(
  defineProps<{
    data: DonutDatum[]
    size?: number
    innerRatio?: number
    centerLabel?: string
    centerSub?: string
    interactive?: boolean
  }>(),
  { size: 200, innerRatio: 0.62, centerLabel: '', centerSub: '', interactive: true },
)

const emit = defineEmits<{ (e: 'select', d: DonutDatum): void }>()

const hover = ref<number | null>(null)

const radius = computed(() => props.size / 2)
const innerR = computed(() => radius.value * props.innerRatio)

interface Arc {
  d: string
  datum: DonutDatum
  midAngle: number
  share: number
}

const arcs = computed<Arc[]>(() => {
  const total = props.data.reduce((s, d) => s + d.value, 0)
  if (!total) return []
  let acc = -Math.PI / 2 // 从正上方开始
  return props.data.filter(datum => datum.value > 0).map(datum => {
    const frac = datum.value / total
    const pad = Math.min(0.02, frac * Math.PI * 2 * 0.15)
    const a0 = acc + pad / 2
    const a1 = acc + frac * Math.PI * 2 - pad / 2
    acc += frac * Math.PI * 2

    const rO = radius.value
    const rI = innerR.value
    const large = a1 - a0 > Math.PI ? 1 : 0
    const p = (r: number, a: number) => `${radius.value + r * Math.cos(a)},${radius.value + r * Math.sin(a)}`
    const d =
      `M ${p(rO, a0)} A ${rO} ${rO} 0 ${large} 1 ${p(rO, a1)}` +
      ` L ${p(rI, a1)} A ${rI} ${rI} 0 ${large} 0 ${p(rI, a0)} Z`
    const arc: Arc = { d, datum, midAngle: (a0 + a1) / 2, share: frac }
    return arc
  })
})

const totalValue = computed(() => props.data.reduce((s, d) => s + d.value, 0))

function legendValue(d: DonutDatum): string {
  const pct = totalValue.value ? Math.round((d.value / totalValue.value) * 100) : 0
  return `${d.value} · ${pct}%`
}
</script>

<template>
  <div class="donut-wrap">
    <div class="donut-svg-holder" :style="{ width: size + 'px', height: size + 'px' }">
      <svg :viewBox="`-4 -4 ${size + 8} ${size + 8}`" :width="size" :height="size" aria-label="状态分布">
        <circle :cx="radius" :cy="radius" :r="(radius + innerR) / 2" fill="none" stroke="var(--c-border-light)" :stroke-width="radius - innerR" />
        <g>
          <path
            v-for="(a, i) in arcs"
            :key="i"
            :d="a.d"
            :fill="a.datum.color"
            stroke="var(--c-bg-page)"
            stroke-width="2"
            :style="{ cursor: interactive ? 'pointer' : 'default' }"
            @mouseenter="hover = data.indexOf(a.datum)"
            @mouseleave="hover = null"
            @click="interactive && emit('select', a.datum)"
          ><title>{{ a.datum.label }}：{{ legendValue(a.datum) }}</title></path>
        </g>
      </svg>
      <div class="donut-center">
        <span class="dc-main">{{ centerLabel || totalValue }}</span>
        <span class="dc-sub">{{ centerSub || '总计' }}</span>
      </div>
    </div>

    <ul class="donut-legend">
      <li
        v-for="(d, i) in data"
        :key="d.label"
        :class="{ dim: hover !== null && hover !== i }"
        @mouseenter="hover = i"
        @mouseleave="hover = null"
      >
        <button type="button" :disabled="!interactive" @click="emit('select', d)" :aria-label="`${d.label}，${legendValue(d)}`">
        <span class="dl-dot" :style="{ background: d.color }" />
        <span class="dl-label">{{ d.label }}</span>
        <span class="dl-value">{{ legendValue(d) }}</span>
        </button>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.donut-wrap {
  display: flex;
  align-items: center;
  gap: 18px;
  flex-wrap: wrap;
  justify-content: center;
}
.donut-svg-holder {
  position: relative;
  flex-shrink: 0;
}
.donut-center {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  pointer-events: none;
}
.dc-main {
  font-size: 26px;
  font-weight: 700;
  color: var(--c-text);
}
.dc-sub {
  font-size: var(--text-sm);
  color: var(--c-text-secondary);
}
.donut-legend {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 7px;
  min-width: 130px;
}
.donut-legend li {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  transition: opacity var(--motion-fast) var(--ease-out);
}
.donut-legend button { display: flex; align-items: center; gap: 8px; width: 100%; padding: 5px 0; border: 0; background: transparent; font: inherit; text-align: left; cursor: pointer; }
.donut-legend { flex: 1; }
.donut-legend button:disabled { cursor: default; opacity: 1; }
.donut-legend li.dim { opacity: 0.4; }
.dl-dot { width: 9px; height: 9px; border-radius: 3px; flex-shrink: 0; }
.dl-label { flex: 1; font-size: var(--text-base); color: var(--c-text-regular); }
.dl-value { font-size: var(--text-sm); color: var(--c-text-secondary); font-family: var(--font-mono); }
</style>
