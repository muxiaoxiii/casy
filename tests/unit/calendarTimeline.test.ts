// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils'
import { createRouter, createMemoryHistory } from 'vue-router'
import { createI18n } from 'vue-i18n'
import ElementPlus from 'element-plus'
import CalendarView from '../../src/modules/calendar/views/CalendarView.vue'
import { fixtureServices } from '../fixtures/calendar-ui/data.js'

const context = vi.hoisted(() => ({ on: () => () => {} } as Record<string, unknown>))
vi.mock('../../src/core/plugin/context', () => ({ casyContext: context }))
let wrapper: VueWrapper | undefined
afterEach(() => wrapper?.unmount())
async function setup(mode = 'mixed') {
  Object.assign(context, fixtureServices(mode))
  const router = createRouter({ history: createMemoryHistory(), routes: [
    { path: '/calendar', component: { template: '<div />' } },
    { path: '/cases/:id', name: 'case-detail', component: { template: '<div />' } },
    { path: '/tasks', name: 'tasks', component: { template: '<div />' } },
  ] })
  await router.push('/calendar?date=2026-09-25&view=timeline')
  await router.isReady()
  wrapper = mount(CalendarView, { global: {
    plugins: [router, ElementPlus, createI18n({ legacy: false, locale: 'zh-CN', missingWarn: false, fallbackWarn: false, messages: {} })],
    stubs: { PersonalDaysDialog: true, ElDialog: { props: ['modelValue', 'title'], template: '<section v-if="modelValue" role="dialog" :aria-label="title"><slot/><slot name="footer"/></section>' } },
  } })
  await flushPromises()
  return { wrapper, router }
}
describe('calendar timeline interactions', () => {
  it('shows holiday-only dates with source, name and an explicit empty schedule', async () => {
    const { wrapper } = await setup('holidays')
    expect(wrapper.findAll('.timeline-date-group')).toHaveLength(5)
    expect(wrapper.findAll('.timeline-event-card')).toHaveLength(0)
    expect(wrapper.get('[aria-label="2026年9月25日"]').text()).toContain('个人休息安排')
    expect(wrapper.get('[aria-label="2026年9月25日"]').text()).toContain('法定休')
    expect(wrapper.get('[aria-label="2026年9月25日"]').text()).toContain('自休')
    expect(wrapper.findAll('.timeline-holiday-note').every(row => row.text().includes('当日暂无已排期事项'))).toBe(true)
  })
  it('shows a useful empty state when no work or holidays exist', async () => {
    const { wrapper } = await setup('empty')
    expect(wrapper.get('.timeline-empty-hint').text()).toContain('暂无已排期')
  })
  it('opens the original task fields without duplicating its calendar projection', async () => {
    const { wrapper } = await setup()
    const cards = wrapper.findAll('.timeline-event-card').filter(card => card.text().includes('核对证据目录'))
    expect(cards).toHaveLength(1)
    await cards[0].trigger('click')
    const dialog = wrapper.get('[aria-label="编辑任务详情"]')
    expect((dialog.get('input[type=date]').element as HTMLInputElement).value).toBe('2026-09-25')
    expect((dialog.get('input[type=time]').element as HTMLInputElement).value).toBe('09:00')
    expect((dialog.get('input[type=number]').element as HTMLInputElement).value).toBe('90')
    expect((dialog.get('select').element as HTMLSelectElement).value).toBe('case1')
    expect((dialog.get('textarea').element as HTMLTextAreaElement).value).toBe('原始任务说明')
  })
  it('opens ordinary events as events with their end time and notes', async () => {
    const { wrapper } = await setup()
    await wrapper.findAll('.timeline-event-card').find(card => card.text().includes('独立日程'))!.trigger('click')
    const dialog = wrapper.get('[aria-label="编辑日程排期"]')
    expect(dialog.findAll('input[type=time]').map(el => (el.element as HTMLInputElement).value)).toEqual(['11:00', '12:00'])
    expect((dialog.get('textarea').element as HTMLTextAreaElement).value).toBe('原始日程备注')
  })
  it.each([['开庭核对', 'hearings'], ['上诉期限', 'tracks']])('routes %s to its original case tab', async (title, tab) => {
    const { wrapper, router } = await setup()
    await wrapper.findAll('.timeline-event-card').find(card => card.text().includes(title))!.trigger('click')
    await flushPromises()
    expect(router.currentRoute.value.params.id).toBe('case1')
    expect(router.currentRoute.value.query.tab).toBe(tab)
  })
  it('supports checkbox change for case filtering, keeping holiday context', async () => {
    const { wrapper } = await setup()
    await wrapper.findAll('.case-native-checkbox')[2].setValue(true)
    expect(wrapper.text()).not.toContain('核对证据目录')
    expect(wrapper.text()).toContain('中秋节')
    await wrapper.findAll('.case-native-checkbox')[0].setValue(true)
    expect(wrapper.text()).toContain('核对证据目录')
    await wrapper.findAll('.case-native-checkbox')[0].setValue(false)
    expect(wrapper.text()).not.toContain('核对证据目录')
    expect((wrapper.findAll('.case-native-checkbox')[0].element as HTMLInputElement).checked).toBe(false)
  })
})
