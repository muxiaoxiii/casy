/**
 * AI 工具调用器（占位）
 * TODO: 对接 Tauri 后端 AI 工具系统
 */
export const aiToolCaller = {
  async call(toolName: string, params: Record<string, unknown>) {
    // TODO: 对接后端后在此分发 toolName/params
    void toolName
    void params
    return { ok: true, data: null }
  },

  getAvailableTools() {
    return []
  },
}
