<script setup lang="ts">
/**
 * MarkdownWysiwygEditor —— 知识库所见即所得 Markdown 编辑器
 *
 * 契约：v-model 为 Markdown 字符串；内部经 mdBridge 双向转换
 * （mdToHtml 喂给 tiptap，htmlToMd 防抖 400ms 序列化回写）。
 * WikiLink（[[标题]]）为组件内定义的内联 atom 节点，
 * HTML 结构 <span data-wiki-link data-title="标题">标题</span>，与 mdBridge 双向对齐。
 * compact 模式：无工具栏，供块表单等轻量场景复用。
 */
import { watch, ref, onBeforeUnmount } from "vue";
import { useEditor, EditorContent } from "@tiptap/vue-3";
import { BubbleMenu } from "@tiptap/vue-3/menus";
import { Node, mergeAttributes } from "@tiptap/core";
import StarterKit from "@tiptap/starter-kit";
import Highlight from "@tiptap/extension-highlight";
import Placeholder from "@tiptap/extension-placeholder";
import TaskList from "@tiptap/extension-task-list";
import { LinkedTaskItem } from "./LinkedTaskItem";
import { useEditorTasks } from "./useEditorTasks";
import { tauriCallSafe } from "../../core/tauriBridge";
import { ResizableImage } from "./ResizableImage";
import { useBlockMenu } from './useBlockMenu';
import EditorToolbar from "./EditorToolbar.vue";
import { documentExtensions } from "./schema";
import documentStyle from "./document-style.json";
import { flushSourceEditors, hasSourceDraft } from './SourceNodeView';
import { MarkdownPreservation } from './markdownPreservation';
import 'katex/dist/katex.min.css';
import './sourceNodes.css';
import "./checklist.css";
import type { Extensions } from "@tiptap/core";
import { Table } from "@tiptap/extension-table";
import TableRow from "@tiptap/extension-table-row";
import TableCell from "@tiptap/extension-table-cell";
import TableHeader from "@tiptap/extension-table-header";
import TextAlign from "@tiptap/extension-text-align";
import {
  ElMessageBox,
  ElMessage,
  ElDialog,
  ElButton,
  ElRadioGroup,
  ElRadioButton,
  ElSelect,
  ElOption,
} from "element-plus";
import { mdToHtml, htmlToMd } from "../markdown/mdBridge";
import WikiLinkSuggestion from "../../modules/docs/extensions/WikiLinkSuggestion";
import QuoteSourceMenu from "../components/QuoteSourceMenu.vue";
import {
  RefreshLeft,
  RefreshRight,
  Link,
  Connection,
  Grid,
  List,
  Finished,
  Minus,
  EditPen,
} from "../icons";

interface NoteTitleItem {
  id?: string;
  title: string;
  category?: string;
  categoryLabel?: string;
}

interface OutlineItem {
  id: string;
  level: number;
  text: string;
}

const props = withDefaults(
  defineProps<{
    modelValue: string;
    contentFormat?: "markdown" | "html";
    extraExtensions?: Extensions;
    sourceType?: "knowledge" | "doc";
    sourceId?: string;
    caseId?: string | null;
    /** wiki 补全源（标题含方括号者不可链接，与后端解析一致） */
    noteTitles?: NoteTitleItem[];
    placeholder?: string;
    compact?: boolean;
    /** Enter 行为：paragraph=正常换段；submit=触发 submit 事件（Shift+Enter 仍换行） */
    enterMode?: "paragraph" | "submit";
  }>(),
  {
    contentFormat: "markdown",
    extraExtensions: () => [],
    noteTitles: () => [],
    placeholder: "开始书写，输入 / 插入内容，[[ 链接笔记…",
    compact: false,
    enterMode: "paragraph",
  },
);

const emit = defineEmits<{
  "update:modelValue": [value: string];
  ready: [editor: any];
  transaction: [];
  "active-block": [index: number];
  save: [];
  change: [];
  blur: [];
  submit: [];
  "wiki-link-click": [payload: { title: string }];
  "outline-change": [items: OutlineItem[]];
}>();

/**
 * 最小 WikiLink 内联 atom 节点（不依赖 docs 模块的 Mark 版扩展，避免与并行开发耦合）。
 * 结构固定：<span data-wiki-link="" data-title="标题">标题</span>
 */
// ── 双向同步守卫（参考 MarkdownCodeMirror 的 applyingExternal 模式）─────────
let serializeTimer: ReturnType<typeof setTimeout> | null = null;
let lastEmitted = props.modelValue || "";
let contentChanged = false;
let outlineTimer: ReturnType<typeof setTimeout> | undefined;
const markdownPreservation = new MarkdownPreservation();

