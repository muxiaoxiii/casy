// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { nextTick, markRaw } from 'vue'
import { Editor } from '@tiptap/core'
import StarterKit from '@tiptap/starter-kit'
import SlashCommandMenu from '../../src/modules/docs/components/SlashCommandMenu.vue'

let wrapper: ReturnType<typeof mount> | undefined
let editor: Editor | undefined
afterEach(() => { wrapper?.unmount(); editor?.destroy(); document.body.innerHTML = '' })
describe('文书插入菜单', () => {
  async function open() {
    editor = markRaw(new Editor({ extensions: [StarterKit], content: '<p>/</p>' }))
    wrapper = mount(SlashCommandMenu, { attachTo: document.body, global: { stubs: { 'el-icon': true } }, props: {
      editor, visible: true, position: { x: 200, y: 300 }, triggerRange: { from: 1, to: 2 },
    } })
    await nextTick()
  }
  it('搜索结果以回车执行，删除触发字符且不插入额外换行', async () => {
    await open()
    const input = document.querySelector('input')!
    input.value = 'h2'; input.dispatchEvent(new Event('input')); await nextTick()
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }))
    expect(editor!.getJSON().content?.[0].type).toBe('heading')
    expect(editor!.getText().trim()).toBe('')
    expect(editor!.getJSON().content?.filter(node => node.type === 'heading')).toHaveLength(1)
    expect(wrapper!.emitted('close')).toHaveLength(1)
  })
  it('无匹配时方向键不破坏后续选择，卸载移除全局键盘监听', async () => {
    await open()
    const input = document.querySelector('input')!
    input.value = 'does-not-exist'; input.dispatchEvent(new Event('input')); await nextTick()
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }))
    input.value = '表格'; input.dispatchEvent(new Event('input')); await nextTick()
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))
    expect(wrapper!.emitted('open-table')).toHaveLength(1)
    const listener = vi.fn()
    window.addEventListener('keydown', listener)
    wrapper!.unmount(); wrapper = undefined
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter' }))
    expect(listener).toHaveBeenCalledOnce()
    window.removeEventListener('keydown', listener)
  })
})
