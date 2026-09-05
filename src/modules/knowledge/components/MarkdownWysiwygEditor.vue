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
import { watch, onBeforeUnmount } from 'vue'
import { useEditor, EditorContent } from '@tiptap/vue-3'
import { BubbleMenu } from '@tiptap/vue-3/menus'
import { Node, mergeAttributes } from '@tiptap/core'
import StarterKit from '@tiptap/starter-kit'
import Highlight from '@tiptap/extension-highlight'
import Placeholder from '@tiptap/extension-placeholder'
import TaskList from '@tiptap/extension-task-list'
import TaskItem from '@tiptap/extension-task-item'
import Image from '@tiptap/extension-image'
import { Table } from '@tiptap/extension-table'
import TableRow from '@tiptap/extension-table-row'
import TableCell from '@tiptap/extension-table-cell'
import TableHeader from '@tiptap/extension-table-header'
import TextAlign from '@tiptap/extension-text-align'
import { ElMessageBox } from 'element-plus'
import { mdToHtml, htmlToMd } from '../../../shared/markdown/mdBridge'
import WikiLinkSuggestion from '../../docs/extensions/WikiLinkSuggestion'

interface NoteTitleItem {
  id?: string
  title: string
  category?: string
  categoryLabel?: string
}

interface OutlineItem {
  id: string
  level: number
  text: string
}

const props = withDefaults(defineProps<{
  modelValue: string
  /** wiki 补全源（标题含方括号者不可链接，与后端解析一致） */
  noteTitles?: NoteTitleItem[]
  placeholder?: string
  compact?: boolean
  /** Enter 行为：paragraph=正常换段；submit=触发 submit 事件（Shift+Enter 仍换行） */
  enterMode?: 'paragraph' | 'submit'
}>(), {
  noteTitles: () => [],
  placeholder: '开始记录，输入 [[ 链接其他笔记…',
  compact: false,
  enterMode: 'paragraph',
})

const emit = defineEmits<{
  'update:modelValue': [value: string]
  save: []
  change: []
  blur: []
  submit: []
  'wiki-link-click': [payload: { title: string }]
  'outline-change': [items: OutlineItem[]]
}>()

/**
 * 最小 WikiLink 内联 atom 节点（不依赖 docs 模块的 Mark 版扩展，避免与并行开发耦合）。
 * 结构固定：<span data-wiki-link="" data-title="标题">标题</span>
 */
const WikiLinkNode = Node.create({
  name: 'wikiLink',
  group: 'inline',
  inline: true,
  atom: true,
  selectable: true,

  addAttributes() {
    return {
      title: {
        default: '',
        parseHTML: element => element.getAttribute('data-title') || element.textContent || '',
        renderHTML: attributes => ({ 'data-title': attributes.title }),
      },
    }
  },

  parseHTML() {
    return [{ tag: 'span[data-wiki-link]' }]
  },

  renderHTML({ node, HTMLAttributes }) {
    return [
      'span',
      mergeAttributes(HTMLAttributes, { 'data-wiki-link': '', class: 'wiki-link' }),
      String(node.attrs.title || ''),
    ]
  },
})

function decodeRawHtml(value: string | null): string {
  try {
    return decodeURIComponent(value || '')
  } catch {
    return value || ''
  }
}

const RawHtmlInline = Node.create({
  name: 'rawHtmlInline',
  group: 'inline',
  inline: true,
  atom: true,
  selectable: true,
  addAttributes() {
    return {
      raw: {
        default: '',
        parseHTML: element => decodeRawHtml(element.getAttribute('data-raw-html')),
        renderHTML: attributes => ({ 'data-raw-html': encodeURIComponent(attributes.raw || '') }),
      },
    }
  },
  parseHTML() {
    return [{ tag: 'span[data-raw-html]' }]
  },
  renderHTML({ HTMLAttributes }) {
    return ['span', mergeAttributes(HTMLAttributes, {
      'data-raw-html-kind': 'inline',
      class: 'raw-html-placeholder raw-html-placeholder--inline',
      title: '原始 HTML 已原样保留，请在源码模式编辑',
    }), 'HTML']
  },
})

