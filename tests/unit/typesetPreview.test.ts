// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import TypesetPreview from '../../src/shared/editor/TypesetPreview.vue';
const { call }=vi.hoisted(()=>({call:vi.fn()}));
vi.mock('../../src/core/tauriBridge',()=>({tauriCallSafe:call}));
const layout={title:'测试',caseNo:'',marginMm:20,bindingMm:5,firstLineIndent:true,header:true,skipFirstHeader:true};
const doc={type:'doc',content:[{type:'paragraph'}]};
const reply={ok:true,data:{pages:[{svg:'<svg/>',width:595,height:842}],anchors:[{block:0,page:1,x:70,y:70}],warnings:[],elapsedMs:10}};
afterEach(()=>{vi.useRealTimers();vi.restoreAllMocks();call.mockReset();});
it('coalesces pending edits, rejects stale responses and disables stale block navigation',async()=>{
  vi.useFakeTimers();URL.createObjectURL=vi.fn(()=> 'blob:test');URL.revokeObjectURL=vi.fn();
  let release!:(value:any)=>void;call.mockImplementationOnce(()=>new Promise(resolve=>release=resolve)).mockResolvedValue(reply);
  const wrapper=mount(TypesetPreview,{props:{document:doc,layout,activeBlock:0}});
  await vi.advanceTimersByTimeAsync(550);expect(call).toHaveBeenCalledTimes(1);
  await wrapper.setProps({document:{...doc,content:[{type:'paragraph',content:[{type:'text',text:'新内容'}]}]}});
  await vi.advanceTimersByTimeAsync(550);expect(call).toHaveBeenCalledTimes(1);
  release(reply);await flushPromises();expect(call).toHaveBeenCalledTimes(2);
  expect(call.mock.calls[1][1].document.content[0].content[0].text).toBe('新内容');
  expect(URL.createObjectURL).toHaveBeenCalledTimes(1);
  await wrapper.get('.typeset-anchor').trigger('click');expect(wrapper.emitted('select-block')?.[0]).toEqual([0]);
  await wrapper.setProps({layout:{...layout,marginMm:25}});
  expect(wrapper.get('.typeset-anchor').attributes('disabled')).toBeDefined();
  wrapper.unmount();expect(URL.revokeObjectURL).toHaveBeenCalledWith('blob:test');
});
