// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, shallowMount, type VueWrapper } from '@vue/test-utils'
import KnowledgeNotebookView from '../../src/modules/knowledge/views/KnowledgeNotebookView.vue'

const mocks = vi.hoisted(() => ({
  list: vi.fn(), get: vi.fn(), update: vi.fn(), register: vi.fn(), close: vi.fn(), stop: vi.fn(), error: vi.fn(),
}))
vi.mock('vue-router', () => ({ useRoute: () => ({ query: {} }), useRouter: () => ({ push: vi.fn() }), onBeforeRouteLeave: vi.fn() }))
vi.mock('element-plus', () => ({ ElMessage: { error: mocks.error, success: vi.fn() }, ElMessageBox: { confirm: vi.fn() } }))
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => ({ onCloseRequested: mocks.register, close: mocks.close }) }))
vi.mock('../../src/core/plugin/context', () => ({ casyContext: {
  on: () => vi.fn(),
  knowledge: { list: mocks.list, getWithBlocks: mocks.get, update: mocks.update },
  cases: { list: async () => ({ ok: true, data: [] }) },
} }))

let wrapper: VueWrapper | undefined
let closeRequested: (event: { preventDefault: () => void }) => Promise<void>
const note = { id: 'a', title: '原始标题', content: '原始正文', category: 'reference' }
beforeEach(() => {
  vi.clearAllMocks()
  Object.assign(window, { __TAURI_INTERNALS__: { metadata: { currentWindow: { label: 'main' } } } })
  mocks.list.mockResolvedValue({ ok: true, data: [note] })
  mocks.get.mockResolvedValue({ ok: true, data: { item: note } })
  mocks.update.mockResolvedValue({ ok: true })
  mocks.close.mockResolvedValue(undefined)
  mocks.register.mockImplementation(async handler => { closeRequested = handler; return mocks.stop })
})
afterEach(() => {
  wrapper?.unmount()
  wrapper = undefined
  Reflect.deleteProperty(window, '__TAURI_INTERNALS__')
})
function mountNotebook() {
  wrapper = shallowMount(KnowledgeNotebookView, { global: {
    directives: { loading: {} },
    stubs: { ElDrawer: true, ElDropdown: true, ElDropdownMenu: true, ElDropdownItem: true },
  } })
  return wrapper
}

describe('notebook native close lifecycle', () => {
  it('keeps the window open after a failed save and closes only after a successful retry', async () => {
    const view = mountNotebook()
    await flushPromises()
    await view.get('.title-editor').setValue('修改后标题')
    mocks.update.mockResolvedValueOnce({ ok: false, error: '保存失败测试' })
    const firstEvent = { preventDefault: vi.fn() }
    await closeRequested(firstEvent)
    expect(firstEvent.preventDefault).toHaveBeenCalledOnce()
    expect(mocks.close).not.toHaveBeenCalled()
    expect(view.get('.title-editor').element).toHaveProperty('value', '修改后标题')
    expect(mocks.error).toHaveBeenCalledWith('保存失败测试')
    await closeRequested({ preventDefault: vi.fn() })
    expect(mocks.update).toHaveBeenLastCalledWith('a', expect.objectContaining({ title: '修改后标题' }))
    expect(mocks.close).toHaveBeenCalledOnce()
  })

  it('removes a close listener whose registration finishes after leaving the view', async () => {
    let finish!: (stop: () => void) => void
    mocks.register.mockImplementation(() => new Promise(resolve => { finish = resolve }))
    mountNotebook()
    await flushPromises()
    wrapper!.unmount()
    wrapper = undefined
    finish(mocks.stop)
    await flushPromises()
    expect(mocks.stop).toHaveBeenCalledOnce()
    expect(mocks.list).not.toHaveBeenCalled()
  })
})
