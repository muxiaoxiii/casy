import { onMounted, onBeforeUnmount } from 'vue'
import { onBeforeRouteLeave } from 'vue-router'
import { ElMessage } from 'element-plus'

export function useSaveBeforeLeave(isDirty: () => boolean, save: () => Promise<boolean>) {
  let disposed = false
  let closing = false
  let unlisten: (() => void) | undefined
  const beforeUnload = (event: BeforeUnloadEvent) => {
    if (isDirty()) { event.preventDefault(); event.returnValue = '' }
  }
  onBeforeRouteLeave(async () => !isDirty() || await save())
  onMounted(async () => {
    window.addEventListener('beforeunload', beforeUnload)
    if (!(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__) return
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      const current = getCurrentWindow()
      const stop = await current.onCloseRequested(async event => {
        if (!isDirty()) return
        event.preventDefault()
        if (closing) return
        closing = true
        try { if (await save()) await current.close() }
        finally { closing = false }
      })
      if (disposed) stop()
      else unlisten = stop
    } catch { ElMessage.error('无法启用窗口关闭保护，请保存后再退出') }
  })
  onBeforeUnmount(() => { disposed = true; unlisten?.(); window.removeEventListener('beforeunload', beforeUnload) })
}
