// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest'
import { Editor } from '@tiptap/core'
import { CellSelection } from '@tiptap/pm/tables'
import { documentExtensions } from '../../src/shared/editor/schema'
import { tableActions } from '../../src/shared/editor/tableActions'
let editor: Editor
function setup() {
  editor = new Editor({ extensions: documentExtensions(''), content: '<table><tbody><tr><td><p>甲</p></td><td><p>乙</p></td></tr><tr><td><p>丙</p></td><td><p>丁</p></td></tr></tbody></table><p>表后文字</p>' })
  const cells: number[] = []
  editor.state.doc.descendants((node, pos) => { if (node.type.name === 'tableCell') cells.push(pos) })
  editor.commands.setTextSelection(cells[0] + 2)
  return cells
}
afterEach(() => editor?.destroy())
describe('table contextual actions', () => {
  it('restores the original cell before inserting a row and supports one-step undo', () => {
    const cells = setup(); const original = editor.getJSON()
    const action = tableActions(editor).find(a => a.label === '在上方插入行')!
    editor.commands.setTextSelection(cells[3] + 2)
    action.run!()
    expect(editor.state.doc.child(0).childCount).toBe(3)
    expect(editor.state.doc.child(0).child(0).textContent).toBe('')
    editor.commands.undo(); expect(editor.getJSON()).toEqual(original)
  })
  it('merges a multi-cell selection while disabling unavailable splitting', () => {
    const cells = setup()
    editor.view.dispatch(editor.state.tr.setSelection(CellSelection.create(editor.state.doc, cells[0], cells[1])))
    const actions = tableActions(editor)
    expect(actions.find(a => a.label === '拆分单元格')?.disabled).toBe(true)
    const merge = actions.find(a => a.label === '合并选中单元格')!
    expect(merge.disabled).toBe(false); merge.run!()
    expect(editor.state.doc.child(0).child(0).childCount).toBe(1)
    expect(editor.state.doc.child(0).child(0).textContent).toBe('甲乙')
  })
  it('never applies an old menu after document content changes', () => {
    setup(); const action = tableActions(editor).find(a => a.label === '删除整张表格')!
    editor.commands.insertContent('修改')
    action.run!()
    expect(editor.state.doc.child(0).type.name).toBe('table')
    editor.setEditable(false)
    expect(tableActions(editor).every(action => action.disabled)).toBe(true)
  })
})
