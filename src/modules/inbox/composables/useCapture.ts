/**
 * 多通道捕获 composable（设计哲学 §10）
 * 截屏、剪贴板、语音速记
 */
import { ref } from 'vue'
import { tauriCallSafe } from '../../../core/tauriBridge'

export function useCapture() {
  const capturing = ref(false)
  const lastCapture = ref(null)

  /** 截屏捕获 */
  async function captureScreenshot() {
    capturing.value = true
    const result = await tauriCallSafe('capture_screenshot', {})
    capturing.value = false
    if (result.ok) {
      lastCapture.value = result.data
      return result.data
    }
    return null
  }

  /** 剪贴板捕获 */
  async function captureClipboard() {
    capturing.value = true
    const result = await tauriCallSafe('capture_clipboard', {})
    capturing.value = false
    if (result.ok) {
      lastCapture.value = result.data
      return result.data
    }
    return null
  }

  /** 启动剪贴板监听 */
  async function startClipboardMonitor() {
    return await tauriCallSafe('start_clipboard_monitor', {})
  }

  return {
    capturing,
    lastCapture,
    captureScreenshot,
    captureClipboard,
    startClipboardMonitor,
  }
}
