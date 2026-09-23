<script setup>
import HolidayBadges from './HolidayBadges.vue'
defineProps({ groups: { type: Array, required: true } })
const emit = defineEmits(['open'])
</script>

<template>
  <section class="timeline-stream" aria-label="按日期排列的日程">
    <div v-if="!groups.length" class="timeline-empty-hint">暂无已排期的事件、任务或休班安排</div>
    <section v-for="group in groups" :key="group.dateStr" class="timeline-date-group" :aria-label="group.dateLabel">
      <header class="group-date-label">
        <time :datetime="group.dateStr">{{ group.dateLabel }}</time>
        <span class="group-weekday">{{ group.weekday }}</span>
        <HolidayBadges :entries="group.holidays" />
      </header>
      <div class="group-cards-stack">
        <div v-if="group.holidays.length" class="timeline-holiday-note">
          <p v-for="entry in group.holidays" :key="`${entry.source}-${entry.date}`">
            <strong>{{ entry.source === 'personal' ? '个人安排' : '法定安排' }}</strong>
            {{ entry.name || (entry.kind === 'holiday' ? '休息日' : '调休工作日') }}
          </p>
          <span v-if="!group.items.length">当日暂无已排期事项</span>
        </div>
        <button v-for="item in group.items" :key="item.id" type="button" class="timeline-event-card" :class="item.type" @click="emit('open', item)">
          <span class="event-metadata"><span class="event-time">{{ item.time }}</span><span class="event-tag">{{ item.tag1 }}</span><span class="event-tag" :class="item.tag2Type">{{ item.tag2 }}</span><span v-if="item.duration" class="event-duration">{{ item.duration }}</span></span>
          <strong class="event-title">{{ item.title }}</strong>
          <span class="event-case">{{ item.caseName }}</span>
        </button>
      </div>
    </section>
  </section>
</template>

<style scoped>
.timeline-stream { padding: 24px; container-type: inline-size; }
.timeline-date-group { display: grid; grid-template-columns: 132px minmax(0, 1fr); gap: 24px; position: relative; padding-bottom: 28px; }
.timeline-date-group:last-child { padding-bottom: 0; }
.group-date-label { display: flex; flex-direction: column; align-items: flex-start; gap: 7px; min-width: 0; padding-top: 3px; font-size: 12px; font-weight: 650; line-height: 1.6; }
.group-weekday { color: var(--c-text-secondary); font-weight: 400; }
.group-cards-stack { display: flex; flex-direction: column; gap: 10px; position: relative; min-width: 0; }
.group-cards-stack::before { content: ''; position: absolute; left: -13px; top: 0; bottom: -28px; width: 1px; background: var(--c-border); }
.timeline-date-group:last-child .group-cards-stack::before { bottom: 0; }
.group-cards-stack::after { content: ''; position: absolute; left: -17px; top: 9px; width: 7px; height: 7px; border: 1px solid var(--c-primary); border-radius: 50%; background: var(--c-bg-page); }
.timeline-holiday-note { padding: 12px 14px; border: 1px dashed var(--c-border-strong); border-radius: 8px; color: var(--c-text-secondary); font-size: 12px; line-height: 1.7; overflow-wrap: anywhere; }
.timeline-holiday-note p { margin: 0; }
.timeline-holiday-note strong { color: var(--c-text); font-weight: 500; margin-right: 8px; }
.timeline-holiday-note > span { display: block; margin-top: 4px; font-size: 11px; }
.timeline-event-card { display: flex; flex-direction: column; gap: 7px; width: 100%; min-width: 0; padding: 14px 16px; border: 1px solid var(--c-border); border-left: 3px solid var(--c-primary); border-radius: 8px; background: var(--c-bg-card); color: var(--c-text); text-align: left; font: inherit; cursor: pointer; overflow-wrap: anywhere; }
.timeline-event-card.court { border-left-color: var(--c-danger); }
.timeline-event-card:hover { background: var(--c-bg-hover); border-top-color: var(--c-border-strong); }
.timeline-event-card:focus-visible { outline: 2px solid var(--c-primary); outline-offset: 2px; }
.event-metadata { display: flex; align-items: center; flex-wrap: wrap; gap: 6px; width: 100%; color: var(--c-text-secondary); font-size: 11px; }
.event-time { font-variant-numeric: tabular-nums; margin-right: 4px; }
.event-tag { border-radius: 3px; padding: 2px 5px; background: var(--c-bg-subtle); }
.event-tag.risk { background: var(--c-danger-light); color: var(--c-danger); }
.event-tag.warning { background: var(--c-warning-light); color: var(--c-warning); }
.event-duration { margin-left: auto; }
.event-title { font-size: 14px; line-height: 1.6; font-weight: 600; }
.event-case { font-size: 12px; color: var(--c-text-secondary); line-height: 1.6; }
.timeline-empty-hint { padding: 32px 0; text-align: center; color: var(--c-text-secondary); font-size: 13px; }
@container (max-width: 480px) {
  .timeline-date-group { grid-template-columns: minmax(0, 1fr); gap: 12px; padding-left: 14px; }
  .group-date-label { flex-direction: row; align-items: center; flex-wrap: wrap; gap: 6px 12px; }
  .timeline-event-card { padding: 12px; }
}
</style>
