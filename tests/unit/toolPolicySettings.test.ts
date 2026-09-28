// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import { createPinia } from 'pinia'
import ToolPolicy from '../../src/modules/settings/components/ToolPolicySettings.vue'
const mocks = vi.hoisted(() => ({ get: vi.fn(), save: vi.fn(), setToolPolicy: vi.fn(), getRegisteredTools: vi.fn(), on: vi.fn(() => vi.fn()) }))
vi.mock('../../src/core/plugin/context', () => ({ casyContext: { settings: mocks, ...mocks } }))
const setup = () => mount(ToolPolicy, { global: { plugins: [createPinia()], stubs: {
  'el-input': true, 'el-empty': true, 'el-alert': { props: ['title'], template: '<div role="alert">{{title}}</div>' },
  'el-switch': { props: ['modelValue', 'disabled'], template: '<button :disabled="disabled" :aria-pressed="modelValue" @click="$emit(\'update:modelValue\', !modelValue)">toggle</button>' },
  'el-select': true, 'el-option': true,
} } })
beforeEach(() => {
  vi.clearAllMocks()
  mocks.get.mockResolvedValue({ ok: true, data: { ai_tool_policy: { disabled: ['read'], writeApproval: {} } } })
  mocks.getRegisteredTools.mockReturnValue([{ name: 'read', description: '读取知识', category: 'knowledge', policy: { write: false } }])
})
describe('tool policy persistence', () => {
  it('includes disabled tools so users can enable them again', async () => {
    mocks.save.mockResolvedValue({ ok: true })
    const view = setup(); await flushPromises()
    const toggle = view.find('.tool-row button')
    expect(toggle.attributes('aria-pressed')).toBe('false')
    await toggle.trigger('click'); await flushPromises()
    expect(mocks.save).toHaveBeenCalledWith({ ai_tool_policy: { disabled: [], writeApproval: {} } })
    expect(mocks.setToolPolicy).toHaveBeenCalledWith({ disabled: [], writeApproval: {} })
    expect(view.text()).toContain('已保存并生效')
    view.unmount()
  })
  it('keeps runtime policy unchanged after a failed write', async () => {
    mocks.save.mockResolvedValue({ ok: false, error: '磁盘不可写' })
    const view = setup(); await flushPromises()
    await view.find('.tool-row button').trigger('click'); await flushPromises()
    expect(mocks.setToolPolicy).not.toHaveBeenCalled()
    expect(view.find('.tool-row button').attributes('aria-pressed')).toBe('false')
    expect(view.find('[role="alert"]').text()).toContain('磁盘不可写')
    view.unmount()
  })
})
