// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest'
import { Editor } from '@tiptap/core'
import StarterKit from '@tiptap/starter-kit'
import { setQuoteDepth } from '../../src/shared/markdown/quoteSources'
import { mdToHtml, htmlToMd } from '../../src/shared/markdown/mdBridge'
let editor: Editor | undefined
afterEach(() => editor?.destroy())
describe('quote source identity', () => {
  it('preserves all four source depths through rich/source conversion and undo', () => {
    editor = new Editor({ extensions: [StarterKit], content: mdToHtml('正文') })
    for (const depth of [1, 2, 3, 4, 1, 0]) {
      expect(setQuoteDepth(editor, depth)).toBe(true)
      const html = editor.getHTML()
      expect((html.match(/<blockquote>/g) || []).length).toBe(depth)
      expect((mdToHtml(htmlToMd(html)).match(/<blockquote>/g) || []).length).toBe(depth)
      expect(editor.getText()).toContain('正文')
    }
    expect(editor.commands.undo()).toBe(true)
  })
  it('does not silently flatten quotes with different sources in the same outer block', () => {
    editor = new Editor({ extensions: [StarterKit], content: mdToHtml('> 原告陈述\n>\n>> 被告回复') })
    editor.commands.setTextSelection(3)
    const before = editor.getHTML()
    expect(setQuoteDepth(editor, 4)).toBe(false)
    expect(editor.getHTML()).toBe(before)
  })
})
