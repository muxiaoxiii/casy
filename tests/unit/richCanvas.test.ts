// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { shallowMount, flushPromises } from '@vue/test-utils'
import RichCanvas from '../../src/modules/whiteboard/components/RichCanvas.vue'
const mock=vi.hoisted(()=>({call:vi.fn(),push:vi.fn(),props:null as any,leave:null as any,update:null as any,state:{scrollX:0,scrollY:0,width:800,height:600,zoom:{value:1}},elements:[] as any[]}))
vi.mock('../../src/core/tauriBridge',()=>({tauriCallSafe:mock.call}))
vi.mock('../../src/core/plugin/context',()=>({casyContext:{}}))
vi.mock('vue-router',()=>({useRouter:()=>({push:mock.push}),onBeforeRouteLeave:(fn:any)=>mock.leave=fn,onBeforeRouteUpdate:(fn:any)=>mock.update=fn}))
vi.mock('element-plus',()=>({ElMessage:{error:vi.fn(),info:vi.fn(),warning:vi.fn(),success:vi.fn()},ElMessageBox:{confirm:vi.fn()}}))
vi.mock('react-dom/client',()=>({createRoot:()=>({render:(element:any)=>{mock.props=element.props;document.querySelector('.excalidraw-host')?.append(document.createElement('canvas'));mock.props.onChange([],mock.state,{});mock.props.excalidrawAPI({getSceneElementsIncludingDeleted:()=>mock.elements,getAppState:()=>mock.state,getFiles:()=>({}),updateScene:vi.fn(),scrollToContent:vi.fn()})},unmount:vi.fn()})}))
vi.mock('@excalidraw/excalidraw',()=>({Excalidraw:()=>null,MainMenu:Object.assign(()=>null,{Item:()=>null}),convertToExcalidrawElements:()=>[],CaptureUpdateAction:{NEVER:'NEVER',IMMEDIATELY:'IMMEDIATELY'},exportToBlob:vi.fn(async()=>new Blob(['png'],{type:'image/png'}))}))
let view:any
beforeEach(()=>{vi.clearAllMocks();vi.stubGlobal('requestAnimationFrame',(cb:any)=>cb());mock.call.mockImplementation((name:string)=>Promise.resolve(name==='get_whiteboard_document'?{ok:true,data:{scene:{sceneJson:'{"elements":[],"appState":{},"files":{}}',revision:0,preview:null},facts:[],edges:[],factsHash:'hash'}}:{ok:true,data:{scene:{revision:1},factsHash:'new'}}))})
afterEach(()=>{view?.unmount();document.body.innerHTML='';vi.unstubAllGlobals()})
describe('offline canvas persistence',()=>{
  it('opens file references with an optional page and the recorded owning case',async()=>{
    view=shallowMount(RichCanvas,{attachTo:document.body,props:{caseId:'case-a',boardId:'board-a'}});await flushPromises()
    for(const [link,query] of [
      ['casy-ref:file:file-id:case-b:',{select:'file-id'}],
      ['casy-ref:file:file-id:case-b',{select:'file-id'}],
      ['casy-ref:file:file-id:case-b:12',{select:'file-id',anchor:'page:12'}],
    ] as const){
      const event={preventDefault:vi.fn()}
      mock.props.onLinkOpen({link},event);await flushPromises()
      expect(event.preventDefault).toHaveBeenCalled()
      expect(mock.push).toHaveBeenLastCalledWith({name:'files',params:{caseId:'case-b'},query})
    }
  })

  it('saves referenced screenshot data and excludes unused image data and transient state',async()=>{
    view=shallowMount(RichCanvas,{attachTo:document.body,props:{caseId:'case-a',boardId:'board-a'}});await flushPromises()
    mock.props.onChange([{id:'image',type:'image',fileId:'pic',isDeleted:false}],{selectedElementIds:{image:true},collaborators:new Map(),scrollX:2},{pic:{id:'pic',dataURL:'data:image/png;base64,eA=='},unused:{dataURL:'data:image/png;base64,eQ=='}})
    expect(await view.vm.flush()).toBe(true)
    const args=mock.call.mock.calls.find((c:any)=>c[0]==='save_whiteboard_document')![1].input
    expect(args.caseId).toBe('case-a');expect(args.whiteboardId).toBe('board-a')
    const scene=JSON.parse(args.sceneJson)
    expect(scene.files.pic.dataURL).toContain('eA==');expect(scene.files.unused).toBeUndefined();expect(scene.appState.selectedElementIds).toBeUndefined()
  })
  it('blocks navigation on a conflicting save and keeps the original board target',async()=>{
    view=shallowMount(RichCanvas,{attachTo:document.body,props:{caseId:'case-a',boardId:'board-a'}});await flushPromises()
    mock.call.mockImplementation((name:string)=>Promise.resolve(name==='save_whiteboard_document'?{ok:false,error:'白板已在其他窗口更新'}:{ok:true,data:[]}))
    mock.props.onChange([{id:'text',type:'text',isDeleted:false}],{},{});
    await view.setProps({caseId:'case-b',boardId:'board-b'})
    expect(await mock.leave()).toBe(false)
    const args=mock.call.mock.calls.find((c:any)=>c[0]==='save_whiteboard_document')![1].input
    expect(args.caseId).toBe('case-a');expect(view.text()).toContain('白板已在其他窗口更新')
  })
})
