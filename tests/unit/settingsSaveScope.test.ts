import { beforeEach, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useSettingsStore } from '../../src/stores/settings'
const api = vi.hoisted(() => ({ get: vi.fn(), save: vi.fn() }))
vi.mock('../../src/core/plugin/context', () => ({ casyContext: { settings: api } }))
beforeEach(() => { setActivePinia(createPinia()); vi.resetAllMocks(); api.get.mockResolvedValue({ok:true,data:{theme:'system',smtp_host:'saved-host'}}) })
it('saves one group without persisting or erasing another group', async () => {
  const store = useSettingsStore(); await store.load()
  store.theme = 'dark'; store.smtp_host = 'unsaved-host'
  api.save.mockResolvedValue({ok:true}); await store.save(['theme'])
  expect(api.save).toHaveBeenCalledWith({theme:'dark'})
  expect(store.isDirty(['theme'])).toBe(false)
  expect(store.isDirty(['smtp_host'])).toBe(true)
  store.discard(); expect(store.smtp_host).toBe('saved-host'); expect(store.theme).toBe('dark')
})
it('preserves input made while a save is pending and rejects duplicate writes', async () => {
  const store = useSettingsStore(); await store.load(); store.smtp_host='first'
  let finish!: (value: unknown) => void
  api.save.mockImplementation(() => new Promise(resolve => { finish=resolve }))
  const pending=store.save(['smtp_host']); store.smtp_host='second'
  expect((await store.save(['smtp_host'])).ok).toBe(false)
  finish({ok:true}); await pending
  expect(store.smtp_host).toBe('second'); expect(store.isDirty(['smtp_host'])).toBe(true)
  store.discard(); expect(store.smtp_host).toBe('first')
})
it('does not save defaults after load failure and releases locks after exceptions', async () => {
  const store=useSettingsStore(); api.get.mockRejectedValue(new Error('offline')); await store.load()
  expect(store.loading).toBe(false); expect((await store.save(['theme'])).ok).toBe(false)
  expect(api.save).not.toHaveBeenCalled()
})
