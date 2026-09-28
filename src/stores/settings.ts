import { defineStore } from 'pinia'
import { casyContext } from '../core/plugin/context'

// 设置键与后端 settings 表一致（snake_case，见 ai/mod.rs load_ai_config）
// loading 是本地 UI 状态，不参与持久化
export const useSettingsStore = defineStore('settings', {
  state: () => ({
    theme: 'system',
    document_theme: 'legal',
    quote_sources: ['来源 1', '来源 2', '来源 3', '来源 4'],
    workspace_sync: { register: false, ocr: false, knowledge: false, embeddings: false, name: false, content_name: false },
    language: 'zh-CN',
    caseFolderBase: '',
    ai_mode: 'none',
    ai_backend: 'ollama',
    ai_api_url: 'http://localhost:11434',
    ai_api_key: '',
    ai_model: 'qwen2.5:14b',
    ai_daily_limit: 50,
    webdavUrl: '',
    webdavUsername: '',
    webdavPassword: '',
    webdavAutoSync: false,
    webdavPassword_configured: false,
    smtp_pass_configured: false,
    caldav_pass_configured: false,
    feishu_app_secret: '',
    feishu_app_secret_configured: false,
    clearedSecrets: [] as string[],
    smtp_host: '',
    smtp_port: 465,
    smtp_user: '',
    smtp_pass: '',
    caldav_url: '',
    caldav_user: '',
    caldav_pass: '',
    calendar_sync_enabled: false,
    calendar_mask_case_name: false,
    mcp_server_enabled: false,
    daily_brief_style: 'gazette',
    weekly_report_style: 'dossier',
    ai_tool_policy: { disabled: [] as string[], writeApproval: {} as Record<string, 'always_ask' | 'always_approve' | 'always_reject'> },
    loading: false,
  }),

  actions: {
    async load() {
      this.loading = true
      const result = await casyContext.settings.get()
      if (result.ok && result.data) {
        // 防止历史脏数据里的 loading 等键覆盖本地状态
        const { loading: _ignored, clearedSecrets: _ignoredSecrets, workspace_sync, quote_sources, ...settings } = result.data
        Object.assign(this, settings)
        if (workspace_sync && typeof workspace_sync === 'object' && !Array.isArray(workspace_sync)) Object.assign(this.workspace_sync, workspace_sync)
        if (Array.isArray(quote_sources) && quote_sources.length === 4 && quote_sources.every(v => typeof v === 'string')) this.quote_sources = quote_sources as string[]
      }
      this.loading = false
    },

    async save() {
      // 只发送设置键，剔除 loading 等本地状态
      const { loading, clearedSecrets, ...state } = this.$state
      const settings: Record<string, any> = {...state}
      for(const key of clearedSecrets)if(!settings[key])settings[key]=null
      const result = await casyContext.settings.save(settings)
      if(result.ok){this.webdavPassword='';this.smtp_pass='';this.caldav_pass='';this.clearedSecrets=[];await this.load()}
      return result
    },
    clearSecret(key: 'webdavPassword' | 'smtp_pass' | 'caldav_pass') {
      this[key]='';this[`${key}_configured`]=false
      if(!this.clearedSecrets.includes(key))this.clearedSecrets.push(key)
    },
  },
})
