// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils'
import TaskGantt from '../../src/modules/calendar/components/TaskGantt.vue'
const save=vi.hoisted(()=>vi.fn())
const holidayLookup=vi.hoisted(()=>vi.fn())
const eventLookup=vi.hoisted(()=>vi.fn())
vi.mock('../../src/core/plugin/context',()=>({casyContext:{calendar:{saveTaskPlan:save,holidays:holidayLookup,events:eventLookup}}}))
const tasks=[{id:'t',taskName:'准备材料',caseId:'c',startDate:'2026-09-01',dueDate:'2026-09-30',completed:0},{id:'u',taskName:'未排任务',completed:0}]
const plan={taskId:'t',startDate:'2026-09-25',endDate:'2026-09-27',revision:4}
let wrapper:VueWrapper
const dialog={props:['modelValue'],template:'<section v-if="modelValue" role="dialog"><slot/></section>'}
function setup(extra={}) {
  holidayLookup.mockImplementation(async(year:number)=>({ok:true,data:{year,entries:[]}}))
  eventLookup.mockResolvedValue({ok:true,data:[]})
  wrapper=mount(TaskGantt,{props:{date:'2026-09-25',tasks,plans:[plan],cases:[{id:'c',caseName:'案件甲'}],holidays:[],events:[{id:'d',title:'举证期限',date:'2026-09-29',caseId:'c',type:'deadline'}],...extra},global:{stubs:{ElDialog:dialog}}})
  return wrapper
}
afterEach(()=>{wrapper?.unmount();vi.resetAllMocks()})
describe('Gantt scheduling UI',()=>{
  it('requires confirmation for keyboard movement and only sends independent plan fields',async()=>{
    setup();await wrapper.get('[aria-label="移动计划：准备材料"]').trigger('keydown',{key:'ArrowRight',shiftKey:true})
    expect(save).not.toHaveBeenCalled()
    const dates=wrapper.findAll('input[type=date]')
    expect(dates.map(d=>(d.element as HTMLInputElement).value)).toEqual(['2026-10-02','2026-10-04'])
    save.mockResolvedValue({ok:true,data:{...plan,startDate:'2026-10-02',endDate:'2026-10-04',revision:5}})
    await flushPromises();await wrapper.get('form').trigger('submit');await flushPromises()
    expect(save).toHaveBeenCalledWith({taskId:'t',startDate:'2026-10-02',endDate:'2026-10-04',expectedRevision:4})
    expect(wrapper.emitted('saved')?.[0][0]).toMatchObject({revision:5})
    expect(tasks[0].dueDate).toBe('2026-09-30')
  })
  it('retains the original bar and draft on failure, then reloads the new revision explicitly',async()=>{
    setup();await wrapper.get('[aria-label="调整结束：准备材料"]').trigger('keydown',{key:'ArrowRight'})
    save.mockResolvedValue({ok:false,error:'PLAN_CONFLICT: 已修改'})
    await flushPromises();await wrapper.get('form').trigger('submit');await flushPromises()
    expect(wrapper.get('[role=alert]').text()).toContain('PLAN_CONFLICT')
    expect(wrapper.get('[aria-label="移动计划：准备材料"]').attributes('title')).toContain('2026-09-25 — 2026-09-27')
    expect(wrapper.findAll('input[type=date]').map(d=>(d.element as HTMLInputElement).value)).toEqual(['2026-09-25','2026-09-28'])
    expect(wrapper.emitted('saved')).toBeUndefined();expect(wrapper.emitted('refresh')).toHaveLength(1)
    await wrapper.setProps({plans:[{...plan,startDate:'2026-10-01',endDate:'2026-10-03',revision:5}]})
    await wrapper.findAll('button').find(b=>b.text()==='载入最新计划')!.trigger('click')
    save.mockResolvedValue({ok:true,data:{...plan,revision:6}})
    await flushPromises();await wrapper.get('form').trigger('submit');await flushPromises()
    expect(save).toHaveBeenLastCalledWith({taskId:'t',startDate:'2026-10-01',endDate:'2026-10-03',expectedRevision:5})
  })
  it('schedules undated work and clears plans with version protection',async()=>{
    setup();await wrapper.findAll('.gantt-label').find(b=>b.text().includes('未排任务'))!.trigger('click')
    save.mockResolvedValue({ok:true,data:{taskId:'u',startDate:'2026-09-25',endDate:'2026-09-25',revision:1}})
    await flushPromises();await wrapper.get('form').trigger('submit');await flushPromises()
    expect(save).toHaveBeenLastCalledWith({taskId:'u',startDate:'2026-09-25',endDate:'2026-09-25',expectedRevision:0})
    await wrapper.get('[aria-label="移动计划：准备材料"]').trigger('click')
    await wrapper.get('.clear-plan input').setValue(true)
    await flushPromises();await wrapper.get('form').trigger('submit');await flushPromises()
    expect(save).toHaveBeenLastCalledWith({taskId:'t',startDate:null,endDate:null,expectedRevision:4})
  })
  it('validates dates and blocks duplicate submissions while awaiting the backend',async()=>{
    setup();await wrapper.get('[aria-label="移动计划：准备材料"]').trigger('click')
    await wrapper.findAll('input[type=date]')[1].setValue('2026-09-24')
    await flushPromises();await wrapper.get('form').trigger('submit');expect(save).not.toHaveBeenCalled()
    await wrapper.findAll('input[type=date]')[1].setValue('2026-09-27')
    let resolve!:(value:unknown)=>void;save.mockReturnValue(new Promise(r=>resolve=r))
    await flushPromises();await wrapper.get('form').trigger('submit');await flushPromises();await wrapper.get('form').trigger('submit')
    expect(save).toHaveBeenCalledTimes(1)
    resolve({ok:false,error:'offline'});await flushPromises()
  })
  it('keeps fixed markers and completed tasks read only and provides an empty filter state',async()=>{
    setup({tasks:[{...tasks[0],completed:1}]})
    expect(wrapper.text()).toContain('没有符合条件的任务')
    await wrapper.get('.gantt-filters input[type=checkbox]').setValue(true)
    expect(wrapper.get('[aria-label="移动计划：准备材料"]').attributes('disabled')).toBeDefined()
    expect(wrapper.get('.task-deadline').attributes('title')).toContain('2026-09-30')
    expect(wrapper.findAll('.fixed-row')).toHaveLength(1)
    await wrapper.get('[aria-label="筛选甘特任务或案件"]').setValue('no match')
    expect(wrapper.text()).toContain('没有符合条件的任务')
  })
  it('reviews cross-year dates and blocks save if holiday or deadline checks fail',async()=>{
    setup();await wrapper.get('[aria-label="移动计划：准备材料"]').trigger('click');await flushPromises()
    const dates=wrapper.findAll('input[type=date]')
    await dates[1].setValue('2027-01-03');await dates[0].setValue('2026-12-30');await flushPromises()
    expect(holidayLookup).toHaveBeenCalledWith(2027)
    expect(eventLookup).toHaveBeenLastCalledWith(2026,12,2)
    holidayLookup.mockResolvedValue({ok:false,error:'offline'})
    await dates[1].setValue('2027-01-04');await flushPromises()
    await wrapper.get('form').trigger('submit')
    expect(save).not.toHaveBeenCalled()
    expect(wrapper.get('[role=alert]').text()).toContain('无法核对')
    holidayLookup.mockImplementation(async(year:number)=>({ok:true,data:{year,entries:[]}}))
    await wrapper.findAll('button').find(b=>b.text()==='重试核对')!.trigger('click');await flushPromises()
    expect(wrapper.find('[role=alert]').exists()).toBe(false)
  })
  it('cancels pointer previews without writing and retains the gesture revision on conflict',async()=>{
    setup();const button=wrapper.get('[aria-label="移动计划：准备材料"]')
    ;(button.element as HTMLElement).setPointerCapture=vi.fn()
    const pointer=async(type:string,clientX=0)=>{ button.element.dispatchEvent(new MouseEvent(type,{bubbles:true,button:0,clientX})); await wrapper.vm.$nextTick() }
    await pointer('pointerdown',100)
    await pointer('pointermove',144)
    await pointer('pointercancel')
    expect(wrapper.find('form').exists()).toBe(false);expect(save).not.toHaveBeenCalled()
    await pointer('pointerdown',100)
    await pointer('pointermove',144)
    await wrapper.setProps({plans:[{...plan,revision:5}]})
    await pointer('pointerup');await flushPromises()
    save.mockResolvedValue({ok:false,error:'PLAN_CONFLICT'})
    await wrapper.get('form').trigger('submit');await flushPromises()
    expect(save).toHaveBeenCalledWith({taskId:'t',startDate:'2026-09-26',endDate:'2026-09-28',expectedRevision:4})
  })

})
