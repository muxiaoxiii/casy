<script setup lang="ts">
withDefaults(defineProps<{ tone?: 'neutral' | 'danger' | 'warning' | 'success'; review?: boolean }>(), { tone: 'neutral', review: false })
</script>
<template><span class="ui-pill" :data-tone="review ? 'warning' : tone"><span v-if="review" aria-hidden="true">△</span><slot /><span v-if="review"> · 待核对</span></span></template>
<style scoped>
.ui-pill { display: inline-flex; align-items: center; gap: 4px; padding: 2px 8px; border: 1px solid var(--c-border); border-radius: 4px; color: var(--c-text-secondary); background: var(--c-bg-page); font-size: 12px; line-height: 1.6; white-space: nowrap; }
.ui-pill[data-tone=danger] { color: var(--c-danger); border-color: currentColor; }
.ui-pill[data-tone=warning] { color: var(--c-warning-dark, var(--c-warning)); border-color: currentColor; }
.ui-pill[data-tone=success] { color: var(--c-success); border-color: currentColor; }
</style>
