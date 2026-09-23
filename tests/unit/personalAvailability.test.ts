// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils'
import PersonalDaysDialog from '../../src/modules/calendar/components/PersonalDaysDialog.vue'
import TimeGrid from '../../src/modules/calendar/components/TimeGrid.vue'
import HolidayBadges from '../../src/modules/calendar/components/HolidayBadges.vue'
import { isPlanningWorkday, planningRestIntervals } from '../../src/modules/calendar/calendarDates'
import { planningWarnings } from '../../src/modules/calendar/taskPlanning'
const service=vi.hoisted(()=>({get:vi.fn(),save:vi.fn()}))
vi.mock('../../src/core/plugin/context',()=>({casyContext:{settings:service}}))
let wrapper:VueWrapper|undefined
const partial={date:'2027-03-11',kind:'holiday',source:'personal' as const,name:'请假',startTime:'13:00',endTime:'18:00'}
afterEach(()=>{wrapper?.unmount();vi.resetAllMocks()})
it('keeps half-day availability and distinguishes partial from whole-day rest across badges and grid',()=>{
  const date=new Date(2027,2,11)
  expect(isPlanningWorkday(date,[partial])).toBe(true)
  expect(planningRestIntervals(date,[partial])).toEqual([[780,1080]])
  expect(planningRestIntervals(date,[partial,{kind:'holiday',source:'official'}])).toEqual([[0,1440]])
  expect(planningRestIntervals(date,[{...partial,kind:'workday'},{kind:'holiday',source:'official'}])).toEqual([[0,780],[1080,1440]])
  wrapper=mount(HolidayBadges,{props:{entries:[partial]}})
  expect(wrapper.text()).toContain('13:00–18:00');expect(wrapper.get('.personal').text()).toContain('时休');wrapper.unmount()
  wrapper=mount(TimeGrid,{props:{days:[{date:partial.date,items:[],rest:[{start:780,end:1080,label:'个人休息 13:00–18:00'}]}]}})
  expect(wrapper.get('.time-rest').attributes('style')).toContain('top: 728px');expect(wrapper.get('.time-rest').attributes('style')).toContain('height: 280px')
  const warnings=planningWarnings({id:'t',taskName:'task'},{start:partial.date,end:partial.date},[],[],[partial])
  expect(warnings.join()).toContain('部分休息时段');expect(warnings.join()).toContain('13:00–18:00');expect(warnings.join()).not.toContain('1 个休息日')
})
it('persists precise leave, retains legacy whole days and preserves drafts on failure',async()=>{
  service.get.mockResolvedValue({ok:true,data:{personal_calendar_days:[{date:'2027-03-10',name:'旧全天',kind:'holiday'}]}})
  service.save.mockResolvedValue({ok:true})
  wrapper=mount(PersonalDaysDialog,{props:{modelValue:false,date:partial.date},global:{stubs:{ElDialog:{template:'<section><slot/></section>'},ElButton:{template:'<button><slot/></button>'}}}})
  await wrapper.setProps({modelValue:true});await flushPromises()
  await wrapper.get('select').setValue('afternoon')
  expect(wrapper.findAll('input[type=time]').map(i=>(i.element as HTMLInputElement).value)).toEqual(['13:00','18:00'])
  await wrapper.get('form').trigger('submit');await flushPromises()
  expect(service.save).toHaveBeenLastCalledWith({personal_calendar_days:[{date:'2027-03-10',name:'旧全天',kind:'holiday',startTime:null,endTime:null},{date:partial.date,name:'',kind:'holiday',startTime:'13:00',endTime:'18:00'}]})
  await wrapper.get('select').setValue('custom');await wrapper.findAll('input[type=time]')[1].setValue('12:00');await wrapper.get('form').trigger('submit')
  expect(service.save).toHaveBeenCalledTimes(1);expect(wrapper.get('[role=alert]').text()).toContain('结束须晚于开始')
  await wrapper.findAll('input[type=time]')[1].setValue('17:30');service.save.mockResolvedValue({ok:false,error:'只读'})
  await wrapper.get('form').trigger('submit');await flushPromises()
  expect(wrapper.get('[role=alert]').text()).toContain('只读');expect((wrapper.findAll('input[type=time]')[1].element as HTMLInputElement).value).toBe('17:30')
  expect(wrapper.emitted('saved')).toHaveLength(1)
  service.save.mockResolvedValue({ok:true});await wrapper.get('select').setValue('all');await wrapper.get('form').trigger('submit');await flushPromises()
  expect(service.save.mock.calls.at(-1)?.[0].personal_calendar_days.at(-1)).toMatchObject({startTime:null,endTime:null})
})
