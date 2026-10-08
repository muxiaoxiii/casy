<script setup lang="ts">
/** 期限紧急度通道（bunny 10 第 4 步）：全应用唯一的期限语义视觉。
 *  level: R1 立即行动 / R2 临近 / R3 等待 / R4 完成；review 表示日期待核对（优先级高于 level）。 */
import { computed } from 'vue'

const props = withDefaults(defineProps<{
  level?: 'R1' | 'R2' | 'R3' | 'R4' | null
  review?: boolean
  /** 距届满天数（负数=已过）；仅用于展示，不参与分级 */
  daysLeft?: number | null
  text?: string
}>(), { level: null, review: false, daysLeft: null, text: '' })

const daysText = computed(() => {
  if (props.daysLeft === null || props.daysLeft === undefined) return ''
  if (props.daysLeft < 0) return `已过 ${Math.abs(props.daysLeft)} 天`
  if (props.daysLeft === 0) return '今日届满'
  return `${props.daysLeft} 天`
})
const label = computed(() => props.text || daysText.value || '')
const cls = computed(() => props.review ? 'dl-chip--review' : props.level ? `dl-chip--${props.level.toLowerCase()}` : '')
</script>
<template>
  <span v-if="review || level || label" class="dl-chip" :class="cls">
    <span v-if="review" class="dl-chip__mark" aria-hidden="true">△</span>
    <span v-if="review">待核对</span>
    <span v-else-if="level && label">{{ level }} · {{ label }}</span>
    <span v-else-if="level">{{ level }}</span>
    <span v-else>{{ label }}</span>
  </span>
</template>
