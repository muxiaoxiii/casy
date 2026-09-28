// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import History from '../../src/modules/docs/components/DraftHistoryDialog.vue'
const mocks = vi.hoisted(() => ({ draftVersions: vi.fn(), restoreDraftVersion: vi.fn(), confirm: vi.fn() }))
vi.mock('../../src/core/plugin/context', () => ({ casyContext: { docs: mocks } }))
vi.mock('element-plus', () => ({ ElMessageBox: { confirm: mocks.confirm }, ElMessage: { success: vi.fn() } }))
const draft = { id: 'draft-1', title: '现稿', content: '<p>当前内容</p>', version: 3 } as any
function render(beforeRestore = vi.fn().mockResolvedValue(true)) {
  return mount(History, { props: { modelValue: true, draft, beforeRestore }, global: {
    directives: { loading: () => {} }, stubs: {
      'el-dialog': { template: '<section><slot/><slot name="footer"/></section>' },
      'el-button': { template: '<button><slot/></button>' },
      'el-alert': { props: ['title'], template: '<aside role="alert">{{ title }}<slot/></aside>' },
    },
  } })
}
async function restore(view: ReturnType<typeof render>) {
  await flushPromises()
  await view.findAll('button').find(b => b.text() === '恢复此版本')!.trigger('click')
  await flushPromises()
}
beforeEach(() => {
  vi.clearAllMocks()
  mocks.draftVersions.mockResolvedValue({ ok: true, data: [{ draftId: draft.id, version: 1, title: '历史稿', content: '<p>历史正文</p>', savedAt: '2026-09-28 09:00:00' }] })
  mocks.confirm.mockResolvedValue(true)
})
describe('draft restoration safety', () => {
  it('never restores when saving current edits fails', async () => {
    const view = render(vi.fn().mockResolvedValue(false))
    await restore(view)
    expect(mocks.restoreDraftVersion).not.toHaveBeenCalled()
    expect(view.emitted('restored')).toBeUndefined()
    view.unmount()
  })
  it('keeps the dialog and historical content on version conflict', async () => {
    mocks.restoreDraftVersion.mockResolvedValue({ ok: false, error: '版本冲突，请重新加载' })
    const view = render()
    await restore(view)
    expect(mocks.restoreDraftVersion).toHaveBeenCalledWith('draft-1', 1, 3)
    expect(view.find('[role="alert"]').text()).toContain('版本冲突')
    expect(view.find('.version-preview').text()).toContain('历史正文')
    expect(view.emitted('update:modelValue')).toBeUndefined()
    view.unmount()
  })
  it('does not restore after switching drafts during confirmation', async () => {
    let confirm!: () => void
    mocks.confirm.mockImplementationOnce(() => new Promise<void>(resolve => { confirm = resolve }))
    const view = render()
    await restore(view)
    await view.setProps({ draft: { ...draft, id: 'draft-2' } })
    confirm()
    await flushPromises()
    expect(mocks.restoreDraftVersion).not.toHaveBeenCalled()
    view.unmount()
  })
})
