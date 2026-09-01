import { defineStore } from 'pinia'
import { ref, watch } from 'vue'

export const useAiSettingsStore = defineStore('aiSettings', () => {
  const provider = ref('deepseek') // 'deepseek' | 'openai' | 'local'
  const apiKey = ref('')
  const baseUrl = ref('https://api.deepseek.com/v1')
  const model = ref('deepseek-chat')
  const systemPrompt = ref('你是一个专业的法律AI助手 (Casy Copilot)。')

  function load() {
    try {
      const saved = localStorage.getItem('casy_ai_settings')
      if (saved) {
        const parsed = JSON.parse(saved)
        provider.value = parsed.provider || 'deepseek'
        apiKey.value = parsed.apiKey || ''
        baseUrl.value = parsed.baseUrl || 'https://api.deepseek.com/v1'
        model.value = parsed.model || 'deepseek-chat'
        systemPrompt.value = parsed.systemPrompt || '你是一个专业的法律AI助手 (Casy Copilot)。'
      }
    } catch (e) {
      console.error('Failed to load AI settings', e)
    }
  }

  function save() {
    localStorage.setItem('casy_ai_settings', JSON.stringify({
      provider: provider.value,
      apiKey: apiKey.value,
      baseUrl: baseUrl.value,
      model: model.value,
      systemPrompt: systemPrompt.value
    }))
  }

  // Watch for changes and save automatically
  watch([provider, apiKey, baseUrl, model, systemPrompt], () => {
    save()
  }, { deep: true })

  return {
    provider,
    apiKey,
    baseUrl,
    model,
    systemPrompt,
    load,
    save
  }
})
