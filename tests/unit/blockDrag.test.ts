// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest'
import { Editor } from '@tiptap/core'
import { documentExtensions } from '../../src/shared/editor/schema'
import { moveBlock } from '../../src/shared/editor/blockDrag'
import { MarkdownPreservation } from '../../src/shared/editor/markdownPreservation'
import { mdToHtml } from '../../src/shared/markdown/mdBridge'
let editor: Editor
function setup(content: string) { editor = new Editor({ extensions: documentExtensions(''), content }); return editor }
afterEach(() => editor?.destroy())
describe('top-level block movement', () => {
  it('moves a whole nested list below a heading and undoes in one step', () => {
    const ed = setup('<p>前言</p><ul><li><p>证据一</p></li><li><p>证据二</p></li></ul><h2>结论</h2><p>结尾</p>')
    const original = ed.getJSON()
    expect(moveBlock(ed, 1, 3)).toBe(true)
    expect(ed.state.doc.child(2).type.name).toBe('bulletList')
    expect(ed.state.doc.child(2).childCount).toBe(2)
    ed.commands.undo()
    expect(ed.getJSON()).toEqual(original)
    ed.commands.redo()
    expect(ed.state.doc.child(2).type.name).toBe('bulletList')
  })
  it('preserves original Markdown node identities when moving upward', () => {
    const md = '#  标题\n\n正文 **重点**\n\n> 引用\n'
    const ed = setup(mdToHtml(md))
    const preservation = new MarkdownPreservation()
    preservation.bind(md, ed.state.doc)
    const node = ed.state.doc.child(2)
    expect(moveBlock(ed, 2, 0)).toBe(true)
    expect(ed.state.doc.child(0)).toBe(node)
    expect(preservation.serialize(ed.state.doc)).toContain('#  标题')
  })
  it('rejects invalid or unchanged destinations and read-only editors', () => {
    const ed = setup('<p>一</p><p>二</p>')
    for (const [from, to] of [[0, 0], [0, 1], [-1, 1], [1, 3], [2, 0]]) expect(moveBlock(ed, from, to)).toBe(false)
    ed.setEditable(false)
    expect(moveBlock(ed, 0, 2)).toBe(false)
    expect(ed.getText()).toBe('一\n\n二')
  })
})
