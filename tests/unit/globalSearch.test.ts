// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import ElementPlus from 'element-plus'
import GlobalSearch from '../../src/components/GlobalSearch.vue'

const h = vi.hoisted(() => ({ search:vi.fn(), tasks:vi.fn(), cases:vi.fn(), projects:vi.fn(), push:vi.fn() }))
vi.mock('vue-router', () => ({ useRouter:() => ({push:h.push}) }))
vi.mock('../../src/core/plugin/context', () => ({casyContext:{
  tasks:{searchTasks:h.tasks},cases:{search:h.cases},projects:{list:h.projects},knowledge:{searchIndex:h.search},
}}))
const hit = (id:string) => ({id,title:id,content:'命中正文',source:'semantic',category:'note'})
const response = (id:string) => ({ok:true,data:{results:[hit(id)],semanticStatus:'ready',warning:null}})
let wrapper:ReturnType<typeof mount>
beforeEach(() => {
  vi.useFakeTimers()
  vi.clearAllMocks()
  localStorage.clear()
  h.tasks.mockResolvedValue({ok:true,data:[]})
  h.cases.mockResolvedValue({ok:true,data:[]})
  h.projects.mockResolvedValue({ok:true,data:[]})
  h.push.mockResolvedValue(undefined)
  wrapper = mount(GlobalSearch,{props:{modelValue:true},global:{plugins:[ElementPlus],stubs:{teleport:true,transition:false}}})
})
afterEach(() => {wrapper.unmount();vi.useRealTimers()})

it('shows keywords before semantic completion and opens the actual note',async () => {
  let finish!:(value:unknown)=>void
  h.search.mockImplementation((_q:string,semantic:boolean) => semantic ? new Promise(resolve=>{finish=resolve}) : Promise.resolve(response('关键词笔记')))
  wrapper.findComponent({name:'ElRadioGroup'}).vm.$emit('update:modelValue','hybrid')
  await wrapper.find('input.cmdk-input').setValue('赔偿')
  await vi.advanceTimersByTimeAsync(200)
  await flushPromises()
  expect(wrapper.text()).toContain('关键词笔记')
  await vi.advanceTimersByTimeAsync(400)
  finish(response('语义笔记'))
  await flushPromises()
  expect(wrapper.text()).toContain('语义笔记')
  await wrapper.find('.cmdk-item').trigger('click')
  expect(h.push).toHaveBeenCalledWith('/knowledge?select='+encodeURIComponent('语义笔记'))
})

it('discards late queries after typing a new query or closing the dialog',async () => {
  let old!:(value:unknown)=>void
  h.search.mockImplementation((q:string) => q==='旧查询' ? new Promise(resolve=>{old=resolve}) : Promise.resolve(response('新结果')))
  await wrapper.find('input.cmdk-input').setValue('旧查询')
  await vi.advanceTimersByTimeAsync(200)
  await wrapper.find('input.cmdk-input').setValue('新查询')
  await vi.advanceTimersByTimeAsync(200)
  await flushPromises()
  old(response('过时结果'))
  await flushPromises()
  expect(wrapper.text()).toContain('新结果')
  expect(wrapper.text()).not.toContain('过时结果')
  await wrapper.setProps({modelValue:false})
  expect(wrapper.find('.cmdk-panel').exists()).toBe(false)
})

it('retains keywords and shows a failure when semantic retrieval is unavailable',async () => {
  h.search.mockImplementation((_q:string,semantic:boolean) => Promise.resolve(semantic ? {ok:false,error:'索引不可用'} : response('保留结果')))
  wrapper.findComponent({name:'ElRadioGroup'}).vm.$emit('update:modelValue','hybrid')
  await wrapper.find('input.cmdk-input').setValue('赔偿')
  await vi.advanceTimersByTimeAsync(600)
  await flushPromises()
  expect(wrapper.text()).toContain('保留结果')
  expect(wrapper.text()).toContain('索引不可用')
})
