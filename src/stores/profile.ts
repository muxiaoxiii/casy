import { defineStore } from 'pinia'
import { tauriCallSafe } from '../core/tauriBridge'

// ============================================================
// 律师画像（get_lawyer_profile / save_lawyer_profile）
// state 字段与后端 snake_case 契约对齐
// ============================================================
export const useProfileStore = defineStore('profile', {
  state: () => ({
    name: '',
    practice_areas: [],
    common_case_types: [],
    work_hours: { start_hour: 9, end_hour: 18 },
    reminder_channels: [],
    onboarding_completed: false,
    loaded: false,
  }),

  getters: {
    onboardingCompleted: (state) => !!state.onboarding_completed,
  },

  actions: {
    /** 后端可能返回 snake_case 或 camelCase，做兼容映射 */
    _apply(data) {
      if (!data) return
      this.name = data.name ?? ''
      this.practice_areas = data.practice_areas ?? data.practiceAreas ?? []
      this.common_case_types = data.common_case_types ?? data.commonCaseTypes ?? []
      const hours = data.work_hours ?? data.workHours ?? {}
      this.work_hours = {
        start_hour: hours.start_hour ?? hours.startHour ?? 9,
        end_hour: hours.end_hour ?? hours.endHour ?? 18,
      }
      this.reminder_channels = data.reminder_channels ?? data.reminderChannels ?? []
      this.onboarding_completed = !!(data.onboarding_completed ?? data.onboardingCompleted)
    },

    async load() {
      const result = await tauriCallSafe('get_lawyer_profile', {})
      if (result.ok && result.data) {
        this._apply(result.data)
      }
      this.loaded = true
      return result
    },

    async save(profile) {
      const result = await tauriCallSafe('save_lawyer_profile', { profile })
      if (result.ok) {
        this._apply(profile)
      }
      return result
    },
  },
})
