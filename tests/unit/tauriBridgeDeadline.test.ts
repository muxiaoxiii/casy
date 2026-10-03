import { afterEach, expect, it, vi } from 'vitest'
import { invokeWithDeadline } from '../../src/core/tauriBridge'

const mocks = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }))
vi.mock('element-plus', () => ({ ElMessage: { error: vi.fn() } }))
vi.mock('../../src/core/mockData', () => ({ isTauriRuntime: () => true, tryMockCommand: vi.fn() }))

afterEach(() => { vi.useRealTimers(); vi.resetAllMocks() })

it('keeps a conversion pending past three minutes until the backend saves its output', async () => {
  vi.useFakeTimers()
  let finish!: (result: unknown) => void
  mocks.invoke.mockImplementation(() => new Promise(resolve => { finish = resolve }))
  let status = 'running'
  const pending = invokeWithDeadline('convert_file_to_markdown', { sourcePath: '/long.pdf' })
  pending.then(() => { status = 'completed' }, () => { status = 'failed' })
  await vi.advanceTimersByTimeAsync(60 * 60 * 1000)
  expect(status).toBe('running')
  expect(mocks.invoke).toHaveBeenCalledOnce()
  finish({ outputPath: '/long.md' })
  await expect(pending).resolves.toEqual({ outputPath: '/long.md' })
  expect(status).toBe('completed')
})

it('reports the backend conversion error even after the former frontend deadline', async () => {
  vi.useFakeTimers()
  let fail!: (error: Error) => void
  mocks.invoke.mockImplementation(() => new Promise((_, reject) => { fail = reject }))
  const pending = invokeWithDeadline('convert_file_to_markdown', {})
  const assertion = expect(pending).rejects.toThrow('DOC_ENGINE_TIMEOUT')
  await vi.advanceTimersByTimeAsync(16 * 60 * 1000)
  fail(new Error('DOC_ENGINE_TIMEOUT: no page progress'))
  await assertion
})

it('retains deadlines for ordinary requests and explicit overrides', async () => {
  vi.useFakeTimers()
  mocks.invoke.mockImplementation(() => new Promise(() => {}))
  const ordinary = expect(invokeWithDeadline('get_settings', {})).rejects.toThrow('IPC_TIMEOUT')
  const explicit = expect(invokeWithDeadline('convert_file_to_markdown', {}, 500)).rejects.toThrow('IPC_TIMEOUT')
  await vi.advanceTimersByTimeAsync(60000)
  await Promise.all([ordinary, explicit])
  expect(vi.getTimerCount()).toBe(0)
})

for (const command of ['export_full_backup', 'import_full_backup', 'create_backup', 'restore_backup', 'webdav_push', 'webdav_backup_full', 'webdav_restore_full', 'optimize_document_storage', 'rollback_document_storage', 'correct_document_region']) {
  it(`waits for the actual result of ${command}`, async () => {
    vi.useFakeTimers()
    let finish!: () => void
    mocks.invoke.mockImplementation(() => new Promise<void>(resolve => { finish = resolve }))
    let settled = false
    const pending = invokeWithDeadline(command, {}).then(() => { settled = true })
    await vi.advanceTimersByTimeAsync(60 * 60 * 1000)
    expect(settled).toBe(false)
    finish(); await pending
    expect(settled).toBe(true)
  })
}
