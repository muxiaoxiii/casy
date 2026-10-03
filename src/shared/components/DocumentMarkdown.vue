<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { mdToSafeHtml } from '../markdown/mdBridge'
import { createDocumentAssetLoader, pngMemoryBytes } from '../markdown/documentAssetLoader'
import { tauriCallSafe } from '../../core/tauriBridge'
const props = defineProps<{ markdown: string; fileId: string; jobId: string }>()
const root = ref<HTMLElement>()
const html = computed(() => {
  const safe = mdToSafeHtml(props.markdown)
  // Prevent relative image requests before the observer has authorized their load.
  const template = document.createElement('template'); template.innerHTML = safe
  for (const img of template.content.querySelectorAll('img')) {
    const src = img.getAttribute('src') || ''
    const match = /^assets\/([a-f0-9]{64}\.png)$/.exec(src)
    if (match) {
      img.removeAttribute('src'); img.dataset.assetId = match[1]
      const width = Number(img.getAttribute('width')), height = Number(img.getAttribute('height'))
      if (width > 0 && height > 0) img.style.aspectRatio = `${width} / ${height}`
      img.style.minHeight = '80px'; img.setAttribute('loading', 'lazy')
      const retry = document.createElement('button')
      retry.type = 'button'; retry.className = 'document-image-retry'
      retry.textContent = '图片加载失败 · 重试'; retry.hidden = true
      img.after(retry)
    }
  }
  return template.innerHTML
})
let observer: IntersectionObserver | undefined
let cleanup = () => {}
let generation = 0
watch(() => [props.markdown, props.fileId, props.jobId], async () => {
  const current = ++generation
  observer?.disconnect(); cleanup()
  await nextTick()
  if (current !== generation || !root.value) return
  const images = root.value.querySelectorAll<HTMLImageElement>('img[data-asset-id]')
  if (!images.length) return
  const fileId = props.fileId, jobId = props.jobId
  const memoryCosts = new WeakMap<Blob, number>()
  const loader = createDocumentAssetLoader({
    read: async assetId => {
      const result = await tauriCallSafe('read_document_asset', { fileId, jobId, assetId })
      if (!result.ok || !result.data) throw new Error(result.error || '图片加载失败')
      const binary = atob(result.data)
      const bytes = Uint8Array.from(binary, char => char.charCodeAt(0))
      const cost = pngMemoryBytes(bytes)
      const blob = new Blob([bytes], { type: 'image/png' })
      memoryCosts.set(blob, cost)
      return blob
    }, createURL: blob => URL.createObjectURL(blob), revokeURL: url => URL.revokeObjectURL(url),
    memoryCost: blob => memoryCosts.get(blob) ?? blob.size,
  })
  const visible = new Set<HTMLImageElement>()
  const uses = new Map<string, number>()
  async function show(img: HTMLImageElement) {
    const id = img.dataset.assetId!
    try {
      const url = await loader.load(id)
      if (current !== generation) return
      if (!visible.has(img)) { if (!uses.get(id)) loader.release(id); return }
      img.src = url; img.title = img.alt; img.style.minHeight = ''
      img.removeAttribute('role'); img.removeAttribute('tabindex')
      const retry = img.nextElementSibling as HTMLButtonElement | null
      if (retry?.classList.contains('document-image-retry')) retry.hidden = true
    } catch {
      if (current !== generation || !visible.has(img)) return
      img.title = '图片加载失败，点击或按回车重试'; img.setAttribute('role', 'button'); img.tabIndex = 0
      const retry = img.nextElementSibling as HTMLButtonElement | null
      if (retry?.classList.contains('document-image-retry')) retry.hidden = false
    }
  }
  observer = new IntersectionObserver(entries => {
    for (const entry of entries) {
      const img = entry.target as HTMLImageElement, id = img.dataset.assetId!
      if (entry.isIntersecting && !visible.has(img)) {
        visible.add(img); uses.set(id, (uses.get(id) || 0) + 1); void show(img)
      } else if (!entry.isIntersecting && visible.delete(img)) {
        uses.set(id, (uses.get(id) || 1) - 1)
        img.style.minHeight = `${Math.max(80, img.height)}px`; img.removeAttribute('src')
        if (!uses.get(id)) loader.release(id)
      }
    }
  }, { rootMargin: '300px' })
  for (const img of images) observer.observe(img)
  const retry = (event: Event) => {
    const target = event.target as HTMLElement
    const img = (target.matches?.('.document-image-retry') ? target.previousElementSibling : target) as HTMLImageElement
    if (img.matches?.('img[data-asset-id][role=button]') && (event.type === 'click' || (event as KeyboardEvent).key === 'Enter')) void show(img)
  }
  const element = root.value
  element.addEventListener('click', retry); element.addEventListener('keydown', retry)
  cleanup = () => { element.removeEventListener('click', retry); element.removeEventListener('keydown', retry); loader.dispose() }
}, { immediate: true, flush: 'post' })
onBeforeUnmount(() => { generation++; observer?.disconnect(); cleanup() })
</script>
<template><article ref="root" class="document-markdown" v-html="html" /></template>
<style scoped>
.document-markdown :deep(img) { max-width: 100%; height: auto; }
.document-markdown :deep(img[data-asset-id]:not([src])) { display: block; min-width: 80px; background: var(--c-bg-page); border: 1px dashed var(--c-border); }
.document-markdown :deep(img[role=button]) { cursor: pointer; }
.document-markdown :deep(.document-image-retry) { color: var(--c-text); background: var(--c-bg-page); border: 1px solid var(--c-border); padding: 6px 10px; cursor: pointer; }
</style>
