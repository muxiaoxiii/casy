import { nextTick, onBeforeUnmount, ref, watch, type Ref } from 'vue'

export interface NotebookDraft {
  id: string
  title: string
  content: string
  category: string
  tags: string
  linkedCaseId: string
  parentId: string
}

export function useNotebookSave(options: {
  draft: Ref<NotebookDraft>
  syncEditor: () => void
  update: (id: string, data: Record<string, unknown>) => Promise<{ ok: boolean; error?: string }>
  onSaved: (id: string, data: Record<string, unknown>) => void
  onError: (message: string) => void
  recovery?: {
    write: (draft: NotebookDraft) => Promise<void>
    clear: () => Promise<void>
  }
}) {
  const dirty = ref(false)
  const saving = ref(false)
  const error = ref('')
  let revision = 0
  let hydrating = false
  let timer: ReturnType<typeof setTimeout> | undefined
  let pending: Promise<boolean> | null = null
  let disposed = false
  let baselineContent = options.draft.value.content
  let recoveryPending: Promise<void> = Promise.resolve()
  let recoveryLatest: NotebookDraft | null | undefined
  let recoveryRunning = false

  function checkpoint(draft: NotebookDraft | null) {
    if (!options.recovery) return
    recoveryLatest = draft ? { ...draft } : null
    if (recoveryRunning) return
    recoveryRunning = true
    recoveryPending = (async () => {
      try {
        while (recoveryLatest !== undefined) {
          const snapshot = recoveryLatest
          recoveryLatest = undefined
          if (snapshot) await options.recovery!.write(snapshot)
          else await options.recovery!.clear()
        }
      } finally { recoveryRunning = false }
    })()
    recoveryPending.catch(cause => options.onError(`恢复草稿保存失败：${String(cause)}`))
  }

  function changed() {
    if (hydrating || disposed || !options.draft.value.id) return
    revision++
    dirty.value = true
    error.value = ''
    checkpoint(options.draft.value)
    clearTimeout(timer)
    timer = setTimeout(flush, 900)
  }
  watch(options.draft, changed, { deep: true, flush: 'sync' })

  function hydrate(value: NotebookDraft) {
    hydrating = true
    options.draft.value = value
    baselineContent = value.content
    revision++
    dirty.value = false
    error.value = ''
    hydrating = false
  }

  function flush(): Promise<boolean> {
    clearTimeout(timer)
    if (pending) return pending
    // All callers await the entire drain, including edits made during a write.
    pending = (async () => {
      try {
        await nextTick()
        while (!disposed) {
          options.syncEditor()
          await nextTick()
          if (!dirty.value || !options.draft.value.id) return true
          const draft = options.draft.value
          const id = draft.id
          const savingRevision = revision
          const data = {
            expectedContent: baselineContent,
            title: draft.title.trim() || '无标题笔记', content: draft.content,
            category: draft.category, tags: draft.tags || null,
            linkedCaseId: draft.linkedCaseId || null, parentId: draft.parentId || null,
            status: 'current',
          }
          saving.value = true
          const result = await options.update(id, data)
          if (!result.ok) throw new Error(result.error || '保存失败，修改仍保留在编辑器中')
          baselineContent = data.content
          error.value = ''
          if (options.draft.value.id === id && revision === savingRevision) {
            dirty.value = false
            checkpoint(null)
            await recoveryPending
          }
          options.onSaved(id, data)
        }
        return false
      } catch (cause) {
        error.value = cause instanceof Error ? cause.message : String(cause)
        options.onError(error.value)
        return false
      } finally {
        saving.value = false
        pending = null
        clearTimeout(timer)
      }
    })()
    return pending
  }

  onBeforeUnmount(() => { disposed = true; clearTimeout(timer) })
  return { dirty, saving, error, hydrate, changed, flush }
}