const RawHtmlBlock = Node.create({
  name: 'rawHtmlBlock',
  group: 'block',
  atom: true,
  selectable: true,
  addAttributes() {
    return {
      raw: {
        default: '',
        parseHTML: element => decodeRawHtml(element.getAttribute('data-raw-html')),
        renderHTML: attributes => ({ 'data-raw-html': encodeURIComponent(attributes.raw || '') }),
      },
    }
  },
  parseHTML() {
    return [{ tag: 'div[data-raw-html]' }]
  },
  renderHTML({ HTMLAttributes }) {
    return ['div', mergeAttributes(HTMLAttributes, {
      'data-raw-html-kind': 'block',
      class: 'raw-html-placeholder raw-html-placeholder--block',
      title: '原始 HTML 已原样保留，请在源码模式编辑',
    }), '原始 HTML（已保留）']
  },
})

// ── 双向同步守卫（参考 MarkdownCodeMirror 的 applyingExternal 模式）─────────
let serializeTimer: ReturnType<typeof setTimeout> | null = null
let lastEmitted = props.modelValue || ''
let contentChanged = false

/** 防抖 400ms 把编辑器 HTML 序列化为 Markdown 回写 modelValue */
function scheduleSerialize() {
  if (serializeTimer) clearTimeout(serializeTimer)
  serializeTimer = setTimeout(flushSerialize, 400)
}

function flushSerialize(): string {
  if (serializeTimer) clearTimeout(serializeTimer)
  serializeTimer = null
  const ed = editor.value
  if (!ed) return String(props.modelValue ?? '')
  if (!contentChanged) return lastEmitted
  const md = htmlToMd(ed.getHTML())
  contentChanged = false
  if (md === lastEmitted) return md
  lastEmitted = md
  emit('update:modelValue', md)
  return md
}

// wiki 建议：本地笔记标题过滤（标题含方括号者无法被 [[ ]] 解析，不出现在候选）
function searchNotes(query: string) {
  const q = String(query || '').trim().toLowerCase()
  return Promise.resolve(
    props.noteTitles
      .filter(item => item?.title && !item.title.includes('[') && !item.title.includes(']'))
      .filter(item => !q || item.title.toLowerCase().includes(q))
      .slice(0, 20)
      .map(item => ({
        id: item.id || item.title,
        title: item.title,
        category: item.categoryLabel || item.category || '笔记',
      })),
  )
}

