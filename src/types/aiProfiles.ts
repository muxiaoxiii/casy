export interface AiProfile {
  id: string
  name: string
  mode: 'openai' | 'ollama'
  apiUrl: string
  model: string
  hasApiKey: boolean
  apiKey?: string | null
}

export interface AiProfiles {
  profiles: AiProfile[]
  activeId: string | null
  dailyLimit: number
  systemPrompt: string
}
