/**
 * Casy 插件上下文（cordis 风格内核）
 *
 * 对标 DeepSeek Harness 的 @deepseek-ai/cordis：
 * - Context 服务解析：ctx.cases / ctx.tasks 属性读取 → 自动解析到已注册 Service
 *   （数据通路：视图 → ctx 服务 → tauriBridge → Rust 命令，写入口唯一）
 * - Service 注入：业务模块以 Service 形式注册，互相不 import，经 ctx 协作
 * - Fiber 生命周期：插件/子作用域拥有独立 Fiber，dispose 时自动清理 effects
 * - 作用域事件：监听器登记到当前 Fiber，卸载插件不残留监听
 * - Logger：ctx.logger / getLogger(scope)
 *
 * 同时保留 v3.0 既有 API（工具注册 / AI 提供商 / 确认机制 / 画像），
 * 保证 AIChatPanel、AICompanionView、tool-caller 与 9 个业务插件兼容。
 *
 * 设计哲学对齐：
 * - §11.11 智伴层组件化：没有特权核心，一切皆插件 + 服务
 * - §原则六 双路径铁律：确定性执行永远在 Rust 命令，前端只做通路
 * - §11.4 effective_policy：确认等级 = max(system_minimum, scenario, model, user)
 */

import { ElMessageBox } from 'element-plus'
import type {
  CasyContext,
  CasyPlugin,
  CasyTool,
  CasySkill,
  CasyProvider,
  CasyModel,
  CasyEventHandler,
  ConfirmRequest,
  ConfirmLevel,
  Service,
  Fiber,
  InjectKey,
  CasyLogger,
} from './types'

// ============================================================
// effective_policy 等级排序（§11.4）
// ============================================================

const LEVEL_RANK: Record<ConfirmLevel, number> = { L1: 1, L2: 2, L3: 3 }

function maxLevel(a: ConfirmLevel, b: ConfirmLevel): ConfirmLevel {
  return LEVEL_RANK[a] >= LEVEL_RANK[b] ? a : b
}

// ============================================================
// Fiber —— 生命周期单元（对标 cordis Fiber）
// ============================================================

class FiberImpl implements Fiber {
  readonly name: string
  private cleanups: Array<() => void> = []
  private _disposed = false

  constructor(name: string) {
    this.name = name
  }

  effect(cleanup: () => void): void {
    if (this._disposed) return
    this.cleanups.push(cleanup)
  }

  get effectCount(): number {
    return this.cleanups.length
  }

  get disposed(): boolean {
    return this._disposed
  }

  dispose(): void {
    if (this._disposed) return
    this._disposed = true
    // 逆序执行（后注册的先清理，与 cordis 一致）
    for (let i = this.cleanups.length - 1; i >= 0; i--) {
      try {
        this.cleanups[i]()
      } catch (e) {
        console.error('[Casy] fiber cleanup failed:', e)
      }
    }
    this.cleanups = []
  }
}

// ============================================================
// Logger（对标 cordis logger）
// ============================================================

const baseLogger: CasyLogger = {
  info: (message, ...args) => console.log('[Casy] ' + message, ...args),
  warn: (message, ...args) => console.warn('[Casy] ' + message, ...args),
  error: (message, ...args) => console.error('[Casy] ' + message, ...args),
}

// ============================================================
// 上下文实现
// ============================================================

class CasyContextImpl implements CasyContext {
  private plugins = new Map<string, CasyPlugin>()
  private pluginFibers = new Map<string, FiberImpl>()
  private tools = new Map<string, CasyTool>()
  private skills = new Map<string, CasySkill>()
  private providers = new Map<string, CasyProvider>()
  private services = new Map<string, Service>()
  private listeners = new Map<string, Set<CasyEventHandler>>()
  private profile: Record<string, unknown> = {}

  // ── Fiber 作用域栈 ──
  private rootFiber: FiberImpl
  private fiberStack: FiberImpl[]

  readonly logger: CasyLogger = baseLogger

