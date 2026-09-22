// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest'
import { shallowMount, flushPromises } from '@vue/test-utils'
import DocumentSourceViewer from '../../src/modules/files/components/DocumentSourceViewer.vue'
const call=vi.hoisted(()=>vi.fn())
vi.mock('../../src/core/tauriBridge',()=>({tauriCallSafe:call}))
vi.mock('element-plus',()=>({ElMessage:{error:vi.fn(),success:vi.fn()}}))
describe('document table preview',()=>{
  it('keeps source figures and collapsed OCR while blocking active attributes',async()=>{
    call.mockResolvedValue({ok:true,data:{fileName:'fixture.pdf',totalPages:1,width:400,height:600,imageData:null,regions:[],markdown:'<figure><img src="data:image/png;base64,iVBORw0KGgo=" onerror="alert(1)"><figcaption>公式原图</figcaption></figure><details><summary>辅助识别文字</summary><p>x &lt; y</p></details>'}})
    const view=shallowMount(DocumentSourceViewer,{props:{modelValue:true,fileId:'f',jobId:'j'},global:{stubs:{ElDialog:{template:'<section><slot/></section>'}}}})
    await flushPromises()
    expect(view.find('figure img').attributes('src')).toMatch(/^data:image\/png;base64,/)
    expect(view.find('figure img').attributes('onerror')).toBeUndefined()
    expect(view.find('details').attributes('open')).toBeUndefined()
    expect(view.find('summary').text()).toBe('辅助识别文字')
    view.unmount()
  })
  it('opens the requested page as a rendered table, retaining merged cells and comparison signs',async()=>{
    call.mockResolvedValue({ok:true,data:{fileName:'fixture.pdf',totalPages:3,width:400,height:600,imageData:null,regions:[],markdown:'<table><tr><td rowspan="2">依据</td><td>&lt;0.001</td></tr><tr><td>0.0041</td></tr></table>'}})
    const view=shallowMount(DocumentSourceViewer,{props:{modelValue:true,fileId:'f',jobId:'j',initialPage:3},global:{stubs:{ElDialog:{template:'<section><slot/></section>'}}}})
    await flushPromises()
    expect(call).toHaveBeenCalledWith('get_document_page',{fileId:'f',jobId:'j',pageNumber:3})
    expect(view.find('.source-markdown table').exists()).toBe(true)
    expect(view.find('td').attributes('rowspan')).toBe('2')
    expect(view.find('.source-markdown').text()).toContain('<0.001')
    view.unmount()
  })
})
