// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { Editor } from '@tiptap/core'
import { documentExtensions } from '../../src/shared/editor/schema'
import { edgeScrollDelta, useBlockDrag } from '../../src/shared/editor/blockDrag'
afterEach(() => vi.restoreAllMocks())
describe('block edge scrolling', () => {
  it('scrolls only near the visible edge, with a capped speed', () => {
    expect(edgeScrollDelta(105, 100, 500)).toBeLessThan(0)
    expect(edgeScrollDelta(495, 100, 500)).toBeGreaterThan(0)
    expect(edgeScrollDelta(300, 100, 500)).toBe(0)
    expect(edgeScrollDelta(550, 100, 500)).toBe(0)
    expect(Math.abs(edgeScrollDelta(90, 100, 500))).toBe(16)
  })
  it('keeps a drag active during scrolling and stops animation on cancellation', () => {
    const frames = vi.spyOn(window, 'requestAnimationFrame').mockReturnValue(7)
    const cancelled = vi.spyOn(window, 'cancelAnimationFrame')
    const editor = new Editor({ extensions: documentExtensions(''), content: '<p>一</p><p>二</p>' })
    let drag!: ReturnType<typeof useBlockDrag>
    const view = mount({ setup() { drag = useBlockDrag(() => editor); return () => null } })
    drag.state.index = 0
    drag.start({ button: 0, pointerId: 1, clientX: 50, clientY: 50, preventDefault() {} } as PointerEvent)
    expect(frames).toHaveBeenCalled()
    window.dispatchEvent(new Event('scroll'))
    expect(drag.state.active).toBe(true)
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
    expect(drag.state.active).toBe(false)
    expect(cancelled).toHaveBeenCalledWith(7)
    view.unmount(); editor.destroy()
  })
  it('advances the containing scroller when a dragged block reaches its edge', () => {
    let frame!: FrameRequestCallback
    vi.spyOn(window, 'requestAnimationFrame').mockImplementation(callback => { frame = callback; return 1 })
    vi.spyOn(window, 'cancelAnimationFrame').mockImplementation(() => {})
    const scroller = document.createElement('div')
    scroller.style.overflowY = 'auto'
    document.body.append(scroller)
    Object.defineProperties(scroller, { scrollHeight: { value: 1000 }, clientHeight: { value: 200 } })
    vi.spyOn(scroller, 'getBoundingClientRect').mockReturnValue({ top: 100, bottom: 300, left: 0, right: 300 } as DOMRect)
    const host = document.createElement('div'); scroller.append(host)
    const editor = new Editor({ element: host, extensions: documentExtensions(''), content: '<p>一</p><p>二</p>' })
    vi.spyOn(editor.view.dom, 'getBoundingClientRect').mockReturnValue({ top: 100, bottom: 1100, left: 0, right: 300 } as DOMRect)
    let drag!: ReturnType<typeof useBlockDrag>
    const view = mount({ setup() { drag = useBlockDrag(() => editor); return () => null } })
    drag.state.index = 0
    drag.start({ button: 0, pointerId: 1, clientX: 50, clientY: 150, preventDefault() {} } as PointerEvent)
    const move = new MouseEvent('pointermove', { clientX: 50, clientY: 295 })
    Object.defineProperty(move, 'pointerId', { value: 1 })
    window.dispatchEvent(move)
    frame(16)
    expect(scroller.scrollTop).toBeGreaterThan(0)
    expect(drag.state.active).toBe(true)
    view.unmount(); editor.destroy(); scroller.remove()
  })

})