/** 防抖 400ms 把编辑器 HTML 序列化为 Markdown 回写 modelValue */
function scheduleSerialize() {
  if (serializeTimer) clearTimeout(serializeTimer);
  serializeTimer = setTimeout(flushSerialize, 400);
}

function flushSerialize(): string {
  if (serializeTimer) clearTimeout(serializeTimer);
  serializeTimer = null;
  const ed = editor.value;
  if (!ed) return String(props.modelValue ?? "");
  if (!contentChanged) return lastEmitted;
  const md =
    props.contentFormat === "html" ? ed.getHTML() : markdownPreservation.serialize(ed.state.doc);
  contentChanged = false;
  if (md === lastEmitted) return md;
  lastEmitted = md;
  emit("update:modelValue", md);
  return md;
}

// wiki 建议：本地笔记标题过滤（标题含方括号者无法被 [[ ]] 解析，不出现在候选）
function searchNotes(query: string) {
  const q = String(query || "")
    .trim()
    .toLowerCase();
  return Promise.resolve(
    props.noteTitles
      .filter(
        (item) =>
          item?.title && !item.title.includes("[") && !item.title.includes("]"),
      )
      .filter((item) => !q || item.title.toLowerCase().includes(q))
      .slice(0, 20)
      .map((item) => ({
        id: item.id || item.title,
        title: item.title,
        category: item.categoryLabel || item.category || "笔记",
      })),
  );
}

