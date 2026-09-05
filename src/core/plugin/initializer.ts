/**
 * 插件系统初始化器（真实实现）
 *
 * 启动时安装 9 个业务插件、注册 AI 提供商（从后端 get_ai_config 读取
 * 已配置的模式/模型，合并各提供商默认模型清单）。
 *
 * 设计哲学对齐：
 * - §11.11 智伴层组件化：插件在启动时注册，工具即插即用
 * - §原则六：AI 提供商 = 模型适配器；确定性执行仍在 Rust 命令
 */

import { casyContext } from './context'
import { registerServices } from '../services'
import {
  CasesPlugin,
  TasksPlugin,
  KnowledgePlugin,
  CalendarPlugin,
  InboxPlugin,
  ReminderPlugin,
  FilesPlugin,
  SyncPlugin,
  SettingsPlugin,
} from '../plugins'
import { tauriCallSafe } from '../tauriBridge'

// ============================================================
// AI 提供商默认模型清单（Ollama 不提供 /api/tags 跨域列表，
// 以"已配置模型 + 常用默认"为准；模型名可在设置页手动填写）
// ============================================================

/** Register only saved profiles; the Rust backend owns credentials. */
async function registerProviders(): Promise<void> {
  const cfg = await tauriCallSafe('get_ai_profiles', {})
  if (!cfg.ok || !cfg.data) throw new Error(cfg.error || '读取 AI 配置失败')
  const activeId = cfg.data.activeId
  casyContext.replaceProviders([...cfg.data.profiles].sort((a, b) => Number(b.id === activeId) - Number(a.id === activeId)).map(p => ({
    id: p.id, name: p.name, mode: p.mode, apiUrl: p.apiUrl,
    models: [{ id: p.model, name: p.model }],
  })))
}

/**
 * 初始化插件系统：安装全部业务插件 + 注册 AI 提供商
 */
export async function initializePluginSystem(): Promise<void> {
  // 0. 业务服务（数据通路：ctx.cases / ctx.tasks / ...）
  await registerServices()

  // 1. 业务插件（9 个）
  const plugins = [
    new CasesPlugin(),
    new TasksPlugin(),
    new KnowledgePlugin(),
    new CalendarPlugin(),
    new InboxPlugin(),
    new ReminderPlugin(),
    new FilesPlugin(),
    new SyncPlugin(),
    new SettingsPlugin(),
  ]
  for (const plugin of plugins) {
    await casyContext.use(plugin)
  }

  // 2. AI 提供商
  try {
    await registerProviders()
  } catch (e) {
    console.warn('[Casy] AI 提供商注册失败（不影响业务插件）:', e)
  }

  const toolCount = casyContext.getTools().length
  const providerCount = casyContext.getProviders().length
  console.log(
    `[Casy] 插件系统初始化完成：${plugins.length} 个插件 / ${toolCount} 个工具 / ${providerCount} 个 AI 提供商`
  )
  casyContext.emit('plugins:ready', { tools: toolCount, providers: providerCount })
}
