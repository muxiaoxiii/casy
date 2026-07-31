import { defineStore } from 'pinia'
import { tauriCallSafe } from '../core/tauriBridge.js'
import { notifyDataChange } from '../core/autoPush.js'

export const useCasesStore = defineStore('cases', {
  state: () => ({
    cases: [],
    currentCase: null,
    loading: false,
    total: 0,
    page: 1,
    perPage: 50,
    filter: {
      track: null,
      client: null,
      court: null,
      status: null,
      search: '',
      sortBy: 'filing_date',
      dateFrom: null,
      dateTo: null,
    },
    stats: {
      total: 0,
      active: 0,
      closed: 0,
      byTrack: [],
      byClient: [],
    },
    dashboard: {
      activeCount: 0,
      totalCount: 0,
      closedCount: 0,
      deadlineWarnings: [],
      recentActivities: [],
      byTrack: [],
    },
  }),

  getters: {
    activeCases: (state) => state.cases.filter((c) => c.caseStatus !== '已完结'),

    groupedByClient: (state) => {
      const groups = {}
      for (const c of state.cases) {
        const key = c.clientName || '未知客户'
        if (!groups[key]) groups[key] = []
        groups[key].push(c)
      }
      return groups
    },

    groupedByTrack: (state) => {
      const trackLabels = {
        patent_invalidation: '专利无效',
        admin_litigation: '行政诉讼',
        civil_tort: '民事侵权',
        other: '其他',
      }
      const groups = {}
      for (const c of state.cases) {
        const key = trackLabels[c.track] || c.track || '其他'
        if (!groups[key]) groups[key] = []
        groups[key].push(c)
      }
      return groups
    },
  },

  actions: {
    async loadCases() {
      this.loading = true
      const result = await tauriCallSafe('list_cases', {
        filter: {
          track: this.filter.track || null,
          client: this.filter.client || null,
          court: this.filter.court || null,
          status: this.filter.status || null,
          search: this.filter.search || null,
          sortBy: this.filter.sortBy,
          dateFrom: this.filter.dateFrom || null,
          dateTo: this.filter.dateTo || null,
          page: this.page,
          perPage: this.perPage,
        },
      })
      if (result.ok) {
        this.cases = result.data.items || []
        this.total = result.data.total || 0
      }
      this.loading = false
    },

    async loadCase(id) {
      const result = await tauriCallSafe('get_case', { id })
      if (result.ok) {
        this.currentCase = result.data
      }
      return result
    },

    async createCase(data) {
      const result = await tauriCallSafe('create_case', { data })
      if (result.ok) {
        await this.loadCases()
        await this.loadStats()
        notifyDataChange() // 触发飞书自动推送
      }
      return result
    },

    async updateCase(id, data) {
      const result = await tauriCallSafe('update_case', { id, data })
      if (result.ok) {
        const idx = this.cases.findIndex((c) => c.id === id)
        if (idx >= 0) this.cases[idx] = { ...this.cases[idx], ...result.data }
        if (this.currentCase?.id === id) {
          this.currentCase = { ...this.currentCase, ...result.data }
        }
        notifyDataChange() // 触发飞书自动推送
      }
      return result
    },

    async deleteCase(id) {
      const result = await tauriCallSafe('delete_case', { id })
      if (result.ok) {
        await this.loadCases()
        await this.loadStats()
        notifyDataChange() // 触发飞书自动推送
      }
      return result
    },

    async searchCases(query) {
      const result = await tauriCallSafe('search_cases', { query })
      if (result.ok) {
        this.cases = result.data || []
      }
      return result
    },

    async loadStats() {
      const result = await tauriCallSafe('case_stats')
      if (result.ok) {
        this.stats = result.data
      }
    },

    async loadDashboard() {
      const result = await tauriCallSafe('get_dashboard_stats')
      if (result.ok) {
        this.dashboard = result.data
      }
    },
  },
})
