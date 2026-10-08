// @vitest-environment jsdom
import { describe, expect, it, vi, beforeEach } from 'vitest'
import { mount, shallowMount, flushPromises } from '@vue/test-utils'
import AreaLineChart from '../../src/shared/charts/AreaLineChart.vue'
import DonutChart from '../../src/shared/charts/DonutChart.vue'
import GanttTimeline from '../../src/shared/charts/GanttTimeline.vue'
import DashboardView from '../../src/modules/dashboard/DashboardView.vue'
import EmptyState from '../../src/shared/components/EmptyState.vue'

const h = vi.hoisted(() => ({
  push: vi.fn(), trend: vi.fn(), status: vi.fn(), track: vi.fn(), hearings: vi.fn(), kpis: vi.fn(),
  on: vi.fn((_event: string, _callback: () => void) => vi.fn()),
}))
vi.mock('vue-router', () => ({ useRouter: () => ({ push: h.push }) }))
vi.mock('../../src/core/plugin/context', () => ({ casyContext: { cases: { stats: h.status }, dashboard: {
  monthlyTaskTrend: h.trend, projectStatusDistribution: h.status, trackDistribution: h.track,
  upcomingHearings: h.hearings, todayKpis: h.kpis,
}, on: h.on } }))

describe('chart geometry and accessible data', () => {
  it('keeps donut arcs in their own coordinate system and omits zero-value slices', async () => {
    const wrapper = mount(DonutChart, { props: { data: [
      { label: 'Active', value: 3, color: 'green' }, { label: 'Done', value: 0, color: 'gray' },
    ] } })
    expect(wrapper.find('svg g').attributes('transform')).toBeUndefined()
    expect(wrapper.findAll('path')).toHaveLength(1)
    expect(wrapper.find('.dc-main').text()).toBe('3')
    await wrapper.find('button').trigger('click')
    expect(wrapper.emitted('select')?.[0]?.[0]).toMatchObject({ value: 3 })
  })

  it('maps pointer positions to the scaled viewBox and binds numeric tooltip coordinates', async () => {
    const data = Array.from({ length: 6 }, (_, i) => ({ month: `2026-0${i + 1}`, created: i, completed: 0 }))
    const wrapper = mount(AreaLineChart, { props: { data } })
    const svg = wrapper.find('svg')
    vi.spyOn(svg.element, 'getBoundingClientRect').mockReturnValue({ left: 0, width: 230 } as DOMRect)
    await svg.trigger('mousemove', { clientX: 222 })
    expect(wrapper.find('.tip-title').text()).toBe('2026-06')
    expect(wrapper.find('rect').attributes('y')).toBe('10')
    expect(wrapper.find('.tip-title').attributes('y')).toBe('24')
    expect(wrapper.findAll('tbody tr')).toHaveLength(6)
    expect(svg.attributes('viewBox')).toBe('0 0 460 180')
  })

  it('places hearings using server day offsets, including the end of the window', async () => {
    const item = { id: 'h', title: 'Hearing', date: '2026-10-05', caseId: 'c', daysLeft: 30 }
    const wrapper = mount(GanttTimeline, { props: { items: [item] } })
    expect(wrapper.find('.gt-bar').attributes('style')).toContain('left: 100%')
    await wrapper.find('button').trigger('click')
    expect(wrapper.emitted('select')?.[0]?.[0]).toMatchObject({ caseId: 'c' })
  })
})

describe('dashboard service results and drill-down', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    for (const request of [h.trend, h.track, h.hearings]) request.mockResolvedValue({ ok: true, data: [] })
    h.status.mockResolvedValue({ ok: true, data: { active: 0, closed: 0 } })
    h.kpis.mockResolvedValue({ ok: true, data: { todayEvents: 0, dueToday: 4, waitingOverdue: 2, reviewDue: 1 } })
  })

  it('renders camelCase KPIs including zero and passes exact metric filters', async () => {
    const wrapper = shallowMount(DashboardView)
    await flushPromises()
    expect(wrapper.findAll('.kv').map(el => el.text())).toEqual(['0项', '4项', '2项', '1项'])
    await wrapper.findAll('.kpi-card')[1].trigger('click')
    expect(h.push).toHaveBeenCalledWith({ name: 'tasks', query: { tab: 'all', metric: 'dueToday' } })
    await wrapper.findAll('.kpi-card')[3].trigger('click')
    expect(h.push).toHaveBeenLastCalledWith({ name: 'tasks', query: { tab: 'review' } })
    wrapper.unmount()
  })

  it('isolates a failed chart and keeps successful KPIs visible', async () => {
    h.trend.mockRejectedValue(new Error('offline'))
    const wrapper = shallowMount(DashboardView)
    await flushPromises()
    expect(wrapper.text()).toContain('任务趋势加载失败')
    expect(wrapper.findAll('.kv')[1].text()).toBe('4项')
    expect(wrapper.findAllComponents(EmptyState).some(el => el.props('title') === '暂无案件')).toBe(true)
    wrapper.unmount()
  })

  it('does not present unavailable KPIs as zero', async () => {
    h.kpis.mockResolvedValue({ ok: false, error: 'offline' })
    const wrapper = shallowMount(DashboardView)
    await flushPromises()
    expect(wrapper.text()).toContain('今日指标加载失败')
    expect(wrapper.findAll('.kpi-card').every(el => el.attributes('disabled') !== undefined)).toBe(true)
    wrapper.unmount()
  })

  it('refreshes again when an event arrives during a pending read', async () => {
    let finish: (value: unknown) => void = () => {}
    h.kpis.mockImplementationOnce(() => new Promise(resolve => { finish = resolve }))
    const wrapper = shallowMount(DashboardView)
    const callback = h.on.mock.calls.find(([event]) => event === 'inbox:confirmed')?.[1] as (() => void)
    callback()
    finish({ ok: true, data: { todayEvents: 0, dueToday: 1, waitingOverdue: 0, reviewDue: 0 } })
    await flushPromises()
    expect(h.kpis).toHaveBeenCalledTimes(2)
    expect(wrapper.findAll('.kv')[1].text()).toBe('4项')
    wrapper.unmount()
  })
})