  constructor() {
    this.rootFiber = new FiberImpl('root')
    this.fiberStack = [this.rootFiber]
    // Proxy：属性读取经服务解析器（对标 cordis ReflectService.handler）
    return new Proxy(this, {
      get(target, prop) {
        if (typeof prop === 'string') {
          const svc = target.services.get(prop)
          if (svc) return svc
        }
        return Reflect.get(target, prop)
      },
    })
  }

  private get activeFiber(): FiberImpl {
    return this.fiberStack[this.fiberStack.length - 1]
  }

  // ── 插件管理 ──

  async use(plugin: CasyPlugin): Promise<void> {
    await this.plugin(plugin)
  }

  async plugin(plugin: CasyPlugin): Promise<Fiber> {
    const fiber = new FiberImpl(plugin.name)
    this.plugins.set(plugin.name, plugin)
    this.pluginFibers.set(plugin.name, fiber)
    this.fiberStack.push(fiber)
    try {
      // inject 依赖校验（插件可声明依赖服务）
      const deps = (plugin as CasyPlugin & { inject?: InjectKey[] }).inject ?? []
      const missing = deps.filter((k) => !this.services.has(k))
      if (missing.length > 0) {
        throw new Error('插件 ' + plugin.name + ' 依赖未就绪: ' + missing.join(', '))
      }
      await plugin.install(this)
      this.logger.info('插件已安装: ' + plugin.name + ' v' + plugin.version)
    } catch (e) {
      console.error('[Casy] 插件安装失败: ' + plugin.name, e)
      this.plugins.delete(plugin.name)
      this.pluginFibers.delete(plugin.name)
    } finally {
      this.fiberStack.pop()
    }
    return fiber
  }

  async unuse(name: string): Promise<void> {
    const plugin = this.plugins.get(name)
    if (!plugin) return
    try {
      await plugin.uninstall(this)
    } catch (e) {
      console.error('[Casy] 插件卸载失败: ' + name, e)
    }
    this.pluginFibers.get(name)?.dispose()
    this.pluginFibers.delete(name)
    this.plugins.delete(name)
  }

  getPlugins(): CasyPlugin[] {
    return [...this.plugins.values()]
  }

  // ── 服务注入（cordis 数据通路） ──

  provide(name: string, service: Service, inject: InjectKey[] = []): void {
    const missing = inject.filter((k) => !this.services.has(k))
    if (missing.length > 0) {
      throw new Error('服务 ' + name + ' 依赖未就绪: ' + missing.join(', '))
    }
    this.services.set(name, service)
    this.logger.info('服务已注册: ctx.' + name)
  }

  unprovide(name: string): void {
    const service = this.services.get(name)
    if (!service) return
    void service.dispose()
    this.services.delete(name)
  }

  getService<T extends Service>(name: string): T | undefined {
    return this.services.get(name) as T | undefined
  }

  getServiceNames(): string[] {
    return [...this.services.keys()]
  }

  // ── Fiber / effect ──

  effect(cleanup: () => void): void {
    this.activeFiber.effect(cleanup)
  }

  fork(name: string): Fiber {
    const fiber = new FiberImpl(name)
    this.fiberStack.push(fiber)
    // 返回一个与栈同步的 Fiber：dispose 时自动出栈恢复父级
    const originalDispose = fiber.dispose.bind(fiber)
    fiber.dispose = () => {
      const idx = this.fiberStack.indexOf(fiber)
      if (idx >= 0) this.fiberStack.splice(idx, 1)
      originalDispose()
    }
    return fiber
  }

  getLogger(scope: string): CasyLogger {
    return {
      info: (message, ...args) => baseLogger.info('[' + scope + '] ' + message, ...args),
      warn: (message, ...args) => baseLogger.warn('[' + scope + '] ' + message, ...args),
      error: (message, ...args) => baseLogger.error('[' + scope + '] ' + message, ...args),
    }
  }

  // ── 工具注册 ──

  registerTool(tool: CasyTool): void {
    this.tools.set(tool.name, tool)
  }

  unregisterTool(name: string): void {
    this.tools.delete(name)
  }

  getTools(): CasyTool[] {
    return [...this.tools.values()]
  }

  getTool(name: string): CasyTool | null {
    return this.tools.get(name) ?? null
  }

