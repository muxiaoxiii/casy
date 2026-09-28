import { onBeforeUnmount, onMounted, watch, type Ref } from 'vue'
// Session-only UI state. Never store document content or credentials here.
const memory = new Map<string, Record<string, unknown>>()
const positions = new Map<string, number[]>()
export function useViewMemory(key: string, fields: Record<string, Ref<any>>, selectors = ['#main-content']) {
  const saved = memory.get(key)
  if (saved) for (const [name, field] of Object.entries(fields)) if (name in saved) field.value = structuredClone(saved[name])
  const stop = watch(Object.values(fields), () => {
    memory.set(key, Object.fromEntries(Object.entries(fields).map(([name, field]) => [name, JSON.parse(JSON.stringify(field.value))])))
  }, { deep: true, flush: 'sync' })
  let observer: MutationObserver | undefined
  let timeout: ReturnType<typeof setTimeout> | undefined
  let restoring = true
  const elements = () => selectors.map(selector => document.querySelector<HTMLElement>(selector))
  const cancelRestore = () => { restoring = false; observer?.disconnect(); clearTimeout(timeout) }
  onMounted(() => {
    const targets = positions.get(key)
    const restore = () => {
      if (!restoring || !targets) return
      const nodes = elements()
      nodes.forEach((node, index) => { if (node) node.scrollTop = targets[index] || 0 })
      if (nodes.every((node, index) => node && node.scrollHeight - node.clientHeight >= (targets[index] || 0))) cancelRestore()
    }
    if (targets) {
      observer = new MutationObserver(restore)
      observer.observe(document.getElementById('main-content') || document.body, { childList: true, subtree: true })
      restore(); timeout = setTimeout(cancelRestore, 10000)
    }
    window.addEventListener('wheel', cancelRestore, { passive: true })
    window.addEventListener('pointerdown', cancelRestore)
    window.addEventListener('keydown', cancelRestore)
  })
  onBeforeUnmount(() => {
    positions.set(key, elements().map(el => el?.scrollTop || 0))
    stop(); cancelRestore()
    window.removeEventListener('wheel', cancelRestore)
    window.removeEventListener('pointerdown', cancelRestore)
    window.removeEventListener('keydown', cancelRestore)
  })
}
