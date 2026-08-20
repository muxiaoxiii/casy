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
import type { CasyProvider } from './types'

// ============================================================
// AI 提供商默认模型清单（Ollama 不提供 /api/tags 跨域列表，
// 以"已配置模型 + 常用默认"为准；模型名可在设置页手动填写）
// ============================================================

const DEFAULT_MODELS: Record<string, { id: string; name: string }[]> = {
  ollama: [
    { id: 'qwen2.5:14b', name: 'Qwen2.5 14B' },
    { id: 'qwen2.5:7b', name: 'Qwen2.5 7B' },
    { id: 'deepseek-r1:14b', name: 'DeepSeek-R1 14B' },
    { id: 'llama3.1:8b', name: 'Llama 3.1 8B' },
  ],
  openai: [
    { id: 'gpt-4o-mini', name: 'GPT-4o mini' },
    { id: 'gpt-4o', name: 'GPT-4o' },
  ],
  deepseek: [
    { id: 'deepseek-chat', name: 'DeepSeek Chat' },
    { id: 'deepseek-reasoner', name: 'DeepSeek Reasoner' },
  ],
}

/** 提供商定义（apiUrl 会被后端配置覆盖） */
const PROVIDER_DEFS: Array<Pick<CasyProvider, 'id' | 'name' | 'mode' | 'apiUrl'>> = [
  { id: 'ollama', name: 'Ollama 本地模型', mode: 'ollama', apiUrl: 'http://localhost:11434' },
  { id: 'openai', name: 'OpenAI', mode: 'openai', apiUrl: 'https://api.openai.com/v1' },
  { id: 'deepseek', name: 'DeepSeek', mode: 'openai', apiUrl: 'https://api.deepseek.com/v1' },
]

/**
 * 注册 AI 提供商：后端已配置的模式/模型并入对应提供商，未配置则用默认
 */
async function registerProviders(): Promise<void> {
  const cfg = await tauriCallSafe<{
    mode?: string
    apiUrl?: string | null
    apiKey?: string | null
    model?: string | null
  }>('get_ai_config', {})

  const configured = cfg.ok && cfg.data ? cfg.data : null

  const providers: CasyProvider[] = PROVIDER_DEFS.map((def) => {
    const models = [...(DEFAULT_MODELS[def.id] ?? [])]
    // 后端已配置模型并入对应提供商
    if (configured?.model && def.id === configured.mode) {
      if (!models.some((m) => m.id === configured.model)) {
        models.unshift({ id: configured.model!, name: configured.model! })
      }
    }
    // 后端 apiUrl 覆盖默认
    const apiUrl =
      configured?.apiUrl && configured.mode === def.id ? configured.apiUrl : def.apiUrl
    const apiKey = configured?.apiKey && configured.mode === def.id ? configured.apiKey : undefined
    return {
      ...def,
      apiUrl,
      apiKey,
      models,
    }
  })

  providers.forEach((p) => casyContext.registerProvider(p))
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
