import { ref } from 'vue'
import type { ContextAction } from '../components/ContextMenu.vue'
export function useContextActions() {
  const contextMenu = ref({ open: false, x: 0, y: 0, label: '', actions: [] as ContextAction[] })
  function showContextMenu(event: MouseEvent, label: string, actions: ContextAction[]) {
    event.preventDefault(); event.stopPropagation()
    const bounds = (event.currentTarget as HTMLElement)?.getBoundingClientRect()
    contextMenu.value = { open: true, x: event.clientX || bounds?.left || 8, y: event.clientY || bounds?.top || 8, label, actions }
  }
  return { contextMenu, showContextMenu }
}
