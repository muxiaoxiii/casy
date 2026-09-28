// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest'
import { Editor } from '@tiptap/core'
import StarterKit from '@tiptap/starter-kit'
import { Selection } from '@tiptap/pm/state'
import { BlockSelectionExtension, selectBlocks, toggleBlock, selectedBlockRange } from '../../src/shared/editor/blockSelection'
import { editBlocks } from '../../src/shared/editor/blockActions'
import { moveBlocks } from '../../src/shared/editor/blockDrag'
let ed: Editor
const setup = () => ed = new Editor({ extensions: [StarterKit, BlockSelectionExtension], content: '<p>甲</p><p>保留乙</p><p><strong>丙</strong></p><p>丁</p>' })
const choose = () => { selectBlocks(ed, 0); toggleBlock(ed, 2) }
afterEach(() => ed?.destroy())
describe('discrete block selection', () => {
  it('copies selected blocks only and survives JSON/bookmark round trips', () => {
    setup(); choose()
    const selection = ed.state.selection
    expect(selection.content().content.textBetween(0, selection.content().size, '|')).toBe('甲|丙')
    expect(Selection.fromJSON(ed.state.doc, selection.toJSON()).eq(selection)).toBe(true)
    expect(selection.getBookmark().resolve(ed.state.doc).eq(selection)).toBe(true)
  })
  it('deletes selected blocks while preserving the gap and restores all with undo', () => {
    setup(); const original = ed.getJSON(); choose()
    editBlocks(ed, 0, 2, 'delete')
    expect(ed.getText()).toBe('保留乙\n\n丁')
    ed.commands.undo(); expect(ed.getJSON()).toEqual(original)
  })
  it('duplicates only selected nodes, retaining rich formatting', () => {
    setup(); choose(); editBlocks(ed, 0, 2, 'duplicate')
    expect(ed.state.doc.childCount).toBe(6)
    expect(ed.state.doc.child(3).textContent).toBe('甲')
    expect(ed.state.doc.child(4).firstChild?.marks[0].type.name).toBe('bold')
    expect(selectedBlockRange(ed)?.indices).toEqual([3,4])
  })
  it('moves the group into an unselected gap and undo restores the original document', () => {
    setup(); const original = ed.getJSON(); choose()
    expect(moveBlocks(ed, 0, 2, 2)).toBe(true)
    expect(ed.getText()).toBe('保留乙\n\n甲\n\n丙\n\n丁')
    ed.commands.undo(); expect(ed.getJSON()).toEqual(original)
  })
  it('maps surviving nodes after removal of one selected block', () => {
    setup(); choose(); ed.view.dispatch(ed.state.tr.delete(0, ed.state.doc.child(0).nodeSize))
    expect(selectedBlockRange(ed)?.indices).toEqual([1])
    expect(ed.state.selection.content().content.firstChild?.textContent).toBe('丙')
  })
})
