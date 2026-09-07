import { defineStore } from 'pinia'
import { casyContext } from '../core/plugin/context'
import type { CalendarEvent } from '../types'
import type { HolidayCalendarEntry } from '../types/ipc'

interface CalendarState {
  events: CalendarEvent[]
  loading: boolean
  currentYear: number
  currentMonth: number
  holidayEntries: HolidayCalendarEntry[]
}

export const useCalendarStore = defineStore('calendar', {
  state: (): CalendarState => ({
    events: [],
    loading: false,
    currentYear: new Date().getFullYear(),
    currentMonth: new Date().getMonth() + 1,
    holidayEntries: [],
  }),

  getters: {
    eventsByDate: (state): Record<string, CalendarEvent[]> => {
      const map: Record<string, CalendarEvent[]> = {}
      for (const event of state.events) {
        if (!map[event.date]) map[event.date] = []
        map[event.date].push(event)
      }
      return map
    },
    isWorkday: (state) => {
      return (dateStr: string) => {
        // format: YYYY-MM-DD
        const entry = state.holidayEntries.find(h => h.date === dateStr)
        if (entry) {
          return entry.kind === 'workday'
        }
        // If not explicitly holiday/workday, fallback to weekend check
        const d = new Date(dateStr)
        const day = d.getDay()
        return day !== 0 && day !== 6
      }
    }
  },

  actions: {
    async loadHolidays(year: number): Promise<void> {
      const result = await casyContext.calendar.holidays(year)
      if (result.ok && result.data) {
        this.holidayEntries = result.data.entries || []
      }
    },

    async loadEvents(year?: number, month?: number): Promise<void> {
      this.loading = true
      if (year) this.currentYear = year
      if (month) this.currentMonth = month
      const p1 = casyContext.calendar.events(this.currentYear, this.currentMonth)
      const p2 = this.holidayEntries.length === 0 ? this.loadHolidays(this.currentYear) : Promise.resolve()
      
      const [result] = await Promise.all([p1, p2])
      if (result.ok && result.data) {
        this.events = result.data
      }
      this.loading = false
    },

    prevMonth(): void {
      if (this.currentMonth === 1) {
        this.currentYear--
        this.currentMonth = 12
      } else {
        this.currentMonth--
      }
      this.loadEvents()
    },

    nextMonth(): void {
      if (this.currentMonth === 12) {
        this.currentYear++
        this.currentMonth = 1
      } else {
        this.currentMonth++
      }
      this.loadEvents()
    },

    goToday(): void {
      this.currentYear = new Date().getFullYear()
      this.currentMonth = new Date().getMonth() + 1
      this.loadEvents()
    },
  },
})
