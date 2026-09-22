// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useCasesStore } from '../../src/stores/cases'
const service=vi.hoisted(()=>({list:vi.fn(),get:vi.fn()}))
vi.mock('../../src/core/plugin/context',()=>({casyContext:{cases:service}}))
vi.mock('../../src/core/autoPush',()=>({notifyDataChange:vi.fn()}))
beforeEach(()=>{setActivePinia(createPinia());vi.clearAllMocks()})
describe('case identity under delayed responses',()=>{
  it('keeps the latest selected case and clears a missing target',async()=>{
    let finish:(value:any)=>void=()=>{}
    service.get.mockImplementation(id=>id==='a'?new Promise(resolve=>finish=resolve):Promise.resolve(id==='b'?{ok:true,data:{id:'b'}}:{ok:false,error:'不存在'}))
    const store=useCasesStore();const old=store.loadCase('a')
    await store.loadCase('b');finish({ok:true,data:{id:'a'}});await old
    expect(store.currentCase?.id).toBe('b')
    await store.loadCase('missing');expect(store.currentCase).toBeNull()
  })
  it('does not replace a newer filter result or leave loading stuck on errors',async()=>{
    let finish:(value:any)=>void=()=>{}
    service.list.mockImplementationOnce(()=>new Promise(resolve=>finish=resolve)).mockResolvedValueOnce({ok:true,data:{items:[{id:'b'}],total:1}})
    const store=useCasesStore();const old=store.loadCases();store.filter.search='b';await store.loadCases()
    finish({ok:true,data:{items:[{id:'a'}],total:4}});await old
    expect(store.cases.map(c=>c.id)).toEqual(['b']);expect(store.total).toBe(1)
    service.list.mockRejectedValueOnce(new Error('offline'));await expect(store.loadCases()).rejects.toThrow('offline');expect(store.loading).toBe(false)
  })
})
