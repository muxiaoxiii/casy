/**
 * Casy 插件系统类型定义（v3.0 插件化架构 · 真实实现）
 *
 * 设计哲学 §11.11 智伴层组件化：
 * - 插件是 AI 面向业务能力的"工具注册表"（对外暴露的 MCP 与之同构）
 * - 工具执行永远走 tauriBridge → Rust 命令（写入口唯一，双路径铁律 §原则六）
 * - 确认策略按场景挂载（§11.4 effective_policy，system_minimum 不可降低）
 */

// ============================================================
// 工具
// ============================================================

/** 工具参数 JSON Schema（子集：type/properties/required/items/enum/description） */
export interface ToolParameterSchema {
  type?: string
  description?: string
  properties?: Record<string, ToolParameterSchema>
  required?: string[]
  items?: ToolParameterSchema
  enum?: string[]
}

/** AI 可调用的业务工具（与 MCP 工具定义同构） */
export interface CasyTool {
  name: string
  description: string
  category: string
  parameters: ToolParameterSchema
  /** 执行工具；返回与 tauriCallSafe 一致的 { ok, data, error } */
  execute(params: Record<string, unknown>): Promise<{
    ok: boolean
    data?: unknown
    error?: string
  }>
}

// ============================================================
// 技能（内部访问外部，§11.11 ② —— 按需加载，不占常驻上下文）
// ============================================================

export interface CasySkill {
  name: string
  description: string
  execute(params: Record<string, unknown>): Promise<unknown>
}

// ============================================================
// AI 提供商与模型（智伴层组件化的"模型适配器"）
// ============================================================

export interface CasyModel {
  id: string
  name: string
}

export interface CasyProvider {
  /** 提供商 id：'ollama' | 'openai' | 'deepseek' ... */
  id: string
  name: string
  /** 后端 AiConfig.mode：'ollama' | 'openai' | 'noop' */
  mode: string
  apiUrl: string
  apiKey?: string
  models: CasyModel[]
}

// ============================================================
// 确认机制（§11.4 Confirmer）
// ============================================================

export type ConfirmLevel = 'L1' | 'L2' | 'L3'

export interface ConfirmRequest {
  level: ConfirmLevel
  title: string
  message: string
  /** L2 逐项确认的条目 */
  items?: string[]
  /** 确认成功后回调（可选） */
  onConfirm?: () => void | Promise<void>
}

// ============================================================
// 插件
// ============================================================

export interface CasyPlugin {
  name: string
  version: string
  description: string
  install(ctx: CasyContext): Promise<void> | void
  uninstall(ctx: CasyContext): Promise<void> | void
}

// ============================================================
// 事件
// ============================================================

export type CasyEventHandler = (payload: unknown) => void

// ============================================================
// 上下文（容器）—— 9 个业务插件 + AI 工具循环的唯一入口
// ============================================================

export interface CasyContext {
  // ── 插件管理 ──
  use(plugin: CasyPlugin): Promise<void>
  unuse(name: string): Promise<void>
  getPlugins(): CasyPlugin[]

  // ── 工具注册 ──
  registerTool(tool: CasyTool): void
  unregisterTool(name: string): void
  getTools(): CasyTool[]
  getTool(name: string): CasyTool | null
  getToolDefinitions(): Array<Pick<CasyTool, 'name' | 'description' | 'parameters'>>
  executeTool(name: string, params: Record<string, unknown>): Promise<{
    ok: boolean
    data?: unknown
    error?: string
  }>

  // ── 技能注册 ──
  registerSkill(skill: CasySkill): void
  unregisterSkill(name: string): void
  executeSkill(name: string, params: Record<string, unknown>): Promise<unknown>

  // ── AI 提供商 ──
  registerProvider(provider: CasyProvider): void
  getProviders(): CasyProvider[]
  getModels(): CasyModel[]

  // ── 事件系统（前端级 pub/sub；领域事件见后端 audit_events） ──
  on(event: string, handler: CasyEventHandler): () => void
  emit(event: string, payload?: unknown): void

  // ── 律师画像（§11.2 时机智能） ──
  setProfile(profile: Record<string, unknown>): void
  getProfile(): Record<string, unknown>

  // ── 确认机制（§11.4 effective_policy 前端镜像） ──
  calculateEffectiveLevel(opts: {
    isExternalWrite?: boolean
    modelQuality?: 'local_small' | 'local_large' | 'cloud' | string
    userPolicy?: ConfirmLevel
  }): ConfirmLevel
  requestConfirm(req: ConfirmRequest): Promise<boolean>
}
