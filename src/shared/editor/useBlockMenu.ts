import { computed, ref } from 'vue'
import type { Editor, ChainedCommands } from '@tiptap/core'

export const blockCommands: Array<{ id: string; label: string; hint: string; keywords: string; apply: (chain: ChainedCommands) => ChainedCommands }> = [
  { id: 'paragraph', label: '正文', hint: '连续书写', keywords: 'text p zhengwen', apply: c => c.setParagraph() },
  { id: 'heading1', label: '一级标题', hint: '# 空格', keywords: 'h1 heading 标题', apply: c => c.setHeading({ level: 1 }) },
  { id: 'heading2', label: '二级标题', hint: '## 空格', keywords: 'h2 heading 标题', apply: c => c.setHeading({ level: 2 }) },
  { id: 'heading3', label: '三级标题', hint: '### 空格', keywords: 'h3 heading 标题', apply: c => c.setHeading({ level: 3 }) },
  { id: 'task', label: '待办清单', hint: '可关联任务中心', keywords: 'todo task checklist 任务', apply: c => c.toggleTaskList() },
  { id: 'bullet', label: '无序列表', hint: '− 空格', keywords: 'list bullet 列表', apply: c => c.toggleBulletList() },
  { id: 'ordered', label: '有序列表', hint: '1. 空格', keywords: 'list number 列表', apply: c => c.toggleOrderedList() },
  { id: 'quote', label: '引用', hint: '> 空格', keywords: 'quote blockquote', apply: c => c.toggleBlockquote() },
  { id: 'table', label: '表格', hint: '3 × 3，可继续增删', keywords: 'table biaoge', apply: c => c.insertTable({ rows: 3, cols: 3, withHeaderRow: true }) },
  { id: 'code', label: '代码块', hint: '```', keywords: 'code', apply: c => c.toggleCodeBlock() },
  { id: 'math', label: '数学公式', hint: '编辑 LaTeX 源码', keywords: 'math latex 公式', apply: c => c.insertContent({type:'mathBlock',attrs:{source:'x_i + y_i'}}) },
  { id: 'mermaid', label: '流程图', hint: 'Mermaid 源码与预览', keywords: 'mermaid diagram 图表 流程图', apply: c => c.insertContent({type:'codeBlock',attrs:{language:'mermaid'},content:[{type:'text',text:'graph LR\n  A[立案] --> B[开庭]'}]}) },
  { id: 'wiki', label: '链接笔记', hint: '[[ 双向链接', keywords: 'wiki link 笔记 双链', apply: c => c.insertContent('[[') },
  { id: 'divider', label: '分隔线', hint: '---', keywords: 'divider hr', apply: c => c.setHorizontalRule() },
]

export function useBlockMenu() {
  const state = ref({ open: false, query: '', from: 0, to: 0, x: 0, y: 0, selected: 0 })
  let dismissedAt = -1
  const items = computed(() => blockCommands.filter(c => `${c.label} ${c.keywords}`.toLowerCase().includes(state.value.query.trim().toLowerCase())))
  function update(editor: Editor) {
    const { $from, from, empty } = editor.state.selection
    const before = $from.parent.textBetween(0, $from.parentOffset, undefined, '\ufffc')
    const match = empty && !editor.isActive('codeBlock') && before.match(/^\/([^/]{0,24})$/)
    if (!match) { state.value.open = false; dismissedAt = -1; return }
    if (dismissedAt === from) return
    const start = from - match[0].length
    const reset = state.value.query !== match[1] || state.value.from !== start
    const coords = editor.view.coordsAtPos(from)
    state.value = { open: true, query: match[1], from: start, to: from,
      x: Math.max(12, Math.min(coords.left, window.innerWidth - 292)),
      y: Math.max(12, Math.min(coords.bottom + 8, window.innerHeight - 340)),
      selected: reset ? 0 : state.value.selected }
  }
  function close() { dismissedAt = state.value.to; state.value.open = false }
  function run(editor: Editor, id: string) {
    const item = items.value.find(c => c.id === id)
    if (!item) return
    const { from, to } = state.value
    close()
    item.apply(editor.chain().focus().deleteRange({ from, to })).run()
  }
  function keydown(editor: Editor, event: KeyboardEvent) {
    if (!state.value.open || event.isComposing || editor.view.composing) return false
    if (event.key === 'Escape') { event.preventDefault(); close(); return true }
    if ((event.key === 'ArrowDown' || event.key === 'ArrowUp') && items.value.length) {
      event.preventDefault()
      state.value.selected = (state.value.selected + (event.key === 'ArrowDown' ? 1 : -1) + items.value.length) % items.value.length
      return true
    }
    if (event.key === 'Enter' && items.value[state.value.selected]) {
      event.preventDefault(); run(editor, items.value[state.value.selected].id); return true
    }
    return false
  }
  return { state, items, update, close, run, keydown }
}
