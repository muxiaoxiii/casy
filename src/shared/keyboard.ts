/**
 * KeyboardCenter —— 全局快捷键注册中心（A-UI U-4）
 *
 * 统一此前分散在 App.vue / TasksView 的 window/document 级监听：
 * - registerShortcut(combo, handler, opts) → 返回注销函数（组件 onUnmounted 调用）
 * - 单一 window keydown 分发；组合式解析（meta/ctrl/shift/alt + 主键）
 * - 默认在输入框聚焦时让位原生行为（opts.allowInInput = true 可豁免，如 ⌘K 需要在输入框中也可用）
 *
 * 设计哲学：U-4 目标是「一个中心、可枚举、无冲突」——
 * 冲突在注册时检测并 console.warn（同 combo 后到者覆盖先到者需显式 override）。
 */

type KeyHandler = (e: KeyboardEvent) => void

export interface ShortcutOptions {
  /** 输入框/textarea/contentEditable 聚焦时仍触发（默认 false） */
  allowInInput?: boolean
  /** 允许覆盖已注册的同组合键 */
  override?: boolean
  /** 说明文字（用于调试与未来快捷键帮助面板） */
  description?: string
}

interface RegisteredShortcut {
  combo: string
  handler: KeyHandler
  options: ShortcutOptions
}

const registry = new Map<string, RegisteredShortcut>()
let installed = false

/** 规范化组合串：修饰键排序 + 主键小写，如 'meta+k' / 'ctrl+shift+z' */
export function normalizeCombo(combo: string): string {
  const parts = combo
    .toLowerCase()
    .split('+')
    .map(s => s.trim())
    .filter(Boolean)
  const mods = parts.filter(p => ['meta', 'ctrl', 'shift', 'alt'].includes(p)).sort()
  const key = parts.find(p => !['meta', 'ctrl', 'shift', 'alt'].includes(p))
  if (!key) return mods.join('+')
  return [...mods, key].join('+')
}

function isEditableTarget(t: EventTarget | null): boolean {
  const el = t as HTMLElement | null
  if (!el) return false
  const tag = el.tagName ? String(el.tagName).toLowerCase() : ''
  return tag === 'input' || tag === 'textarea' || !!el.isContentEditable
}

function matchEvent(e: KeyboardEvent, combo: string): boolean {
  const want = new Set(combo.split('+'))
  const key = want.has(e.key.toLowerCase()) ? e.key.toLowerCase() : e.code.toLowerCase().replace('key', '')
  if (!want.has(key)) return false
  const needMeta = want.has('meta')
  const needCtrl = want.has('ctrl')
  const needShift = want.has('shift')
  const needAlt = want.has('alt')
  // macOS 上 Ctrl+C/V 等浏览器保留键不拦截的场景由调用方规避；此处仅精确匹配
  if (needMeta !== e.metaKey || needCtrl !== e.ctrlKey) return false
  if (needShift !== e.shiftKey || needAlt !== e.altKey) return false
  // 排除纯修饰键按下
  return !['Meta', 'Control', 'Shift', 'Alt', 'CapsLock'].includes(e.key)
}

function dispatch(e: KeyboardEvent) {
  for (const [combo, reg] of registry) {
    if (!matchEvent(e, combo)) continue
    if (!reg.options.allowInInput && isEditableTarget(e.target)) continue
    e.preventDefault()
    reg.handler(e)
    return
  }
}

function ensureInstalled() {
  if (installed) return
  window.addEventListener('keydown', dispatch)
  installed = true
}

/**
 * 注册全局快捷键。返回注销函数。
 * combo 形如 'meta+k' / 'ctrl+z' / 'shift+?'（meta 在 Windows/Linux 上由 ctrl 等价处理，
 * 调用方需要双平台时应注册两条或使用 normalizeCombo 自行判断）。
 */
export function registerShortcut(combo: string, handler: KeyHandler, options: ShortcutOptions = {}): () => void {
  const normalized = normalizeCombo(combo)
  if (registry.has(normalized) && !options.override) {
    console.warn(
      `[Keyboard] 快捷键冲突: ${normalized} 已被「${registry.get(normalized)?.options.description ?? '?'}」占用，` +
        `新注册「${options.description ?? '?'}」被忽略（override: true 可强制）`
    )
    return () => {}
  }
  registry.set(normalized, { combo: normalized, handler, options })
  ensureInstalled()
  return () => {
    const cur = registry.get(normalized)
    if (cur && cur.handler === handler) registry.delete(normalized)
  }
}

/** 当前全部注册项（调试/帮助面板用） */
export function listShortcuts(): Array<{ combo: string; description: string }> {
  return Array.from(registry.values()).map(r => ({ combo: r.combo, description: r.options.description ?? '' }))
}
