import { DOMParser, DOMSerializer, type Node as PMNode } from '@tiptap/pm/model';
import { htmlToMd, markdownSourceBlocks } from '../markdown/mdBridge';

/** PM shares unchanged nodes across transactions; source bytes follow that immutable identity. */
export class MarkdownPreservation {
  private originals = new WeakMap<PMNode, {raw:string; index:number}>();
  private trailing = '';
  bind(source:string, doc:PMNode) {
    this.originals=new WeakMap(); this.trailing='';
    let child=0, pending='', index=0;
    for (const block of markdownSourceBlocks(source)) {
      if(block.definition) { this.trailing+=pending+block.raw; pending=''; continue; }
      const el=document.createElement('div'); el.innerHTML=block.html;
      if (!el.childNodes.length) { pending+=block.raw; continue; }
      const parsed=DOMParser.fromSchema(doc.type.schema).parse(el);
      if (parsed.childCount === 1 && child < doc.childCount && parsed.child(0).eq(doc.child(child))) {
        this.originals.set(doc.child(child),{raw:pending+block.raw,index:index++}); child++; pending='';
      } else {
        // Multi-node blocks (e.g. an inline image) cannot safely be split into raw fragments.
        // Keep definitions even when a preceding token cannot be mapped.
        this.trailing+=pending; pending='';
        child+=parsed.childCount;
        index++;
      }
    }
    this.trailing+=pending;
  }
  serialize(doc:PMNode):string {
    let result='', previousIndex=-2;
    const serializer=DOMSerializer.fromSchema(doc.type.schema);
    doc.forEach(node => {
      const original=this.originals.get(node);
      const el=document.createElement('div');
      let raw:string;
      if (original) raw=original.raw;
      else { el.append(serializer.serializeNode(node)); raw=htmlToMd(el.innerHTML); }
      if (!raw) return;
      if (result && !(original && original.index === previousIndex+1) && !/\n\s*\n$/.test(result) && !/^\s*\n/.test(raw)) result+='\n\n';
      result+=raw;
      previousIndex=original?.index ?? -2;
    });
    if (this.trailing.trim()) result+=`${result.endsWith('\n\n') ? '':'\n\n'}${this.trailing}`;
    return result;
  }
}
