// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { Editor } from '@tiptap/core'
import { documentExtensions } from '../../src/shared/editor/schema'
import { useBlockMenu } from '../../src/shared/editor/useBlockMenu'
import { htmlToMd } from '../../src/shared/markdown/mdBridge'

let editor: Editor | undefined
afterEach(() => { editor?.destroy(); editor = undefined; vi.restoreAllMocks() })
function setup(content: string) {
  editor = new Editor({ extensions: documentExtensions(''), content })
  vi.spyOn(editor.view, 'coordsAtPos').mockReturnValue({ left: 30, right: 40, top: 50, bottom: 70 })
  editor.commands.setTextSelection(editor.state.doc.content.size - 1)
  const menu = useBlockMenu()
  menu.update(editor)
  return { ed: editor, menu }
}
describe('writing block commands', () => {
  it('filters a slash command, transforms the block and preserves Markdown', () => {
    const { ed, menu } = setup('<p>/h2</p>')
    expect(menu.items.value.map(i => i.id)).toEqual(['heading2'])
    expect(menu.keydown(ed, new KeyboardEvent('keydown', { key: 'Enter' }))).toBe(true)
    ed.commands.insertContent('争议焦点')
    expect(htmlToMd(ed.getHTML())).toBe('## 争议焦点')
  })
  it('leaves IME composition untouched and keeps Escape dismissed on selection updates', () => {
    const { ed, menu } = setup('<p>/待办</p>')
    expect(menu.keydown(ed, new KeyboardEvent('keydown', { key: 'Enter', isComposing: true }))).toBe(false)
    expect(ed.getText()).toBe('/待办')
    menu.keydown(ed, new KeyboardEvent('keydown', { key: 'Escape' }))
    menu.update(ed)
    expect(menu.state.value.open).toBe(false)
    expect(ed.getText()).toBe('/待办')
  })
  it('does not trigger slash commands inside ordinary prose or code', () => {
    const { ed, menu } = setup('<p>事实 /h2</p>')
    expect(menu.state.value.open).toBe(false)
    ed.commands.setContent('<pre><code>/h2</code></pre>')
    ed.commands.setTextSelection(4)
    menu.update(ed)
    expect(menu.state.value.open).toBe(false)
  })
})
