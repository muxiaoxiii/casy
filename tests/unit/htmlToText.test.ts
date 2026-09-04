// @vitest-environment jsdom
import { describe, expect, it } from 'vitest'
import { htmlToText } from '../../src/shared/utils/htmlToText'

describe('htmlToText · 惰性文本提取（审查 P2-2）', () => {
  it('空 / 纯空白输入返回空串', () => {
    expect(htmlToText('')).toBe('')
    expect(htmlToText('   ')).toBe('')
    expect(htmlToText(undefined as unknown as string)).toBe('')
  })

  it('普通段落与行内标签提取为纯文本', () => {
    expect(htmlToText('<p>正文</p>')).toContain('正文')
    expect(htmlToText('<p>第 <strong>一</strong> 段</p>')).toBe('第 一 段')
  })

  it('不触发图片事件处理器（惰性文档，对比 innerHTML 的差异）', () => {
    ;(window as unknown as { __p2_2_fired?: boolean }).__p2_2_fired = undefined
    const text = htmlToText('<img src="x-nonexistent" onerror="window.__p2_2_fired = true">')
    expect((window as unknown as { __p2_2_fired?: boolean }).__p2_2_fired).toBeUndefined()
    // img 本身不贡献文本内容
    expect(text).toBe('')
  })

  it('script 标签内容不执行、仅按纯文本处理（不破坏周边文本提取）', () => {
    ;(window as unknown as { __p2_2_executed?: boolean }).__p2_2_executed = undefined
    // 用真实输入形态：脚本嵌在正文元素之间（编辑器的序列化结果即片段式 HTML）
    const text = htmlToText('<p>前文</p><script>window.__p2_2_executed = true;</script><p>后文</p>')
    expect((window as unknown as { __p2_2_executed?: boolean }).__p2_2_executed).toBeUndefined()
    // 周边正文照常提取；脚本仅作文本、不真正执行
    expect(text).toContain('前文')
    expect(text).toContain('后文')
  })
})
