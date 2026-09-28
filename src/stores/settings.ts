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
    loaded: false,
    savingCount: 0,
    loadError: '',
    persisted: {} as Record<string, any>,
  }),

  actions: {
    values() {
      const { loading, loaded, loadError, persisted, clearedSecrets, savingCount, ...settings } = this.$state
      return JSON.parse(JSON.stringify(settings)) as Record<string, any>
    },
    isDirty(keys?: string[]) {
      if (!this.loaded) return false
      const values = this.values()
      return (keys || Object.keys(values)).some(key => JSON.stringify(values[key]) !== JSON.stringify(this.persisted[key]) || this.clearedSecrets.includes(key))
    },
    discard(keys?: string[]) {
      const selected = keys || Object.keys(this.persisted)
      for (const key of selected) (this as any)[key] = JSON.parse(JSON.stringify(this.persisted[key] ?? ''))
      this.clearedSecrets = this.clearedSecrets.filter(key => !selected.includes(key))
    },
    async load() {
      if (this.loading) return
      this.loading = true
      try {
        const result = await casyContext.settings.get()
        if (!result.ok || !result.data) throw new Error(result.error || '设置加载失败')
        const values = this.values()
        const data = { ...result.data }
        if (data.workspace_sync && typeof data.workspace_sync === 'object' && !Array.isArray(data.workspace_sync)) data.workspace_sync = { ...this.workspace_sync, ...data.workspace_sync }
        else delete data.workspace_sync
        if (!Array.isArray(data.quote_sources) || data.quote_sources.length !== 4 || !data.quote_sources.every((value: unknown) => typeof value === 'string')) delete data.quote_sources
        for (const key of Object.keys(values)) {
          if (!(key in data)) continue
          // Refresh persisted values without overwriting an open unsaved form.
          if (!this.loaded || !this.isDirty([key])) (this as any)[key] = data[key]
          this.persisted[key] = JSON.parse(JSON.stringify(data[key]))
        }
        for (const key of Object.keys(values)) if (!(key in this.persisted)) this.persisted[key] = values[key]
        this.loaded = true
        this.loadError = ''
      } catch (error) { this.loadError = String(error) }
      finally { this.loading = false }
    },
    async save(keys?: string[]) {
      if (!this.loaded) return { ok: false, error: '设置尚未读取成功，请重试后再保存' }
      if (this.savingCount) return { ok: false, error: '另一组设置正在保存，请稍后重试' }
      this.savingCount++
      try {
      const values = this.values()
      const selected = (keys || Object.keys(values)).filter(key => key in values && !key.endsWith('_configured'))
      const snapshot = Object.fromEntries(selected.map(key => [key, values[key]]))
      const settings = { ...snapshot }
      for (const key of this.clearedSecrets) if (selected.includes(key) && !settings[key]) settings[key] = null
      const result = await casyContext.settings.save(settings)
      if (result.ok) {
        for (const key of selected) {
          const secret = ['webdavPassword', 'smtp_pass', 'caldav_pass', 'feishu_app_secret'].includes(key)
          const saved = secret ? '' : snapshot[key]
          this.persisted[key] = saved
          if (JSON.stringify((this as any)[key]) === JSON.stringify(snapshot[key])) (this as any)[key] = saved
          if (secret && (snapshot[key] || settings[key] === null)) {
            const flag = `${key}_configured`
            ;(this as any)[flag] = settings[key] !== null
            this.persisted[flag] = (this as any)[flag]
          }
        }
        this.clearedSecrets = this.clearedSecrets.filter(key => !selected.includes(key))
      }
      return result
      } catch (error) { return { ok: false, error: String(error) } }
      finally { this.savingCount-- }
    },
    clearSecret(key: 'webdavPassword' | 'smtp_pass' | 'caldav_pass') {
      this[key]='';this[`${key}_configured`]=false
      if(!this.clearedSecrets.includes(key))this.clearedSecrets.push(key)
    },
  },
})
