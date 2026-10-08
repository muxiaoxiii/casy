<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from 'vue'
export interface ContextAction { label: string; run?: () => void; children?: ContextAction[]; disabled?: boolean; danger?: boolean; shortcut?: string; separator?: boolean }
const props = defineProps<{ open: boolean; x: number; y: number; actions: ContextAction[]; label?: string }>()
const emit = defineEmits<{ close: [] }>()
const menu = ref<HTMLElement>()
const submenu = ref<ContextAction | null>(null)
let opening = 0
const position = ref({ left: '0px', top: '0px' })
let origin: HTMLElement | null = null
function close(restore = false) { emit('close'); if (restore && origin?.isConnected) origin.focus() }
function outside(e: PointerEvent) { if (!menu.value?.contains(e.target as Node)) close() }
function scroll(e: Event) { if (!menu.value?.contains(e.target as Node)) close() }
function cleanup() { window.removeEventListener('pointerdown', outside, true); window.removeEventListener('scroll', scroll, true); window.removeEventListener('resize', resize); window.removeEventListener('blur', resize) }
function resize() { close() }
function place() {
  if (!menu.value) return
  const bounds = menu.value.getBoundingClientRect()
  position.value = { left: `${Math.max(8, Math.min(props.x, window.innerWidth - bounds.width - 8))}px`, top: `${Math.max(8, Math.min(props.y, window.innerHeight - bounds.height - 8))}px` }
}
async function back() {
  const parent = submenu.value
  submenu.value = null
  await nextTick()
  place()
  const buttons = Array.from(menu.value?.querySelectorAll<HTMLButtonElement>('button:not(:disabled)') || [])
  buttons.find(button => button.dataset.action === parent?.label)?.focus()
}
async function run(action: ContextAction) {
  if (action.disabled) return
  if (action.children) {
    submenu.value = action
    await nextTick()
    place()
    menu.value?.querySelector<HTMLButtonElement>('[data-action]:not(:disabled)')?.focus()
    return
  }
  close()
  action.run?.()
}
function keydown(e: KeyboardEvent) {
  const buttons = Array.from(menu.value?.querySelectorAll<HTMLButtonElement>('button:not(:disabled)') || [])
  const current = buttons.indexOf(document.activeElement as HTMLButtonElement)
  if (e.key === 'ArrowLeft' && submenu.value) { e.preventDefault(); void back(); return }
  if (e.key === 'ArrowRight') {
    const label = (document.activeElement as HTMLElement)?.dataset.action
    const action = (submenu.value?.children || props.actions).find(action => action.label === label)
    if (action?.children && !action.disabled) { e.preventDefault(); void run(action) }
  }
  if (e.key === 'Escape') { e.preventDefault(); close(true) }
  if (e.key === 'Tab') { e.preventDefault(); close(true) }
  if (['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(e.key)) {
    e.preventDefault()
    const i = e.key === 'Home' ? 0 : e.key === 'End' ? buttons.length - 1 : (current + (e.key === 'ArrowDown' ? 1 : -1) + buttons.length) % buttons.length
    buttons[i]?.focus()
  }
}
watch(() => [props.open, props.x, props.y, props.actions], async () => {
  const request = ++opening
  const open = props.open
  cleanup()
  if (!open) return
  submenu.value = null
  if (!menu.value?.contains(document.activeElement)) origin = document.activeElement as HTMLElement
  await nextTick()
  if (request !== opening || !props.open || !menu.value) return
  place()
  menu.value.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus()
  window.addEventListener('pointerdown', outside, true); window.addEventListener('scroll', scroll, true); window.addEventListener('resize', resize); window.addEventListener('blur', resize)
}, { immediate: true })
onBeforeUnmount(() => { opening++; cleanup() })
</script>
<template>
  <Teleport to="body"><div v-if="open" ref="menu" class="casy-context-menu" :style="position" role="menu" :aria-label="label || '操作菜单'" @keydown="keydown" @contextmenu.prevent @click.stop>
    <button v-if="submenu" role="menuitem" type="button" @click="back">← 返回{{ label }}</button><div v-if="label" class="context-heading">{{ submenu?.label || label }}</div>
    <template v-for="action in (submenu?.children || actions)" :key="action.label"><div v-if="action.separator" class="context-divider" role="separator"/><button type="button" role="menuitem" :data-action="action.label" :disabled="action.disabled" :aria-haspopup="action.children ? 'menu' : undefined" :class="{ danger: action.danger }" @click="run(action)"><span>{{ action.label }}</span><span v-if="action.children" aria-hidden="true">›</span><kbd v-if="action.shortcut" class="ui-text--secondary">{{ action.shortcut }}</kbd></button></template>
  </div></Teleport>
</template>
<style scoped>
.casy-context-menu{position:fixed;z-index:3100;min-width:min(210px, calc(100vw - 16px));max-width:calc(100vw - 16px);max-height:min(480px, calc(100vh - 16px));overflow:auto;padding:6px;border:1px solid var(--c-border);border-radius:10px;background:var(--c-bg-card);color:var(--c-text);box-shadow:0 8px 32px #0002}.context-heading{white-space:nowrap;overflow:hidden;text-overflow:ellipsis;padding:7px 10px;font-size:12px;color:var(--c-text-secondary)}.casy-context-menu button{display:flex;align-items:center;justify-content:space-between;gap:24px;width:100%;padding:8px 10px;border:0;border-radius:5px;background:transparent;color:inherit;text-align:left;font:inherit;font-size:13px;cursor:pointer}.casy-context-menu button:hover,.casy-context-menu button:focus-visible{background:var(--c-bg-selected);outline:none}.casy-context-menu button:disabled{opacity:.4;cursor:default}.casy-context-menu button.danger{color:var(--el-color-danger)}.context-divider{height:1px;background:var(--c-border);margin:5px}.casy-context-menu kbd{font-size:11px;font-family:inherit}
</style>
