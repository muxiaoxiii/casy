import { defineStore } from 'pinia'
import { casyContext } from '../core/plugin/context'

// ============================================================
// 律师画像（get_lawyer_profile / save_lawyer_profile）
// state 字段与后端 snake_case 契约对齐
// ============================================================

/** 后端画像载荷（snake_case / camelCase 双兼容，字段均可选） */
type ProfilePayload = {
  name?: string
  practice_areas?: string[]
  practiceAreas?: string[]
  common_case_types?: string[]
  commonCaseTypes?: string[]
  work_hours?: { start_hour?: number; end_hour?: number; startHour?: number; endHour?: number }
  workHours?: { start_hour?: number; end_hour?: number; startHour?: number; endHour?: number }
  reminder_channels?: string[]
  reminderChannels?: string[]
  onboarding_completed?: boolean
  onboardingCompleted?: boolean
}

interface ProfileState {
  name: string
  practice_areas: string[]
  common_case_types: string[]
  work_hours: { start_hour: number; end_hour: number }
  reminder_channels: string[]
  onboarding_completed: boolean
  loaded: boolean
}

export const useProfileStore = defineStore('profile', {
  state: (): ProfileState => ({
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
    _apply(data: ProfilePayload | null | undefined) {
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
      const result = await casyContext.settings.profile()
      if (result.ok && result.data) {
        this._apply(result.data as ProfilePayload)
      }
      this.loaded = true
      return result
    },

    async save(profile: ProfilePayload) {
      const result = await casyContext.settings.saveProfile({ ...profile })
      if (result.ok) {
        this._apply(profile)
      }
      return result
    },
  },
})
