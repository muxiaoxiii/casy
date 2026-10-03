<script setup lang="ts">
withDefaults(defineProps<{ text: string; lines?: number }>(), { lines: 1 })
</script>
<template><span class="ui-truncate" :title="text" :style="{ WebkitLineClamp: Math.max(1, lines) }">{{ text }}</span></template>
<style scoped>
.ui-truncate { min-width: 0; display: -webkit-box; -webkit-box-orient: vertical; overflow: hidden; overflow-wrap: anywhere; }
</style>
