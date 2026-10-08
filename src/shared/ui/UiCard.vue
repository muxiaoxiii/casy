<script setup lang="ts">
/** 内容卡片原语：统一工作面、圆角、边框与内边距（token 驱动，替代逐文件重复的卡片签名） */
withDefaults(defineProps<{
  padding?: number | string
  tone?: 'surface' | 'sunken' | 'outlined'
}>(), { padding: 16, tone: 'surface' })
</script>
<template>
  <div class="ui-card" :data-tone="tone" :style="{ padding: typeof padding === 'number' ? `${padding}px` : padding }">
    <slot />
  </div>
</template>
<style scoped>
.ui-card {
  background: var(--c-bg-card, var(--c-bg-page));
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg, 6px);
  min-width: 0;
}
.ui-card[data-tone=sunken] { background: var(--c-bg-page); }
.ui-card[data-tone=outlined] { background: transparent; }
</style>
