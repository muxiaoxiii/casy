import { defineStore } from 'pinia'
import { ref } from 'vue'
import { tauriCallSafe } from '../core/tauriBridge'
import { casyContext } from '../core/plugin/context'
import type { AiProfiles } from '../types/aiProfiles'

export const useAiSettingsStore = defineStore('aiSettings', () => {
  const config = ref<AiProfiles>({ profiles: [], activeId: null, dailyLimit: 50, systemPrompt: '' })
  const loading = ref(false)
  const error = ref('')
  const loaded = ref(false)
  async function load() {
    if (loading.value) return false
    loading.value = true
    try {
      const result = await tauriCallSafe('get_ai_profiles', {})
      if (!result.ok || !result.data) throw new Error(result.error || '读取 AI 配置失败')
      config.value = result.data
      loaded.value = true
      error.value = ''
      return true
    } catch (cause) { error.value = String(cause); return false }
    finally { loading.value = false }
  }
  async function save() {
    if (!loaded.value) { error.value = '请先读取 AI 配置，再保存修改'; return false }
    if (loading.value) return false
    loading.value = true
    const snapshot = JSON.parse(JSON.stringify(config.value))
    try {
      const result = await tauriCallSafe('save_ai_profiles', { config: snapshot })
      if (!result.ok || !result.data) throw new Error(result.error || '保存失败')
      config.value = result.data
      loaded.value = true
      error.value = ''
      casyContext.replaceProviders(result.data.profiles.map(p => ({
        id: p.id, name: p.name, mode: p.mode, apiUrl: p.apiUrl,
        models: [{ id: p.model, name: p.model }],
      })))
      casyContext.emit('ai:configured', { activeId: result.data.activeId })
      return true
    } catch (cause) { error.value = String(cause); return false }
    finally { loading.value = false }
  }
  return { config, loading, loaded, error, load, save }
})
