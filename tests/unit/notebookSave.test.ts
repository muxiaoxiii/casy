// @vitest-environment jsdom
import { mount } from '@vue/test-utils'
import { defineComponent, ref, nextTick } from 'vue'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { useNotebookSave, type NotebookDraft } from '../../src/modules/knowledge/composables/useNotebookSave'

const wrappers: ReturnType<typeof mount>[] = []
afterEach(() => { wrappers.splice(0).forEach(wrapper => wrapper.unmount()) })
const draftData = (): NotebookDraft => ({ id: 'a', title: '研究', content: '原文', category: 'reference', tags: '', linkedCaseId: '', parentId: '' })
function setup(update: ReturnType<typeof vi.fn>) {
  const draft = ref(draftData()), onError = vi.fn(), onSaved = vi.fn()
  let save!: ReturnType<typeof useNotebookSave>
  wrappers.push(mount(defineComponent({ setup() {
    save = useNotebookSave({ draft, update, onError, onSaved, syncEditor: () => {} })
    return () => null
  } })))
  return { save, draft, onError, onSaved }
}
function deferred() {
  let resolve!: (value: { ok: boolean }) => void
  const promise = new Promise<{ ok: boolean }>(r => { resolve = r })
  return { resolve, promise }
}
describe('notebook save drain', () => {
  it('waits for edits made during a pending save before allowing a switch', async () => {
    const first = deferred(), second = deferred()
    const update = vi.fn().mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise)
    const { save, draft } = setup(update)
    draft.value.content = '首次修改'
    const flushing = save.flush()
    await nextTick(); await nextTick()
    draft.value.content = '保存期间继续输入'
    const switching = save.flush()
    expect(switching).toBe(flushing)
    first.resolve({ ok: true })
    await nextTick(); await nextTick(); await nextTick()
    expect(update).toHaveBeenCalledTimes(2)
    expect(save.dirty.value).toBe(true)
    expect(update.mock.calls[1][1].content).toBe('保存期间继续输入')
    second.resolve({ ok: true })
    expect(await switching).toBe(true)
    expect(save.dirty.value).toBe(false)
  })
  it('keeps failed changes retryable and clears the in-flight state after exceptions', async () => {
    const update = vi.fn().mockRejectedValueOnce(new Error('database busy')).mockResolvedValue({ ok: true })
    const { save, draft, onError } = setup(update)
    draft.value.content = '不能丢失的修改'
    expect(await save.flush()).toBe(false)
    expect(save.saving.value).toBe(false)
    expect(save.dirty.value).toBe(true)
    expect(onError).toHaveBeenCalledWith('database busy')
    expect(await save.flush()).toBe(true)
    expect(save.dirty.value).toBe(false)
    expect(save.error.value).toBe('')
  })
  it('does not save hydration or leak a previous note timer into the next note', async () => {
    const update = vi.fn().mockResolvedValue({ ok: true })
    const { save, draft } = setup(update)
    save.hydrate({ ...draftData(), id: 'b', content: '下一篇原文' })
    expect(await save.flush()).toBe(true)
    expect(update).not.toHaveBeenCalled()
    draft.value.content += ' 修改'
    expect(await save.flush()).toBe(true)
    expect(update.mock.calls[0][0]).toBe('b')
  })
})
