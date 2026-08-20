/**
 * AI 工具调用器（真实实现）
 *
 * 设计哲学 §11.6 推荐闭环 + §原则六 双路径铁律：
 * - AI 只负责"判断"（选哪个工具、给什么参数），输出 JSON 信封
 * - 工具执行永远经 casyContext.executeTool → tauriBridge → Rust 命令
 *   （确定性路径，写入口唯一；写工具内部自带 Confirmer 确认）
 * - 审计：每次对话经后端 ai_chat 写 ai_runs（模型可见即记录，§11.9）
 */

import { casyContext } from '../plugin/context'
import { tauriCallSafe } from '../tauriBridge'
import type { CasyTool, CasyProvider, CasyModel } from '../plugin/types'

/** 工具循环最大轮数（防止模型无限调用工具） */
const MAX_TOOL_ROUNDS = 5

export interface ToolCallRecord {
  name: string
  params: Record<string, unknown>
}

export interface ChatWithToolsResult {
  content: string
  toolCalls: ToolCallRecord[]
  toolResults: Array<{ ok: boolean; data?: unknown; error?: string }>
}

interface ChatMessageLike {
  role: string
  content: string
}

/** 将 JSON Schema 摘要为模型可读的参数说明 */
function summarizeParams(tool: Pick<CasyTool, "name" | "description" | "parameters">): string {
  const p = tool.parameters
  if (!p || !p.properties) return tool.description || tool.name
  const props = Object.entries(p.properties)
    .map(([k, v]) => {
      const required = p.required?.includes(k) ? '（必填）' : ''
      return k + required + ': ' + (v.description || v.type || 'any')
    })
    .join('；')
  return props ? (tool.description || tool.name) + ' —— 参数: ' + props : tool.description || tool.name
}

/** 构建系统提示词：角色 + 工具清单 + 调用协议 */
function buildSystemPrompt(tools: CasyTool[]): string {
  const toolLines = tools
    .map((t) => {
      return '- ' + t.name + ': ' + summarizeParams(t)
    })
    .join('\n')

  return '你是 Casy AI 助手，帮助专利律师管理案件、任务、日历、收件箱、知识库与提醒。\n\n' +
    '你可以调用以下工具（当用户请求涉及这些能力时，你必须通过工具获取真实数据，不要编造）：\n' +
    (toolLines || '（暂无可用工具）') + '\n\n' +
    '## 工具调用协议\n' +
    '需要调用工具时，只输出一个 JSON 对象（不要输出任何其他文字、不要用 markdown 代码块）：\n' +
    '  {"tool": "工具名", "params": {参数对象}}\n' +
    '工具执行结果会以 [工具结果] 开头返回给你，你根据结果继续回答用户。\n' +
    '不需要调用工具时，直接用中文回答用户。\n' +
    '如果工具执行失败，如实告知用户失败原因。'
}

/** 从模型回复中解析工具调用 JSON；返回 null 表示这是最终答复 */
function tryParseToolCall(reply: string): { name: string; params: Record<string, unknown> } | null {
  if (!reply) return null

  const candidates: string[] = []
  const trimmed = reply.trim()
  // 1. 直接 JSON
  if (trimmed.startsWith('{') && trimmed.endsWith('}')) {
    candidates.push(trimmed)
  }
  // 2. fenced 代码块
  const fenceStart = trimmed.indexOf("```")
  const fenceEnd = trimmed.lastIndexOf("```")
  if (fenceStart >= 0 && fenceEnd > fenceStart + 3) {
    let fenced = trimmed.slice(fenceStart + 3, fenceEnd)
    if (fenced.startsWith("json")) fenced = fenced.slice(4)
    candidates.push(fenced.trim())
  }
  // 3. 第一个 {...} 片段
  const firstBrace = trimmed.indexOf('{')
  const lastBrace = trimmed.lastIndexOf('}')
  if (firstBrace >= 0 && lastBrace > firstBrace) {
    candidates.push(trimmed.slice(firstBrace, lastBrace + 1))
  }

  for (const c of candidates) {
    try {
      const obj = JSON.parse(c)
      const name = obj.tool ?? obj.tool_name ?? obj.function?.name ?? obj.tool_calls?.[0]?.function?.name
      if (!name) continue
      let params: Record<string, unknown> = {}
      if (obj.params && typeof obj.params === 'object') {
        params = obj.params
      } else if (obj.arguments && typeof obj.arguments === 'string') {
        params = JSON.parse(obj.arguments)
      } else if (obj.arguments && typeof obj.arguments === 'object') {
        params = obj.arguments
      } else if (obj.tool_calls?.[0]?.function?.arguments) {
        const args = obj.tool_calls[0].function.arguments
        params = typeof args === 'string' ? JSON.parse(args) : args
      }
      return { name: String(name), params }
    } catch {
      // 继续尝试下一个候选
    }
  }
  return null
}

