// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { reactive } from 'vue'
import { flushPromises, shallowMount } from '@vue/test-utils'
import CaseDetailView from '../../src/modules/cases/views/CaseDetailView.vue'
const state=vi.hoisted(()=>({route:null as any,get:vi.fn(),search:vi.fn(),push:vi.fn(),replace:vi.fn()}))
vi.mock('vue-router',()=>({useRoute:()=>state.route,useRouter:()=>({push:state.push,replace:state.replace})}))
vi.mock('element-plus',()=>({ElMessage:{error:vi.fn(),success:vi.fn()}}))
vi.mock('../../src/core/plugin/context',()=>({casyContext:{
  on:()=>()=>{},cases:{get:state.get,listHearings:async(id:string)=>({ok:true,data:[{id:`h-${id}`,hearingName:`庭审 ${id}`,hearingDate:'2026-10-20'}]}),timeline:async()=>({ok:true,data:[{id:'receipt',sourceTable:'procedure_events',eventDate:'2026-09-08',title:'第二轮收文',detail:'电子送达回执'}]}),relations:async()=>({ok:true,data:[]}),caseTypeMetrics:async()=>({ok:true,data:null})},
  tasks:{list:async()=>({ok:true,data:[]})},files:{list:async()=>({ok:true,data:[]})},knowledge:{search:state.search}
}}))
let view:any
afterEach(()=>{view?.unmount();vi.clearAllMocks()})
describe('case detail data wiring',()=>{
  it('keeps case identity across route changes and renders the backend timeline fields',async()=>{
    state.route=reactive({params:{id:'a'},query:{tab:'overview'}})
    let finish:(v:any)=>void=()=>{}
    state.get.mockImplementation(id=>id==='a'?new Promise(resolve=>finish=resolve):Promise.resolve({ok:true,data:{id,caseName:`案件 ${id}`,track:'civil_tort'}}))
    state.search.mockResolvedValue({ok:true,data:[]})
    view=shallowMount(CaseDetailView,{global:{directives:{loading:()=>{}}}})
    state.route.params.id='b';await flushPromises()
    finish({ok:true,data:{id:'a',caseName:'案件 a'}});await flushPromises()
    expect(view.find('.case-title').text()).toBe('案件 b')
    expect(state.search).toHaveBeenCalledWith('案件 b')
    state.route.query.tab='whiteboard';await flushPromises()
    expect(view.findComponent({name:'WhiteboardEntry'}).exists()).toBe(true)
    state.route.query.tab='timeline';await flushPromises()
    expect(view.find('.timeline-stream').text()).toContain('2026-09-08')
    expect(view.find('.timeline-stream').text()).toContain('电子送达回执')
    await view.find('.btn-back').trigger('click')
    expect(state.push).toHaveBeenCalledWith({name:'cases',query:{caseId:'b'}})
  })
})
