/**
 * Casy 插件上下文（占位）
 * TODO: 对接 Tauri 后端插件系统
 */
export const casyContext = {
  getTools(): Array<{ name: string; category: string }> {
    return []
  },
  getTool(name: string) {
    return null
  },
}
