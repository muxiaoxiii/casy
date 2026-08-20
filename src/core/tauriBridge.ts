import { invoke } from '@tauri-apps/api/core'
import { ElMessage } from 'element-plus'
import type { TauriResult, TauriCallOptions } from '../types'
import { isTauriRuntime, tryMockCommand } from './mockData'

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
 * 不自动弹出错误提示，由调用方决定
 * 浏览器开发模式（无 Tauri）时回退到 mock 数据
 */
export async function tauriCallSafe<T = unknown>(
  command: string,
  args: Record<string, unknown> = {}
): Promise<TauriResult<T>> {
  // 浏览器模式：尝试 mock
  if (!isTauriRuntime()) {
    const mock = tryMockCommand(command, args)
    if (mock !== undefined) {
      return { ok: true, data: mock as T }
    }
    // 没有 mock 的命令：静默返回失败（避免刷错误）
    console.warn(`[Mock] 未提供命令 ${command} 的模拟数据`)
    return { ok: false, error: 'browser-mode: no mock' }
  }

  try {
    const result = await invoke<T>(command, args)
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
export async function openPath(path: string): Promise<TauriResult<void>> {
  return tauriCallSafe('open_path', { path })
}
