import { Extension, type Editor } from '@tiptap/core'
import { Selection, SelectionRange, TextSelection, type SelectionBookmark } from '@tiptap/pm/state'
import { Fragment, Slice, type Node, type ResolvedPos } from '@tiptap/pm/model'
import type { Mappable } from '@tiptap/pm/transform'
import { Plugin } from '@tiptap/pm/state'
import { Decoration, DecorationSet } from '@tiptap/pm/view'
import { CellSelection } from '@tiptap/pm/tables'

export function selectedBlockRange(editor: Editor) {
  const selection = editor.state.selection
  if (selection.empty || selection instanceof CellSelection) return null
  if (selection instanceof BlockSelection) {
    const indices = selection.ranges.map(range => range.$from.index(0))
    return { first: indices[0], last: indices[indices.length - 1], indices }
  }
  const first = selection.$from.index(0)
  const last = selection.$to.depth === 0 ? selection.$to.index(0) - 1 : selection.$to.index(0)
  const end = Math.min(last, editor.state.doc.childCount - 1)
  return first <= end ? { first, last: end, indices: Array.from({ length: end - first + 1 }, (_, i) => first + i) } : null
}

function boundaries(doc: Node, first: number, last: number) {
  if (first < 0 || last < first || last >= doc.childCount) return null
  let from = 0, to = 0
  doc.forEach((node, pos, index) => { if (index === first) from = pos; if (index === last) to = pos + node.nodeSize })
  return { from, to }
}

