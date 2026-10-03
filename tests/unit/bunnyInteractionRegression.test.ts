import { beforeEach, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useCasesStore } from '../../src/stores/cases'
import { tauriCallSafe } from '../../src/core/tauriBridge'
const mocks = vi.hoisted(() => ({ list: vi.fn(), invoke: vi.fn() }))
vi.mock('../../src/core/plugin/context', () => ({ casyContext: { cases: { list: mocks.list } } }))
vi.mock('../../src/core/autoPush', () => ({ notifyDataChange: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }))
vi.mock('../../src/core/mockData', () => ({ isTauriRuntime: () => true, tryMockCommand: vi.fn() }))
vi.mock('element-plus', () => ({ ElMessage: { error: vi.fn() } }))
beforeEach(() => { setActivePinia(createPinia()); vi.clearAllMocks() })
it('changing filters resets pagination and fetches the new matching set', async () => {
  mocks.list.mockResolvedValue({ ok: true, data: { items: [{ id: 'matched' }], total: 1 } })
  const store = useCasesStore(); store.page = 4
  await store.setFilter({ track: 'civil_tort', sortBy: 'case_name' })
  expect(mocks.list).toHaveBeenCalledWith(expect.objectContaining({ track: 'civil_tort', sortBy: 'case_name', page: 1 }))
  expect(store.cases[0].id).toBe('matched'); expect(store.total).toBe(1)
})
it.each(['defer_task', 'clear_task_defer'] as const)('%s distinguishes native null success from failure', async command => {
  mocks.invoke.mockResolvedValueOnce(null).mockRejectedValueOnce(new Error('database locked'))
  const args = { taskId: 't', until: '2026-10-12' }
  expect((await tauriCallSafe(command, args)).ok).toBe(true)
  const failed = await tauriCallSafe(command, args)
  expect(failed.ok).toBe(false); expect(failed.error).toContain('database locked')
})
