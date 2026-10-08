<script setup lang="ts">
/** 段标题原语：标题 + 计数/说明 + 右侧动作（替代 16 文件的段头 flex 签名） */
defineProps<{ title: string; count?: number | string; hint?: string }>()
</script>
<template>
  <div class="ui-section-header">
    <div class="ui-section-header__main">
      <span class="ui-section-header__title">{{ title }}</span>
      <span v-if="count !== undefined" class="ui-section-header__count">{{ count }}</span>
      <span v-if="hint" class="ui-section-header__hint">{{ hint }}</span>
    </div>
    <div v-if="$slots.actions" class="ui-section-header__actions"><slot name="actions" /></div>
  </div>
</template>
<style scoped>
.ui-section-header { display: flex; align-items: center; justify-content: space-between; gap: 8px; margin-bottom: 8px; min-width: 0; }
.ui-section-header__main { display: flex; align-items: baseline; gap: 6px; min-width: 0; }
.ui-section-header__title { font-size: 13px; font-weight: 600; color: var(--c-text-heading); }
.ui-section-header__count { font-size: 12px; color: var(--c-text-secondary); }
.ui-section-header__hint { font-size: 12px; color: var(--c-text-secondary); }
.ui-section-header__actions { display: flex; align-items: center; gap: 4px; flex-shrink: 0; }
</style>
