// @vitest-environment jsdom
import { expect, it, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import DocumentMarkdown from '../../src/shared/components/DocumentMarkdown.vue'
const mocks = vi.hoisted(() => ({ read: vi.fn() }))
vi.mock('../../src/core/tauriBridge', () => ({ tauriCallSafe: mocks.read }))
it('keeps external assets inert until visible and releases them on unmount', async () => {
  let callback!: IntersectionObserverCallback
  const observe = vi.fn(), disconnect = vi.fn()
  vi.stubGlobal('IntersectionObserver', class { constructor(cb: IntersectionObserverCallback) { callback = cb } observe = observe; disconnect = disconnect })
  const create = vi.fn(() => 'blob:verified'), revoke = vi.fn()
  vi.stubGlobal('URL', { createObjectURL: create, revokeObjectURL: revoke })
  mocks.read.mockResolvedValue({ ok: true, data: 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGNQKt8NAAITAVXMpdPnAAAAAElFTkSuQmCC' })
  const id = 'a'.repeat(64) + '.png'
  const view = mount(DocumentMarkdown, { props: { markdown: `<img src="assets/${id}" alt="Evidence">`, fileId: 'f', jobId: 'j' } })
  await flushPromises()
  const image = view.find('img')
  expect(image.attributes('src')).toBeUndefined(); expect(mocks.read).not.toHaveBeenCalled()
  callback([{ target: image.element, isIntersecting: true } as unknown as IntersectionObserverEntry], {} as IntersectionObserver)
  await flushPromises()
  expect(mocks.read).toHaveBeenCalledWith('read_document_asset', { fileId: 'f', jobId: 'j', assetId: id })
  expect(image.attributes('src')).toBe('blob:verified')
  view.unmount(); expect(revoke).toHaveBeenCalledWith('blob:verified'); expect(disconnect).toHaveBeenCalled()
  vi.unstubAllGlobals()
})
it('shares visible duplicates, exposes retry, and releases the image after the last copy leaves view', async () => {
  let callback!: IntersectionObserverCallback
  vi.stubGlobal('IntersectionObserver', class {
    constructor(cb: IntersectionObserverCallback) { callback = cb }
    observe() {}
    disconnect() {}
  })
  const revoke = vi.fn()
  vi.stubGlobal('URL', { createObjectURL: () => 'blob:shared', revokeObjectURL: revoke })
  const png = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGNQKt8NAAITAVXMpdPnAAAAAElFTkSuQmCC'
  mocks.read.mockReset().mockResolvedValueOnce({ ok: false, error: 'temporarily missing' })
    .mockResolvedValue({ ok: true, data: png })
  const id = 'b'.repeat(64) + '.png'
  const view = mount(DocumentMarkdown, { props: { markdown: `<img src="assets/${id}"><img src="assets/${id}">`, fileId: 'f', jobId: 'j' } })
  await flushPromises()
  const images = view.findAll('img')
  const entry = (index: number, isIntersecting: boolean) => ({ target: images[index].element, isIntersecting } as IntersectionObserverEntry)
  callback([entry(0, true), entry(1, true)], {} as IntersectionObserver)
  await flushPromises()
  expect(mocks.read).toHaveBeenCalledTimes(1)
  expect(view.find('.document-image-retry').attributes('hidden')).toBeUndefined()
  await view.find('.document-image-retry').trigger('click')
  await flushPromises()
  await view.findAll('.document-image-retry')[1].trigger('click')
  await flushPromises()
  expect(mocks.read).toHaveBeenCalledTimes(2)
  expect(images[0].attributes('src')).toBe('blob:shared')
  expect(images[1].attributes('src')).toBe('blob:shared')
  callback([entry(0, false)], {} as IntersectionObserver)
  expect(revoke).not.toHaveBeenCalled()
  callback([entry(1, false)], {} as IntersectionObserver)
  expect(revoke).toHaveBeenCalledOnce()
  view.unmount()
  vi.unstubAllGlobals()
})
