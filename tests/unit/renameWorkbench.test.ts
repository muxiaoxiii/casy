// @vitest-environment jsdom
import { mount, flushPromises } from '@vue/test-utils'
import { describe, it, expect, vi } from 'vitest'
import RenameWorkbench from '../../src/modules/cases/components/RenameWorkbench.vue'

const { applyRenames, warning } = vi.hoisted(() => ({ applyRenames: vi.fn(), warning: vi.fn() }))
vi.mock('../../src/core/plugin/context', () => ({ casyContext: { files: { applyRenames } } }))
vi.mock('element-plus', () => ({ ElMessage: { warning, error: vi.fn(), success: vi.fn(), info: vi.fn() } }))

describe('rename workbench', () => {
  it('extracts short Chinese dates and displays the actual collision-safe filename', async () => {
    applyRenames.mockResolvedValue({ ok: true, data: [{ id: 'f', newName: '庭审证据-1.pdf', warning: '旧副本未清理' }] })
    const wrapper = mount(RenameWorkbench, { props: { caseId: 'c', dirName: '卷宗', files: [{ id: 'f', fileName: '8月24日证据.pdf' }] }, global: { stubs: {
      ElButton: { template: '<button><slot /></button>' }, ElIcon: true, EmptyState: true,
    } } })
    await flushPromises()
    expect(wrapper.get('input').attributes('placeholder')).toBe(`${new Date().getFullYear()}0824_证据`)
    await wrapper.get('input').setValue('庭审证据')
    await wrapper.get('.rr-ops button').trigger('click')
    await flushPromises()
    expect(applyRenames).toHaveBeenCalledWith('c', [{ id: 'f', newName: '庭审证据' }])
    expect(wrapper.get('.done-text').text()).toBe('庭审证据-1.pdf')
    expect(warning).toHaveBeenCalledWith('旧副本未清理')
    wrapper.unmount()
  })
})
