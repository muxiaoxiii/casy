// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { Editor } from '@tiptap/core';
import { documentExtensions } from '../../src/shared/editor/schema';
import { mdToHtml, htmlToMd } from '../../src/shared/markdown/mdBridge';
import { flushSourceEditors, hasSourceDraft } from '../../src/shared/editor/SourceNodeView';

const { render }=vi.hoisted(()=>({render:vi.fn()}));
vi.mock('../../src/shared/editor/previewScheduler',()=>({
  renderSourcePreview:render,
  scheduleVisiblePreview:(_el:HTMLElement,run:()=>void)=>{queueMicrotask(run);return ()=>{};},
  captureScrollAnchor:()=>()=>{},
}));
let editor:Editor;
afterEach(()=>{editor?.destroy();document.body.innerHTML='';vi.clearAllMocks();});
function open(source='$x_i$'){
  render.mockResolvedValue('<span>结果</span>');
  const element=document.createElement('div');document.body.append(element);
  editor=new Editor({element,extensions:documentExtensions(),content:mdToHtml(source)});
  return element;
}
const tick=async()=>{await Promise.resolve();await Promise.resolve();};
function begin(el:HTMLElement){el.querySelector<HTMLButtonElement>('.source-node-open')!.click();return el.querySelector<HTMLTextAreaElement>('textarea')!;}
describe('local semantic editing',()=>{
  it('flushes a focused draft through normal history and preserves edited math on reopening',()=>{
    const el=open();const input=begin(el);input.value='a*b*c';
    expect(hasSourceDraft(editor)).toBe(true);flushSourceEditors(editor);
    expect(hasSourceDraft(editor)).toBe(false);
    expect(htmlToMd(editor.getHTML())).toContain('$a*b*c$');
    editor.commands.undo();expect(htmlToMd(editor.getHTML())).toContain('$x_i$');
  });
  it('cancels edits and ignores composition shortcuts; explicit save blocks until composition ends',()=>{
    const el=open();const input=begin(el);input.value='未选字';
    input.dispatchEvent(new CompositionEvent('compositionstart'));
    input.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',metaKey:true,bubbles:true,isComposing:true}));
    expect(()=>flushSourceEditors(editor)).toThrow('选字');
    expect(editor.getJSON().content?.[0].content?.[0].attrs?.source).toBe('x_i');
    input.dispatchEvent(new CompositionEvent('compositionend'));
    input.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}));
    expect(hasSourceDraft(editor)).toBe(false);expect(htmlToMd(editor.getHTML())).toContain('$x_i$');
  });
  it('commits after composition ends when focus already left the node',()=>{
    const el=open();const input=begin(el);input.value='y_j';
    input.dispatchEvent(new CompositionEvent('compositionstart'));input.blur();
    input.dispatchEvent(new CompositionEvent('compositionend'));
    expect(htmlToMd(editor.getHTML())).toContain('$y_j$');
  });
  it('never paints an outdated asynchronous result over a newer node',async()=>{
    let release!:(html:string)=>void;
    const el=open();render.mockImplementationOnce(()=>new Promise<string>(resolve=>{release=resolve;}));
    await tick();const input=begin(el);input.value='z';flushSourceEditors(editor);await tick();
    release('<b>过期结果</b>');await tick();
    expect(el.textContent).not.toContain('过期结果');expect(el.textContent).toContain('结果');
  });
  it('edits Mermaid source without replacing the code node or losing undo',()=>{
    const el=open('```mermaid\nflowchart LR\nA-->B\n```');const input=begin(el);input.value='flowchart TD\nC-->D';
    flushSourceEditors(editor);expect(htmlToMd(editor.getHTML())).toContain('C-->D');
    editor.commands.undo();expect(htmlToMd(editor.getHTML())).toContain('A-->B');
  });
  it('changes only the referenced label and preserves note definition source',()=>{
    const el=open('依据[^law]\n\n[^law]: 第六十五条');const input=begin(el);input.value='law2';flushSourceEditors(editor);
    const saved=htmlToMd(editor.getHTML());expect(saved).toContain('[^law2]');expect(saved).toContain('[^law]: 第六十五条');
  });
});
