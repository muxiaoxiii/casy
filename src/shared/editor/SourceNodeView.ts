import type { NodeViewRendererProps } from '@tiptap/core';
import { scheduleVisiblePreview, renderSourcePreview, captureScrollAnchor } from './previewScheduler';

const pending = new WeakMap<object, Set<() => void>>();
export function flushSourceEditors(editor: object) { for (const commit of pending.get(editor) || []) commit(); }
export function hasSourceDraft(editor: object) { return Boolean(pending.get(editor)?.size); }

/** Editable source stays local until commit; commits use the editor's normal transaction/history. */
export function sourceNodeView(props: NodeViewRendererProps) {
  let node = props.node;
  const kind = node.type.name === 'codeBlock' ? 'mermaid' : node.type.name;
  const inline = node.isInline;
  const dom = document.createElement(inline ? 'span' : 'div');
  dom.className = `source-node source-node--${inline ? 'inline' : 'block'}`;
  dom.contentEditable = 'false';
  dom.dataset.sourceNode = kind;
  const preview = document.createElement(inline ? 'span' : 'div');
  preview.className = 'source-node-preview';
  const open = document.createElement('button'); open.type = 'button';
  open.className = 'source-node-open'; open.textContent = kind.startsWith('footnote') ? '编辑脚注' : '编辑源码';
  const panel = document.createElement('span'); panel.className = 'source-node-editor'; panel.hidden = true;
  const input = document.createElement('textarea'); input.setAttribute('aria-label', kind.startsWith('footnote') ? '脚注正文' : '公式或图表源码');
  input.rows = inline ? 2 : 5;
  const save = document.createElement('button'); save.type='button'; save.textContent='应用';
  const cancel = document.createElement('button'); cancel.type='button'; cancel.textContent='取消';
  panel.append(input,save,cancel); dom.append(preview,open,panel);
  let alive = true, editing = false, composing = false, generation = 0;
  let stop = () => {};
  const source = () => kind === 'mermaid' ? node.textContent : String((kind === 'footnoteReference' ? node.attrs.label : node.attrs.source) || '');
  function paint() {
    stop(); ++generation;
    if (editing) return;
    if (kind.startsWith('footnote')) {
      preview.textContent = kind === 'footnoteReference' ? `[${node.attrs.label}]` : `[${node.attrs.label}] ${source()}`;
      return;
    }
    preview.style.minHeight = `${Math.max(preview.getBoundingClientRect().height, inline ? 24 : 80)}px`;
    preview.textContent = source() || '点击编辑';
    stop = scheduleVisiblePreview(dom, () => {
      const token = ++generation;
      void renderSourcePreview(kind, source()).then(html => {
        if (!alive || token !== generation || editing) return;
        const restore = captureScrollAnchor(dom);
        preview.innerHTML = html; restore();
      }).catch(error => {
        if (alive && token === generation && !editing) preview.textContent = `预览失败：${error.message}（源码已保留）`;
      });
      return () => {
        ++generation;
        preview.style.minHeight = `${Math.max(preview.getBoundingClientRect().height, inline ? 24 : 80)}px`;
        preview.textContent = '预览将在进入视区时恢复';
      };
    });
  }
  function begin() {
    if (!props.editor.isEditable) return;
    editing=true;
    let commits=pending.get(props.editor); if (!commits) { commits=new Set(); pending.set(props.editor,commits); } commits.add(flush);
    ++generation; stop(); input.value=source();
    panel.hidden=false; preview.hidden=true; open.hidden=true;
    input.focus();
  }
  function flush() {
    if (composing) throw new Error('请完成输入法选字后再保存');
    close(true);
    if (editing) throw new Error('请修正脚注标识后再保存');
  }
  function close(commit: boolean) {
    if (!editing || composing) return;
    const value=input.value;
    if (commit && kind === 'footnoteReference' && (!value.trim() || /[\]\n]/.test(value))) { input.setCustomValidity('脚注标识不能为空或包含换行、右方括号'); input.reportValidity(); return; }
    input.setCustomValidity('');
    editing=false; pending.get(props.editor)?.delete(flush); panel.hidden=true; preview.hidden=false; open.hidden=false;
    if (commit && value !== source()) {
      const pos=props.getPos();
      if (typeof pos === 'number') {
        const tr=props.editor.state.tr;
        if (kind === 'mermaid') tr.replaceWith(pos+1,pos+node.nodeSize-1,value ? props.editor.schema.text(value) : []);
        else tr.setNodeMarkup(pos,undefined,{...node.attrs,...(kind==='footnoteReference'?{label:value.trim()}:{source:value}),raw:''});
        props.editor.view.dispatch(tr);
      }
    }
    paint();
  }
  open.addEventListener('click',begin); preview.addEventListener('dblclick',begin);
  save.addEventListener('click',() => close(true)); cancel.addEventListener('click',() => close(false));
  input.addEventListener('compositionstart',()=>{composing=true;});
  input.addEventListener('compositionend',()=>{composing=false; if (!panel.contains(document.activeElement)) close(true);});
  input.addEventListener('keydown',event => {
    if (composing || event.isComposing) return;
    if (event.key === 'Escape') { event.preventDefault(); close(false); }
    else if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') { event.preventDefault(); close(true); }
  });
  panel.addEventListener('focusout',event => {
    if (!panel.contains(event.relatedTarget as globalThis.Node | null)) close(true);
  });
  paint();
  return {
    dom,
    update(next: typeof node) {
      if (next.type !== node.type || (kind === 'mermaid' && next.attrs.language !== 'mermaid')) return false;
      const changed = next.textContent !== node.textContent || next.attrs.source !== node.attrs.source || next.attrs.label !== node.attrs.label;
      node=next;
      if (changed) { editing=false; pending.get(props.editor)?.delete(flush); panel.hidden=true; preview.hidden=false; open.hidden=false; paint(); }
      return true;
    },
    stopEvent: (event: Event) => panel.contains(event.target as globalThis.Node) || event.target === open,
    ignoreMutation: () => true,
    destroy() { pending.get(props.editor)?.delete(flush); alive=false; ++generation; stop(); },
  };
}
