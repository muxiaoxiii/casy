// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import FileConversionDialog from '../../src/shared/components/FileConversionDialog.vue'

const mocks = vi.hoisted(() => ({ listen: vi.fn(), call: vi.fn(), unlisten: vi.fn(), open: vi.fn() }))
vi.mock('../../src/core/tauriBridge', () => ({ tauriCallSafe: mocks.call }))
vi.mock('../../src/core/tauriEvents', () => ({ safeListen: mocks.listen }))
vi.mock('../../src/core/mockData', () => ({ isTauriRuntime: () => true }))
vi.mock('../../src/core/plugin/context', () => ({ casyContext: { files: { reveal: vi.fn() } } }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: mocks.open }))
vi.mock('element-plus', () => ({ ElMessage: { info: vi.fn(), warning: vi.fn() } }))
let view: VueWrapper | undefined
afterEach(() => { view?.unmount(); vi.clearAllMocks() })

it('subscribes before conversion, isolates job events, and displays page progress until the output is saved', async () => {
  let receive!: (event: { payload: Record<string, unknown> }) => void
  let finish!: (value: unknown) => void
  mocks.listen.mockImplementation(async (_, handler) => { receive = handler; return mocks.unlisten })
  mocks.call.mockImplementation(() => new Promise(resolve => { finish = resolve }))
  mocks.open.mockResolvedValue('/output')
  view = mount(FileConversionDialog, { props: { modelValue: true }, global: { stubs: {
    ElDialog: { template: '<div><slot/><slot name="footer"/></div>' },
    ElButton: { template: '<button><slot/></button>' }, ElIcon: true, ElProgress: true,
  } } })
  const button = (label: string) => view!.findAll('button').find(b => b.text() === label)!
  window.dispatchEvent(new CustomEvent('casy:file-drop', { detail: { paths: ['/scan.pdf'] } }))
  await button('输出目录').trigger('click'); await flushPromises()
  await button('转换为 Markdown').trigger('click'); await flushPromises()
  expect(mocks.listen.mock.invocationCallOrder[0]).toBeLessThan(mocks.call.mock.invocationCallOrder[0]!)
  const progress = { jobId: 'active', sourcePath: '/scan.pdf', phase: 'preparing', currentPage: 0, totalPages: 0, elapsedSeconds: 0, remainingSeconds: null }
  receive({ payload: progress })
  receive({ payload: { ...progress, sourcePath: '/canonical/scan.pdf', phase: 'recognizing', currentPage: 3, totalPages: 62, remainingSeconds: 240 } })
  await flushPromises()
  expect(view.text()).toContain('已识别 3 / 62 页')
  expect(view.text()).toContain('预计剩余 4 分 0 秒')
  receive({ payload: { ...progress, jobId: 'old-retry', phase: 'recognizing', currentPage: 50, totalPages: 62 } })
  await flushPromises()
  expect(view.text()).toContain('已识别 3 / 62 页')
  receive({ payload: { ...progress, phase: 'finalizing', currentPage: 62, totalPages: 62 } })
  await flushPromises()
  expect(view.text()).toContain('正在整理结果')
  expect(view.text()).not.toContain('已完成')
  finish({ ok: true, data: { outputPath: '/output/scan.md' } }); await flushPromises()
  expect(view.text()).toContain('已完成')
  expect(mocks.unlisten).toHaveBeenCalledOnce()
})
