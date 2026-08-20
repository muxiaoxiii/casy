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
// ============================================================
// cordis 风格内核（对标 DeepSeek Harness / @deepseek-ai/cordis）
// 数据通路：视图 → ctx 服务（Service 注入）→ tauriBridge → Rust 命令
// ============================================================

/** 服务依赖声明：插件/服务启动前必须已注册的服务名 */
export type InjectKey = string

/**
 * 业务服务基类（数据通路的节点）
 *
 * 每个业务模块一个 Service，注册到 Context 后可通过 ctx.<name> 访问：
 *   ctx.cases.list() / ctx.tasks.create() / ...
 * 服务内部封装 tauriBridge 调用（写入口唯一，双路径铁律）。
 */
export abstract class Service {
  /** 声明依赖：启动本服务前必须已注册的服务名 */
  static inject: InjectKey[] = []

  constructor(public readonly ctx: CasyContext) {}

  /** 服务初始化（可选覆写） */
  async setup(): Promise<void> {}

  /** 服务卸载清理（可选覆写） */
  async dispose(): Promise<void> {}
}

/**
 * Fiber——插件/服务的生命周期单元（对标 cordis Fiber）
 *
 * 插件启动返回 Fiber；dispose() 时自动执行其 effects（事件监听、定时器等），
 * 避免卸载插件后残留监听器。
 */
export interface Fiber {
  /** 作用域名称（插件名/服务名） */
  name: string
  /** 注册一个清理函数，dispose 时执行 */
  effect(cleanup: () => void): void
  /** 已注册的清理函数数量（调试用） */
  readonly effectCount: number
  /** 是否已释放 */
  readonly disposed: boolean
  /** 释放：按注册逆序执行所有清理函数 */
  dispose(): void
}

/** 日志记录器（对标 cordis logger） */
export interface CasyLogger {
  info(message: string, ...args: unknown[]): void
  warn(message: string, ...args: unknown[]): void
  error(message: string, ...args: unknown[]): void
}

/** 服务提供器：把一个 Service 实例注册到 ctx.<name> */
export interface ServiceProvider {
  name: string
  service: Service
  /** 依赖的服务名（服务未就绪则拒绝注册） */
  inject: InjectKey[]
}


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

  // ── cordis 风格：服务注入（数据通路） ──
  /** 注册服务：ctx.<name> 可访问 */
  provide(name: string, service: Service, inject?: InjectKey[]): void
  /** 卸载服务 */
  unprovide(name: string): void
  /** 取服务实例；未注册返回 undefined */
  getService<T extends Service>(name: string): T | undefined
  /** 已注册的服务名列表 */
  getServiceNames(): string[]

  // ── cordis 风格：Fiber 生命周期 ──
  /** 启动一个插件（返回 Fiber；dispose 时自动清理 effects） */
  plugin(plugin: CasyPlugin): Promise<Fiber>
  /** 在指定 Fiber 作用域注册清理函数（默认当前活跃 fiber） */
  effect(cleanup: () => void): void
  /** 创建子作用域 Fiber（隔离 effects，dispose 不触碰父级） */
  fork(name: string): Fiber

  // ── cordis 风格：Logger ──
  logger: CasyLogger
  /** 获取带作用域名的 logger */
  getLogger(scope: string): CasyLogger
}