/** 格式化工具执行结果（截断避免撑爆上下文） */
function formatToolResult(name: string, result: { ok: boolean; data?: unknown; error?: string }): string {
  if (!result.ok) {
    return '[' + name + '] 执行失败: ' + (result.error || '未知错误')
  }
  let text = ""
  try {
    text = JSON.stringify(result.data ?? null, null, 2)
  } catch {
    text = String(result.data ?? null)
  }
  const MAX = 4000
  if (text.length > MAX) {
    text = text.slice(0, MAX) + "…（已截断）"
  }
  return '[' + name + '] 执行成功:\n' + text
}

class AiToolCaller {
  private providerId = 'ollama'
  private modelId = 'qwen2.5:14b'

  /** 设置当前对话使用的提供商与模型 */
  setModel(providerId: string, modelId: string): void {
    this.providerId = providerId || this.providerId
    this.modelId = modelId || this.modelId
  }

  /** 当前可用的工具定义（与 casyContext 一致） */
  getAvailableTools() {
    return casyContext.getToolDefinitions()
  }

  /**
   * 多轮对话 + 工具调用循环
   *
   * @param messages 对话历史（不含 system；system 由本方法统一注入）
   */
  async chatWithTools(
    messages: ChatMessageLike[],
    _opts: { autoConfirm?: boolean } = {}
  ): Promise<ChatWithToolsResult> {
    const tools = casyContext.getTools()
    const provider = casyContext.getProviders().find((p) => p.id === this.providerId)

    const history: ChatMessageLike[] = [
      { role: "system", content: buildSystemPrompt(tools) },
      ...messages.filter((m) => m.role !== "system"),
    ]

    const toolCalls: ToolCallRecord[] = []
    const toolResults: Array<{ ok: boolean; data?: unknown; error?: string }> = []
    let content = ''
    let loopExhausted = false

    for (let round = 0; round < MAX_TOOL_ROUNDS; round++) {
      const reply = await this.chat(history, provider)

      const call = tryParseToolCall(reply)
      if (!call) {
        content = reply // 最终答复
        break
      }

      if (round === MAX_TOOL_ROUNDS - 1) {
        // 最后一轮仍是工具调用：循环耗尽，给出汇总而非原始 JSON
        loopExhausted = true
        break
      }

      const tool = casyContext.getTool(call.name)
      if (!tool) {
        history.push({ role: "assistant", content: reply })
        history.push({
          role: "user",
          content: '[工具结果 ' + call.name + '] 工具不存在，请从工具清单中选择。',
        })
        continue
      }

      // 执行工具（写工具内部自动走确认流程）
      const result = await casyContext.executeTool(call.name, call.params)
      toolCalls.push({ name: call.name, params: call.params })
      toolResults.push(result)

      history.push({ role: "assistant", content: reply })
      history.push({
        role: "user",
        content: formatToolResult(call.name, result),
      })
    }

    if (loopExhausted && !content) {
      content = toolCalls.length > 0
        ? '已连续执行 ' + toolCalls.length + ' 次工具调用，如需继续请告诉我下一步。'
        : '工具调用次数已达上限，请换个说法重试。'
    }

    return { content, toolCalls, toolResults }
  }

  /** 调用后端多轮对话命令（含 ai_runs 审计） */
  private async chat(
    history: ChatMessageLike[],
    provider: CasyProvider | undefined
  ): Promise<string> {
    const result = await tauriCallSafe<string>('ai_chat', {
      messages: history,
      mode: provider?.mode,
      apiUrl: provider?.apiUrl,
      model: this.modelId,
      purpose: 'ai_chat_panel',
    })
    if (!result.ok) {
      throw new Error(result.error || 'AI 调用失败（请检查设置中的 AI 后端配置）')
    }
    return result.data ?? ""
  }
}

/** 全局 AI 工具调用器实例 */
export const aiToolCaller = new AiToolCaller()

/** 导出类型（供 UI 使用） */
export type { CasyModel, CasyProvider }