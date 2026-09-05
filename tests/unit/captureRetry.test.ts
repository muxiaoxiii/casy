// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, shallowMount } from '@vue/test-utils'
import ElementPlus from 'element-plus'
import UnifiedCaptureDialog from '../../src/shared/components/UnifiedCaptureDialog.vue'

const mock = vi.hoisted(() => ({ add: vi.fn(), confirm: vi.fn(), judge: vi.fn() }))
vi.mock('vue-router', () => ({ useRouter: () => ({ push: vi.fn() }) }))
vi.mock('../../src/core/mockData', () => ({ isTauriRuntime: () => false }))
vi.mock('../../src/core/plugin/context', () => ({ casyContext: {
  cases: { list: async () => ({ ok: true, data: { items: [{ id: 'c1', caseName: '合成案件' }] } }) },
  inbox: { add: mock.add, confirmAction: mock.confirm, quickJudge: mock.judge },
} }))
beforeEach(() => {
  vi.clearAllMocks()
  mock.add.mockResolvedValue({ ok: true, data: 'text-id' })
  mock.confirm.mockResolvedValue({ ok: true, data: { success: true, task: { id: 'task-id' } } })
})
async function open(initialAction = 'create_task') {
  const wrapper = shallowMount(UnifiedCaptureDialog, { props: { modelValue: false, initialAction: initialAction as 'create_task' }, global: {
    plugins: [ElementPlus], renderStubDefaultSlot: true,
    stubs: { ElDialog: { template: '<section><slot name="header"/><slot/><slot name="footer"/></section>' } },
  } })
  await wrapper.setProps({ modelValue: true }); await flushPromises()
  const button = (name: string) => wrapper.findAllComponents({ name: 'ElButton' }).find(item => item.text() === name)!
  return { wrapper, button }
}
describe('capture retry and review', () => {
  it('preserves the complete input after a partial attachment failure and retries only the missing capture', async () => {
    const { wrapper, button } = await open()
    try {
      await wrapper.find('textarea').setValue('明天下午三点半提交材料')
      window.dispatchEvent(new CustomEvent('casy:file-drop', { detail: { paths: ['/tmp/synthetic.txt'] } }))
      mock.add.mockResolvedValueOnce({ ok: true, data: 'text-id' }).mockResolvedValueOnce({ ok: false, error: 'file failure' }).mockResolvedValueOnce({ ok: true, data: 'file-id' })
      await button('继续').trigger('click'); await flushPromises()
      expect(wrapper.find('textarea').element).toHaveProperty('value', '明天下午三点半提交材料')
      expect(wrapper.text()).toContain('synthetic.txt')
      await button('继续').trigger('click'); await flushPromises()
      expect(mock.add.mock.calls.filter(call => call[0] === 'note')).toHaveLength(1)
      expect(wrapper.findComponent({ name: 'ElTimePicker' }).props('modelValue')).toBe('15:30')
      wrapper.findComponent({ name: 'ElSelect' }).vm.$emit('update:modelValue', 'c1')
      mock.confirm.mockResolvedValueOnce({ ok: true, data: { success: true, task: { id: 'task-id' } } }).mockResolvedValueOnce({ ok: false, error: 'filing failed' })
      await button('确认处理').trigger('click'); await flushPromises()
      expect(wrapper.text()).toContain('确认处理')
      await button('确认处理').trigger('click'); await flushPromises()
      expect(mock.add).toHaveBeenCalledTimes(3)
      expect(mock.confirm).toHaveBeenLastCalledWith({ inboxItemId: 'file-id', action: 'file_to_case', targetCaseId: 'c1', targetCategory: 'other' })
      expect(wrapper.text()).toContain('处理完成')
    } finally { wrapper.unmount() }
  })
  it('requires an explicit date for an event with no date in the input', async () => {
    const { wrapper, button } = await open('create_event')
    try {
      await wrapper.find('textarea').setValue('核对会议')
      await button('继续').trigger('click'); await flushPromises()
      await button('确认处理').trigger('click'); await flushPromises()
      expect(mock.confirm).not.toHaveBeenCalled()
      wrapper.findComponent({ name: 'ElDatePicker' }).vm.$emit('update:modelValue', '2026-09-18')
      wrapper.findComponent({ name: 'ElTimePicker' }).vm.$emit('update:modelValue', '15:37')
      await button('确认处理').trigger('click'); await flushPromises()
      expect(mock.confirm).toHaveBeenCalledWith(expect.objectContaining({ intent: expect.objectContaining({ eventDate: '2026-09-18', startTime: '15:37' }) }))
    } finally { wrapper.unmount() }
  })
})