/** Separate ranges ensure clipboard and typing never include gaps in a block selection. */
export class BlockSelection extends Selection {
  constructor($from: ResolvedPos, $to: ResolvedPos, indices?: number[]) {
    const doc = $from.doc
    const first = Math.min($from.index(0), $to.index(0)), last = Math.max($from.index(0), $to.index(0)) - 1
    const selected = indices || Array.from({ length: last - first + 1 }, (_, i) => first + i)
    const ranges = selected.map(index => { const range = boundaries(doc, index, index)!; return new SelectionRange(doc.resolve(range.from), doc.resolve(range.to)) })
    super($from, $to, ranges)
  }
  eq(other: Selection) { return other instanceof BlockSelection && other.anchor === this.anchor && other.head === this.head && JSON.stringify(other.toJSON()) === JSON.stringify(this.toJSON()) }
  content() { return new Slice(Fragment.fromArray(this.ranges.map(range => range.$from.nodeAfter!)), 0, 0) }
  map(doc: Node, mapping: Mappable): Selection { return this.getBookmark().map(mapping).resolve(doc) }
  toJSON() { return { type: 'casy-blocks', from: this.anchor, to: this.head, ranges: this.ranges.map(r => ({ from: r.$from.pos, to: r.$to.pos })) } }
  static fromJSON(doc: Node, json: { from: number; to: number; backward?: boolean; ranges?: { from: number; to: number }[] }) {
    return new BlockBookmark(json.ranges || [{ from: Math.min(json.from, json.to), to: Math.max(json.from, json.to) }], json.backward || json.from > json.to).resolve(doc)
  }
  getBookmark(): SelectionBookmark { return new BlockBookmark(this.ranges.map(r => ({ from: r.$from.pos, to: r.$to.pos })), this.anchor > this.head) }
}
BlockSelection.prototype.visible = false
class BlockBookmark implements SelectionBookmark {
  constructor(readonly ranges: { from: number; to: number }[], readonly backward = false) {}
  map(mapping: Mappable) { return new BlockBookmark(this.ranges.map(r => ({ from: mapping.map(r.from, 1), to: mapping.map(r.to, -1) })).filter(r => r.to > r.from), this.backward) }
  resolve(doc: Node): Selection {
    const indices = new Set<number>()
    for (const range of this.ranges) {
      const start = doc.resolve(Math.max(0, Math.min(range.from, doc.content.size)))
      const end = doc.resolve(Math.max(0, Math.min(range.to, doc.content.size)))
      if (end.pos <= start.pos) continue
      const last = end.depth === 0 ? end.index(0) - 1 : end.index(0)
      for (let i = start.index(0); i <= last && i < doc.childCount; i++) indices.add(i)
    }
    const selected = [...indices].sort((a, b) => a - b)
    if (!selected.length) return Selection.near(doc.resolve(Math.min(this.ranges[0]?.from || 0, doc.content.size)))
    const range = boundaries(doc, selected[0], selected[selected.length - 1])!
    return new BlockSelection(doc.resolve(this.backward ? range.to : range.from), doc.resolve(this.backward ? range.from : range.to), selected)
  }
}
export function toggleBlock(editor: Editor, index: number) {
  if (!boundaries(editor.state.doc, index, index)) return false
  const indices = editor.state.selection instanceof BlockSelection ? selectedBlockRange(editor)!.indices : []
  const next = indices.includes(index) ? indices.filter(i => i !== index) : [...indices, index].sort((a, b) => a - b)
  if (!next.length) { editor.view.dispatch(editor.state.tr.setSelection(Selection.near(editor.state.doc.resolve(boundaries(editor.state.doc, index, index)!.from)))); return true }
  const range = boundaries(editor.state.doc, next[0], next[next.length - 1])!
  editor.view.dispatch(editor.state.tr.setSelection(new BlockSelection(editor.state.doc.resolve(range.from), editor.state.doc.resolve(range.to), next)))
  return true
}
export function actionBlockIndices(editor: Editor, first: number, last: number) {
  const selection = selectedBlockRange(editor)
  return editor.state.selection instanceof BlockSelection && selection?.first === first && selection.last === last
    ? selection.indices : Array.from({ length: last - first + 1 }, (_, i) => first + i)
}
// Vite can evaluate this module again during development.
try { Selection.jsonID('casy-blocks', BlockSelection) } catch (error) {
  if (!(error instanceof RangeError) || !error.message.includes('Duplicate')) throw error
}
export function selectBlocks(editor: Editor, first: number, last = first, backward = false) {
  const range = boundaries(editor.state.doc, first, last)
  if (!range) return false
  editor.view.dispatch(editor.state.tr.setSelection(new BlockSelection(editor.state.doc.resolve(backward ? range.to : range.from), editor.state.doc.resolve(backward ? range.from : range.to))))
  return true
}
/** Keep the initial block anchored so changing direction contracts the selection. */
export function extendBlocksTo(editor: Editor, target: number) {
  const selection = editor.state.selection
  const range = selectedBlockRange(editor)
  const anchor = selection instanceof BlockSelection && range
    ? selection.anchor > selection.head ? range.last : range.first
    : Math.min(selection.$anchor.index(0), editor.state.doc.childCount - 1)
  const end = Math.max(0, Math.min(target, editor.state.doc.childCount - 1))
  return selectBlocks(editor, Math.min(anchor, end), Math.max(anchor, end), end < anchor)
}
export function extendBlocksBy(editor: Editor, direction: -1 | 1) {
  const range = selectedBlockRange(editor)
  if (!range) return false
  const head = editor.state.selection.anchor > editor.state.selection.head ? range.first : range.last
  return extendBlocksTo(editor, head + direction)
}
export function selectBlockText(editor: Editor, first: number, last: number) {
  const range = boundaries(editor.state.doc, first, last)
  if (!range) return false
  editor.view.dispatch(editor.state.tr.setSelection(TextSelection.between(editor.state.doc.resolve(range.from), editor.state.doc.resolve(range.to))))
  return true
}
export const BlockSelectionExtension = Extension.create({
  name: 'blockSelection',
  addProseMirrorPlugins() {
    return [new Plugin({ props: { decorations(state) {
      if (!(state.selection instanceof BlockSelection)) return null
      const decorations: Decoration[] = []
      state.doc.forEach((node, pos) => {
        if (state.selection.ranges.some(range => pos >= range.$from.pos && pos < range.$to.pos)) decorations.push(Decoration.node(pos, pos + node.nodeSize, { class: 'casy-block-selected' }))
      })
      return DecorationSet.create(state.doc, decorations)
    } } })]
  },
})
