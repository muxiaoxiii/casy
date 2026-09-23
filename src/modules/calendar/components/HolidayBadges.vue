<script setup lang="ts">
import { availabilityTimeLabel } from '../calendarDates'
import type { HolidayCalendarEntry } from '../../../types/ipc'
defineProps<{ entries: HolidayCalendarEntry[]; compact?: boolean }>()
</script>
<template>
  <span class="holiday-badges">
    <span v-for="entry in entries" :key="`${entry.source || 'official'}-${entry.date}`" class="holiday-badge" :class="[entry.kind, { personal: entry.source === 'personal' }]" :title="`${entry.source === 'personal' ? '个人安排' : '法定安排'} · ${entry.kind === 'holiday' ? '休息' : '上班'} · ${availabilityTimeLabel(entry)}${entry.name ? ` · ${entry.name}` : ''}`">
      {{ entry.source === 'personal' ? (entry.kind === 'holiday' ? (entry.startTime ? '时休' : '自休') : '自班') : compact ? (entry.kind === 'holiday' ? '休' : '班') : (entry.kind === 'holiday' ? '法定休' : '法定班') }}<template v-if="!compact && entry.startTime"> {{ availabilityTimeLabel(entry) }}</template>
    </span>
  </span>
</template>
<style scoped>
.holiday-badges { display: inline-flex; flex-wrap: wrap; gap: 3px; vertical-align: middle; }
.holiday-badge { font-size: 10px; line-height: 1.4; padding: 1px 3px; border: 1px solid transparent; border-radius: 3px; color: var(--c-warning); background: var(--c-warning-light); white-space: nowrap; }
.holiday-badge.workday { color: var(--c-info); background: var(--c-info-light); }
.holiday-badge.personal { border: 1px dashed var(--c-primary); color: var(--c-primary); background: var(--c-bg-card); }
</style>
