/**
 * 工具定义助手（K-2 类型双轨合一）
 *
 * 把工具的 JSON Schema 与 execute 参数类型 P 绑定：
 * - 插件侧获得完全类型检查（params.x 不再需要 `as T` 收窄）
 * - 全项目唯一的边界断言点集中在本文件（raw as P），
 *   未来引入运行时校验（specta/zod）时只改这一处
 */
import type { CasyTool, ToolParameterSchema, ToolPolicy } from './types'

/** 与 tauriCallSafe / CasyTool.execute 一致的返回契约 */
export interface ToolResult {
  ok: boolean
  data?: unknown
  error?: string
}

export function defineTool<P extends object>(
  def: {
    name: string
    description: string
    category: string
    parameters: ToolParameterSchema
    /** 声明式确认策略：由 Context.executeTool 统一强制执行 */
    policy?: ToolPolicy
    execute: (params: P) => Promise<ToolResult>
  },
): CasyTool {
  return {
    name: def.name,
    description: def.description,
    category: def.category,
    parameters: def.parameters,
    ...(def.policy ? { policy: def.policy } : {}),
    // 唯一的边界断言：schema 声明即参数契约（见文件头注释）
    execute: (raw: Record<string, unknown>) => def.execute(raw as P),
  }
}
