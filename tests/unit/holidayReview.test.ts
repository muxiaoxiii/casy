// @vitest-environment jsdom
import { expect, it, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import ElementPlus from 'element-plus'
import HolidayImportReview from '../../src/shared/components/HolidayImportReview.vue'
const mock = vi.hoisted(() => ({ parse: vi.fn(), ai: vi.fn() }))
vi.mock('../../src/core/plugin/context', () => ({ casyContext: { inbox: { parseHolidays: mock.parse }, ai: { askAi: mock.ai } } }))
it('labels local extraction honestly and explicitly invokes AI without filing an inbox record', async () => {
  mock.parse.mockResolvedValue({ ok: true, data: { year: 2027, holidays: ['2027-01-01'], workdays: [] } })
  const view = mount(HolidayImportReview, { props: { content: '2027年1月1日放假' }, global: { plugins: [ElementPlus] } })
  await flushPromises()
  expect(view.text()).toContain('本地规则解析（未调用 AI）')
  expect(mock.ai).not.toHaveBeenCalled()
  mock.ai.mockResolvedValue({ ok: true, text: '{"year":2027,"holidays":["2027-01-01","2027-01-02"],"workdays":[]}' })
  await view.findAll('button').find(b => b.text() === 'AI 解析 JSON')!.trigger('click'); await flushPromises()
  expect(mock.ai).toHaveBeenCalledOnce()
  expect(view.text()).toContain('AI 解析（待核对）')
  expect(view.emitted('change')!.at(-1)![0]).toMatchObject({ holidays: ['2027-01-01','2027-01-02'] })
  mock.ai.mockResolvedValue({ ok: false, error: 'AI 未配置' })
  await view.findAll('button').find(b => b.text() === 'AI 解析 JSON')!.trigger('click'); await flushPromises()
  expect(view.text()).toContain('AI 未配置')
  expect(view.emitted('change')!.at(-1)![0]).toBe(null)
  view.unmount()
})
