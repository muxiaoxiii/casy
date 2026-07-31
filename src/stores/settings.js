import { defineStore } from 'pinia'
import { tauriCallSafe } from '../core/tauriBridge.js'

export const useSettingsStore = defineStore('settings', {
  state: () => ({
    // WebDAV
    webdavUrl: '',
    webdavUsername: '',
    webdavPassword: '',
    webdavAutoSync: true,

    // 飞书
    feishuAppToken: '',
    feishuTableIds: {},
    feishuApiKey: '',

    // AI
    aiMode: 'none',        // 'none' | 'local' | 'remote'
    aiBackend: 'ollama',    // 'ollama' | 'openai' | 'custom'
    aiApiUrl: 'http://localhost:11434',
    aiApiKey: '',
    aiModel: 'qwen2.5:14b',
    aiDailyLimit: 50,
    ocrEngine: 'tesseract', // 'tesseract' | 'vision_llm'

    // IMAP
    imapAccounts: [],

    // 通用
    caseFolderBase: '',
    theme: 'system',
    language: 'zh-CN',

    // 加载状态
    loading: false,
    saving: false,
  }),

  actions: {
    async load() {
      this.loading = true
      const result = await tauriCallSafe('get_settings', {})
      if (result.ok && result.data) {
        // 只更新后端返回的字段，保留默认值
        const allowedKeys = Object.keys(this.$state)
        for (const [key, value] of Object.entries(result.data)) {
          if (allowedKeys.includes(key) && key !== 'loading' && key !== 'saving') {
            this[key] = value
          }
        }
      }
      this.loading = false
    },

    async save() {
      this.saving = true
      // 提取纯配置数据，排除 UI 状态
      const { loading, saving, ...settings } = this.$state
      const result = await tauriCallSafe('save_settings', { settings })
      this.saving = false
      return result
    },
  },
})
