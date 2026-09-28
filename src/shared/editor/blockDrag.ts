import { onBeforeUnmount, reactive } from 'vue'
import type { Editor } from '@tiptap/core'
import type { Node } from '@tiptap/pm/model'
import { Selection } from '@tiptap/pm/state'
import { closeHistory } from '@tiptap/pm/history'

/** Destination is a gap between top-level nodes, in the original document. */
export function moveBlock(editor: Editor, from: number, gap: number): boolean {
  const doc = editor.state.doc
  if (!editor.isEditable || from < 0 || from >= doc.childCount || gap < 0 || gap > doc.childCount || gap === from || gap === from + 1) return false
  let source = 0, destination = 0
  doc.forEach((node, pos, index) => { if (index === from) source = pos; if (index < gap) destination += node.nodeSize })
  const node = doc.child(from)
  if (gap > from) destination -= node.nodeSize
  const tr = closeHistory(editor.state.tr).delete(source, source + node.nodeSize).insert(destination, node)
  tr.setSelection(Selection.near(tr.doc.resolve(destination)))
  editor.view.dispatch(tr.scrollIntoView())
  // A later typing operation must not be absorbed into the move's undo step.
  editor.view.dispatch(closeHistory(editor.state.tr))
  return true
}

export function useBlockDrag(editorRef: () => Editor | undefined | null, prepare: () => void = () => {}) {
  const state = reactive({ active: false, index: -1, x: 0, y: 0, lineX: 0, lineY: 0, lineWidth: 0, gap: -1, message: '' })
  let source = -1
  let snapshot: Node | null = null
  let pointerId = -1
  let startY = 0
  let travelled = false
  function blocks(editor: Editor) {
    const result: { index: number; rect: DOMRect }[] = []
    editor.state.doc.forEach((_node, pos, index) => {
      const dom = editor.view.nodeDOM(pos)
      if (dom instanceof HTMLElement) result.push({ index, rect: dom.getBoundingClientRect() })
    })
    return result
  }
  function show(index: number) {
    const editor = editorRef()
    if (!editor) return
    const block = blocks(editor).find(b => b.index === index)
    if (!block) return
    state.index = index
    state.x = Math.max(4, block.rect.left - 25)
    state.y = block.rect.top
  }
  function hover(event: PointerEvent) {
    if (state.active) return
    const editor = editorRef()
    if (!editor || !editor.isEditable || !(event.target instanceof HTMLElement) || !editor.view.dom.contains(event.target)) return
    const block = blocks(editor).find(b => event.clientY >= b.rect.top && event.clientY <= b.rect.bottom)
    if (block) show(block.index)
  }
  function cleanup() {
    window.removeEventListener('pointermove', onMove)
    window.removeEventListener('pointerup', onUp)
    window.removeEventListener('pointercancel', cancel)
    window.removeEventListener('keydown', keydown)
    window.removeEventListener('blur', cancel)
  }
  function cancel() { cleanup(); state.active = false; state.gap = -1; snapshot = null; source = -1 }
  function hide() { cancel(); state.index = -1 }
  function keydown(event: KeyboardEvent) { if (event.key === 'Escape') { event.preventDefault(); cancel() } }
  function onMove(event: PointerEvent) {
    if (!state.active || event.pointerId !== pointerId) return
    const editor = editorRef()
    if (!editor || editor.state.doc !== snapshot) return hide()
    travelled ||= Math.abs(event.clientY - startY) > 4
    const bounds = editor.view.dom.getBoundingClientRect()
    if (event.clientX < bounds.left - 32 || event.clientX > bounds.right + 32 || event.clientY < bounds.top - 20 || event.clientY > bounds.bottom + 20) { state.gap = -1; return }
    const items = blocks(editor)
    const next = items.find(b => event.clientY < (b.rect.top + b.rect.bottom) / 2)
    const anchor = next || items[items.length - 1]
    if (!anchor) return
    state.gap = next ? next.index : editor.state.doc.childCount
    state.lineY = next ? anchor.rect.top : anchor.rect.bottom
    state.lineX = anchor.rect.left
    state.lineWidth = anchor.rect.width
  }
  function onUp(event: PointerEvent) {
    if (event.pointerId !== pointerId) return
    const editor = editorRef()
    const from = source, gap = state.gap, original = snapshot
    cancel()
    if (editor && travelled && editor.state.doc === original && moveBlock(editor, from, gap)) {
      state.message = '内容块已移动，可撤销'
      state.index = -1
      editor.commands.focus()
    }
  }
  function start(event: PointerEvent) {
    const editor = editorRef()
    if (!editor || event.button !== 0 || state.index < 0 || !editor.isEditable) return
    prepare()
    event.preventDefault()
    cancel()
    source = state.index
    snapshot = editor.state.doc
    pointerId = event.pointerId
    startY = event.clientY; travelled = false
    state.active = true
    state.gap = -1
    window.addEventListener('pointermove', onMove)
    window.addEventListener('pointerup', onUp)
    window.addEventListener('pointercancel', cancel)
    window.addEventListener('keydown', keydown)
    window.addEventListener('blur', cancel)
  }
  function move(direction: -1 | 1) {
    const editor = editorRef()
    if (!editor) return
    prepare()
    const from = state.index
    if (moveBlock(editor, from, direction < 0 ? from - 1 : from + 2)) {
      show(from + direction)
      state.message = direction < 0 ? '内容块已上移，可撤销' : '内容块已下移，可撤销'
    }
  }
  function selectCurrent() {
    const editor = editorRef()
    if (editor) show(editor.state.selection.$from.index(0))
  }
  window.addEventListener('scroll', hide, true)
  window.addEventListener('resize', hide)
  onBeforeUnmount(() => { cleanup(); window.removeEventListener('scroll', hide, true); window.removeEventListener('resize', hide) })
  return { state, start, hover, hide, move, selectCurrent }
}
