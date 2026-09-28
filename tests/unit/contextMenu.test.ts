// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import ContextMenu from '../../src/shared/components/ContextMenu.vue'
let view: ReturnType<typeof mount>
afterEach(() => { view?.unmount(); document.body.innerHTML = '' })
describe('shared object menus', () => {
  it('skips disabled actions with keyboard navigation and supports Escape', async () => {
    view = mount(ContextMenu, { attachTo: document.body, props: { open: true, x: 12, y: 20, actions: [{ label: '编辑', run: vi.fn() }, { label: '禁止', disabled: true, run: vi.fn() }, { label: '打开', run: vi.fn() }] } })
    await flushPromises()
    expect(document.activeElement?.textContent).toBe('编辑')
    document.activeElement?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }))
    expect(document.activeElement?.textContent).toBe('打开')
    document.activeElement?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    expect(view.emitted('close')).toHaveLength(1)
  })
  it('opens submenus without executing or closing, and executes the chosen action once', async () => {
    const run = vi.fn()
    view = mount(ContextMenu, { attachTo: document.body, props: { open: true, x: 12, y: 20, actions: [{ label: '转换', children: [{ label: '正文', run }] }] } })
    await flushPromises()
    ;(document.querySelector('[role="menuitem"]') as HTMLButtonElement).click()
    await flushPromises()
    expect(run).not.toHaveBeenCalled(); expect(view.emitted('close')).toBeUndefined()
    ;(Array.from(document.querySelectorAll('button')).find(b => b.textContent === '正文') as HTMLButtonElement).click()
    expect(run).toHaveBeenCalledTimes(1); expect(view.emitted('close')).toHaveLength(1)
  })
  it('opens a submenu with Right and returns focus to its trigger with Left', async () => {
    view = mount(ContextMenu, { attachTo: document.body, props: { open: true, x: 12, y: 20, actions: [{ label: '复制', run: vi.fn() }, { label: '转换', children: [{ label: '正文', run: vi.fn() }] }] } })
    await flushPromises()
    document.activeElement?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }))
    document.activeElement?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }))
    await flushPromises()
    expect(document.activeElement?.textContent).toBe('正文')
    document.activeElement?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }))
    await flushPromises()
    expect((document.activeElement as HTMLElement)?.dataset.action).toBe('转换')
  })
  it('remeasures the submenu to keep taller content inside the viewport', async () => {
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function () {
      return { width: 210, height: this.textContent?.includes('返回') ? 400 : 100 } as DOMRect
    })
    view = mount(ContextMenu, { attachTo: document.body, props: { open: true, x: 12, y: 700, actions: [{ label: '转换', children: [{ label: '正文', run: vi.fn() }] }] } })
    await flushPromises()
    ;(document.querySelector('[role="menuitem"]') as HTMLButtonElement).click()
    await flushPromises()
    expect(parseInt((document.querySelector('[role="menu"]') as HTMLElement).style.top)).toBeLessThanOrEqual(window.innerHeight - 408)
    vi.restoreAllMocks()
  })

})
