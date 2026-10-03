import { expect, it, vi } from 'vitest'
import { createDocumentAssetLoader, pngMemoryBytes } from '../../src/shared/markdown/documentAssetLoader'
const id = 'a'.repeat(64) + '.png'
it('validates ids, deduplicates requests, enforces a budget and releases URLs', async () => {
  const read = vi.fn(async () => new Blob(['image'], { type: 'image/png' }))
  const revokeURL = vi.fn()
  const loader = createDocumentAssetLoader({ read, createURL: () => 'blob:image', revokeURL, maxBytes: 5 })
  await expect(loader.load('../private')).rejects.toThrow('Invalid')
  const results = await Promise.all([loader.load(id), loader.load(id)])
  expect(results).toEqual(['blob:image', 'blob:image']); expect(read).toHaveBeenCalledTimes(1)
  await expect(loader.load('b'.repeat(64) + '.png')).rejects.toThrow('budget')
  loader.dispose(); expect(revokeURL).toHaveBeenCalledWith('blob:image')
})
it('does not publish images after a document is closed and rejects queued requests', async () => {
  let finish!: (blob: Blob) => void
  const createURL = vi.fn()
  const loader = createDocumentAssetLoader({ read: () => new Promise(resolve => { finish = resolve }), createURL, revokeURL: vi.fn(), concurrency: 1 })
  const first = expect(loader.load(id)).rejects.toThrow('closed')
  const queued = expect(loader.load('b'.repeat(64) + '.png')).rejects.toThrow('closed')
  await Promise.resolve()
  loader.dispose(); finish(new Blob(['late']))
  await Promise.all([first, queued]); expect(createURL).not.toHaveBeenCalled()
})
it('accounts for decoded pixels and rejects synchronous reader errors without blocking the queue', async () => {
  const bytes = Uint8Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGNQKt8NAAITAVXMpdPnAAAAAElFTkSuQmCC'), char => char.charCodeAt(0))
  expect(pngMemoryBytes(bytes)).toBe(bytes.length + 4)
  new DataView(bytes.buffer).setUint32(16, 100000)
  new DataView(bytes.buffer).setUint32(20, 100000)
  expect(pngMemoryBytes(bytes)).toBeGreaterThan(32 * 1024 * 1024)
  const read = vi.fn().mockImplementationOnce(() => { throw new Error('read failed') })
    .mockResolvedValue(new Blob(['small compressed image']))
  const createURL = vi.fn()
  const loader = createDocumentAssetLoader({ read, createURL, revokeURL: vi.fn(), concurrency: 1, memoryCost: () => pngMemoryBytes(bytes) })
  const first = expect(loader.load(id)).rejects.toThrow('read failed')
  const next = expect(loader.load('b'.repeat(64) + '.png')).rejects.toThrow('budget')
  await Promise.all([first, next])
  expect(createURL).not.toHaveBeenCalled()
  loader.dispose()
})
