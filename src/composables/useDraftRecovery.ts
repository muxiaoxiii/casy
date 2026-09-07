import { ElMessage } from 'element-plus'
import { tauriCallSafe } from '../core/tauriBridge'

type DraftSnapshot = { id: string; title: string; content: string; kind?: string }
export function useDraftRecovery() {
  const sessionId = crypto.randomUUID()
  const native = () => Boolean((window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__)
  let latest: DraftSnapshot | null | undefined
  let pending: Promise<void> | null = null
  function write(draft: DraftSnapshot | null): Promise<void> {
    if (!native()) return Promise.resolve()
    latest = draft ? { ...draft, kind: 'document' } : null
    if (pending) return pending
    pending = (async () => {
      try {
        while (latest !== undefined) {
          const snapshot = latest
          latest = undefined
          const result = await tauriCallSafe('save_editor_recovery', { sessionId, draft: snapshot })
          if (!result.ok) throw new Error(result.error || '恢复草稿保存失败')
        }
      } finally { pending = null }
    })()
    pending.catch(error => ElMessage.error(String(error)))
    return pending
  }
  async function recover() {
    if (!native()) return
    const result = await tauriCallSafe('recover_editor_drafts', {})
    if (!result.ok) ElMessage.error(result.error || '读取恢复草稿失败')
    else if (result.data) ElMessage.info(`已找回 ${result.data} 份内容，保留为恢复草稿`)
  }
  return { checkpoint: (draft: DraftSnapshot) => { void write(draft) }, clear: () => write(null), recover }
}
