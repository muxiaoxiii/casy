/**
 * 将 HTML 字符串安全地提取为纯文本。
 *
 * 安全要点（审查 P2-2）：不要再用「临时元素.innerHTML = html」再读 textContent——
 * innerHTML 会把输入当作可执行 HTML 处理，含 `<img onerror>` / 活性脚本时可能触发执行。
 * `DOMParser.parseFromString(html, 'text/html')` 返回惰性文档：不执行脚本、不触发图片加载，
 * 即使输入是任意 / 跨信任边界数据也仅作纯文本处理。
 *
 * @param html 任意 HTML 字符串（可为空 / 纯空白）
 * @returns 提取的纯文本；空或纯空白输入返回 ''
 */
export function htmlToText(html: string): string {
  if (!html || !html.trim()) return ''
  const doc = new DOMParser().parseFromString(html, 'text/html')
  return doc.body.textContent || ''
}
