// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import History from '../../src/modules/knowledge/components/KnowledgeHistoryPanel.vue'
const mocks = vi.hoisted(() => ({ versions: vi.fn(), diffWithCurrent: vi.fn(), restoreVersion: vi.fn(), confirm: vi.fn(), error: vi.fn() }))
vi.mock('../../src/core/plugin/context', () => ({ casyContext: { knowledge: mocks } }))
vi.mock('element-plus', () => ({ ElMessageBox: { confirm: mocks.confirm }, ElMessage: { error: mocks.error, success: vi.fn() } }))
const options = { global: { stubs: { 'el-button': { emits: ['click'], template: '<button @click="$emit(\'click\')"><slot/></button>' } }, directives: { loading: () => {} } } }
beforeEach(() => { vi.clearAllMocks(); mocks.versions.mockResolvedValue({ ok: true, data: [{ id: 'v1', content: '历史正文\n' + '完整内容'.repeat(1000), changedAt: '2026-09-28T09:00:00' }] }); mocks.confirm.mockResolvedValue(true) })
describe('history-first reading', () => {
  it('shows the complete saved version without a diff request', async () => {
    const view = mount(History, { ...options, props: { note: { id: 'a' } } })
    await flushPromises()
    expect(view.find('.version-preview').text()).toContain('完整内容'.repeat(1000))
    expect(mocks.diffWithCurrent).not.toHaveBeenCalled()
    view.unmount()
  })
  it('separates failure from empty and supports retry', async () => {
    mocks.versions.mockResolvedValueOnce({ ok: false, error: '读取失败' })
    const view = mount(History, { ...options, props: { note: { id: 'a' } } })
    await flushPromises()
    expect(view.find('[role="alert"]').text()).toContain('读取失败')
    expect(view.text()).not.toContain('正文发生修改后')
    await view.find('[role="alert"] button').trigger('click')
    await flushPromises()
    expect(view.find('.version-preview').exists()).toBe(true)
    view.unmount()
  })
  it('does not restore a version after the user switches notes during confirmation', async () => {
    let confirm!: () => void
    mocks.confirm.mockImplementationOnce(() => new Promise<void>(resolve => { confirm = resolve }))
    const view = mount(History, { ...options, props: { note: { id: 'a' } } })
    await flushPromises()
    await view.find('.history-actions button').trigger('click')
    await view.setProps({ note: { id: 'b' } })
    confirm()
    await flushPromises()
    expect(mocks.restoreVersion).not.toHaveBeenCalled()
    view.unmount()
  })
})
