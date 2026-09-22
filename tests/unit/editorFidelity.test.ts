// @vitest-environment jsdom
import { describe,it,expect,vi } from 'vitest';
import { Editor } from '@tiptap/core';
import { documentExtensions } from '../../src/shared/editor/schema';
import { mdToHtml,htmlToMd } from '../../src/shared/markdown/mdBridge';
import { MarkdownPreservation } from '../../src/shared/editor/markdownPreservation';
import { scheduleVisiblePreview, captureScrollAnchor } from '../../src/shared/editor/previewScheduler';

function open(source:string) { return new Editor({extensions:documentExtensions(),content:mdToHtml(source)}); }
describe('editor semantic fidelity',()=>{
  it('keeps standalone link definitions when the following paragraph is deleted',()=>{
    const source='参见[原件][ev]。\n\n[ev]: https://example.com\n\n删除本段';
    const editor=open(source),preserve=new MarkdownPreservation();try{
      preserve.bind(source,editor.state.doc);
      const last=editor.state.doc.lastChild!;
      editor.commands.deleteRange({from:editor.state.doc.content.size-last.nodeSize,to:editor.state.doc.content.size});
      const saved=preserve.serialize(editor.state.doc);expect(saved).toContain('[ev]: https://example.com');
      const reopened=open(saved);try {expect(JSON.stringify(reopened.getJSON())).toContain('https://example.com');}finally{reopened.destroy();}
    }finally{editor.destroy();}
  });
  it('uses updated semantic attributes instead of stale raw bytes and preserves multiline source',()=>{
    const editor=open('$x_i$');try{
      editor.view.dispatch(editor.state.tr.setNodeMarkup(1,undefined,{...editor.state.doc.nodeAt(1)!.attrs,source:'a*b*c'}));
      expect(htmlToMd(editor.getHTML())).toContain('$a*b*c$');
      editor.view.dispatch(editor.state.tr.setNodeMarkup(1,undefined,{...editor.state.doc.nodeAt(1)!.attrs,source:'a\n+ b'}));
      const reopened=open(htmlToMd(editor.getHTML()));try{expect(reopened.state.doc.nodeAt(1)!.attrs.source).toBe('a\n+ b');}finally{reopened.destroy();}
    }finally{editor.destroy();}
  });
  it('preserves notes, math, H5/H6, alignment and widths after an unrelated edit and reopening',()=>{
    const source='##### 五级\n\n###### 六级\n\n依据[^law]，公式 $a*b*c$ 与 $x_i + y_i$。\n\n[^law]: 第六十五条。\n\n| 名称 | 金额 |\n| :--- | ---: |\n| 费用 | 100 |';
    const editor=open(source);try {
      editor.commands.insertContentAt(editor.state.doc.content.size,{type:'paragraph',content:[{type:'text',text:'追加'}]});
      const saved=htmlToMd(editor.getHTML());const reopened=open(saved);try{
        expect(saved).toContain('[^law]: 第六十五条。');expect(saved).toContain('$a*b*c$');expect(saved).toContain('$x_i + y_i$');
        const nodes:any[]=[];reopened.state.doc.descendants(n=>{nodes.push(n.toJSON());});
        expect(nodes.filter(n=>n.type==='heading').map(n=>n.attrs.level)).toEqual([5,6]);
        expect(nodes.find(n=>n.type==='footnoteReference').attrs.label).toBe('law');
        expect(nodes.find(n=>n.type==='footnoteDefinition').attrs.source).toBe('第六十五条。');
        expect(nodes.filter(n=>n.type==='tableCell').map(n=>n.attrs.align)).toEqual(['left','right']);
        expect(nodes.filter(n=>n.type==='mathInline').map(n=>n.attrs.source)).toEqual(['a*b*c','x_i + y_i']);
      }finally{reopened.destroy();}
    }finally{editor.destroy();}
  });
  it('reuses untouched original blocks, including reference definitions, through edit and undo',()=>{
    const source='## 原标题\n\n* 原项目\n* 第二项\n\n参见[原件][ev]。\n\n[ev]: https://example.com "标题"\n';
    const editor=open(source),preserve=new MarkdownPreservation();try{
      preserve.bind(source,editor.state.doc);
      editor.commands.insertContentAt(editor.state.doc.content.size,{type:'paragraph',content:[{type:'text',text:'补充'}]});
      const saved=preserve.serialize(editor.state.doc);
      expect(saved).toContain('* 原项目\n* 第二项');expect(saved).toContain('参见[原件][ev]');expect(saved).toContain('[ev]: https://example.com "标题"');
      editor.commands.undo();expect(preserve.serialize(editor.state.doc)).not.toContain('补充');
      expect(preserve.serialize(editor.state.doc)).toContain('* 原项目');
    }finally{editor.destroy();}
  });
  it('keeps math and footnote markers literal in code and blocks executable style during reload',()=>{
    const editor=open('`$x_i$ [^x]`\n\n```text\n$$a*b$$\n[^x]: code\n```');try{
      const json=JSON.stringify(editor.getJSON());expect(json).not.toContain('mathInline');expect(json).not.toContain('footnoteReference');
      const html=mdToHtml('<table><tr><td data-text-align="right" style="background:url(https://evil.example)"><p>x</p></td></tr></table>');
      expect(html).toContain('data-text-align="right"');expect(html).not.toContain('background:');
    }finally{editor.destroy();}
  });
  it('preserves modified merged-cell widths and alignment as safe attributes',()=>{
    const editor=open('| a | b |\n|---|---|\n| x | y |');try{
      let pos=0;editor.state.doc.descendants((n,p)=>{if(!pos&&n.type.name==='tableCell')pos=p;});
      editor.view.dispatch(editor.state.tr.setNodeMarkup(pos,undefined,{...editor.state.doc.nodeAt(pos)!.attrs,align:'right',colwidth:[180]}));
      const reopened=open(htmlToMd(editor.getHTML()));try{
        const cells:any[]=[];reopened.state.doc.descendants(n=>{if(n.type.name==='tableCell')cells.push(n.attrs);});
        expect(cells[0]).toMatchObject({align:'right',colwidth:[180]});
      }finally{reopened.destroy();}
    }finally{editor.destroy();}
  });
});
it('does not start a deferred preview after its NodeView is removed',async()=>{
  const render=vi.fn();const stop=scheduleVisiblePreview(document.createElement('div'),render);stop();await Promise.resolve();expect(render).not.toHaveBeenCalled();
});
it('releases offscreen preview work and restarts it only when visible again',()=>{
  let callback!:(entries:Array<{isIntersecting:boolean}>)=>void;
  const disconnect=vi.fn(),cleanup=vi.fn(),render=vi.fn(()=>cleanup);
  vi.stubGlobal('IntersectionObserver',class { constructor(cb:typeof callback){callback=cb;} observe(){} disconnect=disconnect; });
  try{
    const stop=scheduleVisiblePreview(document.createElement('div'),render);
    callback([{isIntersecting:true}]);callback([{isIntersecting:true}]);expect(render).toHaveBeenCalledTimes(1);
    callback([{isIntersecting:false}]);expect(cleanup).toHaveBeenCalledTimes(1);
    callback([{isIntersecting:true}]);expect(render).toHaveBeenCalledTimes(2);
    stop();expect(cleanup).toHaveBeenCalledTimes(2);expect(disconnect).toHaveBeenCalled();
  }finally{vi.unstubAllGlobals();}
});
it('compensates the height change of a rendered block above the viewport',()=>{
  const root=document.createElement('div'),element=document.createElement('div');root.style.overflowY='auto';root.append(element);document.body.append(root);
  root.scrollTop=100;let bottom=-20;
  root.getBoundingClientRect=()=>({top:0} as DOMRect);element.getBoundingClientRect=()=>({bottom} as DOMRect);
  const restore=captureScrollAnchor(element);bottom=30;restore();expect(root.scrollTop).toBe(150);root.remove();
});