/** 选中建议项：删掉光标前未闭合的 [[query，替换为 wikiLink atom 节点 */
function insertWikiLinkNode(title: string) {
  const ed = editor.value;
  if (!ed) return;
  const { from } = ed.state.selection;
  const textBefore = ed.state.doc.textBetween(
    Math.max(0, from - 80),
    from,
    " ",
    " ",
  );
  const match = textBefore.match(/\[\[([^\]]*)$/);
  const start = match ? from - match[0].length : from;
  ed.chain()
    .focus()
    .insertContentAt(
      { from: start, to: from },
      { type: "wikiLink", attrs: { title } },
    )
    .run();
}

function collectOutline(ed: any): OutlineItem[] {
  const items: OutlineItem[] = [];
  ed?.state?.doc?.descendants?.((node: any, pos: number) => {
    if (node.type?.name === "heading") {
      items.push({
        id: `heading-${pos}`,
        level: Number(node.attrs?.level || 1),
        text: node.textContent || "空标题",
      });
    }
  });
  emit("outline-change", items);
  return items;
}

function shouldShowBubble({
  editor: activeEditor,
  from,
  to,
}: {
  editor: any;
  from: number;
  to: number;
}) {
  return from !== to && !activeEditor.isActive("codeBlock");
}

const blockMenu = useBlockMenu();
const blockMenuState = blockMenu.state;
const blockMenuItems = blockMenu.items;
const taskState = useEditorTasks(() => editor.value || undefined, props);
/** 写作状态：字数/段落，便于长文写作时感知进度 */
const writingStats = ref({ chars: 0, words: 0, paragraphs: 0, headings: 0 })
const focusWriting = ref(false)
function refreshWritingStats() {
  const ed = editor.value
  if (!ed) return
  const text = ed.getText({ blockSeparator: '\n' })
  const chars = text.replace(/\s/g, '').length
  const words = text.trim() ? text.trim().split(/\s+/).length : 0
  let paragraphs = 0, headings = 0
  ed.state.doc.descendants((node) => {
    if (node.isBlock) {
      if (node.type.name === 'heading') headings += 1
      else if (node.type.name === 'paragraph' && node.textContent.trim()) paragraphs += 1
    }
  })
  writingStats.value = { chars, words, paragraphs, headings }
}
const editor = useEditor({
  content:
    props.contentFormat === "html"
      ? props.modelValue
      : mdToHtml(props.modelValue || ""),
  extensions: [
    ...documentExtensions(props.placeholder),
    ...props.extraExtensions,
    WikiLinkSuggestion.configure({
      trigger: "[[",
      minQueryLength: 0,
      search: searchNotes,
      onSelect: (item) => insertWikiLinkNode(item.title),
    }),
  ],
  editorProps: {
    handlePaste(_view, event) {
      const files = Array.from(event.clipboardData?.files || []).filter(
        (file) => file.type.startsWith("image/"),
      );
      if (!files.length) return false;
      event.preventDefault();
      for (const file of files) void addImage(file);
      return true;
    },
    handleDrop(_view, event) {
      const files = Array.from(event.dataTransfer?.files || []).filter((file) =>
        file.type.startsWith("image/"),
      );
      if (!files.length) return false;
      event.preventDefault();
      for (const file of files) void addImage(file);
      return true;
    },
    handleKeyDown(_view, event) {
      if (event.isComposing || _view.composing) return false;
      if (editor.value && blockMenu.keydown(editor.value, event)) return true;
      const mod = event.metaKey || event.ctrlKey;
      // Cmd/Ctrl+S 保存（对齐既有 @save 语义）
      if (mod && event.key.toLowerCase() === "s") {
        event.preventDefault();
        flushSerialize();
        emit("save");
        return true;
      }
      if (
        props.enterMode === "submit" &&
        event.key === "Enter" &&
        !event.shiftKey &&
        !mod
      ) {
        event.preventDefault();
        flushSerialize();
        emit("submit");
        return true;
      }
      return false;
    },
    handleClick(_view, _pos, event) {
      const target = (event.target as HTMLElement | null)?.closest?.(
        "span[data-wiki-link]",
      );
      if (target) {
        event.preventDefault();
        emit("wiki-link-click", {
          title: target.getAttribute("data-title") || "",
        });
        return true;
      }
      return false;
    },
  },
  onUpdate: () => {
    contentChanged = true;
    emit("change");
    if (props.contentFormat === "html") flushSerialize();
    else scheduleSerialize();
    emit("transaction");
    taskState.changed();
    refreshWritingStats();
    if (outlineTimer) clearTimeout(outlineTimer);
    outlineTimer = setTimeout(() => collectOutline(editor.value), 180);
    if (editor.value && props.contentFormat === 'markdown' && !props.compact) blockMenu.update(editor.value);
  },
  onSelectionUpdate: ({ editor: activeEditor }) => {
    emit('active-block', activeEditor.state.selection.$from.index(0));
    if (props.contentFormat === 'markdown' && !props.compact) blockMenu.update(activeEditor);
  },
  onCreate: ({ editor: activeEditor }) => {
    if (props.contentFormat === 'markdown') markdownPreservation.bind(props.modelValue || '', activeEditor.state.doc);
    collectOutline(activeEditor);
    refreshWritingStats();
    emit("ready", activeEditor);
    void taskState.refresh();
  },
  onBlur: () => {
    flushSerialize();
    emit("blur");
    blockMenu.close();
  },
});

// 外部 modelValue 变化（模式切换/切换笔记/版本恢复）→ 重建内容；自身回写不重建
watch(
  () => props.modelValue,
  (value) => {
    const ed = editor.value;
    if (!ed) return;
    const next = String(value ?? "");
    if (next === lastEmitted) return;
    lastEmitted = next;
    contentChanged = false;
    if (serializeTimer) clearTimeout(serializeTimer);
    ed.commands.setContent(
      props.contentFormat === "html" ? next : mdToHtml(next),
      { emitUpdate: false },
    );
    if (props.contentFormat === 'markdown') markdownPreservation.bind(next, ed.state.doc);
  },
);

onBeforeUnmount(() => {
  if (outlineTimer) clearTimeout(outlineTimer);
  // 400ms 防抖窗口内离开页面时必须同步刷盘，否则最新输入会静默丢失。
  try {
    if (editor.value) flushSourceEditors(editor.value);
  } catch {
    // Source panel commit is best-effort; main Markdown still flushes below.
  }
  flushSerialize();
  editor.value?.destroy();
});

// ── 工具栏动作 ──
function cmd(fn: (chain: any) => any) {
  const ed = editor.value;
  if (!ed) return;
  fn(ed.chain().focus()).run();
}

function setHeading(level: 1 | 2 | 3 | 4) {
  cmd((c) =>
    editor.value?.isActive("heading", { level })
      ? c.setParagraph()
      : c.setHeading({ level }),
  );
}

function setTextAlign(alignment: "left" | "center" | "right") {
  cmd((c) => c.setTextAlign(alignment));
}

async function setLink() {
  const ed = editor.value;
  if (!ed) return;
  if (ed.isActive("link")) {
    cmd((c) => c.extendMarkRange("link").unsetLink());
    return;
  }
  try {
    const { value } = await ElMessageBox.prompt(
      "输入链接地址（https://…）",
      "插入链接",
      {
        inputPattern: /^https?:\/\/.+/,
        inputErrorMessage: "链接需以 http:// 或 https:// 开头",
        confirmButtonText: "确定",
        cancelButtonText: "取消",
      },
    );
    if (value) cmd((c) => c.extendMarkRange("link").setLink({ href: value }));
  } catch {
    /* 用户取消 */
  }
}

function insertWikiLinkTrigger() {
  cmd((c) => c.insertContent("[["));
}

function insertTable() {
  cmd((c) => c.insertTable({ rows: 3, cols: 3, withHeaderRow: true }));
}

function scrollToHeading(id: string) {
  const ed = editor.value;
  if (!ed) return false;
  const pos = Number(id.replace("heading-", ""));
  if (!Number.isFinite(pos)) return false;
  try {
    const dom = ed.view.nodeDOM(pos) as HTMLElement | null;
    dom?.scrollIntoView({ behavior: "smooth", block: "center" });
    if (dom)
      ed.commands.setTextSelection(
        Math.min(pos + 1, ed.state.doc.content.size),
      );
    return Boolean(dom);
  } catch {
    return false;
  }
}

const toolbarRef = ref<InstanceType<typeof EditorToolbar>>();
const imageInput = ref<HTMLInputElement>();
async function addImage(file: File) {
  if (file.size > 20 * 1024 * 1024)
    return ElMessage.warning("图片超过 20 MB，请先压缩");
  const ed = editor.value;
  if (!ed) return;
  try {
    const src = await new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result));
      reader.onerror = reject;
      reader.readAsDataURL(file);
    });
    if (ed.isDestroyed) return;
    ed.chain()
      .focus()
      .setImage({ src, alt: file.name })
      .createParagraphNear()
      .run();
  } catch {
    ElMessage.error("图片读取失败");
  }
}
function onImageFile(event: Event) {
  const input = event.target as HTMLInputElement;
  for (const file of Array.from(input.files || [])) void addImage(file);
  input.value = "";
}
const taskDialog = ref(false),
  taskMode = ref("new"),
  taskChoice = ref(""),
  taskOptions = ref<any[]>([]),
  taskSearchBusy = ref(false);
