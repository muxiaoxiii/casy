// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import ClientView from '../../src/modules/clients/views/ClientView.vue'
const mocks = vi.hoisted(() => ({ stats: vi.fn(), list: vi.fn() }))
vi.mock('../../src/core/plugin/context', () => ({ casyContext: { cases: mocks } }))
vi.mock('vue-router', () => ({ useRouter: () => ({ push: vi.fn() }) }))
let view: ReturnType<typeof mount>
function render() {
  view = mount(ClientView, { global: { directives: { loading: () => {} }, stubs: {
    'el-button': { template: '<button><slot/></button>' },
    'el-alert': { props: ['title'], template: '<aside role="alert">{{ title }}<slot/></aside>' },
    'el-input': true, 'el-icon': true, 'el-pagination': true,
  } } })
  return view
}
beforeEach(() => {
  vi.resetAllMocks()
  mocks.stats.mockResolvedValue({ ok: true, data: { byClient: [{ client: '甲公司', count: 1 }, { client: '乙公司', count: 1 }] } })
  mocks.list.mockResolvedValue({ ok: true, data: { items: [], total: 0 } })
})
afterEach(() => view?.unmount())
describe('client load recovery', () => {
  it('does not display empty customers on a failed summary and retries', async () => {
    mocks.stats.mockRejectedValueOnce(new Error('网络中断'))
    render(); await flushPromises()
    expect(view.find('[role="alert"]').text()).toContain('网络中断')
    expect(view.text()).not.toContain('暂无关联客户')
    expect(view.text()).not.toContain('暂无客户案件')
    await view.find('[role="alert"] button').trigger('click'); await flushPromises()
    expect(view.findAll('.client-item')).toHaveLength(2)
    expect(view.find('[role="alert"]').exists()).toBe(false)
  })
  it('recovers from rejected detail reads without bogus counts', async () => {
    mocks.list.mockRejectedValueOnce(new Error('读取失败'))
    render(); await flushPromises()
    expect(view.find('.detail-header').text()).toContain('暂不可用')
    await view.find('[role="alert"] button').trigger('click'); await flushPromises()
    expect(view.find('.detail-header').text()).toContain('0 件案件')
    expect(view.text()).toContain('该客户暂无关联案件')
  })
  it('ignores older responses after selecting another customer', async () => {
    let first!: (value: unknown) => void
    mocks.list.mockImplementationOnce(() => new Promise(resolve => { first = resolve }))
    render(); await flushPromises()
    mocks.list.mockResolvedValueOnce({ ok: true, data: { items: [{ id: 'b', caseName: '乙案' }], total: 1 } })
    await view.findAll('.client-item')[1].trigger('click'); await flushPromises()
    first({ ok: true, data: { items: [{ id: 'a', caseName: '甲案' }], total: 1 } }); await flushPromises()
    expect(view.find('.case-list').text()).toContain('乙案')
    expect(view.find('.case-list').text()).not.toContain('甲案')
  })
})
