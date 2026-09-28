import { onBeforeUnmount, onMounted } from 'vue'
import { onBeforeRouteLeave, useRouter } from 'vue-router'
import { ElMessageBox } from 'element-plus'

/** Shared discard guard. It never saves credentials or starts integrations implicitly. */
export function useUnsavedForm(label: string, isDirty: () => boolean, isBusy: () => boolean = () => false, onDiscard?: () => void) {
  let prompt: Promise<boolean> | null = null
  let disposed = false
  const router = useRouter()
  let discardAfterNavigation = false
  async function canLeave(discard = true) {
    if (isBusy()) return false
    if (!isDirty()) return true
    if (!prompt) {
      prompt = ElMessageBox.confirm(`${label}有未保存的修改。离开后将丢弃这些内容。`, '保留修改？', {
        confirmButtonText: '放弃修改', cancelButtonText: '继续编辑', type: 'warning', closeOnClickModal: false,
      }).then(() => { if (disposed || isBusy()) return false; if (discard) onDiscard?.(); return true }, () => false).finally(() => { prompt = null })
    }
    return prompt
  }
  const beforeUnload = (event: BeforeUnloadEvent) => {
    if (isDirty() || isBusy()) { event.preventDefault(); event.returnValue = '' }
  }
  onBeforeRouteLeave(async () => {
    const dirty = isDirty()
    const allowed = await canLeave(false)
    discardAfterNavigation = allowed && dirty
    return allowed
  })
  const stopNavigation = router.afterEach((_to, _from, failure) => {
    if (discardAfterNavigation && !failure) onDiscard?.()
    discardAfterNavigation = false
  })
  onMounted(() => window.addEventListener('beforeunload', beforeUnload))
  onBeforeUnmount(() => { disposed = true; stopNavigation(); window.removeEventListener('beforeunload', beforeUnload) })
  return { canLeave }
}
