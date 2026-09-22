import type { TokenizerExtension, RendererExtension } from 'marked';

export type SemanticKind = 'mathInline' | 'mathBlock' | 'footnoteReference' | 'footnoteDefinition';
export function semanticHtml(kind: SemanticKind, source: string, raw: string, label = ''): string {
  const tag = kind === 'mathInline' || kind === 'footnoteReference' ? 'span' : 'div';
  const escape = (s: string) => s.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  return `<${tag} data-casy-node="${kind}" data-source="${escape(encodeURIComponent(source))}" data-raw="${escape(encodeURIComponent(raw))}" data-label="${escape(label)}">${escape(source || label)}</${tag}>`;
}

const definition: TokenizerExtension & RendererExtension = {
  name: 'casyFootnoteDefinition', level: 'block',
  start: src => src.search(/^ {0,3}\[\^[^\]\n]+\]:/m),
  tokenizer(src) {
    const match = /^ {0,3}\[\^([^\]\n]+)\]:[ \t]*([^\n]*)(?:\n|$)/.exec(src);
    if (!match) return;
    let raw = match[0];
    let rest = src.slice(raw.length);
    while (rest) {
      const continuation = /^(?:\n(?=[ \t]{4}|\t))?(?: {4}|\t)[^\n]*(?:\n|$)/.exec(rest);
      if (!continuation) break;
      raw += continuation[0]; rest = rest.slice(continuation[0].length);
    }
    const source = raw.replace(/^ {0,3}\[\^[^\]\n]+\]:[ \t]*/, '').replace(/\n(?: {4}|\t)/g, '\n').trimEnd();
    return { type:'casyFootnoteDefinition', raw, source, label:match[1] };
  },
  renderer(token) { return semanticHtml('footnoteDefinition', String(token.source), token.raw, String(token.label)); },
};
const reference: TokenizerExtension & RendererExtension = {
  name:'casyFootnoteReference', level:'inline', start:src => src.indexOf('[^'),
  tokenizer(src) { const m = /^\[\^([^\]\n]+)\]/.exec(src); if (m) return {type:'casyFootnoteReference', raw:m[0], label:m[1]}; },
  renderer(token) { return semanticHtml('footnoteReference', '', token.raw, String(token.label)); },
};
const block: TokenizerExtension & RendererExtension = {
  name:'casyMathBlock', level:'block', start:src => src.search(/^ {0,3}(?:\$\$|\\\[)/m),
  tokenizer(src) {
    const m = /^ {0,3}(\$\$|\\\[)[ \t]*\n?([\s\S]*?)(?:\$\$|\\\])[ \t]*(?:\n|$)/.exec(src);
    if (!m || (m[1] === '$$' ? !m[0].trimEnd().endsWith('$$') : !m[0].trimEnd().endsWith('\\]'))) return;
    return {type:'casyMathBlock',raw:m[0],source:m[2].trim()};
  },
  renderer(token) { return semanticHtml('mathBlock', String(token.source), token.raw); },
};
const inline: TokenizerExtension & RendererExtension = {
  name:'casyMathInline', level:'inline', start:src => src.search(/\$|\\\(/),
  tokenizer(src) {
    const m = /^\\\(([^\n]*?)\\\)/.exec(src) || /^\$(?!\$|\s)((?:\\.|[^$\n\\])+?)\$(?!\d)/.exec(src);
    if (!m || /\s$/.test(m[1])) return;
    return {type:'casyMathInline',raw:m[0],source:m[1]};
  },
  renderer(token) { return semanticHtml('mathInline', String(token.source), token.raw); },
};
export const semanticMarkdownExtensions = [definition, reference, block, inline];

export function decodeSemantic(value: string | null): string {
  try { return decodeURIComponent(value || ''); } catch { return value || ''; }
}
export function semanticSource(kind: string, source: string, label: string, raw = ''): string {
  // Attribute edits made by commands must not revive an old source snapshot.
  if (raw) {
    let original = '';
    if (kind === 'footnoteReference') { if(raw === `[^${label}]`)return raw; }
    else {
      if (kind === 'mathInline') original = raw.startsWith('\\(') ? raw.slice(2,-2) : raw.slice(1,-1);
      else if (kind === 'mathBlock') original = raw.trim().slice(2,-2).trim();
      else if (raw.match(/^ {0,3}\[\^([^\]\n]+)\]:/)?.[1] === label) original=raw.replace(/^ {0,3}\[\^[^\]\n]+\]:[ \t]*/, '').replace(/\n(?: {4}|\t)/g,'\n').trimEnd();
      if(original === source)return raw;
    }
  }
  // Some locally edited sources cannot be represented by a Markdown delimiter.
  if ((kind === 'mathInline' && /[\n$]/.test(source)) || (kind === 'mathBlock' && source.includes('$$'))) return semanticHtml(kind,source,'',label);
  if (kind === 'mathInline') return `$${source}$`;
  if (kind === 'mathBlock') return `$$\n${source}\n$$`;
  if (kind === 'footnoteReference') return `[^${label}]`;
  return `[^${label}]: ${source.replace(/\n/g, '\n    ')}`;
}
