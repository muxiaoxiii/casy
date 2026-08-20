/**
 * 安全的 Tauri 事件监听
 *
 * 浏览器开发模式（无 Tauri）下 listen() 会抛错（transformCallback undefined），
 * 这里统一包装：非 Tauri 环境直接返回 no-op 取消函数。
 */
import { listen as tauriListen } from '@tauri-apps/api/event'
import { isTauriRuntime } from './mockData'

export async function safeListen<T = unknown>(
  event: string,
  handler: (event: { payload: T }) => void
): Promise<() => void> {
  if (!isTauriRuntime()) {
    console.debug(`[Casy] 浏览器模式，跳过事件监听: ${event}`)
    return () => {}
  }
  try {
    return await tauriListen(event, handler as any)
  } catch (e) {
    console.warn(`[Casy] 事件监听未建立: ${event}`, e)
    return () => {}
  }
}
