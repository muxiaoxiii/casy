// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, shallowMount } from '@vue/test-utils'
import Workshop from '../../src/modules/docs/views/DocWorkshopView.vue'
const mocks = vi.hoisted(() => ({ listDrafts: vi.fn(), getDraft: vi.fn(), createDraft: vi.fn(), deleteDraft: vi.fn(), confirm: vi.fn() }))
vi.mock('../../src/core/plugin/context', () => ({ casyContext: { docs: mocks, cases: { list: async () => ({ ok: true, data: [] }) } } }))
vi.mock('vue-router', () => ({ useRoute: () => ({ query: {} }) }))
vi.mock('../../src/core/observeChanges', () => ({ observeChanges: () => () => {} }))
vi.mock('../../src/composables/useSaveBeforeLeave', () => ({ useSaveBeforeLeave: vi.fn() }))
vi.mock('../../src/composables/useDraftRecovery', () => ({ useDraftRecovery: () => ({ recover: async () => {}, clear: async () => {} }) }))
vi.mock('../../src/shared/editor/exportDocument', () => ({ exportDocument: vi.fn() }))
vi.mock('../../src/modules/docs/components/LegalEditor.vue', () => ({ default: { template: '<div />' } }))
vi.mock('../../src/modules/knowledge/components/KnowledgeSidebar.vue', () => ({ default: { template: '<div />' } }))
vi.mock('../../src/shared/editor/TypesetPreview.vue', () => ({ default: { template: '<div />' } }))
vi.mock('../../src/modules/docs/views/TemplateBrowser.vue', () => ({ default: { template: '<div />' } }))
vi.mock('element-plus', () => ({ ElMessage: { error: vi.fn(), success: vi.fn() }, ElMessageBox: { confirm: mocks.confirm } }))
let view: ReturnType<typeof shallowMount>
const drafts = [{ id: 'a', title: '当前稿', content: '<p>A</p>', version: 1 }, { id: 'b', title: '其他稿', content: '<p>B</p>', version: 1 }]
async function render() {
  view = shallowMount(Workshop, { global: { mocks: { $t: (s: string) => s }, directives: { loading: () => {} }, stubs: { 'el-alert': true, 'el-empty': true, 'el-input': true, 'el-select': true, 'el-option': true, 'el-dropdown': true, 'el-dropdown-menu': true, 'el-dropdown-item': true, 'el-icon': true, 'el-button': { template: '<button><slot/></button>' } } } })
  await flushPromises()
}
beforeEach(() => {
  vi.resetAllMocks()
  mocks.listDrafts.mockResolvedValue({ ok: true, data: drafts.map(d => ({ ...d })) })
  mocks.getDraft.mockImplementation(async id => ({ ok: true, data: { ...drafts.find(d => d.id === id) } }))
  mocks.confirm.mockResolvedValue(true)
  mocks.deleteDraft.mockResolvedValue({ ok: true })
})
afterEach(() => view?.unmount())
describe('draft workshop lifecycle', () => {
  it('never creates a blank draft because loading failed', async () => {
    mocks.listDrafts.mockRejectedValueOnce(new Error('数据库不可用'))
    await render()
    expect(mocks.createDraft).not.toHaveBeenCalled()
    expect(view.find('el-alert-stub').attributes('title')).toContain('数据库不可用')
  })
  it('deleting another draft keeps the current document selected', async () => {
    await render()
    await view.findAll('.draft-delete')[1].trigger('click'); await flushPromises()
    expect(mocks.deleteDraft).toHaveBeenCalledWith('b')
    expect(view.find('.draft-item.active').text()).toContain('当前稿')
    expect(mocks.getDraft).toHaveBeenCalledTimes(1)
    expect(mocks.createDraft).not.toHaveBeenCalled()
  })
  it('cancelled deletion leaves both drafts untouched', async () => {
    mocks.confirm.mockRejectedValueOnce('cancel')
    await render()
    await view.findAll('.draft-delete')[0].trigger('click'); await flushPromises()
    expect(mocks.deleteDraft).not.toHaveBeenCalled()
    expect(view.findAll('.draft-item')).toHaveLength(2)
  })
})
