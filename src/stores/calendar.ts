import { defineStore } from 'pinia'
import { tauriCallSafe } from '../core/tauriBridge'
import type { CalendarEvent } from '../types'

interface CalendarState {
  events: CalendarEvent[]
  loading: boolean
  currentYear: number
  currentMonth: number
}

export const useCalendarStore = defineStore('calendar', {
  state: (): CalendarState => ({
    events: [],
    loading: false,
    currentYear: new Date().getFullYear(),
    currentMonth: new Date().getMonth() + 1,
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
  },

  actions: {
    async loadEvents(year?: number, month?: number): Promise<void> {
      this.loading = true
      if (year) this.currentYear = year
      if (month) this.currentMonth = month
      const result = await tauriCallSafe<CalendarEvent[]>('get_calendar_events', {
        year: this.currentYear,
        month: this.currentMonth,
      })
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
