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
  mocks.listen.mockImplementation((_, handler) => { receive = handler; return Object.assign(mocks.unlisten,{ready:Promise.resolve()}) })
  mocks.call.mockImplementation((command) => command === 'register_conversion_batch' ? Promise.resolve({ok:true,data:['active']}) : new Promise(resolve => { finish = resolve }))
  mocks.open.mockResolvedValue('/output')
  view = mount(FileConversionDialog, { props: { modelValue: true }, global: { stubs: {
    ElDialog: { template: '<div><slot/><slot name="footer"/></div>' },
    ElButton: { template: '<button><slot/></button>' }, ElIcon: true, ElProgress: true,
  } } })
  const button = (label: string) => view!.findAll('button').find(b => b.text() === label)!
  window.dispatchEvent(new CustomEvent('casy:file-drop', { detail: { paths: ['/scan.pdf'] } }))
  await button('输出目录').trigger('click'); await flushPromises()
  await button('转换为 Markdown').trigger('click'); await flushPromises()
  expect(mocks.listen.mock.invocationCallOrder[0]).toBeLessThan(mocks.call.mock.invocationCallOrder[1]!)
  const progress = { jobId: 'active', sourcePath: '/scan.pdf', phase: 'preparing', currentPage: 0, totalPages: 0, elapsedSeconds: 0, remainingSeconds: null }
  receive({ payload: progress })
  receive({ payload: { ...progress, sourcePath: '/canonical/scan.pdf', phase: 'recognizing', currentPage: 3, totalPages: 62, elapsedSeconds: 20, remainingSeconds: 240, pageTiming:{renderMs:500,ocrMs:2500,layoutMs:1000,totalMs:4200} } })
  await flushPromises()
  expect(view.text()).toContain('已识别 3 / 62 页')
  expect(view.text()).toContain('预计剩余 4 分 0 秒')
  expect(view.text()).toContain('最近一页：渲染 1 秒 · 文字 3 秒 · 版面 1 秒')
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

it('registers the batch, hides while running, and cancels the current task plus the remainder', async () => {
  let finish!: (value: unknown) => void
  mocks.listen.mockReturnValue(Object.assign(mocks.unlisten,{ready:Promise.resolve()}))
  mocks.call.mockImplementation(command => {
    if (command === 'register_conversion_batch') return Promise.resolve({ok:true,data:['first','second']})
    if (command === 'cancel_queued_conversions' || command === 'cancel_conversion') return Promise.resolve({ok:true})
    return new Promise(resolve => { finish=resolve })
  })
  mocks.open.mockResolvedValue('/output')
  view=mount(FileConversionDialog,{props:{modelValue:true},global:{stubs:{ElDialog:{template:'<div><slot/><slot name="footer"/></div>'},ElButton:{template:'<button><slot/></button>'},ElRadioGroup:true,ElRadioButton:true,ElIcon:true,ElProgress:true}}})
  const button=(label:string)=>view!.findAll('button').find(b=>b.text()===label)!
  window.dispatchEvent(new CustomEvent('casy:file-drop',{detail:{paths:['/first.pdf','/second.pdf']}}))
  await button('输出目录').trigger('click');await flushPromises()
  await button('转换为 Markdown').trigger('click');await flushPromises()
  expect(mocks.call).toHaveBeenCalledWith('register_conversion_batch',{sourcePaths:['/first.pdf','/second.pdf']})
  expect(mocks.call).toHaveBeenCalledWith('convert_file_to_markdown',{sourcePath:'/first.pdf',outputDir:'/output',jobId:'first',targetFormat:'markdown'})
  await button('收起到后台').trigger('click');expect(view.emitted('update:modelValue')).toEqual([[false]])
  await button('取消当前并停止').trigger('click');await flushPromises()
  expect(mocks.call).toHaveBeenCalledWith('cancel_conversion',{jobId:'first'})
  finish({ok:false,error:'CONVERSION_CANCELLED: 转换已取消'});await flushPromises()
  expect(view.text()).toContain('已取消')
  expect(mocks.call).toHaveBeenCalledWith('cancel_queued_conversions',{jobIds:['second']})
  expect(mocks.call.mock.calls.filter(([command])=>command==='convert_file_to_markdown')).toHaveLength(1)
})
