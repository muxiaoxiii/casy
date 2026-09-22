// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { shallowMount, flushPromises } from '@vue/test-utils'
import WhiteboardEntry from '../../src/modules/whiteboard/components/WhiteboardEntry.vue'
const mock=vi.hoisted(()=>({call:vi.fn(),prompt:vi.fn(),push:vi.fn()}))
vi.mock('../../src/core/tauriBridge',()=>({tauriCall:mock.call}))
vi.mock('vue-router',()=>({useRouter:()=>({push:mock.push})}))
vi.mock('element-plus',()=>({ElMessage:{success:vi.fn(),error:vi.fn()},ElMessageBox:{prompt:mock.prompt}}))
let view:any
afterEach(()=>{view?.unmount();vi.clearAllMocks()})
describe('case whiteboard entry',()=>{
  it('keeps the new case preview when the old case responds late',async()=>{
    let finish:(value:any)=>void=()=>{}
    mock.call.mockImplementation((_name,args)=>args.caseId==='a'?new Promise(resolve=>finish=resolve):Promise.resolve([{id:'wb',name:'B白板',caseId:'b',nodeCount:1,preview:'data:image/png;base64,eA=='}]))
    view=shallowMount(WhiteboardEntry,{props:{caseId:'a'},global:{directives:{loading:()=>{}}}})
    await view.setProps({caseId:'b'});await flushPromises();finish([{id:'wa',name:'A白板',caseId:'a'}]);await flushPromises()
    expect(view.text()).toContain('B白板');expect(view.text()).not.toContain('A白板');expect(view.find('img').attributes('src')).toContain('eA==')
    await view.find('.whiteboard-entry__row').trigger('click')
    expect(mock.push).toHaveBeenCalledWith({path:'/whiteboard/b',query:{board:'wb'}})
  })
})
