// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest'
import { shallowMount, flushPromises } from '@vue/test-utils'
import SmtpMcpSettings from '../../src/modules/settings/components/SmtpMcpSettings.vue'
const keychainStatus = vi.hoisted(()=>vi.fn().mockResolvedValue({ok:true,data:{accounts:[]}}))
vi.mock('../../src/core/plugin/context',()=>({casyContext:{settings:{keychainStatus,mcpPendingWrites:vi.fn().mockResolvedValue({ok:true,data:[]})},calendar:{calendarSyncStatus:vi.fn().mockResolvedValue({ok:true,data:{}})},mcp:{listPendingWrites:vi.fn().mockResolvedValue({ok:true,data:[]})}}}))
vi.mock('../../src/stores/settings',()=>({useSettingsStore:()=>({})}))
describe('credential access is deliberate',()=>{
  it('does not probe the system keychain on mount',async()=>{
    const view=shallowMount(SmtpMcpSettings,{global:{renderStubDefaultSlot:true,stubs:{ElButton:{template:'<button><slot/></button>'},ElCard:{template:'<section><slot/></section>'},ElForm:{template:'<form><slot/></form>'},ElFormItem:{template:'<div><slot/></div>'}}}})
    await flushPromises()
    expect(keychainStatus).not.toHaveBeenCalled()
    const button=view.findAll('button').find(b=>b.text()==='检测钥匙串')!
    await button.trigger('click')
    await flushPromises()
    expect(keychainStatus).toHaveBeenCalledOnce()
    view.unmount()
  })
})
