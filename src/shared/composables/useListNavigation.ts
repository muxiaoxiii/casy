import { onMounted, onUnmounted, ref, watch, type ComputedRef, type Ref } from 'vue'

/**
 * 列表键盘流（bunny 10 第 4 步，UX 审计 U-4）：
 * j/↓ 下一项、k/↑ 上一项、Enter 打开、Esc 清除光标。
 * 输入框/textarea/contentEditable 聚焦或带修饰键时让位原生行为。
 * 容器加 tabindex="0" 与 @keydown="onKeydown"；行元素可绑 :class="{ 'nav-cursor': id === cursorId }"。
 */
export function useListNavigation<T extends { id: string }>(
  items: ComputedRef<T[]> | Ref<T[]>,
  handlers: {
    onOpen: (item: T) => void
    onSelect?: (item: T | null) => void
    container?: Ref<HTMLElement | null>
  },
) {
  const cursorId = ref<string | null>(null)

  const index = () => items.value.findIndex((it) => it.id === cursorId.value)

  function move(delta: number) {
    const list = items.value
    if (!list.length) return
    const current = index()
    const next = current < 0 ? (delta > 0 ? 0 : list.length - 1) : Math.min(Math.max(current + delta, 0), list.length - 1)
    cursorId.value = list[next].id
    handlers.onSelect?.(list[next])
    scrollIntoView(list[next].id)
  }

  function scrollIntoView(id: string) {
    const root = handlers.container?.value
    const el = root?.querySelector(`[data-nav-id="${id}"]`) ?? document.querySelector(`[data-nav-id="${id}"]`)
    el?.scrollIntoView({ block: 'nearest' })
  }

  function clear() {
    cursorId.value = null
    handlers.onSelect?.(null)
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.metaKey || e.ctrlKey || e.altKey || e.shiftKey) return
    const target = e.target as HTMLElement | null
    if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable)) return
    if (e.key === 'j' || e.key === 'ArrowDown') { e.preventDefault(); move(1) }
    else if (e.key === 'k' || e.key === 'ArrowUp') { e.preventDefault(); move(-1) }
    else if (e.key === 'Enter') {
      const item = items.value.find((it) => it.id === cursorId.value)
      if (item) { e.preventDefault(); handlers.onOpen(item) }
    } else if (e.key === 'Escape') { clear() }
  }

  // 列表变化后光标失效则清除（筛选/刷新场景）
  watch(items, (list) => {
    if (cursorId.value && !list.some((it) => it.id === cursorId.value)) clear()
  })

  onMounted(() => document.addEventListener('keydown', onKeydown))
  onUnmounted(() => document.removeEventListener('keydown', onKeydown))

  return { cursorId, onKeydown, clear }
}
