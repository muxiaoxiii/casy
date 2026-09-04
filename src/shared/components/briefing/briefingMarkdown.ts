/**
 * BriefingModal 简报正文的轻量 Markdown → HTML 渲染器。
 *
 * 这是一个纯字符串函数：输入 Markdown 字符串，输出受控的白名单 HTML 片段，
 * 供组件内 `.markdown-body` 使用 `v-html` 渲染。它不依赖 DOM。
 *
 * 设计约束（触发本拆分的原因之一）：原本它内联在组件内，无法被独立单测，
 * 且对输出标签有隐式的结构约定（`brief-p` / `brief-ul` / `brief-h3`…），
 * 这些约定与组件 CSS 深度绑定。抽出后既能在 Node 环境验证内联/转义/列表行为，
 * 也把「生成 HTML」与「组件渲染」两个关注点分开。
 *
 * 支持的语法（刻意裁剪，足够覆盖简报正文）：
 * - 行内：`**加粗**`；HTML 特殊字符一律转义（对内容做 HTML 消毒，不做信任假设）。
 * - 块级：`### ` / `## ` / `# ` 标题；`- ` / `* ` 列表项；其余非空行为段落。
 */
export function renderBriefingMarkdown(markdown: string): string {
  if (!markdown) return ''
  const escapeHtml = (value: string) => value.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
  const inline = (value: string) => escapeHtml(value).replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
  let html = ''
  let inList = false

  for (const line of markdown.split('\n')) {
    const text = line.trim()
    if (text.startsWith('- ') || text.startsWith('* ')) {
      if (!inList) {
        html += '<ul class="brief-ul">'
        inList = true
      }
      html += `<li class="brief-li">${inline(text.slice(2))}</li>`
      continue
    }
    if (inList) {
      html += '</ul>'
      inList = false
    }
    if (!text) continue
    if (text.startsWith('### ')) html += `<h5 class="brief-h5">${inline(text.slice(4))}</h5>`
    else if (text.startsWith('## ')) html += `<h4 class="brief-h4">${inline(text.slice(3))}</h4>`
    else if (text.startsWith('# ')) html += `<h3 class="brief-h3">${inline(text.slice(2))}</h3>`
    else html += `<p class="brief-p">${inline(text)}</p>`
  }
  if (inList) html += '</ul>'
  return html
}