/** 选中建议项：删掉光标前未闭合的 [[query，替换为 wikiLink atom 节点 */
function insertWikiLinkNode(title: string) {
  const ed = editor.value
  if (!ed) return
  const { from } = ed.state.selection
  const textBefore = ed.state.doc.textBetween(Math.max(0, from - 80), from, ' ', ' ')
  const match = textBefore.match(/\[\[([^\]]*)$/)
  const start = match ? from - match[0].length : from
  ed.chain().focus().insertContentAt(
    { from: start, to: from },
    { type: 'wikiLink', attrs: { title } },
  ).run()
}

function collectOutline(ed: any): OutlineItem[] {
  const items: OutlineItem[] = []
  ed?.state?.doc?.descendants?.((node: any, pos: number) => {
    if (node.type?.name === 'heading') {
      items.push({ id: `heading-${pos}`, level: Number(node.attrs?.level || 1), text: node.textContent || '空标题' })
    }
  })
  emit('outline-change', items)
  return items
}

function shouldShowBubble({ editor: activeEditor, from, to }: { editor: any; from: number; to: number }) {
  return from !== to && !activeEditor.isActive('codeBlock')
}

const editor = useEditor({
  content: mdToHtml(props.modelValue || ''),
  extensions: [
    StarterKit.configure({
      heading: { levels: [1, 2, 3, 4] },
      link: { openOnClick: false },
    }),
    Highlight,
    TaskList,
    TaskItem.configure({ nested: true }),
    Image.configure({ allowBase64: true, inline: false }),
    Table.configure({ resizable: false }),
    TableRow,
    TableHeader,
    TableCell,
    TextAlign.configure({ types: ['heading', 'paragraph'] }),
    Placeholder.configure({ placeholder: props.placeholder }),
    WikiLinkNode,
    RawHtmlInline,
    RawHtmlBlock,
    WikiLinkSuggestion.configure({
      trigger: '[[',
      minQueryLength: 0,
      search: searchNotes,
      onSelect: (item) => insertWikiLinkNode(item.title),
    }),
  ],
  editorProps: {
    handleKeyDown(_view, event) {
      const mod = event.metaKey || event.ctrlKey
      // Cmd/Ctrl+S 保存（对齐既有 @save 语义）
      if (mod && event.key.toLowerCase() === 's') {
        event.preventDefault()
        flushSerialize()
        emit('save')
        return true
      }
      if (props.enterMode === 'submit' && event.key === 'Enter' && !event.shiftKey && !mod) {
        event.preventDefault()
        flushSerialize()
        emit('submit')
        return true
      }
      return false
    },
    handleClick(_view, _pos, event) {
      const target = (event.target as HTMLElement | null)?.closest?.('span[data-wiki-link]')
      if (target) {
        event.preventDefault()
        emit('wiki-link-click', { title: target.getAttribute('data-title') || '' })
        return true
      }
      return false
    },
  },
  onUpdate: () => {
    contentChanged = true
    emit('change')
    scheduleSerialize()
    collectOutline(editor.value)
  },
  onCreate: ({ editor: activeEditor }) => collectOutline(activeEditor),
  onBlur: () => {
    flushSerialize()
    emit('blur')
  },
})

// 外部 modelValue 变化（模式切换/切换笔记/版本恢复）→ 重建内容；自身回写不重建
watch(() => props.modelValue, (value) => {
  const ed = editor.value
  if (!ed) return
  const next = String(value ?? '')
  if (next === lastEmitted) return
  lastEmitted = next
  contentChanged = false
  if (serializeTimer) clearTimeout(serializeTimer)
  ed.commands.setContent(mdToHtml(next), { emitUpdate: false })
})

onBeforeUnmount(() => {
  if (serializeTimer) clearTimeout(serializeTimer)
  editor.value?.destroy()
})

// ── 工具栏动作 ──
function cmd(fn: (chain: any) => any) {
  const ed = editor.value
  if (!ed) return
  fn(ed.chain().focus()).run()
}

function setHeading(level: 1 | 2 | 3 | 4) {
  cmd(c => editor.value?.isActive('heading', { level }) ? c.setParagraph() : c.setHeading({ level }))
}

function setTextAlign(alignment: 'left' | 'center' | 'right') {
  cmd(c => c.setTextAlign(alignment))
}

async function setLink() {
  const ed = editor.value
  if (!ed) return
  if (ed.isActive('link')) {
    cmd(c => c.extendMarkRange('link').unsetLink())
    return
  }
  try {
    const { value } = await ElMessageBox.prompt('输入链接地址（https://…）', '插入链接', {
      inputPattern: /^https?:\/\/.+/,
      inputErrorMessage: '链接需以 http:// 或 https:// 开头',
      confirmButtonText: '确定',
      cancelButtonText: '取消',
    })
    if (value) cmd(c => c.extendMarkRange('link').setLink({ href: value }))
  } catch { /* 用户取消 */ }
}

function insertWikiLinkTrigger() {
  cmd(c => c.insertContent('[['))
}

function insertTable() {
  cmd(c => c.insertTable({ rows: 3, cols: 3, withHeaderRow: true }))
}

function scrollToHeading(id: string) {
  const ed = editor.value
  if (!ed) return false
  const pos = Number(id.replace('heading-', ''))
  if (!Number.isFinite(pos)) return false
  try {
    const dom = ed.view.nodeDOM(pos) as HTMLElement | null
    dom?.scrollIntoView({ behavior: 'smooth', block: 'center' })
    if (dom) ed.commands.setTextSelection(Math.min(pos + 1, ed.state.doc.content.size))
    return Boolean(dom)
  } catch {
    return false
  }
}

defineExpose({
  focus: () => editor.value?.commands.focus(),
  setMarkdown: (markdown: string) => {
    const next = String(markdown ?? '')
    editor.value?.commands.setContent(mdToHtml(next), { emitUpdate: false })
    contentChanged = true
    return flushSerialize()
  },
  /**
   * 笔记切换前由父组件显式调用。返回旧编辑器的权威 Markdown，
   * 避免 onBeforeUnmount 时父级 draft 已切换而发生丢失或串写。
   */
  flushAndGetMarkdown: () => flushSerialize(),
  getDocumentJson: () => editor.value?.getJSON() || null,
  getHtml: () => editor.value?.getHTML() || '',
  getOutline: () => collectOutline(editor.value),
  scrollToHeading,
})
</script>

<template>
  <div class="md-wysiwyg" :class="{ compact }">
    <div v-if="!compact && editor" class="md-wysiwyg-toolbar">
      <button type="button" :class="{ active: editor.isActive('heading', { level: 1 }) }" title="一级标题" @click="setHeading(1)">H1</button>
      <button type="button" :class="{ active: editor.isActive('heading', { level: 2 }) }" title="二级标题" @click="setHeading(2)">H2</button>
      <button type="button" :class="{ active: editor.isActive('heading', { level: 3 }) }" title="三级标题" @click="setHeading(3)">H3</button>
      <button type="button" :class="{ active: editor.isActive('heading', { level: 4 }) }" title="四级标题" @click="setHeading(4)">H4</button>
      <span class="tb-sep" />
      <button type="button" :class="{ active: editor.isActive('bold') }" title="粗体 (Cmd/Ctrl+B)" @click="cmd(c => c.toggleBold())"><b>B</b></button>
      <button type="button" :class="{ active: editor.isActive('italic') }" title="斜体 (Cmd/Ctrl+I)" @click="cmd(c => c.toggleItalic())"><i>I</i></button>
      <button type="button" :class="{ active: editor.isActive('underline') }" title="下划线 (Cmd/Ctrl+U)" @click="cmd(c => c.toggleUnderline())"><u>U</u></button>
      <button type="button" :class="{ active: editor.isActive('strike') }" title="删除线" @click="cmd(c => c.toggleStrike())"><s>S</s></button>
      <button type="button" :class="{ active: editor.isActive('highlight') }" title="高亮" @click="cmd(c => c.toggleHighlight())">高亮</button>
      <button type="button" :class="{ active: editor.isActive('code') }" title="行内代码" @click="cmd(c => c.toggleCode())"><code>码</code></button>
      <span class="tb-sep" />
      <button type="button" :class="{ active: editor.isActive({ textAlign: 'left' }) }" title="左对齐" @click="setTextAlign('left')">左</button>
      <button type="button" :class="{ active: editor.isActive({ textAlign: 'center' }) }" title="居中" @click="setTextAlign('center')">中</button>
      <button type="button" :class="{ active: editor.isActive({ textAlign: 'right' }) }" title="右对齐" @click="setTextAlign('right')">右</button>
      <span class="tb-sep" />
      <button type="button" :class="{ active: editor.isActive('blockquote') }" title="引用" @click="cmd(c => c.toggleBlockquote())">引用</button>
      <button type="button" :class="{ active: editor.isActive('codeBlock') }" title="代码块" @click="cmd(c => c.toggleCodeBlock())">代码块</button>
      <button type="button" title="分隔线" @click="cmd(c => c.setHorizontalRule())">分隔线</button>
      <span class="tb-sep" />
      <button type="button" :class="{ active: editor.isActive('bulletList') }" title="无序列表" @click="cmd(c => c.toggleBulletList())">列表</button>
      <button type="button" :class="{ active: editor.isActive('orderedList') }" title="有序列表" @click="cmd(c => c.toggleOrderedList())">编号</button>
      <button type="button" :class="{ active: editor.isActive('taskList') }" title="任务列表" @click="cmd(c => c.toggleTaskList())">待办</button>
      <button type="button" title="插入表格" @click="insertTable">表格</button>
      <span class="tb-sep" />
      <button type="button" :class="{ active: editor.isActive('link') }" title="插入/移除链接" @click="setLink">链接</button>
      <button type="button" title="插入知识双向链接（输入 [[ 也可触发）" @click="insertWikiLinkTrigger">双链</button>
      <span class="tb-sep" />
      <button type="button" title="撤销 (Cmd/Ctrl+Z)" :disabled="!editor.can().undo()" @click="cmd(c => c.undo())">撤销</button>
      <button type="button" title="重做 (Cmd/Ctrl+Shift+Z)" :disabled="!editor.can().redo()" @click="cmd(c => c.redo())">重做</button>
    </div>
    <div v-if="!compact && editor?.isActive('table')" class="md-table-toolbar">
      <span>表格</span>
      <button type="button" @click="cmd(c => c.addColumnBefore())">左侧加列</button>
      <button type="button" @click="cmd(c => c.addColumnAfter())">右侧加列</button>
      <button type="button" @click="cmd(c => c.deleteColumn())">删列</button>
      <button type="button" @click="cmd(c => c.addRowBefore())">上方加行</button>
      <button type="button" @click="cmd(c => c.addRowAfter())">下方加行</button>
      <button type="button" @click="cmd(c => c.deleteRow())">删行</button>
      <button type="button" @click="cmd(c => c.mergeOrSplit())">合并/拆分</button>
      <button type="button" class="danger" @click="cmd(c => c.deleteTable())">删除表格</button>
    </div>
    <BubbleMenu
      v-if="!compact && editor"
      :editor="editor"
      :should-show="shouldShowBubble"
      :options="{ placement: 'top', offset: 8, flip: true, shift: true }"
      class="md-bubble-menu"
    >
      <button type="button" :class="{ active: editor.isActive('bold') }" @click="cmd(c => c.toggleBold())"><b>B</b></button>
      <button type="button" :class="{ active: editor.isActive('italic') }" @click="cmd(c => c.toggleItalic())"><i>I</i></button>
      <button type="button" :class="{ active: editor.isActive('underline') }" @click="cmd(c => c.toggleUnderline())"><u>U</u></button>
      <button type="button" :class="{ active: editor.isActive('highlight') }" @click="cmd(c => c.toggleHighlight())">高亮</button>
      <button type="button" :class="{ active: editor.isActive('link') }" @click="setLink">链接</button>
    </BubbleMenu>
    <EditorContent :editor="editor" class="md-wysiwyg-body" />
  </div>
</template>

<style scoped>
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

.md-wysiwyg-toolbar::-webkit-scrollbar { display: none; }

.md-wysiwyg-toolbar button {
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

.md-table-toolbar span { font-size: 10px; font-weight: 750; color: var(--c-primary); margin-right: 4px; }
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
.md-bubble-menu button.active { background: var(--c-bg-hover); color: var(--c-primary); }
.md-table-toolbar button.danger { color: var(--status-risk); }
.md-bubble-menu {
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 5px;
  border: 1px solid var(--c-border);
  border-radius: 9px;
  background: var(--c-bg-card);
  box-shadow: var(--shadow-lg, 0 10px 30px rgba(0,0,0,.14));
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
  font-size: 14.5px;
  line-height: 1.8;
  color: var(--c-text);
  font-family: var(--font-family);
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

.md-wysiwyg-body :deep(.tiptap p) { margin: 0 0 8px; }

.md-wysiwyg-body :deep(.tiptap h1),
.md-wysiwyg-body :deep(.tiptap h2),
.md-wysiwyg-body :deep(.tiptap h3),
.md-wysiwyg-body :deep(.tiptap h4) {
  color: var(--c-text-heading);
  font-weight: 700;
  margin: 18px 0 10px;
}

.md-wysiwyg-body :deep(.tiptap h1) { font-size: 24px; border-bottom: 1px solid var(--c-border-light); padding-bottom: 6px; }
.md-wysiwyg-body :deep(.tiptap h2) { font-size: 19px; }
.md-wysiwyg-body :deep(.tiptap h3) { font-size: 16px; }
.md-wysiwyg-body :deep(.tiptap h4) { font-size: 14.5px; }

.md-wysiwyg-body :deep(.tiptap ul),
.md-wysiwyg-body :deep(.tiptap ol) { padding-left: 22px; margin: 8px 0; }
.md-wysiwyg-body :deep(.tiptap li) { margin-bottom: 3px; }
.md-wysiwyg-body :deep(.tiptap li p) { margin: 0; }

/* 任务列表 */
.md-wysiwyg-body :deep(.tiptap ul[data-type="taskList"]) { list-style: none; padding-left: 2px; }
.md-wysiwyg-body :deep(.tiptap li[data-type="taskItem"]) {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  margin-bottom: 4px;
}
.md-wysiwyg-body :deep(.tiptap li[data-type="taskItem"] > label) { margin-top: 4px; }
.md-wysiwyg-body :deep(.tiptap li[data-type="taskItem"] input[type="checkbox"]) {
  accent-color: var(--c-primary);
  cursor: pointer;
}
.md-wysiwyg-body :deep(.tiptap li[data-type="taskItem"] > div) { flex: 1; min-width: 0; }

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
