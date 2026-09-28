// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest'
import { Editor } from '@tiptap/core'
import { documentExtensions } from '../../src/shared/editor/schema'
import { editBlock } from '../../src/shared/editor/blockActions'
let editor: Editor
function setup() { editor = new Editor({ extensions: documentExtensions(''), content: '<p>前言</p><ul><li><p>证据<strong>原文</strong></p></li></ul><p>结尾</p>' }); return editor }
afterEach(() => editor?.destroy())
describe('context block operations', () => {
  it('duplicates a whole nested block and preserves marks, with independent undo', () => {
    const ed = setup(); const original = ed.getJSON()
    expect(editBlock(ed, 1, 'duplicate')).toBe(true)
    expect(ed.state.doc.child(2).toJSON()).toEqual(ed.state.doc.child(1).toJSON())
    expect(ed.state.doc.childCount).toBe(4)
    ed.commands.undo(); expect(ed.getJSON()).toEqual(original)
  })
  it('restores deleted content and formatting on undo', () => {
    const ed = setup(); const original = ed.getJSON()
    editBlock(ed, 1, 'delete'); expect(ed.getText()).not.toContain('证据')
    ed.commands.undo(); expect(ed.getJSON()).toEqual(original)
  })
  it('inserts adjacent paragraphs and rejects read-only changes', () => {
    const ed = setup()
    editBlock(ed, 1, 'before'); expect(ed.state.doc.child(1).type.name).toBe('paragraph')
    expect(ed.state.doc.child(2).type.name).toBe('bulletList')
    ed.setEditable(false); expect(editBlock(ed, 2, 'delete')).toBe(false)
    expect(editBlock(ed, -1, 'duplicate')).toBe(false)
  })
})