let taskSearchSequence = 0;
function linkTask() {
  const item = taskState.currentItem();
  if (!item) return ElMessage.info("先将光标放在需要关联的待办项中");
  if (item.node.attrs.taskId)
    return ElMessage.info("此待办已关联任务，可在任务中心查看");
  taskDialog.value = true;
  taskChoice.value = "";
  taskOptions.value = [];
}
async function searchTasks(query: string) {
  const seq = ++taskSearchSequence;
  taskSearchBusy.value = true;
  const result = await tauriCallSafe("search_tasks", { query });
  if (seq !== taskSearchSequence) return;
  taskSearchBusy.value = false;
  taskOptions.value = result.ok ? result.data || [] : [];
}
async function applyTask() {
  if (taskMode.value === "existing" && !taskChoice.value) return;
  await taskState.bind(taskMode.value === "existing" ? taskChoice.value : null);
  if (!taskState.status.value) taskDialog.value = false;
}

defineExpose({
  openTable: () => toolbarRef.value?.openTable(),
  getEditor: () => editor.value,
  focus: () => editor.value?.commands.focus(),
  getWritingStats: () => writingStats.value,
  setMarkdown: (markdown: string) => {
    const next = String(markdown ?? "");
    editor.value?.commands.setContent(mdToHtml(next), { emitUpdate: false });
    if (editor.value && props.contentFormat === 'markdown') markdownPreservation.bind(next, editor.value.state.doc);
    contentChanged = true;
    return flushSerialize();
  },
  /**
   * 笔记切换前由父组件显式调用。返回旧编辑器的权威 Markdown，
   * 避免 onBeforeUnmount 时父级 draft 已切换而发生丢失或串写。
   */
  flushAndGetMarkdown: (commitSources = true) => { if(commitSources && editor.value) flushSourceEditors(editor.value); return flushSerialize(); },
  hasSourceDraft: () => editor.value ? hasSourceDraft(editor.value) : false,
  /** 400ms 防抖尚未落盘时为 true，离开守卫据此拦截路由切换 */
  hasPendingSerialize: () => contentChanged || serializeTimer !== null,
  getDocumentJson: () => { if(editor.value) flushSourceEditors(editor.value); return editor.value?.getJSON() || null; },
  getHtml: () => { if(editor.value) flushSourceEditors(editor.value); if (props.contentFormat === "html") flushSerialize(); return editor.value?.getHTML() || ""; },
  getOutline: () => collectOutline(editor.value),
  scrollToHeading,
});
</script>

