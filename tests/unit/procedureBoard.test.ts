// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, shallowMount } from '@vue/test-utils'
import ProcedureBoard from '../../src/modules/cases/components/ProcedureBoard.vue'
const call=vi.hoisted(()=>vi.fn())
vi.mock('../../src/core/tauriBridge',()=>({tauriCallSafe:call}))
vi.mock('element-plus',()=>({ElMessage:{error:vi.fn(),success:vi.fn()},ElMessageBox:{prompt:vi.fn()}}))
const board=(id:string)=>({cases:[{id,name:`案件 ${id}`,track:'patent_invalidation',ourRole:'专利权人',status:'进行中'}],events:[],items:[],coordination:[],ruleVersion:'test'})
const global={renderStubDefaultSlot:true,directives:{loading:()=>{}},stubs:{'el-button':{template:'<button><slot/></button>'},'el-dialog':{template:'<div />'},'el-tag':{template:'<span><slot/></span>'}}}
let view:ReturnType<typeof shallowMount>|undefined
afterEach(()=>{view?.unmount();vi.clearAllMocks()})
describe('procedure board',()=>{
  it('ignores a stale case response after navigating to a linked case',async()=>{
    let finishA:(value:unknown)=>void=()=>{}
    call.mockImplementation((_command,args)=>args.caseId==='a'?new Promise(resolve=>{finishA=resolve}):Promise.resolve({ok:true,data:board('b')}))
    view=shallowMount(ProcedureBoard,{props:{caseId:'a'},global})
    await view.setProps({caseId:'b'});await flushPromises()
    finishA({ok:true,data:board('a')});await flushPromises()
    expect(view.text()).toContain('案件 b');expect(view.text()).not.toContain('案件 a')
    await view.find('.case-chip').trigger('click')
    expect(view.emitted('openCase')).toEqual([['b']])
  })
  it('shows uncertain dates and opponent duties without labelling them our deadline',async()=>{
    const data={...board('a'),items:[{id:'i',eventId:'',caseId:'a',caseName:'案件 a',track:'patent_invalidation',ourRole:'专利权人',actorRole:'请求人',owner:'opponent',title:'请求人补充理由',kind:'deadline',dueOn:'2026-10-08',rawDueOn:'2026-10-07',source:'legacy',explanation:'核对原始通知',legalBasis:'第71条',legalUrl:'',status:'open',stateNote:'',fingerprint:'x',daysLeft:30,needsReview:true,editable:false}]}
    call.mockResolvedValue({ok:true,data})
    view=shallowMount(ProcedureBoard,{props:{caseId:'a'},global});await flushPromises()
    const row=view.find('.procedure-item').text()
    expect(row).toContain('对方期限');expect(row).toContain('旧字段待核对');expect(row).toContain('2026-10-08');expect(row).not.toContain('我方办理')
    expect(view.findAll('.procedure-item')).toHaveLength(1)
  })
})
