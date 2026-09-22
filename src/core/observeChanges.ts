import type { CasyContext } from './plugin/types'

/** Coalesce bursts and serialize refreshes. A change during a read queues a new read. */
export function observeChanges(
  ctx: Pick<CasyContext, 'on'>,
  domains: string[],
  refresh: () => Promise<unknown>,
  onError: (error: unknown) => void = console.error,
) {
  let disposed = false
  let running = false
  let dirty = false
  let timer: ReturnType<typeof setTimeout> | undefined
  async function drain() {
    timer = undefined
    if (disposed || running) return
    running = true
    try {
      while (dirty && !disposed) {
        dirty = false
        try { await refresh() } catch (error) { onError(error) }
      }
    } finally { running = false }
  }
  const schedule = () => {
    if (disposed) return
    dirty = true
    if (!running && !timer) timer = setTimeout(() => void drain(), 80)
  }
  const off = domains.flatMap(domain =>
    ['changed', 'created', 'updated', 'deleted', 'completed', 'confirmed', 'imported'].map(action =>
      ctx.on(`${domain}:${action}`, schedule),
    ),
  )
  return () => {
    disposed = true
    if (timer) clearTimeout(timer)
    off.forEach(dispose => dispose())
  }
}