<template>
  <div
    class="md-wysiwyg"
    :class="{ compact, 'focus-writing': focusWriting }"
    :style="{
      '--document-font-size': `${(documentStyle.bodySize * 4) / 3}px`,
      '--document-line-height': documentStyle.lineHeight,
      '--document-h1': `${(documentStyle.headingSizes[0] * 4) / 3}px`,
      '--document-h2': `${(documentStyle.headingSizes[1] * 4) / 3}px`,
    }"
  >
    <EditorToolbar
      v-if="!compact && editor"
      ref="toolbarRef"
      :editor="editor"
      @link="setLink"
      @wiki="insertWikiLinkTrigger"
      @image="imageInput?.click()"
      @task="linkTask"
    />
    <input
      ref="imageInput"
      type="file"
      accept="image/png,image/jpeg,image/webp,image/gif"
      hidden
      @change="onImageFile"
    />
    <div v-if="!compact && editor?.isActive('table')" class="md-table-toolbar">
      <span>表格</span>
      <button type="button" @click="cmd((c) => c.addColumnBefore())">
        左侧加列
      </button>
      <button type="button" @click="cmd((c) => c.addColumnAfter())">
        右侧加列
      </button>
      <button type="button" @click="cmd((c) => c.deleteColumn())">删列</button>
      <button type="button" @click="cmd((c) => c.addRowBefore())">
        上方加行
      </button>
      <button type="button" @click="cmd((c) => c.addRowAfter())">
        下方加行
      </button>
      <button type="button" @click="cmd((c) => c.deleteRow())">删行</button>
      <button type="button" @click="cmd((c) => c.mergeOrSplit())">
        合并/拆分
      </button>
      <button type="button" class="danger" @click="cmd((c) => c.deleteTable())">
        删除表格
      </button>
    </div>
    <BubbleMenu
      v-if="!compact && editor"
      :editor="editor"
      :should-show="shouldShowBubble"
      :options="{ placement: 'top', offset: 8, flip: true, shift: true }"
      class="md-bubble-menu"
    >
      <button
        type="button"
        :class="{ active: editor.isActive('bold') }"
        @click="cmd((c) => c.toggleBold())"
      >
        <b>B</b>
      </button>
      <button
        type="button"
        :class="{ active: editor.isActive('italic') }"
        @click="cmd((c) => c.toggleItalic())"
      >
        <i>I</i>
      </button>
      <button
        type="button"
        :class="{ active: editor.isActive('underline') }"
        @click="cmd((c) => c.toggleUnderline())"
      >
        <u>U</u>
      </button>
      <button
        type="button"
        :class="{ active: editor.isActive('highlight') }"
        @click="cmd((c) => c.toggleHighlight())"
      >
        高亮
      </button>
      <button
        type="button"
        :class="{ active: editor.isActive('link') }"
        @click="setLink"
      >
        链接
      </button>
    </BubbleMenu>
    <div
      v-if="!compact"
      class="writing-stats"
      role="status"
      aria-live="polite"
    >
      <span>{{ writingStats.chars }} 字</span>
      <span>{{ writingStats.paragraphs }} 段</span>
      <span>{{ writingStats.headings }} 标题</span>
      <button type="button" class="focus-toggle" :class="{ active: focusWriting }" @click="focusWriting = !focusWriting">
        {{ focusWriting ? '退出专注' : '专注写作' }}
      </button>
    </div>
    <div
      v-if="taskState.status.value || editor?.isActive('taskItem')"
      class="document-task-status"
    >
      <span>{{
        taskState.status.value ||
        (editor?.getAttributes("taskItem").taskId
          ? "已关联任务 · 完成状态同步"
          : "文内待办 · 可关联到任务中心")
      }}</span
      ><button @click="linkTask" :disabled="taskState.busy.value">
        关联任务</button
      ><button @click="taskState.refresh()">刷新状态</button>
    </div>
    <ElDialog
      v-model="taskDialog"
      title="关联任务"
      width="440px"
      append-to-body
    >
      <p>
        关联后，本文勾选与任务中心的完成状态会同步。删除文内待办不会删除任务。
      </p>
      <ElRadioGroup v-model="taskMode"
        ><ElRadioButton value="new">创建新任务</ElRadioButton
        ><ElRadioButton value="existing"
          >关联已有任务</ElRadioButton
        ></ElRadioGroup
      >
      <ElSelect
        v-if="taskMode === 'existing'"
        v-model="taskChoice"
        filterable
        remote
        :remote-method="searchTasks"
        :loading="taskSearchBusy"
        placeholder="搜索任务名称"
        style="width: 100%; margin-top: 16px"
        ><ElOption
          v-for="task in taskOptions"
          :key="task.id"
          :value="task.id"
          :label="task.taskName"
      /></ElSelect>
      <template #footer
        ><ElButton @click="taskDialog = false">取消</ElButton
        ><ElButton
          type="primary"
          :loading="taskState.busy.value"
          :disabled="taskMode === 'existing' && !taskChoice"
          @click="applyTask"
          >确认关联</ElButton
        ></template
      >
    </ElDialog>
    <EditorContent :editor="editor" class="md-wysiwyg-body" @scroll.capture="blockMenu.close" />
    <Teleport to="body">
      <div v-if="blockMenuState.open && editor" class="writing-block-menu" role="listbox" aria-label="插入内容块" :style="{ left: `${blockMenuState.x}px`, top: `${blockMenuState.y}px` }">
        <header>插入内容 <small>↑ ↓ 选择 · Enter 插入</small></header>
        <div class="block-options">
          <button v-for="(item, index) in blockMenuItems" :key="item.id" type="button" role="option" :aria-selected="index === blockMenuState.selected" :class="{ selected: index === blockMenuState.selected }" @mousedown.prevent="blockMenu.run(editor, item.id)"><span class="block-symbol">{{ { heading1: 'H₁', heading2: 'H₂', heading3: 'H₃', task: '☑', bullet: '•', ordered: '1.', quote: '❞', table: '▦', code: '{ }', wiki: '↗', divider: '—', paragraph: '¶' }[item.id] }}</span><span><strong>{{ item.label }}</strong><small>{{ item.hint }}</small></span></button>
          <p v-if="!blockMenuItems.length">没有匹配的内容块</p>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.document-task-status {
  display: flex;
  gap: 8px;
  align-items: center;
  margin-top: 8px;
  padding: 6px 10px;
  border: 1px solid var(--c-border);
  border-radius: 8px;
  font-size: 12px;
  color: var(--c-text-secondary);
}
.writing-stats {
  display: flex;
  gap: 12px;
  align-items: center;
  margin-top: 8px;
  padding: 4px 8px;
  font-size: 11px;
  color: var(--c-text-secondary);
  border-top: 1px solid var(--c-border-light);
}
.writing-stats .focus-toggle {
  margin-left: auto;
  border: 1px solid var(--c-border);
  border-radius: 6px;
  background: transparent;
  padding: 2px 10px;
  font: inherit;
  cursor: pointer;
}
.writing-stats .focus-toggle.active {
  background: var(--c-primary-light);
  color: var(--c-primary);
  border-color: var(--c-primary);
}
.md-wysiwyg.focus-writing .document-toolbar {
  display: none;
}
.md-wysiwyg.focus-writing :deep(.ProseMirror) {
  max-width: 42rem;
  margin-inline: auto;
  padding-block: 24px;
}
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
  padding: 8px 14px;
  font-size: 12px;
  border-bottom: 1px solid var(--c-border);
  color: var(--c-text-secondary);
}
.document-task-status button {
  border: 0;
  background: transparent;
  color: var(--c-primary);
  cursor: pointer;
}
.md-wysiwyg {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  background: var(--c-bg-card);
  color: var(--c-text);
}

