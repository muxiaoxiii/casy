import { closeHistory } from '@tiptap/pm/history'
import type { Editor } from '@tiptap/core'
import type { ContextAction } from '../components/ContextMenu.vue'

/** Retain the cell selection while focus moves into the contextual menu. */
export function tableActions(editor: Editor): ContextAction[] {
  const snapshot = editor.state.doc
  const selection = editor.state.selection.getBookmark()
  const entries = [
    ['addRowBefore', '在上方插入行'], ['addRowAfter', '在下方插入行'],
    ['addColumnBefore', '在左侧插入列'], ['addColumnAfter', '在右侧插入列'],
    ['mergeCells', '合并选中单元格'], ['splitCell', '拆分单元格'],
    ['toggleHeaderRow', '切换表头行'], ['toggleHeaderColumn', '切换表头列'],
    ['deleteRow', '删除所在行'], ['deleteColumn', '删除所在列'], ['deleteTable', '删除整张表格'],
  ] as const
  return entries.map(([command, label], index) => ({
    label,
    disabled: !editor.isEditable || !editor.can()[command](),
    separator: index === 4 || index === 6 || index === 8,
    danger: command.startsWith('delete'),
    shortcut: command.startsWith('delete') ? '可撤销' : undefined,
    run: () => {
      if (editor.isDestroyed || !editor.isEditable || editor.state.doc !== snapshot) return
      editor.view.dispatch(closeHistory(editor.state.tr).setSelection(selection.resolve(editor.state.doc)))
      editor.chain().focus()[command]().run()
      editor.view.dispatch(closeHistory(editor.state.tr))
    },
  }))
}
