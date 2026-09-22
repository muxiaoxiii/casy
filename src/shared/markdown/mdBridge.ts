import {
  Marked,
  type TokenizerExtension,
  type RendererExtension,
} from "marked";
import TurndownService from "turndown";
import { gfm } from "turndown-plugin-gfm";
import DOMPurify from "dompurify";
import { semanticMarkdownExtensions, decodeSemantic, semanticSource } from './semanticMarkdown';

/**
 * 知识库 Markdown 桥：
 * - mdToHtml: Markdown → HTML（供 tiptap 消费），[[标题]] 转为 WikiLink 节点 HTML
 * - htmlToMd: HTML → Markdown（编辑器序列化回写），WikiLink 节点还原为 [[标题]]
 *
 * WikiLink 节点 HTML 结构（与 MarkdownWysiwygEditor 内联节点 parseHTML 对齐）：
 *   <span data-wiki-link="" data-title="标题">标题</span>
 *   - data-wiki-link：必须存在（parseHTML 的 tag 选择器）
 *   - data-title：权威标题（htmlToMd 还原时优先取它）
 */

function escapeHtmlText(value: string): string {
  return value.replace(
    /[&<>]/g,
    (ch) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" })[ch] as string,
  );
}

function escapeHtmlAttr(value: string): string {
  return escapeHtmlText(value).replace(/"/g, "&quot;");
}

// ── mdToHtml：marked + [[标题]] inline tokenizer ──────────────────────────

// 标题不允许含方括号（与后端 parse_wiki_titles、CodeMirror 补全一致）
const WIKI_LINK_RE = /^\[\[([^\[\]\n]{1,200})\]\]/;

const wikiLinkTokenizer: TokenizerExtension = {
  name: "wikiLink",
  level: "inline",
  start(src: string) {
    return src.indexOf("[[");
  },
  tokenizer(src: string) {
    const match = WIKI_LINK_RE.exec(src);
    if (!match) return undefined;
    return {
      type: "wikiLink",
      raw: match[0],
      title: match[1].trim(),
      tokens: [],
    };
  },
};

const wikiLinkRenderer: RendererExtension = {
  name: "wikiLink",
  renderer(token) {
    const title = String((token as unknown as { title: string }).title || "");
    return `<span data-wiki-link="" data-title="${escapeHtmlAttr(title)}">${escapeHtmlText(title)}</span>`;
  },
};

/**
 * 原始 HTML 在富文本编辑器中用不可编辑占位节点承载；属性保存 URI 编码后的原文。
 * 预览不使用此 renderer，而是交给 DOMPurify，因此占位机制不承担安全职责。
 */
const rawHtmlRenderer: RendererExtension = {
  name: "html",
  renderer(token) {
    const htmlToken = token as unknown as {
      raw?: string;
      text?: string;
      block?: boolean;
    };
    const raw = String(htmlToken.raw ?? htmlToken.text ?? "");
    // Rich structures emitted by this editor retain their attributes on reopening.
    if (
      /^\s*<(?:img|table|p|h[1-6]|ul|ol|li|u|mark|span|div)\b/i.test(raw) &&
      /(?:width=|rowspan=|colspan=|data-type=|data-task-id=|data-evidence-link|data-text-align=|data-casy-node=|style=|<table)/i.test(
        raw,
      )
    )
      return sanitizePreviewHtml(raw);
    if (/^<\/?(?:u|mark)>$/i.test(raw.trim())) return raw;
    const encoded = encodeURIComponent(raw);
    const tag = htmlToken.block ? "div" : "span";
    return `<${tag} data-raw-html="${encoded}" data-raw-html-kind="${htmlToken.block ? "block" : "inline"}">HTML</${tag}>`;
  },
};

const evidenceToken: TokenizerExtension = {
  name: "editorEvidence",
  level: "inline",
  start: (src) => src.indexOf("<span"),
  tokenizer(src) {
    const match = /^<span\b[^>]*data-evidence-link[^>]*>[\s\S]*?<\/span>/i.exec(
      src,
    );
    return match ? { type: "editorEvidence", raw: match[0] } : undefined;
  },
};
const evidenceRenderer: RendererExtension = {
  name: "editorEvidence",
  renderer: (token) => sanitizePreviewHtml(String(token.raw)),
};
const taskMarker: TokenizerExtension = {
  name: "editorTaskMarker",
  level: "inline",
  start: (src) => src.indexOf("<!--casy-task:"),
  tokenizer(src) {
    const match = /^<!--casy-task:([a-zA-Z0-9-]{1,80})-->/.exec(src);
    return match
      ? { type: "editorTaskMarker", raw: match[0], taskId: match[1] }
      : undefined;
  },
};
const taskMarkerRenderer: RendererExtension = {
  name: "editorTaskMarker",
  renderer(token) {
    return `<span data-editor-task-id="${escapeHtmlAttr(String(token.taskId))}"></span>`;
  },
};
function createTaskListRenderer(preview = false): RendererExtension {
  return {
    name: "list",
    renderer(token) {
      const list = token as any;
      if (!list.items?.some((item: any) => item.task)) return false;
      let html = "",
        kind = "";
      for (const item of list.items) {
        const next = item.task ? "task" : "normal";
        if (next !== kind) {
          if (kind) html += "</ul>";
          html += next === "task" ? '<ul data-type="taskList">' : "<ul>";
          kind = next;
        }
        let body = this.parser
          .parse(item.tokens)
          .replace(/<input[^>]*type="checkbox"[^>]*>\s*/i, "");
        const id = String(item.text || "")
          .split("\n")[0]
          ?.match(/<!--casy-task:([a-zA-Z0-9-]{1,80})-->/)?.[1];
        if (id)
          body = body.replace(`<span data-editor-task-id="${id}"></span>`, "");
        if (item.task && preview)
          body = `<label><input type="checkbox" disabled${item.checked ? " checked" : ""} aria-label="${item.checked ? "已完成" : "未完成"}"></label><div>${body}</div>`;
        html += item.task
          ? `<li data-type="taskItem" data-checked="${!!item.checked}"${id ? ` data-task-id="${id}"` : ""}>${body}</li>`
          : `<li>${body}</li>`;
      }
      return html + (kind ? "</ul>" : "");
    },
  };
}
const taskListRenderer = createTaskListRenderer();

const markedBridge = new Marked(
  { gfm: true, breaks: false },
  {
    extensions: [
      ...semanticMarkdownExtensions,
      wikiLinkTokenizer,
      wikiLinkRenderer,
      rawHtmlRenderer,
      taskMarker,
      taskMarkerRenderer,
      taskListRenderer,
      evidenceToken,
      evidenceRenderer,
    ],
  },
);

const previewMarkedBridge = new Marked(
  { gfm: true, breaks: false },
  {
    extensions: [
      ...semanticMarkdownExtensions,
      wikiLinkTokenizer,
      wikiLinkRenderer,
      taskMarker,
      taskMarkerRenderer,
      createTaskListRenderer(true),
    ],
  },
);

export function mdToHtml(md: string): string {
  if (!md || !md.trim()) return "";
  return markedBridge.parse(md, { async: false });
}

/** Token boundaries and original bytes used to retain untouched Markdown blocks. */
export function markdownSourceBlocks(source: string): Array<{ raw: string; html: string; definition: boolean }> {
  const tokens = markedBridge.lexer(source);
  return tokens.map(token => {
    const part = Object.assign([token], { links: tokens.links });
    return { raw: token.raw, html: markedBridge.parser(part), definition: token.type === 'def' };
  });
}

// ── htmlToMd：turndown + gfm + WikiLink / tiptap TaskItem 自定义规则 ────────

function createTurndown(): TurndownService {
  const td = new TurndownService({
    headingStyle: "atx",
    codeBlockStyle: "fenced",
    bulletListMarker: "-",
    emDelimiter: "*",
    hr: "---",
    // 未知标签保留内容不丢失（默认行为即如此；显式声明意图）
    blankReplacement: (content, node) =>
      (node as unknown as { isBlock?: boolean }).isBlock ? "\n\n" : "",
  });

  td.use(gfm);
  td.addRule('semanticNode', {
    filter: node => (node as HTMLElement).hasAttribute?.('data-casy-node') === true,
    replacement: (_content, node) => {
      const el = node as HTMLElement;
      const kind = el.getAttribute('data-casy-node') || '';
      const raw = semanticSource(kind, decodeSemantic(el.getAttribute('data-source')), el.getAttribute('data-label') || '', decodeSemantic(el.getAttribute('data-raw')));
      return kind === 'mathInline' || kind === 'footnoteReference' ? raw : `\n\n${raw}\n\n`;
    },
  });

  // 富文本中的 RawHtmlInline/RawHtmlBlock → 原始 Markdown HTML，禁止静默丢弃。
  td.addRule("rawHtmlPlaceholder", {
    filter: (node) =>
      (node as unknown as HTMLElement).hasAttribute?.("data-raw-html") === true,
    replacement: (_content, node) => {
      const encoded =
        (node as unknown as HTMLElement).getAttribute("data-raw-html") || "";
      try {
        return decodeURIComponent(encoded);
      } catch {
        return encoded;
      }
    },
  });

  // WikiLink 节点 → [[标题]]（addRule 前置，优先于通用规则）
  td.addRule("wikiLink", {
    filter: (node) =>
      (node as unknown as HTMLElement).hasAttribute?.("data-wiki-link") ===
      true,
    replacement: (content, node) => {
      const title =
        (node as unknown as HTMLElement).getAttribute("data-title")?.trim() ||
        content.trim();
      return `[[${title}]]`;
    },
  });

  // 删除线统一为 ~~...~~（gfm 插件默认单 ~，marked 虽可解析但回写不一致）
  td.addRule("strikethrough", {
    filter: ["del", "s"] as unknown as TurndownService.Filter,
    replacement: (content) => `~~${content}~~`,
  });

  // 表格单元格：tiptap 在 th/td 内包 <p>，会产生换行破坏表格行，这里清理
  td.addRule("tableCell", {
    filter: ["th", "td"],
    replacement: (content, node) => {
      const clean = content.replace(/\n+/g, " ").trim().replace(/\|/g, "\\|");
      const siblings = node.parentNode
        ? Array.from((node.parentNode as unknown as HTMLElement).childNodes)
        : [];
      const index = siblings.indexOf(node as unknown as ChildNode);
      return (index <= 0 ? "| " : " ") + clean + " |";
    },
  });

  // 列表项：默认实现前缀带 3 个空格（"-   item"），收紧为单空格，序列化更干净
  td.addRule("listItem", {
    filter: "li",
    replacement: (content, node, options) => {
      content = content
        .replace(/^\n+/, "")
        .replace(/\n+$/, "\n")
        .replace(/\n/gm, "\n    ")
        // marked 任务列表的 checkbox 后自带一个空格，与 [x] 标记叠加成双空格，归一
        .replace(/^(\[[ xX]\])\s+/, "$1 ");
      let prefix = `${options.bulletListMarker} `;
      const parent = node.parentNode as unknown as HTMLElement | null;
      if (parent && parent.nodeName === "OL") {
        const start = parent.getAttribute("start");
        const index = Array.prototype.indexOf.call(parent.children, node);
        prefix = `${start ? Number(start) + index : index + 1}. `;
      }
      return (
        prefix +
        content +
        (node.nextSibling && !/\n$/.test(content) ? "\n" : "")
      );
    },
  });

  // tiptap TaskItem：<li data-type="taskItem" data-checked="true|false">
  // gfm 自带 taskListItems 只认 checkbox 直接位于 li 下的结构（marked 输出），
  // tiptap 会把 input 包在 label 里，因此需要这条规则兜底。
  // 注意：须在 listItem 之后 addRule，使其排在更前面优先匹配。
  td.addRule("tiptapTaskItem", {
    filter: (node) =>
      node.nodeName === "LI" &&
      (node as unknown as HTMLElement).getAttribute?.("data-type") ===
        "taskItem",
    replacement: (content, node, options) => {
      content = content.trim().replace(/\n/gm, "\n    ");
      const checked =
        (node as unknown as HTMLElement).getAttribute("data-checked") === "true"
          ? "x"
          : " ";
      const taskId = (node as unknown as HTMLElement).getAttribute(
        "data-task-id",
      );
      if (taskId && /^[a-zA-Z0-9-]{1,80}$/.test(taskId))
        content = content.replace(/^([^\n]*)/, `$1 <!--casy-task:${taskId}-->`);
      const prefix = `${options.bulletListMarker} [${checked}] `;
      return (
        prefix +
        content +
        (node.nextSibling && !/\n$/.test(content) ? "\n" : "")
      );
    },
  });

  td.addRule("sizedImage", {
    filter: (node) =>
      node.nodeName === "IMG" &&
      (node as unknown as HTMLElement).hasAttribute("width"),
    replacement: (_content, node) =>
      `\n\n${(node as unknown as HTMLElement).outerHTML}\n\n`,
  });
  td.addRule("mergedTable", {
    filter: (node) =>
      node.nodeName === "TABLE" &&
      (!(node as unknown as HTMLElement).querySelector("tr:first-child th") ||
        !!(node as unknown as HTMLElement).querySelector(
          '[colspan]:not([colspan="1"]),[rowspan]:not([rowspan="1"]),[data-text-align],[data-colwidth]',
        )),
    replacement: (_content, node) =>
      `\n\n${(node as unknown as HTMLElement).outerHTML}\n\n`,
  });
  td.addRule("evidenceLink", {
    filter: (node) =>
      (node as unknown as HTMLElement).hasAttribute?.("data-evidence-link") ===
      true,
    replacement: (_content, node) => (node as unknown as HTMLElement).outerHTML,
  });
  td.addRule("alignedParagraph", {
    filter: (node) =>
      ["P", "H1", "H2", "H3", "H4", "H5", "H6"].includes(node.nodeName) &&
      ["center", "right", "justify"].includes(
        (node as unknown as HTMLElement).style.textAlign,
      ),
    replacement: (_content, node) => {
      const el = (node as unknown as HTMLElement).cloneNode(
        true,
      ) as HTMLElement;
      el.setAttribute("data-text-align", el.style.textAlign);
      el.removeAttribute("style");
      return `\n\n${el.outerHTML}\n\n`;
    },
  });
  td.addRule("richMarks", {
    filter: ["u", "mark"],
    replacement: (_content, node) => (node as unknown as HTMLElement).outerHTML,
  });

  // tiptap 段落空节点（<p></p>）压平，避免空行膨胀
  td.addRule("emptyParagraph", {
    filter: (node) =>
      node.nodeName === "P" &&
      !node.textContent?.trim() &&
      !(node as unknown as HTMLElement).querySelector("img,br,hr"),
    replacement: () => "",
  });

  return td;
}

const turndown = createTurndown();

export function htmlToMd(html: string): string {
  if (!html || !html.trim()) return "";
  return turndown
    .turndown(html)
    .trim();
}

// ── 预览渲染：marked 透传原始 HTML，v-html 前必须消毒 ──────────────────────

const PREVIEW_ALLOWED_TAGS = [
  "label",
  "div",
  "a",
  "blockquote",
  "br",
  "code",
  "del",
  "div",
  "em",
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  "hr",
  "img",
  "input",
  "li",
  "mark",
  "ol",
  "p",
  "pre",
  "s",
  "span",
  "strong",
  "table",
  "tbody",
  "td",
  "th",
  "thead",
  "tr",
  "u",
  "ul",
  "details",
  "summary",
  "figure",
  "figcaption",
];

const PREVIEW_ALLOWED_ATTR = [
  "aria-label",
  "alt",
  "checked",
  "class",
  "data-checked",
  "data-title",
  "data-type",
  "data-wiki-link",
  "disabled",
  "href",
  "rel",
  "src",
  "start",
  "target",
  "title",
  "type",
  "rowspan",
  "colspan",
  "width",
  "height",
  "data-task-id",
  "data-text-align",
  "data-evidence-link",
  "data-link-id",
  "data-target-type",
  "data-target-id",
  "data-case-id",
  "data-anchor",
  "data-label",
  "data-casy-node",
  "data-source",
  "data-raw",
  "data-colwidth",
];

/**
 * 预览使用 DOM 级白名单消毒；禁止用正则处理 HTML 安全边界。
 * DOMPurify 会先由浏览器解析实体/无引号属性，再执行协议与属性过滤，
 * 因此能覆盖 `href=javascript:...`、实体编码协议和 SVG/事件属性等绕过。
 */
function sanitizeHtml(html: string): string {
  return DOMPurify.sanitize(html, {
    ALLOWED_TAGS: PREVIEW_ALLOWED_TAGS,
    ALLOWED_ATTR: PREVIEW_ALLOWED_ATTR,
    ALLOW_DATA_ATTR: false,
    ADD_DATA_URI_TAGS: ["img"],
    ADD_URI_SAFE_ATTR: ["data-anchor"],
  });
}

/** 供只读预览 v-html 使用：完整 GFM 渲染（表格/任务列表/wiki 链接）+ 消毒 */
export function mdToSafeHtml(md: string): string {
  if (!md || !md.trim()) return "";
  return sanitizeHtml(previewMarkedBridge.parse(md, { async: false }));
}

/**
 * 行内高亮白名单：仅保留无安全语义的展示型标签。
 * 用于「内容是行内片段、且标签由代码生成、但文本部分来自外部数据」的场景：
 * - 首页 AI 推荐文本：`<strong>` 由模板字面量生成，但任务名/案件名来自用户或同步数据
 * - FTS5 snippet 高亮：`<b>` 由 SQLite snippet() 生成，但片段正文来自知识库/案卷正文
 * 这两处原本直接 v-html 渲染，外部数据中的 `<img onerror>` 会被原样执行（审查 P0-3）。
 */
const INLINE_ALLOWED_TAGS = ["b", "strong", "em", "i", "mark"];

/** 供只读行内片段 v-html 使用：消毒 + 仅保留展示型标签，剥离一切属性与事件 */
export function sanitizeInlineHtml(html: string): string {
  if (!html) return "";
  return DOMPurify.sanitize(html, {
    ALLOWED_TAGS: INLINE_ALLOWED_TAGS,
    ALLOWED_ATTR: [],
    ALLOW_DATA_ATTR: false,
  });
}

/** 供只读 HTML 预览 v-html 使用：内容已是 HTML（非 Markdown），如文书模板渲染结果 */
export function sanitizePreviewHtml(html: string): string {
  if (!html) return "";
  return sanitizeHtml(html);
}