.md-wysiwyg-toolbar {
  display: flex;
  flex-wrap: nowrap;
  align-items: center;
  gap: 2px;
  padding: 6px 10px;
  border-bottom: 1px solid var(--c-border);
  background: var(--c-bg-subtle);
  flex-shrink: 0;
  overflow-x: auto;
  scrollbar-width: none;
}

.md-wysiwyg-toolbar::-webkit-scrollbar {
  display: none;
}

.md-wysiwyg-toolbar button {
  display: inline-grid;
  place-items: center;
  min-width: 30px;
  height: 30px;
  flex-shrink: 0;
  white-space: nowrap;
  border: 0;
  background: transparent;
  color: var(--c-text-regular);
  font-size: 12px;
  padding: 4px 8px;
  border-radius: var(--c-radius-md, 6px);
  cursor: pointer;
  transition: background var(--motion-fast) var(--ease-out, ease-out);
}
.md-wysiwyg-toolbar button svg {
  width: 16px;
  height: 16px;
}

.md-wysiwyg-toolbar button:hover:not(:disabled) {
  background: var(--c-bg-hover);
  color: var(--c-text);
}

.md-wysiwyg-toolbar button.active {
  background: var(--c-bg-selected);
  color: var(--c-primary);
  font-weight: 700;
}

.md-wysiwyg-toolbar button:disabled {
  opacity: 0.4;
  cursor: default;
}

.tb-sep {
  flex-shrink: 0;
  width: 1px;
  height: 14px;
  background: var(--c-border);
  margin: 0 5px;
}

.md-table-toolbar {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 5px 10px;
  border-bottom: 1px solid var(--c-border);
  background: color-mix(in srgb, var(--c-primary) 5%, var(--c-bg-card));
  overflow-x: auto;
  flex-shrink: 0;
}

