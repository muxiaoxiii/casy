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

export async function invokeWithDeadline<T>(command: string, args: Record<string, unknown>, timeoutMs?: number): Promise<T> {
  // Conversion duration depends on page count. The backend owns progress and stall
  // detection; abandoning its promise would report failure and start the next job
  // while this one still writes output in the background.
  const backendOwnsCompletion = ['convert_file_to_markdown', 'optimize_document_storage', 'rollback_document_storage', 'correct_document_region'].includes(command) || /^(create_backup|restore_backup|export_|import_|webdav_(push|pull|resolve_|backup_full|restore_full)|feishu_(sync_|import_)|sync_feishu_)/.test(command)
  if (backendOwnsCompletion && timeoutMs === undefined) return invoke<T>(command, args)
  const limit=timeoutMs ?? (/export|import|backup|sync|ai_chat|preview_editor|transcrib|convert/.test(command)?180000:60000)
  let timer:ReturnType<typeof setTimeout>|undefined
  try {
    return await Promise.race([
      invoke<T>(command,args),
      new Promise<never>((_,reject)=>{timer=setTimeout(()=>reject(new Error('IPC_TIMEOUT: 等待响应超时。后台操作可能仍在进行，请先刷新核对结果；不要重复提交写入。')),limit)}),
    ])
  } finally {clearTimeout(timer)}
}

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
    return { ok: false, error: '浏览器预览未提供此功能的模拟数据，请在 Casy 桌面应用中使用。' }
  }

  try {
    const result = await invokeWithDeadline<CommandMap[K]['result']>(command, invokeArgs)
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
    const result = await invokeWithDeadline<CommandMap[K]['result']>(command, invokeArgs)
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
