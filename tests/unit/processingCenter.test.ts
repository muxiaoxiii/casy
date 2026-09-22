// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils'
import ProcessingCenter from '../../src/shared/components/ProcessingCenter.vue'
const mocks=vi.hoisted(()=>({call:vi.fn(),push:vi.fn(),reveal:vi.fn()}))
vi.mock('../../src/core/tauriBridge',()=>({tauriCallSafe:mocks.call}))
vi.mock('../../src/core/mockData',()=>({isTauriRuntime:()=>true}))
vi.mock('../../src/core/plugin/context',()=>({casyContext:{files:{reveal:mocks.reveal}}}))
vi.mock('vue-router',()=>({useRouter:()=>({push:mocks.push})}))
vi.mock('element-plus',()=>({ElMessage:{error:vi.fn()}}))
let view:VueWrapper|undefined
const state=(jobs:unknown[]=[],total=jobs.length)=>({jobs,services:[],total,active:jobs.length,failed:0})
const job=(id:string,extra={})=>({id,kind:'document',title:id,status:'running',stage:'indexing',caseId:'case-b',caseName:'案件乙',fileId:'file-b',knowledgeId:null,outputPath:null,error:null,current:3,total:3,progress:1,elapsedSeconds:65,remainingSeconds:30,pageTiming:{renderMs:500,ocrMs:2500,layoutMs:1000,totalMs:4200},createdAt:'2026-09-08',updatedAt:'2026-09-08',canCancel:false,...extra})
function mountView(){ view=mount(ProcessingCenter,{global:{stubs:{ElDrawer:{template:'<aside><slot/></aside>'},ElButton:{template:'<button :disabled="$attrs.disabled"><slot/></button>'},ElAlert:{template:'<div>{{$attrs.title}}</div>'},ElProgress:true}}}); return view }
const button=(label:string)=>view!.findAll('button').find(b=>b.text()===label)!
afterEach(()=>{view?.unmount();vi.useRealTimers();vi.clearAllMocks()})
it('shows cross-case indexing as active and navigates with the file view select contract',async()=>{
 mocks.call.mockResolvedValue({ok:true,data:state([job('报告')])});mountView();await flushPromises()
 expect(view!.text()).toContain('建立文档索引');expect(view!.text()).toContain('案件乙')
 expect(view!.text()).toContain('已用 1 分 5 秒');expect(view!.text()).toContain('预计剩余 30 秒')
 expect(view!.find('el-progress-stub').exists()).toBe(false)
 await button('查看来源').trigger('click');expect(mocks.push).toHaveBeenCalledWith({path:'/files/case-b',query:{select:'file-b'}})
})
it('paginates complete history and supports knowledge navigation and real cancellation',async()=>{
 mocks.call.mockImplementation((cmd)=>Promise.resolve(cmd==='get_processing_center'?{ok:true,data:state([job('知识',{kind:'knowledge',knowledgeId:'note',fileId:null,caseId:null,canCancel:true,stage:'embedding'})],65)}:{ok:true}))
 mountView();await flushPromises();await button('下一页').trigger('click');await flushPromises()
 expect(mocks.call).toHaveBeenCalledWith('get_processing_center',{filter:'all',offset:30,limit:30})
 await button('查看来源').trigger('click');expect(mocks.push).toHaveBeenCalledWith({name:'knowledge',query:{select:'note'}})
 await button('取消').trigger('click');await flushPromises();expect(mocks.call).toHaveBeenCalledWith('cancel_knowledge_index_job',{jobId:'知识'})
})
it('never overlaps polling, rejects stale filtered responses, and stops on unmount',async()=>{
 vi.useFakeTimers({toFake:['setTimeout','clearTimeout']})
 let finish!:(v:unknown)=>void
 mocks.call.mockImplementationOnce(()=>new Promise(r=>{finish=r})).mockResolvedValue({ok:false,error:'读取失败'})
 mountView();await button('失败 / 需重建').trigger('click');await vi.advanceTimersByTimeAsync(10000)
 expect(mocks.call).toHaveBeenCalledTimes(1)
 finish({ok:true,data:state([job('过期响应')])});await flushPromises();await vi.advanceTimersByTimeAsync(1);await flushPromises()
 expect(view!.text()).not.toContain('过期响应');expect(view!.text()).toContain('读取失败')
 expect(mocks.call).toHaveBeenLastCalledWith('get_processing_center',{filter:'failed',offset:0,limit:30})
 view!.unmount();view=undefined;const count=mocks.call.mock.calls.length;await vi.advanceTimersByTimeAsync(10000);expect(mocks.call).toHaveBeenCalledTimes(count)
})
