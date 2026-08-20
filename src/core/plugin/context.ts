/**
 * Casy 插件上下文（真实实现）
 *
 * v3.0 插件化架构的容器：插件注册 / 工具注册 / 技能注册 / AI 提供商 /
 * 前端事件 / 律师画像 / 确认机制（effective_policy 前端镜像）。
 *
 * 设计哲学对齐：
 * - §11.11 智伴层组件化：没有特权核心，一切皆插件（9 个业务插件）
 * - §原则六 双路径铁律：工具 execute 只做"转发"——真实执行在 Rust 命令
 *   （tauriBridge → tauriCallSafe），前端插件层不触碰数据库
 * - §11.4 effective_policy：确认等级 = max(system_minimum, scenario,
 *   model, user)；system_minimum（外部写 = L3）代码层硬编码，不可降低
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
} from './types'

// ============================================================
// effective_policy 等级排序（§11.4）
// ============================================================

const LEVEL_RANK: Record<ConfirmLevel, number> = { L1: 1, L2: 2, L3: 3 }

function maxLevel(a: ConfirmLevel, b: ConfirmLevel): ConfirmLevel {
  return LEVEL_RANK[a] >= LEVEL_RANK[b] ? a : b
}

// ============================================================
// 上下文实现
// ============================================================

class CasyContextImpl implements CasyContext {
  private plugins = new Map<string, CasyPlugin>()
  private tools = new Map<string, CasyTool>()
  private skills = new Map<string, CasySkill>()
  private providers = new Map<string, CasyProvider>()
  private listeners = new Map<string, Set<CasyEventHandler>>()
  private profile: Record<string, unknown> = {}

  // ── 插件管理 ──

  async use(plugin: CasyPlugin): Promise<void> {
    this.plugins.set(plugin.name, plugin)
    try {
      await plugin.install(this)
      console.log(`[Casy] 插件已安装: ${plugin.name} v${plugin.version}`)
    } catch (e) {
      console.error(`[Casy] 插件安装失败: ${plugin.name}`, e)
      this.plugins.delete(plugin.name)
    }
  }

  async unuse(name: string): Promise<void> {
    const plugin = this.plugins.get(name)
    if (!plugin) return
    try {
      await plugin.uninstall(this)
    } catch (e) {
      console.error(`[Casy] 插件卸载失败: ${name}`, e)
    }
    this.plugins.delete(name)
  }

  getPlugins(): CasyPlugin[] {
    return [...this.plugins.values()]
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
      return { ok: false, error: `工具不存在: ${name}` }
    }
    try {
      return await tool.execute(params)
    } catch (e) {
      console.error(`[Casy] 工具执行异常: ${name}`, e)
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
      throw new Error(`技能不存在: ${name}`)
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

  // ── 事件系统（前端级 pub/sub） ──

  on(event: string, handler: CasyEventHandler): () => void {
    if (!this.listeners.has(event)) {
      this.listeners.set(event, new Set())
    }
    this.listeners.get(event)!.add(handler)
    return () => {
      this.listeners.get(event)?.delete(handler)
    }
  }

  emit(event: string, payload?: unknown): void {
    const handlers = this.listeners.get(event)
    if (!handlers) return
    handlers.forEach((h) => {
      try {
        h(payload)
      } catch (e) {
        console.error(`[Casy] 事件处理异常: ${event}`, e)
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
    // 1. 系统安全下限（硬编码，不可被降低）
    let effective: ConfirmLevel = 'L1'
    if (opts.isExternalWrite) {
      effective = maxLevel(effective, 'L3')
    }
    // 2. 模型质量（本地小模型 +1 级）
    if (opts.modelQuality === 'local_small') {
      effective = maxLevel(effective, 'L2')
    }
    // 3. 用户设置（可提高，不能降低）
    if (opts.userPolicy) {
      effective = maxLevel(effective, opts.userPolicy)
    }
    return effective
  }

  async requestConfirm(req: ConfirmRequest): Promise<boolean> {
    const { level, title, message, onConfirm } = req

    if (level === 'L3') {
      // 双人复核：必须输入"确认"（§11.4 L3）
      try {
        await ElMessageBox.prompt(message, `${title}（L3 双人复核）`, {
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
      // 逐项确认（§11.4 L2）
      try {
        await ElMessageBox.confirm(message, `${title}（L2 逐项确认）`, {
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

    // L1 可读确认：看一眼即过，低打扰（§11.4）
    await onConfirm?.()
    return true
  }
}

/** 全局唯一的 Casy 上下文实例 */
export const casyContext: CasyContext = new CasyContextImpl()
