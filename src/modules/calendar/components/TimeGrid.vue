<script setup lang="ts">
import { computed, ref, onMounted } from 'vue'
import { layoutTimedItems, type TimedItem } from '../timeLayout'
const props = defineProps<{ days: Array<{ date: string; items: TimedItem[]; rest?: Array<{start:number;end:number;label:string}> }> }>()
const emit = defineEmits<{ open: [item: TimedItem, date: string]; dropTask: [event: DragEvent, date: string, hour: number] }>()
const scroller = ref<HTMLElement>()
const columns = computed(() => props.days.map(day => ({ ...day, items: layoutTimedItems(day.items) })))
onMounted(() => { if (scroller.value) scroller.value.scrollTop = 7 * 56 })
</script>
<template>
  <div ref="scroller" class="time-grid-scroll" aria-label="按实际时长显示的日程">
    <div class="time-grid" :style="{ gridTemplateColumns: `48px repeat(${days.length}, minmax(0, 1fr))` }">
      <div class="time-gutter"><span v-for="h in 24" :key="h" :style="{ top: `${(h - 1) * 56}px` }">{{ String(h - 1).padStart(2, '0') }}:00</span></div>
      <div v-for="day in columns" :key="day.date" class="time-day">
        <div v-for="h in 24" :key="h" class="time-drop-slot" :aria-label="`${day.date} ${h - 1}:00`" @dragover.prevent="($event.currentTarget as HTMLElement).classList.add('dragging')" @dragleave="($event.currentTarget as HTMLElement).classList.remove('dragging')" @drop.prevent="($event.currentTarget as HTMLElement).classList.remove('dragging'); emit('dropTask', $event, day.date, h - 1)" />
        <div v-for="(rest,index) in day.rest || []" :key="`rest-${index}`" class="time-rest" :style="{top:`${rest.start/60*56}px`,height:`${(rest.end-rest.start)/60*56}px`}" :aria-label="rest.label"><span>{{ rest.label }}</span></div>
        <button v-for="item in day.items" :key="item.kind + item.id" type="button" class="time-event" :class="{ completed: item.completed }" :title="`${item.startTime}–${item.endTime || ''} ${item.title}`" :style="{ top: `${item.start / 60 * 56}px`, height: `${(item.end - item.start) / 60 * 56 - 2}px`, left: `calc(${item.lane / item.lanes * 100}% + 3px)`, width: `calc(${100 / item.lanes}% - 6px)`, '--event-color': item.color || 'var(--c-primary)' }" @click="emit('open', item, day.date)">
          <strong>{{ item.completed ? '✓ ' : '' }}{{ item.title }}</strong><small>{{ item.startTime }}<template v-if="item.endTime"> – {{ item.endTime }}</template></small>
        </button>
      </div>
    </div>
  </div>
</template>
<style scoped>
.time-grid-scroll { max-height: min(68vh, 740px); min-height: 320px; overflow: auto; background: var(--c-bg-card); }
.time-grid { display: grid; height: 1344px; }
.time-gutter { position: relative; color: var(--c-text-secondary); font-size: 10px; font-variant-numeric: tabular-nums; }
.time-gutter span { position: absolute; left: 6px; padding-top: 3px; }
.time-day { position: relative; min-width: 0; border-left: 1px solid var(--c-border); }
.time-drop-slot { box-sizing: border-box; height: 56px; border-top: 1px solid var(--c-border); }
.time-drop-slot.dragging { background: var(--c-primary-light); box-shadow: inset 0 0 0 1px var(--c-primary); }
.time-rest { position:absolute; left:0; right:0; pointer-events:none; box-sizing:border-box; border-block:1px dashed var(--c-primary); color:var(--c-primary); background:repeating-linear-gradient(135deg,transparent 0,transparent 6px,color-mix(in srgb,var(--c-primary) 9%,transparent) 6px,color-mix(in srgb,var(--c-primary) 9%,transparent) 8px); font-size:10px; }.time-rest span { display:block; padding:3px; }.time-event { position: absolute; box-sizing: border-box; text-align: left; padding: 5px 7px; overflow: hidden; border: 0; border-left: 3px solid var(--event-color); border-radius: 5px; background: color-mix(in srgb, var(--event-color) 12%, var(--c-bg-card)); color: var(--c-text); font: inherit; cursor: pointer; min-height: 12px; }
.time-event strong { display: block; font-size: 12px; line-height: 1.4; font-weight: 550; overflow: hidden; }
.time-event small { display: block; font-size: 10px; margin-top: 4px; color: var(--c-text-secondary); }
.time-event:hover { filter: brightness(.96); z-index: 2; }
.time-event.completed { opacity: .6; }
.time-event.completed strong { text-decoration: line-through; }
.time-event:focus-visible { outline: 2px solid var(--c-primary); outline-offset: 1px; z-index: 3; }

</style>
