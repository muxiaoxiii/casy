import { defineStore } from 'pinia'
import { tauriCallSafe } from '../core/tauriBridge'

// 设置键与后端 settings 表一致（snake_case，见 ai/mod.rs load_ai_config）
// loading 是本地 UI 状态，不参与持久化
export const useSettingsStore = defineStore('settings', {
  state: () => ({
    ai_mode: 'none',
    ai_backend: 'ollama',
    ai_api_url: 'http://localhost:11434',
    ai_api_key: '',
    ai_model: 'qwen2.5:14b',
    ai_daily_limit: 50,
    loading: false,
  }),

  actions: {
    async load() {
      this.loading = true
      const result = await tauriCallSafe('get_settings', {})
      if (result.ok && result.data) {
        // 防止历史脏数据里的 loading 等键覆盖本地状态
        const { loading: _ignored, ...settings } = result.data
        Object.assign(this, settings)
      }
      this.loading = false
    },

    async save() {
      // 只发送设置键，剔除 loading 等本地状态
      const { loading, ...settings } = this.$state
      const result = await tauriCallSafe('save_settings', { settings })
      return result
    },
  },
})
