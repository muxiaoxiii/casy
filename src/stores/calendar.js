import { defineStore } from 'pinia'
import { tauriCallSafe } from '../core/tauriBridge.js'

export const useCalendarStore = defineStore('calendar', {
  state: () => ({
    events: [],
    loading: false,
    currentYear: new Date().getFullYear(),
    currentMonth: new Date().getMonth() + 1,
  }),

  getters: {
    eventsByDate: (state) => {
      const map = {}
      for (const event of state.events) {
        if (!map[event.date]) map[event.date] = []
        map[event.date].push(event)
      }
      return map
    },
  },

  actions: {
    async loadEvents(year, month) {
      this.loading = true
      if (year) this.currentYear = year
      if (month) this.currentMonth = month
      const result = await tauriCallSafe('get_calendar_events', {
        year: this.currentYear,
        month: this.currentMonth,
      })
      if (result.ok) {
        this.events = result.data || []
      }
      this.loading = false
    },

    prevMonth() {
      if (this.currentMonth === 1) {
        this.currentYear--
        this.currentMonth = 12
      } else {
        this.currentMonth--
      }
      this.loadEvents()
    },

    nextMonth() {
      if (this.currentMonth === 12) {
        this.currentYear++
        this.currentMonth = 1
      } else {
        this.currentMonth++
      }
      this.loadEvents()
    },

    goToday() {
      this.currentYear = new Date().getFullYear()
      this.currentMonth = new Date().getMonth() + 1
      this.loadEvents()
    },
  },
})
