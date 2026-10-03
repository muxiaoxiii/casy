export interface AssetLoaderOptions {
  read(id: string): Promise<Blob>
  createURL(blob: Blob): string
  revokeURL(url: string): void
  memoryCost?(blob: Blob): number
  concurrency?: number
  maxBytes?: number
}
export function createDocumentAssetLoader(options: AssetLoaderOptions) {
  const cache = new Map<string, { url: string; size: number }>()
  const pending = new Map<string, Promise<string>>()
  const queue: Array<() => void> = []
  let active = 0, bytes = 0, disposed = false
  const maxBytes = options.maxBytes ?? 32 * 1024 * 1024
  function release(id: string) {
    const entry = cache.get(id)
    if (entry) { options.revokeURL(entry.url); bytes -= entry.size; cache.delete(id) }
  }
  function pump() {
    while (!disposed && active < (options.concurrency ?? 2) && queue.length) {
      active++; queue.shift()!()
    }
  }
  function load(id: string): Promise<string> {
    if (disposed) return Promise.reject(new Error('Document closed'))
    if (!/^[a-f0-9]{64}\.png$/.test(id)) return Promise.reject(new Error('Invalid document asset'))
    const hit = cache.get(id)
    if (hit) return Promise.resolve(hit.url)
    const inflight = pending.get(id)
    if (inflight) return inflight
    const promise = new Promise<string>((resolve, reject) => {
      queue.push(() => {
        if (disposed) { reject(new Error('Document closed')); return }
        Promise.resolve().then(() => {
          if (disposed) throw new Error('Document closed')
          return options.read(id)
        }).then(blob => {
          if (disposed) throw new Error('Document closed')
          const size = options.memoryCost?.(blob) ?? blob.size
          // Never evict images currently on screen. Caller releases offscreen images.
          if (!Number.isSafeInteger(size) || size < 0 || bytes + size > maxBytes) throw new Error('Image memory budget reached')
          const url = options.createURL(blob)
          cache.set(id, { url, size }); bytes += size
          resolve(url)
        }).catch(reject).finally(() => { active--; pending.delete(id); pump() })
      })
    })
    pending.set(id, promise); pump(); return promise
  }
  function dispose() {
    disposed = true
    for (const id of cache.keys()) release(id)
    // Drain callbacks so queued promises reject rather than remain pending forever.
    while (queue.length) queue.shift()!()
  }
  return { load, release, dispose }
}

export function pngMemoryBytes(bytes: Uint8Array): number {
  const signature = [137, 80, 78, 71, 13, 10, 26, 10]
  if (bytes.length < 33 || signature.some((value, index) => bytes[index] !== value)
    || String.fromCharCode(...bytes.subarray(12, 16)) !== 'IHDR') throw new Error('Invalid PNG')
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  const width = view.getUint32(16), height = view.getUint32(20)
  const decoded = width * height * 4
  if (!width || !height || !Number.isSafeInteger(decoded)) throw new Error('Invalid PNG dimensions')
  return bytes.byteLength + decoded
}
