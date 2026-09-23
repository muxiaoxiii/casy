// @vitest-environment jsdom
import { expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import HolidayBadges from '../../src/modules/calendar/components/HolidayBadges.vue'
import YearHeatmap from '../../src/modules/calendar/components/YearHeatmap.vue'
import { isPlanningWorkday } from '../../src/modules/calendar/calendarDates'
const entries = [{ date: '2027-01-02', kind: 'holiday', name: '元旦', source: 'official' as const }, { date: '2027-01-02', kind: 'workday', name: '个人补班', source: 'personal' as const }]
it('keeps statutory and personal badges visible on the same date, including year cells', () => {
  const badges = mount(HolidayBadges, { props: { entries } })
  expect(badges.text()).toContain('法定休')
  expect(badges.text()).toContain('自班')
  expect(badges.find('.personal').attributes('title')).toContain('个人补班')
  const year = mount(YearHeatmap, { props: { year: 2027, tasks: [], holidays: entries }, global: { stubs: { ElButton: true } } })
  const day = year.find('button[aria-label^="2027-01-02"]')
  expect(day.text()).toContain('休')
  expect(day.text()).toContain('自班')
  expect(day.findAll('.holiday-badge')).toHaveLength(2)
  badges.unmount(); year.unmount()
})
it('uses personal availability for planning while preserving statutory badges', () => {
  expect(isPlanningWorkday(new Date(2027, 0, 2), entries)).toBe(true)
  expect(isPlanningWorkday(new Date(2027, 0, 4), [{ kind: 'holiday', source: 'personal' }])).toBe(false)
  expect(isPlanningWorkday(new Date(2027, 0, 2), entries.filter(entry => entry.source === 'official'))).toBe(false)
})
