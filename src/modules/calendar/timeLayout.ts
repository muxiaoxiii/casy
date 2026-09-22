import { minutes } from './parseCalendarCapture'

export interface TimedItem {
  id: string
  title: string
  startTime: string
  endTime?: string | null
  kind: 'event' | 'task'
  color?: string | null
  completed?: boolean
}
/** Interval partitioning: overlapping events occupy lanes; adjoining events share one. */
export function layoutTimedItems(items: TimedItem[]) {
  const sorted = items.flatMap(item => {
    const start = minutes(item.startTime.slice(0, 5))
    if (start === null) return []
    const end = Math.min(1440, Math.max(start + 15, item.endTime ? minutes(item.endTime.slice(0, 5)) ?? start + 60 : start + 60))
    return [{ ...item, start, end, lane: 0, lanes: 1 }]
  }).sort((a, b) => a.start - b.start || b.end - a.end || a.id.localeCompare(b.id))
  let group: typeof sorted = []
  let groupEnd = -1
  let laneEnds: number[] = []
  function finish() { group.forEach(item => { item.lanes = laneEnds.length }) }
  for (const item of sorted) {
    if (item.start >= groupEnd) { finish(); group = []; laneEnds = []; groupEnd = -1 }
    let lane = laneEnds.findIndex(end => end <= item.start)
    if (lane === -1) lane = laneEnds.length
    laneEnds[lane] = item.end; item.lane = lane
    group.push(item); groupEnd = Math.max(groupEnd, item.end)
  }
  finish()
  return sorted
}
