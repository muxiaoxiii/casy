<script setup lang="ts">
defineProps<{ label?: string }>()
</script>
<template><div class="ui-toolbar" :aria-label="label"><div class="ui-toolbar__primary"><slot /></div><div v-if="$slots.actions" class="ui-toolbar__actions"><slot name="actions" /></div></div></template>
<style scoped>
.ui-toolbar { display: flex; align-items: center; justify-content: space-between; gap: 12px; flex-wrap: wrap; min-width: 0; }
.ui-toolbar__primary, .ui-toolbar__actions { display: flex; align-items: center; gap: 8px; min-width: 0; flex-wrap: wrap; }
.ui-toolbar__primary { flex: 1; }
@media (max-width: 899px) { .ui-toolbar__actions { width: 100%; } }
</style>
