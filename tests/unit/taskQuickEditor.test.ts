// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils'
import { createRouter, createMemoryHistory } from 'vue-router'
import TaskQuickEditor from '../../src/modules/tasks/components/TaskQuickEditor.vue'
import type { Task } from '../../src/types'

const calls = vi.hoisted(() => ({ update: vi.fn(), createEvent: vi.fn() }))
vi.mock('../../src/core/plugin/context', () => ({ casyContext: { tasks: { update: calls.update }, calendar: { createEvent: calls.createEvent } } }))
let wrapper: VueWrapper | undefined
afterEach(() => { wrapper?.unmount(); vi.resetAllMocks() })
async function setup() {
  const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/tasks', component: { template: '<div />' } }, { path: '/calendar', component: { template: '<div />' } }] })
  await router.push('/tasks'); await router.isReady()
  calls.update.mockResolvedValue({ ok: true }); calls.createEvent.mockResolvedValue({ ok: true })
  wrapper = mount(TaskQuickEditor, { props: { task: { id: 't1', taskName: '核对证据', description: '原备注', dueDate: '2026-09-30', startBucket: 'today', caseId: 'c1', estimatedMinutes: 45 } as Task, cases: [{ id: 'c1', caseName: '案件' }] }, global: { plugins: [router], stubs: { RelatedWork: true } } })
  return { wrapper, router }
}
describe('task inline editing', () => {
  it('flushes only modified fields, preserving unrelated background changes', async () => {
    const { wrapper } = await setup()
    await wrapper.get('textarea').setValue('核实后的备注')
    const saved = await (wrapper.vm as unknown as { saveIfDirty: () => Promise<boolean> }).saveIfDirty()
    expect(saved).toBe(true)
    expect(calls.update).toHaveBeenCalledWith({ id: 't1', description: '核实后的备注' })
  })
  it('keeps a failed edit available and returns false to block navigation', async () => {
    const { wrapper } = await setup()
    calls.update.mockResolvedValue({ ok: false, error: '资料库只读' })
    await wrapper.get('textarea').setValue('不能丢失的内容')
    expect(await (wrapper.vm as unknown as { saveIfDirty: () => Promise<boolean> }).saveIfDirty()).toBe(false)
    expect(wrapper.get('textarea').element.value).toBe('不能丢失的内容')
    expect(wrapper.get('[role=alert]').text()).toContain('资料库只读')
  })
  it('schedules a linked time block without rewriting the deadline', async () => {
    const { wrapper, router } = await setup()
    await wrapper.get('[aria-expanded]').trigger('click')
    const button = wrapper.findAll('button').find(b => b.text() === '安排并打开日历')!
    await button.trigger('click'); await flushPromises()
    expect(calls.update).not.toHaveBeenCalled()
    expect(calls.createEvent).toHaveBeenCalledWith(expect.objectContaining({ taskId: 't1', caseId: 'c1', startTime: '09:00', endTime: '09:45', allDay: false }))
    expect(router.currentRoute.value.path).toBe('/calendar')
  })
})
