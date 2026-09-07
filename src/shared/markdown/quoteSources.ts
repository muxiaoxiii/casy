import type { Editor } from '@tiptap/core'
import { Fragment } from '@tiptap/pm/model'
import { TextSelection } from '@tiptap/pm/state'

export const quoteColors = ['#986b68', '#69816f', '#627c94', '#9a8557']

/** Replace a uniform quote chain without flattening mixed-depth quotations. */
export function setQuoteDepth(editor: Editor, depth: number): boolean {
  if (!Number.isInteger(depth) || depth < 0 || depth > 4) return false
  const { state } = editor
  const { $from, $to } = state.selection
  let outer = 0
  for (let i = 1; i <= $from.depth; i++) {
    if ($from.node(i).type.name === 'blockquote') { outer = i; break }
  }
  if (!outer) {
    if (!depth) return true
    const chain = editor.chain().focus()
    for (let i = 0; i < depth; i++) chain.wrapIn('blockquote')
    return chain.run()
  }
  if ($to.pos > $from.end(outer)) return false
  let content = $from.node(outer).content
  while (content.childCount === 1 && content.firstChild?.type.name === 'blockquote') content = content.firstChild.content
  let mixed = false
  content.descendants(node => { if (node.type.name === 'blockquote') mixed = true })
  if (mixed) return false
  for (let i = 0; i < depth; i++) content = Fragment.from(state.schema.nodes.blockquote.create(null, content))
  const start = $from.before(outer)
  const tr = state.tr.replaceWith(start, $from.after(outer), content)
  tr.setSelection(TextSelection.near(tr.doc.resolve(start + depth + 1)))
  editor.view.dispatch(tr.scrollIntoView())
  editor.commands.focus()
  return true
}
