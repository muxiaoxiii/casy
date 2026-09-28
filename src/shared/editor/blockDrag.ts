import { onBeforeUnmount, reactive } from 'vue'
import type { Editor } from '@tiptap/core'
import { Fragment, type Node } from '@tiptap/pm/model'
import { Selection } from '@tiptap/pm/state'
import { selectBlocks, selectedBlockRange, actionBlockIndices } from './blockSelection'
import { closeHistory } from '@tiptap/pm/history'

/** Destination is a gap between top-level nodes, in the original document. */
export function moveBlock(editor: Editor, from: number, gap: number): boolean {
  return moveBlocks(editor, from, from, gap)
}

export function moveBlocks(editor: Editor, first: number, last: number, gap: number): boolean {
  const doc = editor.state.doc
  if (!editor.isEditable || first < 0 || last < first || last >= doc.childCount || gap < 0 || gap > doc.childCount) return false
  const indices = actionBlockIndices(editor, first, last)
  if (indices.length === last - first + 1 && gap >= first && gap <= last + 1) return false
  const blocks: { node: Node; pos: number; index: number }[] = []
  let destination = 0
  doc.forEach((node, pos, index) => {
    if (indices.includes(index)) blocks.push({ node, pos, index })
    if (index < gap) destination += node.nodeSize
  })
  destination -= blocks.filter(block => block.index < gap).reduce((sum, block) => sum + block.node.nodeSize, 0)
  const backward = editor.state.selection.anchor > editor.state.selection.head
  const tr = closeHistory(editor.state.tr)
  for (const block of [...blocks].reverse()) tr.delete(block.pos, block.pos + block.node.nodeSize)
  tr.insert(destination, Fragment.fromArray(blocks.map(block => block.node)))
  tr.setSelection(Selection.near(tr.doc.resolve(destination)))
  editor.view.dispatch(tr.scrollIntoView())
  editor.view.dispatch(closeHistory(editor.state.tr))
  const start = gap - indices.filter(index => index < gap).length
  selectBlocks(editor, start, start + indices.length - 1, backward)
  return true
}

/** Pointer proximity to a visible scroll edge; capped to keep long-document moves controllable. */
export function edgeScrollDelta(y: number, top: number, bottom: number): number {
  if (y < top - 20 || y > bottom + 20 || bottom <= top) return 0
  const edge = Math.min(48, (bottom - top) / 3)
  if (y < top + edge) return -Math.ceil(16 * Math.min(1, (top + edge - y) / edge))
  if (y > bottom - edge) return Math.ceil(16 * Math.min(1, (y - bottom + edge) / edge))
  return 0
}

export function useBlockDrag(editorRef: () => Editor | undefined | null, prepare: () => void = () => {}) {
  const state = reactive({ active: false, index: -1, x: 0, y: 0, lineX: 0, lineY: 0, lineWidth: 0, gap: -1, message: '' })
  let source = -1
  let sourceLast = -1
  let snapshot: Node | null = null
  let pointerId = -1
  let startY = 0
  let travelled = false
  let pointerX = 0, pointerY = 0
  let animation = 0
  let scrollParents: HTMLElement[] = []
  function scrollFrame() {
    animation = 0
    const editor = editorRef()
    if (!state.active || !editor || editor.state.doc !== snapshot) return hide()
    if (travelled) {
      const bounds = editor.view.dom.getBoundingClientRect()
      if (pointerX >= bounds.left - 32 && pointerX <= bounds.right + 32) {
        const visibleTop = Math.max(0, ...scrollParents.map(el => el === document.scrollingElement ? 0 : el.getBoundingClientRect().top))
        const visibleBottom = Math.min(window.innerHeight, ...scrollParents.map(el => el === document.scrollingElement ? window.innerHeight : el.getBoundingClientRect().bottom))
        const delta = edgeScrollDelta(pointerY, visibleTop, visibleBottom)
        for (const parent of scrollParents) {
          const before = parent.scrollTop
          if (delta < 0 && before <= 0 || delta > 0 && before >= parent.scrollHeight - parent.clientHeight) continue
          parent.scrollTop += delta
          if (parent.scrollTop !== before) { updateTarget(); break }
        }
      }
    }
    animation = window.requestAnimationFrame(scrollFrame)
  }
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
    window.cancelAnimationFrame(animation); animation = 0
    scrollParents = []
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
    pointerX = event.clientX; pointerY = event.clientY
    updateTarget()
  }
  function updateTarget() {
    const editor = editorRef()
    if (!editor || !state.active) return
    const bounds = editor.view.dom.getBoundingClientRect()
    if (pointerX < bounds.left - 32 || pointerX > bounds.right + 32 || pointerY < bounds.top - 20 || pointerY > bounds.bottom + 20) { state.gap = -1; return }
    const items = blocks(editor)
    const next = items.find(b => pointerY < (b.rect.top + b.rect.bottom) / 2)
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
    const from = source, last = sourceLast, gap = state.gap, original = snapshot
    cancel()
    if (editor && travelled && editor.state.doc === original && moveBlocks(editor, from, last, gap)) {
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
    const selected = selectedBlockRange(editor)
    const includesHandle = selected && selected.indices.includes(state.index)
    source = includesHandle ? selected.first : state.index
    sourceLast = includesHandle ? selected.last : state.index
    snapshot = editor.state.doc
    pointerId = event.pointerId
    startY = event.clientY; travelled = false
    pointerX = event.clientX; pointerY = event.clientY
    for (let parent = editor.view.dom.parentElement; parent; parent = parent.parentElement) {
      if (/(auto|scroll)/.test(getComputedStyle(parent).overflowY) && parent.scrollHeight > parent.clientHeight) scrollParents.push(parent)
    }
    const root = document.scrollingElement
    if (root instanceof HTMLElement && !scrollParents.includes(root)) scrollParents.push(root)
    state.active = true
    state.gap = -1
    window.addEventListener('pointermove', onMove)
    window.addEventListener('pointerup', onUp)
    window.addEventListener('pointercancel', cancel)
    window.addEventListener('keydown', keydown)
    window.addEventListener('blur', cancel)
    animation = window.requestAnimationFrame(scrollFrame)
  }
  function move(direction: -1 | 1) {
    const editor = editorRef()
    if (!editor) return
    prepare()
    const selected = selectedBlockRange(editor)
    const includesHandle = selected && selected.indices.includes(state.index)
    const from = includesHandle ? selected.first : state.index
    const last = includesHandle ? selected.last : state.index
    if (moveBlocks(editor, from, last, direction < 0 ? from - 1 : last + 2)) {
      show(from + direction)
      state.message = direction < 0 ? '内容块已上移，可撤销' : '内容块已下移，可撤销'
    }
  }
  function selectCurrent() {
    const editor = editorRef()
    if (editor) show(editor.state.selection.$from.index(0))
  }
  function onScroll() { if (state.active) updateTarget(); else hide() }
  window.addEventListener('scroll', onScroll, true)
  window.addEventListener('resize', hide)
  onBeforeUnmount(() => { cleanup(); window.removeEventListener('scroll', onScroll, true); window.removeEventListener('resize', hide) })
  return { state, start, hover, hide, move, selectCurrent }
}
