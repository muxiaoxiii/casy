// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, shallowMount } from '@vue/test-utils'
import ElementPlus from 'element-plus'
import UnifiedCaptureDialog from '../../src/shared/components/UnifiedCaptureDialog.vue'
import ClientView from '../../src/modules/clients/views/ClientView.vue'

const h = vi.hoisted(() => ({ list: vi.fn(), stats: vi.fn(), get: vi.fn(), add: vi.fn(), confirm: vi.fn(), push: vi.fn() }))
vi.mock('vue-router', () => ({ useRouter: () => ({ push: h.push }) }))
vi.mock('../../src/core/mockData', () => ({ isTauriRuntime: () => false }))
vi.mock('../../src/core/plugin/context', () => ({ casyContext: {
  cases: { list: h.list, stats: h.stats, get: h.get },
  inbox: { add: h.add, confirmAction: h.confirm },
} }))
const global = {
  plugins: [ElementPlus],
  renderStubDefaultSlot: true,
  stubs: { ElDialog: { template: '<section><slot name="header"/><slot/><slot name="footer"/></section>' } },
}

beforeEach(() => {
  vi.clearAllMocks()
  h.list.mockResolvedValue({ ok: true, data: { items: [{ id: 'c1', caseName: '关联案件' }], total: 1 } })
  h.stats.mockResolvedValue({ ok: true, data: { byClient: [{ client: '客户甲', count: 21 }] } })
  h.add.mockResolvedValue({ ok: true, data: 'inbox-1' })
  h.confirm.mockResolvedValue({ ok: true, data: { success: true } })
})

describe('capture review', () => {
  async function openCapture() {
    const wrapper = shallowMount(UnifiedCaptureDialog, { props: { modelValue: false, initialAction: 'create_task' }, global })
    await wrapper.setProps({ modelValue: true })
    await flushPromises()
    return wrapper
  }

  it('loads paginated case data, submits edited values, and allows clearing the suggested case', async () => {
    const wrapper = await openCapture()
    expect(wrapper.findComponent({ name: 'ElOption' }).exists()).toBe(true)
    expect(wrapper.findAllComponents({ name: 'ElOption' }).some(option => option.props('label') === '关联案件')).toBe(true)
    await wrapper.find('textarea').setValue('明天提交材料')
    wrapper.findAllComponents({ name: 'ElSelect' })[1].vm.$emit('update:modelValue', 'c1')
    await wrapper.findAllComponents({ name: 'ElButton' }).find(button => button.text() === '继续')!.trigger('click')
    await flushPromises()
    wrapper.findComponent({ name: 'ElInput' }).vm.$emit('update:modelValue', '核对后提交材料')
    wrapper.findComponent({ name: 'ElDatePicker' }).vm.$emit('update:modelValue', '2026-10-09')
    wrapper.findComponent({ name: 'ElSelect' }).vm.$emit('update:modelValue', '')
    await wrapper.findAllComponents({ name: 'ElButton' }).find(button => button.text() === '确认处理')!.trigger('click')
    await flushPromises()
    expect(h.confirm).toHaveBeenCalledWith(expect.objectContaining({
      inboxItemId: 'inbox-1', targetCaseId: null,
      intent: expect.objectContaining({ taskName: '核对后提交材料', dueDate: '2026-10-09', caseId: null }),
    }))
    expect(wrapper.text()).toContain('处理完成')
    await wrapper.setProps({ modelValue: false })
    await wrapper.setProps({ modelValue: true })
    expect(wrapper.find('textarea').element.value).toBe('')
    expect(wrapper.findAllComponents({ name: 'ElSelect' })[1].props('modelValue')).toBe('')
    wrapper.unmount()
  })

  it('retains the review and permits retry after a rejected confirmation', async () => {
    const wrapper = await openCapture()
    await wrapper.find('textarea').setValue('提交材料')
    await wrapper.findAllComponents({ name: 'ElButton' }).find(button => button.text() === '继续')!.trigger('click')
    await flushPromises()
    h.confirm.mockRejectedValueOnce(new Error('offline'))
    const confirm = () => wrapper.findAllComponents({ name: 'ElButton' }).find(button => button.text() === '确认处理')!
    await confirm().trigger('click')
    await flushPromises()
    expect(confirm().props('loading')).toBe(false)
    await confirm().trigger('click')
    await flushPromises()
    expect(wrapper.text()).toContain('处理完成')
    expect(h.add).toHaveBeenCalledTimes(1)
    wrapper.unmount()
  })
})

describe('customers derived from cases', () => {
  it('uses server aggregation, paginates by the selected customer and opens an editable case', async () => {
    const wrapper = shallowMount(ClientView, { global })
    await flushPromises()
    expect(wrapper.find('.client-item').text()).toContain('客户甲')
    expect(h.list).toHaveBeenCalledWith({ client: '客户甲', page: 1, perPage: 20 })
    await wrapper.find('.case-row').trigger('click')
    expect(h.push).toHaveBeenCalledWith({ name: 'case-detail', params: { id: 'c1' } })
    h.list.mockResolvedValue({ ok: true, data: { items: [], total: 21 } })
    await wrapper.find('.client-item').trigger('click')
    await flushPromises()
    const pagination = wrapper.findComponent({ name: 'ElPagination' })
    pagination.vm.$emit('update:currentPage', 2)
    pagination.vm.$emit('current-change', 2)
    await flushPromises()
    expect(h.list).toHaveBeenLastCalledWith({ client: '客户甲', page: 2, perPage: 20 })
    wrapper.unmount()
  })
})
