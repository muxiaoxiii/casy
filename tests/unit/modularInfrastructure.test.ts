// @vitest-environment jsdom
import {describe,it,expect,vi,afterEach} from 'vitest'
import {effectScope} from 'vue'
const mocks=vi.hoisted(()=>({invoke:vi.fn(),listen:vi.fn()}))
vi.mock('@tauri-apps/api/core',()=>({invoke:mocks.invoke}))
vi.mock('@tauri-apps/api/event',()=>({listen:mocks.listen}))
vi.mock('../../src/core/mockData',()=>({isTauriRuntime:()=>true,tryMockCommand:()=>undefined}))
vi.mock('element-plus',()=>({ElMessage:{error:vi.fn()}}))
import {safeListen} from '../../src/core/tauriEvents'
import {invokeWithDeadline} from '../../src/core/tauriBridge'
import {useLocalDay} from '../../src/shared/useLocalDay'
afterEach(()=>{vi.useRealTimers();vi.clearAllMocks()})
describe('modular infrastructure regressions',()=>{
 it('cleans up listeners that resolve after owner disposal and suppresses late events',async()=>{
  let resolve!:(off:()=>void)=>void,handler!:(e:any)=>void
  mocks.listen.mockImplementation((_event,cb)=>{handler=cb;return new Promise(r=>resolve=r)})
  const scope=effectScope(),receive=vi.fn(),off=vi.fn()
  scope.run(()=>safeListen('late',receive));scope.stop();resolve(off);await Promise.resolve()
  handler({payload:'late'});expect(off).toHaveBeenCalledOnce();expect(receive).not.toHaveBeenCalled()
 })
 it('bounds a hung IPC wait without retrying or claiming cancellation',async()=>{
  vi.useFakeTimers();let finish!:(value:string)=>void
  mocks.invoke.mockImplementation(()=>new Promise(r=>finish=r))
  const request=invokeWithDeadline('save_draft',{},50)
  const rejection=expect(request).rejects.toThrow('后台操作可能仍在进行')
  await vi.advanceTimersByTimeAsync(51);await rejection
  finish('committed');await Promise.resolve();expect(mocks.invoke).toHaveBeenCalledOnce()
 })
 it('updates mounted rows across local midnight and stops the shared timer',()=>{
  vi.useFakeTimers();vi.setSystemTime(new Date(2026,8,17,23,59,59))
  const scope=effectScope();const today=scope.run(()=>useLocalDay())!
  expect(today.value).toBe('2026-09-17');vi.advanceTimersByTime(30000);expect(today.value).toBe('2026-09-18')
  scope.stop();expect(vi.getTimerCount()).toBe(0)
 })
})
