import type { Editor } from '@tiptap/core'
import { Selection } from '@tiptap/pm/state'
import { Fragment } from '@tiptap/pm/model'
import { selectBlocks, actionBlockIndices } from './blockSelection'
import { closeHistory } from '@tiptap/pm/history'

export function blockAt(editor: Editor, index: number) {
  if (index < 0 || index >= editor.state.doc.childCount) return null
  let pos = 0
  for (let i = 0; i < index; i++) pos += editor.state.doc.child(i).nodeSize
  return { node: editor.state.doc.child(index), pos }
}
export function editBlock(editor: Editor, index: number, action: 'duplicate' | 'delete' | 'before' | 'after') {
  return editBlocks(editor, index, index, action)
}
export function editBlocks(editor: Editor, first: number, last: number, action: 'duplicate' | 'delete' | 'before' | 'after') {
  const start = blockAt(editor, first), end = blockAt(editor, last)
  if (!start || !end || last < first || !editor.isEditable) return false
  const indices = actionBlockIndices(editor, first, last)
  const blocks = indices.map(index => blockAt(editor, index)!)
  const from = start.pos, to = end.pos + end.node.nodeSize
  const backward = editor.state.selection.anchor > editor.state.selection.head
  const tr = closeHistory(editor.state.tr)
  let destination = from
  if (action === 'delete') { for (const block of [...blocks].reverse()) tr.delete(block.pos, block.pos + block.node.nodeSize) }
  else {
    destination = action === 'before' ? from : to
    tr.insert(destination, action === 'duplicate' ? Fragment.fromArray(blocks.map(block => block.node)) : editor.schema.nodes.paragraph.create())
  }
  tr.setSelection(Selection.near(tr.doc.resolve(Math.min(destination, tr.doc.content.size))))
  editor.view.dispatch(tr.scrollIntoView())
  editor.view.dispatch(closeHistory(editor.state.tr))
  if (action === 'duplicate' && first !== last) selectBlocks(editor, last + 1, last + indices.length, backward)
  editor.commands.focus()
  return true
}
