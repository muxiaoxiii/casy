import { Node, mergeAttributes, generateJSON } from "@tiptap/core";
import StarterKit from "@tiptap/starter-kit";
import Highlight from "@tiptap/extension-highlight";
import Placeholder from "@tiptap/extension-placeholder";
import TaskList from "@tiptap/extension-task-list";
import { LinkedTaskItem } from "./LinkedTaskItem";
import { ResizableImage } from "./ResizableImage";
import { Table } from "@tiptap/extension-table";
import TableRow from "@tiptap/extension-table-row";
import TableCell from "@tiptap/extension-table-cell";
import TableHeader from "@tiptap/extension-table-header";
import TextAlign from "@tiptap/extension-text-align";
import { EvidenceLink } from "../../modules/docs/extensions/EvidenceLink";
import { mdToHtml } from "../markdown/mdBridge";
import { semanticNodes, PreviewCodeBlock } from './semanticNodes';

const cellAttributes = {
  align: { default:null, parseHTML:(el:HTMLElement) => {
    const value=el.getAttribute('data-text-align') || el.getAttribute('align') || el.style.textAlign;
    return ['left','center','right','justify'].includes(value) ? value : null;
  }, renderHTML:(attrs:Record<string,unknown>) => attrs.align ? {'data-text-align':attrs.align,style:`text-align: ${attrs.align}`} : {} },
  colwidth: { default:null, parseHTML:(el:HTMLElement) => {
    const value=el.getAttribute('data-colwidth') || el.getAttribute('colwidth');
    if (!value || !/^\d+(,\d+)*$/.test(value)) return null;
    const widths=value.split(',').map(Number);
    return widths.length<=100 && widths.every(n=>n>0 && n<=10000) ? widths : null;
  }, renderHTML:(attrs:Record<string,unknown>) => Array.isArray(attrs.colwidth) ? {'data-colwidth':attrs.colwidth.join(',')} : {} },
};
const PreservedTableCell=TableCell.extend({addAttributes(){return {...this.parent?.(),...cellAttributes};}});
const PreservedTableHeader=TableHeader.extend({addAttributes(){return {...this.parent?.(),...cellAttributes};}});
const WikiLinkNode = Node.create({
  name: "wikiLink",
  group: "inline",
  inline: true,
  atom: true,
  selectable: true,

  addAttributes() {
    return {
      knowledgeId: { default: null, parseHTML: (element) => element.getAttribute("data-knowledge-id"), renderHTML: (attrs) => attrs.knowledgeId ? { "data-knowledge-id": attrs.knowledgeId } : {} },
      title: {
        default: "",
        parseHTML: (element) =>
          element.getAttribute("data-title") || element.textContent || "",
        renderHTML: (attributes) => ({ "data-title": attributes.title }),
      },
    };
  },

  parseHTML() {
    return [{ tag: "span[data-wiki-link]" }, { tag: "a[data-wiki-link]" }];
  },

  renderHTML({ node, HTMLAttributes }) {
    return [
      "span",
      mergeAttributes(HTMLAttributes, {
        "data-wiki-link": "",
        class: "wiki-link",
      }),
      String(node.attrs.title || ""),
    ];
  },
});

function decodeRawHtml(value: string | null): string {
  try {
    return decodeURIComponent(value || "");
  } catch {
    return value || "";
  }
}

const RawHtmlInline = Node.create({
  name: "rawHtmlInline",
  group: "inline",
  inline: true,
  atom: true,
  selectable: true,
  addAttributes() {
    return {
      raw: {
        default: "",
        parseHTML: (element) =>
          decodeRawHtml(element.getAttribute("data-raw-html")),
        renderHTML: (attributes) => ({
          "data-raw-html": encodeURIComponent(attributes.raw || ""),
        }),
      },
    };
  },
  parseHTML() {
    return [{ tag: "span[data-raw-html]" }];
  },
  renderHTML({ HTMLAttributes }) {
    return [
      "span",
      mergeAttributes(HTMLAttributes, {
        "data-raw-html-kind": "inline",
        class: "raw-html-placeholder raw-html-placeholder--inline",
        title: "原始 HTML 已原样保留，请在源码模式编辑",
      }),
      "HTML",
    ];
  },
});

const RawHtmlBlock = Node.create({
  name: "rawHtmlBlock",
  group: "block",
  atom: true,
  selectable: true,
  addAttributes() {
    return {
      raw: {
        default: "",
        parseHTML: (element) =>
          decodeRawHtml(element.getAttribute("data-raw-html")),
        renderHTML: (attributes) => ({
          "data-raw-html": encodeURIComponent(attributes.raw || ""),
        }),
      },
    };
  },
  parseHTML() {
    return [{ tag: "div[data-raw-html]" }];
  },
  renderHTML({ HTMLAttributes }) {
    return [
      "div",
      mergeAttributes(HTMLAttributes, {
        "data-raw-html-kind": "block",
        class: "raw-html-placeholder raw-html-placeholder--block",
        title: "原始 HTML 已原样保留，请在源码模式编辑",
      }),
      "原始 HTML（已保留）",
    ];
  },
});

export function documentExtensions(
  placeholder = "开始输入正文…",
  resolveAsset?: import("./ResizableImage").AssetResolver,
) {
  return [
    StarterKit.configure({
      heading: { levels: [1, 2, 3, 4, 5, 6] },
      codeBlock: false,
      link: { openOnClick: false },
    }),
    Highlight,
    ...semanticNodes,
    PreviewCodeBlock,
    TaskList,
    LinkedTaskItem,
    resolveAsset
      ? ResizableImage.configure({ resolveAsset })
      : ResizableImage,
    Table.configure({ resizable: true }),
    TableRow,
    PreservedTableHeader,
    PreservedTableCell,
    TextAlign.extend({addGlobalAttributes(){return [{types:["heading","paragraph"],attributes:{textAlign:{default:null,parseHTML:element=>element.getAttribute("data-text-align") || element.style.textAlign || null,renderHTML:attrs=>attrs.textAlign?{style:`text-align: ${attrs.textAlign}`}:{}}}}]}}).configure({ types: ["heading", "paragraph"] }),
    Placeholder.configure({ placeholder }),
    WikiLinkNode,
    RawHtmlInline,
    RawHtmlBlock,
    EvidenceLink,
  ];
}
export function documentFromContent(
  content: string,
  format: "markdown" | "html" = "markdown",
) {
  return generateJSON(
    format === "markdown" ? mdToHtml(content) : content,
    documentExtensions(),
  );
}
