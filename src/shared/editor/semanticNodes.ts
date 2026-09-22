import { Node, mergeAttributes, nodeInputRule } from '@tiptap/core';
import CodeBlock from '@tiptap/extension-code-block';
import { decodeSemantic } from '../markdown/semanticMarkdown';
import { sourceNodeView } from './SourceNodeView';

function semanticNode(name: string, inline: boolean) {
  return Node.create({
    name, group:inline ? 'inline' : 'block', inline, atom:true, selectable:true,
    addAttributes() { return {
      source:{default:'',parseHTML:el=>decodeSemantic(el.getAttribute('data-source')),renderHTML:a=>({'data-source':encodeURIComponent(a.source)})},
      raw:{default:'',parseHTML:el=>decodeSemantic(el.getAttribute('data-raw')),renderHTML:a=>({'data-raw':encodeURIComponent(a.raw)})},
      label:{default:'',parseHTML:el=>el.getAttribute('data-label') || '',renderHTML:a=>({'data-label':a.label})},
    }; },
    parseHTML() { return [{tag:`${inline ? 'span' : 'div'}[data-casy-node="${name}"]`}]; },
    renderHTML({node,HTMLAttributes}) { return [inline ? 'span':'div',mergeAttributes(HTMLAttributes,{'data-casy-node':name}),String(node.attrs.source || node.attrs.label)]; },
    addNodeView() { return sourceNodeView; },
    addInputRules() {
      if (name === 'mathInline') return [nodeInputRule({find:/\$([^$\n]+)\$$/,type:this.type,getAttributes:match=>({source:match[1],raw:match[0]})})];
      if (name === 'footnoteReference') return [nodeInputRule({find:/\[\^([^\]\n]+)\]$/,type:this.type,getAttributes:match=>({label:match[1],raw:match[0]})})];
      return [];
    },
  });
}
export const semanticNodes = [semanticNode('mathInline',true),semanticNode('mathBlock',false),semanticNode('footnoteReference',true),semanticNode('footnoteDefinition',false)];
export const PreviewCodeBlock = CodeBlock.extend({
  addNodeView() { return props => {
    if (props.node.attrs.language === 'mermaid') return sourceNodeView(props);
    const dom=document.createElement('pre'), contentDOM=document.createElement('code');
    dom.append(contentDOM);
    return {dom,contentDOM,update:next=>next.type === props.node.type && next.attrs.language !== 'mermaid'};
  }; },
});
