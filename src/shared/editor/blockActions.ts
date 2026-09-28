import type { Editor } from '@tiptap/core'
import { Selection } from '@tiptap/pm/state'
import { closeHistory } from '@tiptap/pm/history'

export function blockAt(editor: Editor, index: number) {
  if (index < 0 || index >= editor.state.doc.childCount) return null
  let pos = 0
  for (let i = 0; i < index; i++) pos += editor.state.doc.child(i).nodeSize
  return { node: editor.state.doc.child(index), pos }
}
export function editBlock(editor: Editor, index: number, action: 'duplicate' | 'delete' | 'before' | 'after') {
  const block = blockAt(editor, index)
  if (!block || !editor.isEditable) return false
  const { pos, node } = block
  const tr = closeHistory(editor.state.tr)
  let destination = pos
  if (action === 'delete') tr.delete(pos, pos + node.nodeSize)
  else {
    destination = action === 'before' ? pos : pos + node.nodeSize
    tr.insert(destination, action === 'duplicate' ? node : editor.schema.nodes.paragraph.create())
  }
  tr.setSelection(Selection.near(tr.doc.resolve(Math.min(destination, tr.doc.content.size))))
  editor.view.dispatch(tr.scrollIntoView())
  editor.view.dispatch(closeHistory(editor.state.tr))
  editor.commands.focus()
  return true
}
