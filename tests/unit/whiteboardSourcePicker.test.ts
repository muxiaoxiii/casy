// @vitest-environment jsdom
import {describe,it,expect,vi} from 'vitest'
import {shallowMount,flushPromises} from '@vue/test-utils'
import WhiteboardSourcePicker from '../../src/modules/whiteboard/components/WhiteboardSourcePicker.vue'
const mock=vi.hoisted(()=>({call:vi.fn()}))
vi.mock('../../src/core/tauriBridge',()=>({tauriCallSafe:mock.call}))
const item=(title:string)=>({id:title,kind:'knowledge',title,preview:'核对原文',caseId:null,caseName:null,totalPages:null})
const stubs={'el-dialog':{template:'<div><slot/><slot name="footer"/></div>'},'el-button':{props:['disabled'],template:'<button :disabled="disabled"><slot/></button>'}}
describe('source picker',()=>{
 it('does not let a late prior search replace the selected scope and reference',async()=>{
  let first:(v:any)=>void=()=>{};mock.call.mockImplementationOnce(()=>new Promise(resolve=>first=resolve)).mockResolvedValueOnce({ok:true,data:{items:[item('知识引用')],total:1}})
  const view=shallowMount(WhiteboardSourcePicker,{props:{modelValue:false,caseId:'case-a'},global:{stubs,directives:{loading:()=>{}}}})
  await view.setProps({modelValue:true});await view.find('el-radio-group').trigger('change');await flushPromises()
  first({ok:true,data:{items:[item('过期结果')],total:1}});await flushPromises()
  expect(view.text()).toContain('知识引用');expect(view.text()).not.toContain('过期结果')
  await view.find('.source-result').trigger('click');expect(view.text()).toContain('核对原文')
  await view.findAll('button').find(b=>b.text()==='使用此来源')!.trigger('click')
  expect(view.emitted('select')?.[0]?.[0]).toEqual(item('知识引用'));view.unmount()
 })
})
