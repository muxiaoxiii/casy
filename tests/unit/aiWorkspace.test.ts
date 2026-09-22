import { afterEach, describe, expect, it, vi } from 'vitest'
import { casyContext } from '../../src/core/plugin/context'
import { defineTool } from '../../src/core/plugin/defineTool'
import { aiToolCaller } from '../../src/core/ai/tool-caller'
import { tauriCallSafe } from '../../src/core/tauriBridge'

vi.mock('../../src/core/tauriBridge', () => ({ tauriCallSafe: vi.fn(), tauriCall: vi.fn() }))
afterEach(() => { casyContext.unregisterTool('test_read'); casyContext.unregisterTool('test_undeclared'); vi.resetAllMocks() })
describe('AI workspace tools', () => {
  it('rejects bad arguments, repairs them and reports actual tool progress', async () => {
    const execute = vi.fn().mockResolvedValue({ ok: true, data: { title: '事实笔记' } })
    casyContext.registerTool(defineTool<{ id: string }>({ name: 'test_read', category: 'knowledge', description: '读取笔记', policy: { write: false, level: 'L1' }, parameters: { type: 'object', properties: { id: { type: 'string' } }, required: ['id'] }, execute }))
    vi.mocked(tauriCallSafe)
      .mockResolvedValueOnce({ ok: true, data: { content: '{"tool":"test_read","params":{"id":4}}' } } as never)
      .mockResolvedValueOnce({ ok: true, data: { content: '{"tool":"test_read","params":{"id":"k1"}}' } } as never)
      .mockResolvedValueOnce({ ok: true, data: { content: '已读取事实笔记。' } } as never)
    const onProgress = vi.fn()
    const result = await aiToolCaller.chatWithTools([{ role: 'user', content: '读取笔记' }], { onProgress })
    expect(execute).toHaveBeenCalledOnce()
    expect(execute).toHaveBeenCalledWith({ id: 'k1' })
    expect(result.toolResults).toEqual([{ ok: true, data: { title: '事实笔记' } }])
    expect(onProgress).toHaveBeenCalledWith('读取 · 读取笔记')
  })
  it('does not execute an undeclared tool or execute after cancellation', async () => {
    const execute = vi.fn().mockResolvedValue({ ok: true })
    casyContext.registerTool(defineTool({ name: 'test_undeclared', category: 'settings', description: '未声明权限', parameters: { type: 'object' }, execute }))
    vi.mocked(tauriCallSafe)
      .mockResolvedValueOnce({ ok: true, data: { content: '{"tool":"test_undeclared","params":{}}' } } as never)
      .mockResolvedValueOnce({ ok: true, data: { content: '此工具不可用。' } } as never)
    await aiToolCaller.chatWithTools([{ role: 'user', content: 'test' }])
    expect(execute).not.toHaveBeenCalled()
    const controller = new AbortController(); controller.abort()
    const calls = vi.mocked(tauriCallSafe).mock.calls.length
    const result = await aiToolCaller.chatWithTools([], { signal: controller.signal })
    expect(result.content).toContain('已停止')
    expect(vi.mocked(tauriCallSafe).mock.calls.length).toBe(calls)
  })
})

it('stops a repeatedly failing tool after two attempts',async()=>{
 const execute=vi.fn().mockResolvedValue({ok:false,error:'数据库暂不可用'})
 casyContext.registerTool(defineTool({name:'test_read',category:'knowledge',description:'读取',policy:{write:false,level:'L1'},parameters:{type:'object'},execute}))
 vi.mocked(tauriCallSafe).mockResolvedValue({ok:true,data:{content:'{"tool":"test_read","params":{}}'}} as never)
 const result=await aiToolCaller.chatWithTools([{role:'user',content:'读取'}])
 expect(execute).toHaveBeenCalledTimes(2);expect(result.content).toContain('停止重复调用')
})
it('keeps oversized tool output inside a valid JSON envelope',async()=>{
 casyContext.registerTool(defineTool({name:'test_read',category:'knowledge',description:'读取',policy:{write:false,level:'L1'},parameters:{type:'object'},execute:async()=>({ok:true,data:{content:'长文'.repeat(10000)}})}))
 vi.mocked(tauriCallSafe).mockResolvedValueOnce({ok:true,data:{content:'{"tool":"test_read","params":{}}'}} as never).mockResolvedValueOnce({ok:true,data:{content:'需要缩小查询'}} as never)
 await aiToolCaller.chatWithTools([{role:'user',content:'读取'}])
 const args=vi.mocked(tauriCallSafe).mock.calls[1]![1] as any
 const response=args.messages.at(-1).content as string
 const envelope=JSON.parse(response.slice(response.indexOf('\n')+1))
 expect(envelope.truncated).toBe(true);expect(envelope.totalCharacters).toBeGreaterThan(12000)
})
