import { reactive } from 'vue'
export interface SessionOperation {
  id: string
  title: string
  status: 'running' | 'completed' | 'failed' | 'cancelled'
  startedAt: string
  updatedAt: string
  error: string
  outputPath?: string
  retry?: () => Promise<unknown>
}
// Metadata is memory-only. Credentials and request payloads are never recorded.
export const sessionOperations = reactive<SessionOperation[]>([])
export async function runSessionOperation<T>(title: string, run: () => Promise<T>, retry?: () => Promise<unknown>): Promise<T> {
  const job: SessionOperation = reactive({ id: crypto.randomUUID(), title, status: 'running', startedAt: new Date().toISOString(), updatedAt: new Date().toISOString(), error: '' })
  sessionOperations.unshift(job)
  while (sessionOperations.length > 50) {
    let index = sessionOperations.length - 1
    while (index >= 0 && sessionOperations[index].status === 'running') index--
    if (index < 0) break
    sessionOperations.splice(index, 1)
  }
  try {
    const result = await run()
    const response = result as { ok?: boolean; error?: string } | null
    if (response && typeof response === 'object' && response.ok === false) {
      job.status = 'failed'; job.error = response.error || '操作失败'
      job.retry = retry
    } else {
      job.status = result === null ? 'cancelled' : 'completed'
      if (typeof result === 'string' && /^(?:\/|[A-Za-z]:[\\/])/.test(result)) job.outputPath = result
    }
    return result
  } catch (cause) {
    job.status = 'failed'; job.error = String(cause); job.retry = retry
    throw cause
  } finally { job.updatedAt = new Date().toISOString() }
}
export async function retrySessionOperation(job: SessionOperation) {
  const retry = job.retry
  if (!retry) return
  job.retry = undefined
  try { await retry() } catch { /* The new operation records its own error. */ }
}