  getToolDefinitions() {
    return this.getTools().map(({ name, description, parameters }) => ({
      name,
      description,
      parameters,
    }))
  }

  async executeTool(
    name: string,
    params: Record<string, unknown>
  ): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    const tool = this.tools.get(name)
    if (!tool) {
      return { ok: false, error: '工具不存在: ' + name }
    }
    try {
      return await tool.execute(params)
    } catch (e) {
      console.error('[Casy] 工具执行异常: ' + name, e)
      return { ok: false, error: e instanceof Error ? e.message : String(e) }
    }
  }

  // ── 技能注册（按需加载，不占常驻上下文） ──

  registerSkill(skill: CasySkill): void {
    this.skills.set(skill.name, skill)
  }

  unregisterSkill(name: string): void {
    this.skills.delete(name)
  }

  async executeSkill(name: string, params: Record<string, unknown>): Promise<unknown> {
    const skill = this.skills.get(name)
    if (!skill) {
      throw new Error('技能不存在: ' + name)
    }
    return skill.execute(params)
  }

  // ── AI 提供商 ──

  registerProvider(provider: CasyProvider): void {
    this.providers.set(provider.id, provider)
  }

  getProviders(): CasyProvider[] {
    return [...this.providers.values()]
  }

  getModels(): CasyModel[] {
    return this.getProviders().flatMap((p) => p.models)
  }

  // ── 事件系统（作用域化：监听器登记到当前 Fiber，dispose 自动清理） ──

  on(event: string, handler: CasyEventHandler): () => void {
    if (!this.listeners.has(event)) {
      this.listeners.set(event, new Set())
    }
    const set = this.listeners.get(event)!
    set.add(handler)
    const off = () => {
      set.delete(handler)
    }
    // 登记到当前 Fiber：插件卸载时自动移除其监听
    this.activeFiber.effect(off)
    return off
  }

  emit(event: string, payload?: unknown): void {
    const handlers = this.listeners.get(event)
    if (!handlers) return
    handlers.forEach((h) => {
      try {
        h(payload)
      } catch (e) {
        console.error('[Casy] 事件处理异常: ' + event, e)
      }
    })
  }

  // ── 律师画像 ──

  setProfile(profile: Record<string, unknown>): void {
    this.profile = { ...profile }
  }

  getProfile(): Record<string, unknown> {
    return { ...this.profile }
  }

  // ── 确认机制（§11.4 effective_policy 前端镜像） ──

  calculateEffectiveLevel(opts: {
    isExternalWrite?: boolean
    modelQuality?: string
    userPolicy?: ConfirmLevel
  }): ConfirmLevel {
    let effective: ConfirmLevel = 'L1'
    if (opts.isExternalWrite) {
      effective = maxLevel(effective, 'L3')
    }
    if (opts.modelQuality === 'local_small') {
      effective = maxLevel(effective, 'L2')
    }
    if (opts.userPolicy) {
      effective = maxLevel(effective, opts.userPolicy)
    }
    return effective
  }

  async requestConfirm(req: ConfirmRequest): Promise<boolean> {
    const { level, title, message, onConfirm } = req

    if (level === 'L3') {
      try {
        await ElMessageBox.prompt(message, title + '（L3 双人复核）', {
          confirmButtonText: '确认执行',
          cancelButtonText: '拒绝',
          inputPlaceholder: '输入 确认',
          inputPattern: /^确认$/,
          inputErrorMessage: '请输入 确认 以继续',
          type: 'warning',
        })
        await onConfirm?.()
        return true
      } catch {
        return false
      }
    }

    if (level === 'L2') {
      try {
        await ElMessageBox.confirm(message, title + '（L2 逐项确认）', {
          confirmButtonText: '确认',
          cancelButtonText: '拒绝',
          type: 'warning',
        })
        await onConfirm?.()
        return true
      } catch {
        return false
      }
    }

    await onConfirm?.()
    return true
  }
}

/** 全局唯一的 Casy 上下文实例（Proxy：ctx.cases 等服务属性自动解析） */
export const casyContext: CasyContext = new CasyContextImpl()
