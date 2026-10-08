<script setup lang="ts">
/** 横向布局原语：消灭 "align-items,display,gap" 类重复签名（审计 Top1，41 文件） */
withDefaults(defineProps<{
  gap?: number | string
  align?: 'start' | 'center' | 'end' | 'stretch' | 'baseline'
  justify?: 'start' | 'center' | 'end' | 'between' | 'around'
  wrap?: boolean
}>(), { gap: 8, align: 'center', justify: 'start', wrap: false })
</script>
<template>
  <div class="ui-row" :style="{
    gap: typeof gap === 'number' ? `${gap}px` : gap,
    alignItems: align === 'start' ? 'flex-start' : align === 'end' ? 'flex-end' : align,
    justifyContent: justify === 'start' ? 'flex-start' : justify === 'between' ? 'space-between' : justify === 'around' ? 'space-around' : justify,
    flexWrap: wrap ? 'wrap' : 'nowrap',
  }"><slot /></div>
</template>
<style scoped>
.ui-row { display: flex; min-width: 0; }
</style>
