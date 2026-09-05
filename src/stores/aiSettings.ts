import { defineStore } from 'pinia'
import { ref } from 'vue'
import { tauriCallSafe } from '../core/tauriBridge'
import { casyContext } from '../core/plugin/context'
import type { AiProfiles } from '../types/aiProfiles'

export const useAiSettingsStore = defineStore('aiSettings', () => {
  const config = ref<AiProfiles>({ profiles: [], activeId: null, dailyLimit: 50, systemPrompt: '' })
  const loading = ref(false)
  const error = ref('')
  async function load() {
    loading.value = true
    const result = await tauriCallSafe('get_ai_profiles', {})
    loading.value = false
    if (!result.ok || !result.data) { error.value = result.error || '读取 AI 配置失败'; return false }
    config.value = result.data
    error.value = ''
    return true
  }
  async function save() {
    loading.value = true
    const result = await tauriCallSafe('save_ai_profiles', { config: config.value })
    loading.value = false
    if (!result.ok || !result.data) { error.value = result.error || '保存失败'; return false }
    config.value = result.data
    error.value = ''
    casyContext.replaceProviders(result.data.profiles.map(p => ({
      id: p.id, name: p.name, mode: p.mode, apiUrl: p.apiUrl,
      models: [{ id: p.model, name: p.model }],
    })))
    casyContext.emit('ai:configured', { activeId: result.data.activeId })
    return true
  }
  return { config, loading, error, load, save }
})