.md-table-toolbar span {
  font-size: 10px;
  font-weight: 750;
  color: var(--c-primary);
  margin-right: 4px;
}
.md-table-toolbar button,
.md-bubble-menu button {
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: var(--c-text-regular);
  padding: 5px 7px;
  font-size: 11px;
  cursor: pointer;
  white-space: nowrap;
}
.md-table-toolbar button:hover,
.md-bubble-menu button:hover,
.md-bubble-menu button.active {
  background: var(--c-bg-hover);
  color: var(--c-primary);
}
.md-table-toolbar button.danger {
  color: var(--status-risk);
}
.md-bubble-menu {
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 5px;
  border: 1px solid var(--c-border);
  border-radius: 9px;
  background: var(--c-bg-card);
  box-shadow: var(--shadow-lg, 0 10px 30px rgba(0, 0, 0, 0.14));
  z-index: 80;
}

.md-wysiwyg-body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}

/* ── 正文排版（全部 CSS 变量，暗色自适应） ── */
.md-wysiwyg-body :deep(.tiptap) {
  outline: none;
  min-height: 100%;
  padding: 20px 28px 60px;
  font-size: var(--document-font-size);
  line-height: var(--document-line-height);
  color: var(--c-text);
  font-family: "Noto Sans CJK SC", var(--font-family);
}

.md-wysiwyg-body :deep(.tiptap img) {
  display: block;
  max-width: 100%;
  height: auto;
  margin: 14px auto;
  border-radius: var(--c-radius-md, 6px);
}

.md-wysiwyg-body :deep(.raw-html-placeholder) {
  border: 1px dashed var(--c-border);
  border-radius: var(--c-radius-md, 6px);
  background: var(--c-bg-subtle);
  color: var(--c-text-secondary);
  font-family: var(--font-mono, monospace);
  font-size: 12px;
  cursor: not-allowed;
}

.md-wysiwyg-body :deep(.raw-html-placeholder--inline) {
  padding: 1px 5px;
}

.md-wysiwyg-body :deep(.raw-html-placeholder--block) {
  margin: 12px 0;
  padding: 10px 12px;
}

.compact .md-wysiwyg-body :deep(.tiptap) {
  padding: 10px 12px;
  font-size: 13.5px;
  line-height: 1.65;
  min-height: 120px;
}

.md-wysiwyg-body :deep(.tiptap p) {
  margin: 0 0 8px;
}

.md-wysiwyg-body :deep(.tiptap h1),
.md-wysiwyg-body :deep(.tiptap h2),
.md-wysiwyg-body :deep(.tiptap h3),
.md-wysiwyg-body :deep(.tiptap h4) {
  color: var(--c-text-heading);
  font-weight: 700;
  margin: 18px 0 10px;
}

.md-wysiwyg-body :deep(.tiptap h1) {
  font-size: 24px;
  border-bottom: 1px solid var(--c-border-light);
  padding-bottom: 6px;
}
.md-wysiwyg-body :deep(.tiptap h2) {
  font-size: 19px;
}
.md-wysiwyg-body :deep(.tiptap h3) {
  font-size: 16px;
}
.md-wysiwyg-body :deep(.tiptap h4) {
  font-size: 14.5px;
}

.md-wysiwyg-body :deep(.tiptap ul),
.md-wysiwyg-body :deep(.tiptap ol) {
  padding-left: 22px;
  margin: 8px 0;
}
.md-wysiwyg-body :deep(.tiptap li) {
  margin-bottom: 3px;
}
.md-wysiwyg-body :deep(.tiptap li p) {
  margin: 0;
}

/* 引用 / 代码 */
.md-wysiwyg-body :deep(.tiptap blockquote) {
  border-left: 3px solid var(--c-primary);
  padding: 8px 14px;
  margin: 12px 0;
  color: var(--c-text-secondary);
  background: var(--c-bg-subtle);
  border-radius: 0 var(--c-radius-lg, 8px) var(--c-radius-lg, 8px) 0;
}

.md-wysiwyg-body :deep(.tiptap code) {
  font-family: var(--font-mono, monospace);
  font-size: 0.9em;
  background: var(--c-bg-subtle);
  border: 1px solid var(--c-border-light);
  border-radius: 4px;
  padding: 1px 5px;
  color: var(--c-primary);
}

.md-wysiwyg-body :deep(.tiptap pre) {
  background: var(--c-bg-subtle);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg, 8px);
  padding: 12px 16px;
  margin: 12px 0;
  overflow-x: auto;
}

.md-wysiwyg-body :deep(.tiptap pre code) {
  background: transparent;
  border: 0;
  padding: 0;
  color: var(--c-text);
  font-size: 13px;
  line-height: 1.6;
}

