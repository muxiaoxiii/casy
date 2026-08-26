<script setup lang="ts">
/** HBarChart —— 水平条形分布图（对标设计稿 TrackChart） */
import { computed } from 'vue'

export interface BarDatum {
  label: string
  value: number
  color?: string
}

const props = withDefaults(defineProps<{ data: BarDatum[]; max?: number }>(), { max: 0 })

const emit = defineEmits<{ (e: 'select', d: BarDatum): void }>()

const maxValue = computed(() => Math.max(1, props.max || Math.max(...props.data.map(d => d.value), 0)))
</script>

<template>
  <div class="hbar">
    <div v-for="d in data" :key="d.label" class="hb-row" style="cursor:pointer" @click="emit('select', d)">
      <span class="hb-label">{{ d.label }}</span>
      <div class="hb-track">
        <div
          class="hb-fill"
          :style="{ width: (d.value / maxValue) * 100 + '%', background: d.color || 'var(--c-primary)' }"
        />
      </div>
      <span class="hb-value">{{ d.value }}</span>
    </div>
    <div v-if="!data.length" class="hb-empty">暂无数据</div>
  </div>
</template>

<style scoped>
.hbar { display: flex; flex-direction: column; gap: 10px; }
.hb-row { display: flex; align-items: center; gap: 10px; }
.hb-label {
  width: 76px; flex-shrink: 0;
  font-size: var(--text-base); color: var(--c-text-regular);
  white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
}
.hb-track {
  flex: 1; height: 14px; border-radius: 4px;
  background: var(--gray-100); overflow: hidden;
}
.hb-fill {
  height: 100%; border-radius: 4px;
  transition: width var(--motion-slow) var(--ease-out);
}
.hb-value {
  width: 30px; text-align: right; font-size: var(--text-sm);
  color: var(--c-text-secondary); font-family: var(--font-mono);
}
.hb-empty { font-size: var(--text-base); color: var(--c-text-secondary); padding: 12px 0; text-align: center; }
</style>
