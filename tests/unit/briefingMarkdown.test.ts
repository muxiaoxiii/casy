import { describe, expect, it } from 'vitest'
import { renderBriefingMarkdown } from '../../src/shared/components/briefing/briefingMarkdown'

describe('renderBriefingMarkdown · 简报正文渲染', () => {
  it('空 / 纯空白输入返回空串', () => {
    expect(renderBriefingMarkdown('')).toBe('')
    expect(renderBriefingMarkdown('   ')).toBe('')
  })

  it('普通行渲染为段落并保持 class 约定', () => {
    expect(renderBriefingMarkdown('一行正文')).toBe('<p class="brief-p">一行正文</p>')
  })

  it('行内加粗解析为 <strong>', () => {
    expect(renderBriefingMarkdown('**重点**')).toBe('<p class="brief-p"><strong>重点</strong></p>')
  })

  it('多级标题分别映射到 h3/h4/h5', () => {
    expect(renderBriefingMarkdown('# 一级')).toBe('<h3 class="brief-h3">一级</h3>')
    expect(renderBriefingMarkdown('## 二级')).toBe('<h4 class="brief-h4">二级</h4>')
    expect(renderBriefingMarkdown('### 三级')).toBe('<h5 class="brief-h5">三级</h5>')
  })

  it('连续列表项合并进同一个 ul（- 与 * 均可）', () => {
    expect(renderBriefingMarkdown('- 甲\n* 乙\n- 丙')).toBe(
      '<ul class="brief-ul"><li class="brief-li">甲</li><li class="brief-li">乙</li><li class="brief-li">丙</li></ul>',
    )
  })

  it('列表结束后回到段落会正确闭合 ul', () => {
    const html = renderBriefingMarkdown('- 甲\n\n正文')
    expect(html).toContain('</ul><p class="brief-p">正文</p>')
  })

  it('对 HTML 特殊字符一律转义，不把内容当可执行 HTML 处理', () => {
    const sneaky = '<img src="x" onerror="window.__xss=1">\n<script>alert(1)</script>'
    const html = renderBriefingMarkdown(sneaky)
    expect(html).not.toContain('<img')
    expect(html).not.toContain('<script')
    expect(html).toContain('&lt;img')
    expect(html).toContain('&lt;script&gt;')
  })

  it('加粗发生在转义之后，等宽/特殊字符不受影响', () => {
    expect(renderBriefingMarkdown('**a < b**')).toBe('<p class="brief-p"><strong>a &lt; b</strong></p>')
  })
})