.md-wysiwyg-body :deep(.tiptap mark) {
  background: color-mix(in srgb, var(--c-primary) 22%, transparent);
  color: inherit;
  padding: 0 2px;
  border-radius: 3px;
}

/* 表格 */
.md-wysiwyg-body :deep(.tiptap table) {
  border-collapse: collapse;
  margin: 14px 0;
  width: 100%;
  table-layout: fixed;
  border: 1px solid var(--c-border);
}

.md-wysiwyg-body :deep(.tiptap th) {
  background: var(--c-bg-subtle);
  font-weight: 600;
  text-align: left;
  padding: 8px 10px;
  border: 1px solid var(--c-border);
  color: var(--c-text-heading);
}

.md-wysiwyg-body :deep(.tiptap td) {
  padding: 7px 10px;
  border: 1px solid var(--c-border);
}

.md-wysiwyg-body :deep(.tiptap .selectedCell) {
  background: color-mix(in srgb, var(--c-primary) 10%, transparent);
}

.md-wysiwyg-body :deep(.tiptap hr) {
  border: none;
  border-top: 1px solid var(--c-border);
  margin: 20px 0;
}

.md-wysiwyg-body :deep(.tiptap a) {
  color: var(--c-primary);
  text-decoration: underline;
  text-underline-offset: 2px;
}

/* WikiLink 内联节点 */
.md-wysiwyg-body :deep(.tiptap span[data-wiki-link]) {
  color: var(--c-primary);
  background: color-mix(in srgb, var(--c-primary) 9%, transparent);
  border-bottom: 1px dashed var(--c-primary);
  border-radius: 4px;
  padding: 0 4px;
  cursor: pointer;
  transition: background var(--motion-fast) var(--ease-out, ease-out);
}

.md-wysiwyg-body :deep(.tiptap span[data-wiki-link]:hover) {
  background: color-mix(in srgb, var(--c-primary) 18%, transparent);
}

.md-wysiwyg-body :deep(.tiptap span[data-wiki-link].ProseMirror-selectednode) {
  outline: 2px solid var(--c-primary);
  outline-offset: 1px;
}

/* 占位符 */
.md-wysiwyg-body :deep(.tiptap p.is-editor-empty:first-child::before) {
  content: attr(data-placeholder);
  float: left;
  height: 0;
  pointer-events: none;
  color: var(--c-text-placeholder);
}

/* WikiLink 补全弹窗（插件内联样式为亮色，这里覆盖为变量色，暗色兼容） */
:deep(.wiki-link-suggestions) {
  background: var(--c-bg-card) !important;
  border: 1px solid var(--c-border) !important;
  box-shadow: var(--shadow-md) !important;
}

:deep(.wiki-link-item) {
  color: var(--c-text);
}
</style>

<style>
.document-image img {
  margin: 0 !important;
}
.notion-legal-editor-shell {
  min-height: 0;
  overflow: hidden !important;
}
.notion-legal-editor-shell > .md-wysiwyg {
  flex: 1;
  min-height: 0;
}
.md-table-toolbar {
  flex-wrap: wrap;
  overflow: visible !important;
}
</style>

<style scoped>
.md-wysiwyg-body :deep(.tiptap h1) {
  font-size: var(--document-h1);
}
.md-wysiwyg-body :deep(.tiptap h2) {
  font-size: var(--document-h2);
}
</style>

<style>
.writing-block-menu { position: fixed; z-index: 3100; width: 280px; border: 1px solid var(--c-border); border-radius: 10px; background: var(--c-bg-card); color: var(--c-text); box-shadow: 0 12px 40px #0002; padding: 6px; }
.writing-block-menu header { display: flex; justify-content: space-between; padding: 9px; font-size: 11px; color: var(--c-text-secondary); }
.writing-block-menu .block-options { max-height: 270px; overflow: auto; }
.writing-block-menu button { display: flex; align-items: center; gap: 12px; width: 100%; padding: 9px; text-align: left; border: 0; border-radius: 5px; background: transparent; color: inherit; cursor: pointer; font: inherit; }
.writing-block-menu button.selected, .writing-block-menu button:hover { background: var(--c-primary-light); }
.writing-block-menu .block-symbol { display: grid; place-items: center; width: 30px; height: 30px; border: 1px solid var(--c-border); background: var(--c-bg-card); border-radius: 5px; font-size: 16px; }
.writing-block-menu strong { display: block; font-size: 12px; font-weight: 500; }
.writing-block-menu small { display: block; font-size: 10px; margin-top: 3px; color: var(--c-text-secondary); }
.writing-block-menu p { font-size: 12px; padding: 8px; color: var(--c-text-secondary); }
</style>
