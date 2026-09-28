// @vitest-environment jsdom
import { expect, it, vi } from 'vitest'
import { ref } from 'vue'
import { mount, flushPromises } from '@vue/test-utils'
import SaveConflictDialog from '../../src/shared/components/SaveConflictDialog.vue'
vi.mock('element-plus',()=>({ElMessage:{success:vi.fn()}}))
it('requires a successful copy of the current local content before applying latest', async () => {
 const local=ref({id:'one',title:'文书',content:'本地正文'})
 const copy=vi.fn().mockRejectedValueOnce(new Error('磁盘满')).mockResolvedValue(undefined)
 const apply=vi.fn()
 const view=mount(SaveConflictDialog,{props:{local:()=>local.value,latest:async()=>({...local.value,content:'其他页面正文'}),copy,apply},global:{stubs:{ElDialog:{template:'<section><slot/><slot name="footer"/></section>'},ElButton:{template:'<button :disabled="$attrs.disabled"><slot/></button>'},ElAlert:{template:'<div>{{$attrs.title}}</div>'}}}})
 const button=(label:string)=>view.findAll('button').find(item=>item.text()===label)!
 ;(view.vm as any).open(); await flushPromises()
 expect(button('载入最新版本').attributes('disabled')).toBeDefined()
 await button('另存本地副本').trigger('click'); await flushPromises()
 expect(view.text()).toContain('磁盘满'); expect(apply).not.toHaveBeenCalled()
 await button('另存本地副本').trigger('click'); await flushPromises()
 expect(button('载入最新版本').attributes('disabled')).toBeUndefined()
 local.value.content='继续编辑后的正文'; await flushPromises()
 expect(button('载入最新版本').attributes('disabled')).toBeDefined()
 await button('另存本地副本').trigger('click'); await flushPromises()
 await button('载入最新版本').trigger('click')
 expect(copy.mock.calls.at(-1)?.[0].content).toBe('继续编辑后的正文')
 expect(apply).toHaveBeenCalledWith(expect.objectContaining({content:'其他页面正文'}))
 view.unmount()
})
