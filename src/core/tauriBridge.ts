import { invoke } from '@tauri-apps/api/core'
import { ElMessage } from 'element-plus'
import type { TauriResult, TauriCallOptions } from '../types'
import { isTauriRuntime, tryMockCommand } from './mockData'
import type { CommandMap } from '../types/commandMap'

// 是否启用全局错误提示（可通过设置关闭）
let globalErrorNotify = true

/**
 * 设置全局错误提示开关
 */
export function setGlobalErrorNotify(enabled: boolean): void {
  globalErrorNotify = enabled
}

/**
 * 安全调用 Tauri 命令，返回 { ok, data, error }
 *
 * 重载 1（D-3/B1）：已登记进 CommandMap 的命令获得参数/返回的编译期检查；
 * 重载 2：未登记命令保持原通用签名（含显式泛型），行为不变。
 * 浏览器开发模式（无 Tauri）时回退到 mock 数据。
 */
export async function tauriCallSafe<K extends keyof CommandMap & string>(
  command: K,
  args: CommandMap[K]['params']
): Promise<TauriResult<CommandMap[K]['result']>>
export async function tauriCallSafe<R = unknown>(
  command: string,
  args?: Record<string, unknown>
): Promise<TauriResult<R>>
export async function tauriCallSafe(
  command: string,
  args: Record<string, unknown> = {}
): Promise<TauriResult<unknown>> {
  // 浏览器模式：尝试 mock
  if (!isTauriRuntime()) {
    const mock = tryMockCommand(command, args)
    if (mock !== undefined) {
      return { ok: true, data: mock }
    }
    // 没有 mock 的命令：静默返回失败（避免刷错误）
    console.warn(`[Mock] 未提供命令 ${command} 的模拟数据`)
    return { ok: false, error: 'browser-mode: no mock' }
  }

  try {
    const result = await invoke<unknown>(command, args)
    return { ok: true, data: result }
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err)
    console.error(`[Casy] ${command} failed:`, message)
    return { ok: false, error: message }
  }
}

/**
 * 调用 Tauri 命令，失败时自动显示 ElMessage.error
 * 返回 result 数据，失败返回 null
 */
export async function tauriCall<T = unknown>(
  command: string,
  args: Record<string, unknown> = {},
  options: TauriCallOptions = {}
): Promise<T | null> {
  const { silent = false, errorMessage } = options
  // 浏览器模式：尝试 mock
  if (!isTauriRuntime()) {
    const mock = tryMockCommand(command, args)
    if (mock !== undefined) {
      return mock as T
    }
    return null
  }

  try {
    const result = await invoke<T>(command, args)
    return result
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err)
    console.error(`[Casy] ${command} failed:`, message)
    if (!silent && globalErrorNotify) {
      const displayMsg = errorMessage || `${command} 失败: ${message}`
      ElMessage.error(displayMsg)
    }
    return null
  }
}

/**
 * 打开文件/目录
 */
export async function openPath(path: string): Promise<TauriResult<null>> {
  return tauriCallSafe('open_file_with_default', { path })
}
