import { defineStore } from 'pinia'
import { tauriCallSafe } from '../core/tauriBridge'
import { notifyDataChange } from '../core/autoPush'
import type {
  Case,
  CaseFilter,
  CaseListResponse,
  CaseStats,
  DashboardStats,
  CreateCaseInput,
  UpdateCaseInput,
  CaseStatus,
  TrackType,
  CivilStatus,
  InvalidationStatus,
  AdminStatus,
  CaseRoute,
} from '../types'

interface CasesState {
  cases: Case[]
  currentCase: Case | null
  loading: boolean
  total: number
  page: number
  perPage: number
  filter: CaseFilter
  stats: CaseStats
  dashboard: DashboardStats
}

export const useCasesStore = defineStore('cases', {
  state: (): CasesState => ({
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
      civilStatus: null,
      invalidationStatus: null,
      adminStatus: null,
      caseRoute: null,
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
    activeCases: (state): Case[] =>
      state.cases.filter((c) => c.caseStatus !== '已完结'),

    groupedByClient: (state): Record<string, Case[]> => {
      const groups: Record<string, Case[]> = {}
      for (const c of state.cases) {
        const key = c.clientName || '未知客户'
        if (!groups[key]) groups[key] = []
        groups[key].push(c)
      }
      return groups
    },

    groupedByTrack: (state): Record<string, Case[]> => {
      const trackLabels: Record<TrackType, string> = {
        patent_invalidation: '专利无效',
        admin_litigation: '行政诉讼',
        civil_tort: '民事侵权',
        other: '其他',
      }
      const groups: Record<string, Case[]> = {}
      for (const c of state.cases) {
        const key = trackLabels[c.track] || c.track || '其他'
        if (!groups[key]) groups[key] = []
        groups[key].push(c)
      }
      return groups
    },

    /** 按轨道路由分组（新状态机） */
    groupedByRoute: (state): Record<string, Case[]> => {
      const groups: Record<string, Case[]> = {}
      for (const c of state.cases) {
        const key = c.caseRoute || '民事诉讼'
        if (!groups[key]) groups[key] = []
        groups[key].push(c)
      }
      return groups
    },
  },

  actions: {
    async loadCases(): Promise<void> {
      this.loading = true
      const result = await tauriCallSafe<CaseListResponse>('list_cases', {
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
          // 新状态机筛选
          civilStatus: this.filter.civilStatus || null,
          invalidationStatus: this.filter.invalidationStatus || null,
          adminStatus: this.filter.adminStatus || null,
          caseRoute: this.filter.caseRoute || null,
        },
      })
      if (result.ok && result.data) {
        this.cases = result.data.items || []
        this.total = result.data.total || 0
      }
      this.loading = false
    },

    async loadCase(id: string): Promise<ReturnType<typeof tauriCallSafe<Case>>> {
      const result = await tauriCallSafe<Case>('get_case', { id })
      if (result.ok && result.data) {
        this.currentCase = result.data
      }
      return result
    },

    async createCase(data: CreateCaseInput): Promise<ReturnType<typeof tauriCallSafe<Case>>> {
      const result = await tauriCallSafe<Case>('create_case', { data })
      if (result.ok) {
        await this.loadCases()
        await this.loadStats()
        notifyDataChange()
      }
      return result
    },

    async updateCase(id: string, data: UpdateCaseInput): Promise<ReturnType<typeof tauriCallSafe<Case>>> {
      const result = await tauriCallSafe<Case>('update_case', { id, data })
      if (result.ok && result.data) {
        const idx = this.cases.findIndex((c) => c.id === id)
        if (idx >= 0) this.cases[idx] = { ...this.cases[idx], ...result.data }
        if (this.currentCase?.id === id) {
          this.currentCase = { ...this.currentCase, ...result.data }
        }
        notifyDataChange()
      }
      return result
    },

    async deleteCase(id: string): Promise<ReturnType<typeof tauriCallSafe<void>>> {
      const result = await tauriCallSafe<void>('delete_case', { id })
      if (result.ok) {
        await this.loadCases()
        await this.loadStats()
        notifyDataChange()
      }
      return result
    },

    async searchCases(query: string): Promise<ReturnType<typeof tauriCallSafe<Case[]>>> {
      const result = await tauriCallSafe<Case[]>('search_cases', { query })
      if (result.ok && result.data) {
        this.cases = result.data
      }
      return result
    },

    async loadStats(): Promise<void> {
      const result = await tauriCallSafe<CaseStats>('case_stats')
      if (result.ok && result.data) {
        this.stats = result.data
      }
    },

    async loadDashboard(): Promise<void> {
      const result = await tauriCallSafe<DashboardStats>('get_dashboard_stats')
      if (result.ok && result.data) {
        this.dashboard = result.data
      }
    },
  },
})
