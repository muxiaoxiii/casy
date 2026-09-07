import { invoke } from '@tauri-apps/api/core'
import { ElMessage } from 'element-plus'
import type { TauriResult, TauriCallOptions } from '../types'
import { isTauriRuntime, tryMockCommand } from './mockData'
import type { CommandMap } from '../types/commandMap'

type CommandArgs<K extends keyof CommandMap & string> =
  keyof CommandMap[K]['params'] extends never
    ? [args?: CommandMap[K]['params']]
    : [args: CommandMap[K]['params']]

type CommandArgsWithOptions<K extends keyof CommandMap & string> =
  keyof CommandMap[K]['params'] extends never
    ? [args?: CommandMap[K]['params'], options?: TauriCallOptions]
    : [args: CommandMap[K]['params'], options?: TauriCallOptions]

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
 * 仅允许调用已登记进 CommandMap 的命令，参数与返回值均由契约推导。
 * 浏览器开发模式（无 Tauri）时回退到 mock 数据。
 */
export async function tauriCallSafe<K extends keyof CommandMap & string>(
  command: K,
  ...rest: CommandArgs<K>
): Promise<TauriResult<CommandMap[K]['result']>> {
  const [args] = rest
  const invokeArgs = (args ?? {}) as Record<string, unknown>
  // 浏览器模式：尝试 mock
  if (!isTauriRuntime()) {
    if (['ai_chat', 'test_ai_profile', 'save_ai_profiles', 'test_ai_connection'].includes(command)) {
      return { ok: false, error: '请在 Casy 桌面应用中配置并调用 AI；浏览器预览不执行模型请求。' }
    }
    const mock = tryMockCommand(command, invokeArgs)
    if (mock !== undefined) {
      return { ok: true, data: mock as CommandMap[K]['result'] }
    }
    // 没有 mock 的命令：静默返回失败（避免刷错误）
    console.warn(`[Mock] 未提供命令 ${command} 的模拟数据`)
    return { ok: false, error: 'browser-mode: no mock' }
  }

  try {
    const result = await invoke<CommandMap[K]['result']>(command, invokeArgs)
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
export async function tauriCall<K extends keyof CommandMap & string>(
  command: K,
  ...rest: CommandArgsWithOptions<K>
): Promise<CommandMap[K]['result'] | null> {
  const [args, options = {}] = rest
  const invokeArgs = (args ?? {}) as Record<string, unknown>
  const { silent = false, errorMessage } = options
  // 浏览器模式：尝试 mock
  if (!isTauriRuntime()) {
    if (['ai_chat', 'test_ai_profile', 'save_ai_profiles', 'test_ai_connection'].includes(command)) return null
    const mock = tryMockCommand(command, invokeArgs)
    if (mock !== undefined) {
      return mock as CommandMap[K]['result']
    }
    return null
  }

  try {
    const result = await invoke<CommandMap[K]['result']>(command, invokeArgs)
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
