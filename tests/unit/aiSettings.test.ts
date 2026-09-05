// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useAiSettingsStore } from '../../src/stores/aiSettings'

const { invoke, replaceProviders } = vi.hoisted(() => ({ invoke: vi.fn(), replaceProviders: vi.fn() }))
vi.mock('../../src/core/tauriBridge', () => ({ tauriCallSafe: invoke }))
vi.mock('../../src/core/plugin/context', () => ({ casyContext: { replaceProviders, emit: vi.fn() } }))

beforeEach(() => { setActivePinia(createPinia()); vi.clearAllMocks(); localStorage.clear() })
describe('Native AI configuration', () => {
  it('loads saved native profiles without using browser credentials', async () => {
    localStorage.setItem('casy_ai_settings', JSON.stringify({ apiKey: 'old-browser-key' }))
    const config = { profiles: [], activeId: null, dailyLimit: 80, systemPrompt: 'Native prompt' }
    invoke.mockResolvedValue({ ok: true, data: config })
    const store = useAiSettingsStore()
    expect(await store.load()).toBe(true)
    expect(invoke).toHaveBeenCalledWith('get_ai_profiles', {})
    expect(store.config.systemPrompt).toBe('Native prompt')
    expect(localStorage.getItem('casy_ai_settings')).toContain('old-browser-key')
  })
  it('retains an unsaved configuration when the backend rejects a save', async () => {
    const store = useAiSettingsStore()
    store.config.profiles.push({ id: 'one', name: 'One', mode: 'openai', apiUrl: 'https://example.com/v1', model: 'one', apiKey: 'unsaved-key', hasApiKey: false })
    invoke.mockResolvedValue({ ok: false, error: 'Keychain unavailable' })
    expect(await store.save()).toBe(false)
    expect(store.config.profiles[0].apiKey).toBe('unsaved-key')
    expect(store.error).toBe('Keychain unavailable')
    expect(replaceProviders).not.toHaveBeenCalled()
    expect(localStorage.length).toBe(0)
  })
  it('clears entered keys after successful save and refreshes selectable models', async () => {
    const store = useAiSettingsStore()
    const profile = { id: 'one', name: 'One', mode: 'openai' as const, apiUrl: 'https://example.com/v1', model: 'custom', hasApiKey: true }
    store.config.profiles.push({ ...profile, apiKey: 'synthetic-key' })
    invoke.mockResolvedValue({ ok: true, data: { ...store.config, profiles: [profile] } })
    expect(await store.save()).toBe(true)
    expect(store.config.profiles[0].apiKey).toBeUndefined()
    expect(replaceProviders).toHaveBeenCalledWith([{ id: 'one', name: 'One', mode: 'openai', apiUrl: 'https://example.com/v1', models: [{ id: 'custom', name: 'custom' }] }])
    expect(localStorage.length).toBe(0)
  })
})
