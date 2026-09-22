/** One lifecycle per NodeView. Never render stale work after edit, replacement or removal. */
export function scheduleVisiblePreview(target: HTMLElement, render: () => void | (() => void)): () => void {
  let stopped = false;
  let active = false;
  let cleanup: void | (() => void);
  let observer: IntersectionObserver | undefined;
  const clear = () => { if(typeof cleanup==='function')cleanup(); cleanup=undefined;active=false; };
  const run = () => { if (!stopped && !active) { active=true; cleanup=render(); } };
  if (typeof IntersectionObserver !== 'undefined') {
    observer = new IntersectionObserver(entries => { if (entries.some(e => e.isIntersecting)) run(); else clear(); }, { rootMargin:'400px 0px' });
    observer.observe(target);
  } else queueMicrotask(run);
  return () => { stopped = true; observer?.disconnect(); clear(); };
}

export function captureScrollAnchor(element: HTMLElement): () => void {
  let scroll = element.parentElement;
  while (scroll && !/(auto|scroll)/.test(getComputedStyle(scroll).overflowY)) scroll = scroll.parentElement;
  if (!scroll) return () => {};
  const root = scroll;
  const top = root.scrollTop;
  const before = element.getBoundingClientRect().bottom;
  return () => {
    if (!element.isConnected || Math.abs(root.scrollTop - top) > 1) return;
    // Only compensate content above the viewport; resizing the block being read should not drag it.
    if (before < root.getBoundingClientRect().top) root.scrollTop += element.getBoundingClientRect().bottom - before;
  };
}

const cache = new Map<string, string>();
let mermaidReady: Promise<typeof import('mermaid')> | undefined;
let diagramId = 0;
export async function renderSourcePreview(kind: string, source: string): Promise<string> {
  if (source.length > 100_000) throw new Error('内容过长，请缩短公式或图表后预览');
  const key = `${kind}:${source}`;
  const hit = cache.get(key);
  if (hit) return hit;
  let html: string;
  if (kind === 'mermaid') {
    mermaidReady ??= import('mermaid').then(module => {
      module.default.initialize({ startOnLoad:false, securityLevel:'strict', suppressErrorRendering:true, htmlLabels:false, flowchart:{htmlLabels:false} });
      return module;
    });
    const { default: mermaid } = await mermaidReady;
    html = (await mermaid.render(`casy-diagram-${++diagramId}`, source)).svg;
  } else {
    const { default: katex } = await import('katex');
    html = katex.renderToString(source, {displayMode:kind === 'mathBlock',throwOnError:true,trust:false,strict:'error',maxExpand:1000});
  }
  const { default: purify } = await import('dompurify');
  html = purify.sanitize(html);
  if (cache.size >= 64) cache.delete(cache.keys().next().value!);
  cache.set(key, html);
  return html;
}
