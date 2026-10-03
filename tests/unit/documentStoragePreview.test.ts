// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { shallowMount, flushPromises } from '@vue/test-utils'
import WorkspaceDocumentPreview from '../../src/modules/knowledge/components/WorkspaceDocumentPreview.vue'

const mocks = vi.hoisted(() => ({ call: vi.fn(), confirm: vi.fn() }))
vi.mock('../../src/core/tauriBridge', () => ({ tauriCallSafe: mocks.call }))
vi.mock('../../src/core/plugin/context', () => ({ casyContext: { files: {}, knowledge: {} } }))
vi.mock('element-plus', () => ({ ElMessage: { error: vi.fn(), success: vi.fn() }, ElMessageBox: { confirm: mocks.confirm } }))
const source = { fileId: 'f', caseId: 'c', fileName: 'Evidence.pdf', filePath: '/evidence.pdf', jobId: 'upgraded', status: 'completed', totalPages: 1, error: null, missing: false }
const options = {
  props: { source },
  global: {
    directives: { loading: {} },
    stubs: { ElButton: { template: '<button><slot /></button>' }, ElAlert: { props: ['title'], template: '<p>{{ title }}</p>' } },
  },
}
afterEach(() => vi.resetAllMocks())

it('recovers rollback after reopening even when legacy content exceeds the preview limit', async () => {
  mocks.call.mockImplementation(async (command: string) => command === 'get_document_storage_state'
    ? { ok: true, data: { jobId: 'upgraded', externalImages: true, canOptimize: false, canRollback: true } }
    : { ok: false, error: '正文超过 16 MiB' })
  const first = shallowMount(WorkspaceDocumentPreview, options)
  await flushPromises()
  expect(first.text()).toContain('回退存储升级')
  expect(first.text()).not.toContain('尚未提取正文')
  first.unmount()
  const reopened = shallowMount(WorkspaceDocumentPreview, options)
  await flushPromises()
  await reopened.findAll('button').find(button => button.text() === '回退存储升级')!.trigger('click')
  await flushPromises()
  expect(mocks.call).toHaveBeenCalledWith('rollback_document_storage', { fileId: 'f', jobId: 'upgraded' })
  reopened.unmount()
})

it('does not optimize a different document if the selection changes during confirmation', async () => {
  let confirm!: () => void
  mocks.confirm.mockImplementation(() => new Promise<void>(resolve => { confirm = resolve }))
  mocks.call.mockImplementation(async (command: string, args: { fileId: string }) => command === 'get_document_storage_state'
    ? { ok: true, data: { jobId: args.fileId + '-old', externalImages: false, canOptimize: true, canRollback: false } }
    : { ok: false, error: '正文超过 16 MiB' })
  const view = shallowMount(WorkspaceDocumentPreview, options)
  await flushPromises()
  await view.findAll('button').find(button => button.text().startsWith('优化存储'))!.trigger('click')
  await view.setProps({ source: { ...source, fileId: 'other', jobId: 'other-old' } })
  await flushPromises()
  confirm()
  await flushPromises()
  expect(mocks.call.mock.calls.some(([command]) => command === 'optimize_document_storage')).toBe(false)
  view.unmount()
})
