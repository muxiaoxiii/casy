<script setup lang="ts">
/** 区块面板：标题栏 + 内容区。替代 "align-items,display,justify-content,margin-bottom" 段头签名（16 文件） */
defineProps<{ title?: string; subtitle?: string }>()
</script>
<template>
  <section class="ui-panel">
    <header v-if="title || $slots.actions" class="ui-panel__head">
      <div class="ui-panel__titles">
        <h3 v-if="title" class="ui-panel__title">{{ title }}</h3>
        <p v-if="subtitle" class="ui-panel__subtitle">{{ subtitle }}</p>
      </div>
      <div v-if="$slots.actions" class="ui-panel__actions"><slot name="actions" /></div>
    </header>
    <div class="ui-panel__body"><slot /></div>
  </section>
</template>
<style scoped>
.ui-panel { display: flex; flex-direction: column; gap: 12px; min-width: 0; }
.ui-panel__head { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.ui-panel__titles { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.ui-panel__title { margin: 0; font-size: 14px; font-weight: 600; color: var(--c-text-heading); }
.ui-panel__subtitle { margin: 0; font-size: 12px; color: var(--c-text-secondary); }
.ui-panel__body { min-width: 0; }
</style>
