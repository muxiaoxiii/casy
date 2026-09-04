// @vitest-environment jsdom
import { describe, expect, it } from 'vitest'
import {
  mdToHtml,
  htmlToMd,
  mdToSafeHtml,
  sanitizeInlineHtml,
  sanitizePreviewHtml,
} from '../../src/shared/markdown/mdBridge'

/** 语义等价断言：去掉首尾空白后比较，允许内部空白差异 */
function expectRoundTrip(md: string, expected?: string) {
  const back = htmlToMd(mdToHtml(md))
  expect(back.trim()).toBe((expected ?? md).trim())
}

describe('mdBridge · mdToHtml', () => {
  it('wiki 链接转为 WikiLink 节点 HTML（与扩展 parseHTML 对齐）', () => {
    const html = mdToHtml('参考 [[专利法实施细则]] 与 [[无效宣告请求审查指南]]')
    expect(html).toContain('data-wiki-link')
    expect(html).toContain('data-title="专利法实施细则"')
    expect(html).toContain('data-title="无效宣告请求审查指南"')
  })

  it('行内代码中的 [[ ]] 不转为 wiki 链接', () => {
    const html = mdToHtml('`[[不是链接]]`')
    expect(html).not.toContain('data-wiki-link')
    expect(html).toContain('[[不是链接]]')
  })

  it('GFM 表格与任务列表开启', () => {
    const html = mdToHtml('| A | B |\n|---|---|\n| 1 | 2 |\n\n- [x] 已完成\n- [ ] 未完成')
    expect(html).toContain('<table>')
    expect(html).toContain('type="checkbox"')
    expect(html).toContain('checked')
  })

  it('wiki 标题中的 HTML 特殊字符只转义一次', () => {
    const html = mdToHtml('参考 [[A & B]]')
    const box = document.createElement('div')
    box.innerHTML = html
    const link = box.querySelector('span[data-wiki-link]')
    expect(link?.textContent).toBe('A & B')
    expect(link?.getAttribute('data-title')).toBe('A & B')
  })
})

describe('mdBridge · htmlToMd', () => {
  it('WikiLink 节点 HTML 还原为 [[标题]]', () => {
    const md = htmlToMd('<p>见 <span data-wiki-link="" data-title="证据规则">证据规则</span> 第三条</p>')
    expect(md).toContain('[[证据规则]]')
  })

  it('图片节点还原为 Markdown 图片并保留 alt/title', () => {
    expect(htmlToMd('<p>前</p><img src="asset://local/evidence.png" alt="证据" title="截图"><p>后</p>'))
      .toContain('![证据](asset://local/evidence.png "截图")')
  })

  it('tiptap TaskItem 结构（li[data-type=taskItem]）还原为任务列表', () => {
    const html =
      '<ul data-type="taskList">' +
      '<li data-type="taskItem" data-checked="true"><label><input type="checkbox" checked><span></span></label><div><p>写答辩状</p></div></li>' +
      '<li data-type="taskItem" data-checked="false"><label><input type="checkbox"><span></span></label><div><p>整理证据</p></div></li>' +
      '</ul>'
    const md = htmlToMd(html)
    expect(md).toContain('- [x] 写答辩状')
    expect(md).toContain('- [ ] 整理证据')
  })

  it('tiptap 表格（tbody 内首行 th，无 thead）还原为 GFM 表格', () => {
    const html =
      '<table><tbody>' +
      '<tr><th><p>名称</p></th><th><p>数量</p></th></tr>' +
      '<tr><td><p>证据一</p></td><td><p>3</p></td></tr>' +
      '</tbody></table>'
    const md = htmlToMd(html)
    expect(md).toContain('| 名称 | 数量 |')
    expect(md).toContain('| --- | --- |')
    expect(md).toContain('| 证据一 | 3 |')
  })

  it('未知 HTML 标签透传不丢内容', () => {
    const md = htmlToMd('<p>前文 <custom-widget data-x="1">内部文字</custom-widget> 后文</p>')
    expect(md).toContain('内部文字')
    expect(md).toContain('前文')
    expect(md).toContain('后文')
  })
})

describe('mdBridge · 往返保真', () => {
  it('标题与段落', () => {
    expectRoundTrip('# 一级标题\n\n## 二级标题\n\n正文段落。')
  })

  it('粗体 / 斜体 / 删除线 / 行内码', () => {
    expectRoundTrip('这是 **粗体** 和 *斜体* 以及 ~~删除线~~ 与 `code` 的组合。')
  })

  it('代码块（含语言）', () => {
    expectRoundTrip('```js\nconst a = 1\nconsole.log(a)\n```')
  })

  it('无序列表与嵌套列表', () => {
    // 嵌套项序列化为 4 空格缩进（turndown 约定），再次往返幂等
    expectRoundTrip(
      '- 第一项\n- 第二项\n  - 子项甲\n  - 子项乙\n- 第三项',
      '- 第一项\n- 第二项\n    - 子项甲\n    - 子项乙\n- 第三项',
    )
  })

  it('有序列表', () => {
    expectRoundTrip('1. 第一步\n2. 第二步\n3. 第三步')
  })

  it('任务列表', () => {
    expectRoundTrip('- [x] 已完成事项\n- [ ] 待办事项')
  })

  it('引用块', () => {
    expectRoundTrip('> 专利权的保护范围以其权利要求的内容为准。')
  })

  it('链接', () => {
    expectRoundTrip('详见 [国家知识产权局](https://www.cnipa.gov.cn) 官网。')
  })

  it('分隔线', () => {
    expectRoundTrip('上文\n\n---\n\n下文')
  })

  it('表格', () => {
    expectRoundTrip('| 当事人 | 角色 |\n| --- | --- |\n| 张三 | 原告 |\n| 李四 | 被告 |')
  })

  it('wiki 链接往返', () => {
    expectRoundTrip('参见 [[专利法]] 与 [[审查指南]] 的规定。')
  })

  it('图片往返', () => {
    expectRoundTrip('图片：\n\n![证据截图](asset://local/evidence.png "原件")')
  })

  it('暂不支持的原始 HTML 以占位节点往返，不静默删除', () => {
    const source = '正文前 <kbd data-key="Enter">Enter</kbd> 正文后\n\n<div data-custom="x">块内容</div>'
    const editorHtml = mdToHtml(source)
    expect(editorHtml).toContain('data-raw-html')
    expect(htmlToMd(editorHtml)).toBe(source)
  })

  it('混合文档（标题+列表+代码+表格+wiki）', () => {
    const md = [
      '# 案件分析',
      '',
      '## 争议焦点',
      '',
      '参见 [[庭审笔录]] 的记录。',
      '',
      '- 焦点一：**创造性** 判断',
      '- 焦点二：`权利要求1` 解释',
      '',
      '```\n证据清单核对\n```',
      '',
      '| 证据 | 证明目的 |',
      '| --- | --- |',
      '| 公证书 | 在先公开 |',
      '',
      '> 注意举证期限。',
      '',
      '---',
      '',
      '以上。',
    ].join('\n')
    const back = htmlToMd(mdToHtml(md))
    // 结构不丢：逐项断言关键语义标记存在
    expect(back).toContain('# 案件分析')
    expect(back).toContain('## 争议焦点')
    expect(back).toContain('[[庭审笔录]]')
    expect(back).toContain('- 焦点一：**创造性** 判断')
    expect(back).toContain('`权利要求1`')
    expect(back).toContain('证据清单核对')
    expect(back).toContain('| 证据 | 证明目的 |')
    expect(back).toContain('| 公证书 | 在先公开 |')
    expect(back).toContain('> 注意举证期限。')
    expect(back).toContain('---')
    expect(back).toContain('以上。')
  })
})

describe('mdBridge · 安全预览', () => {
  it.each([
    '<a href=javascript:alert(1)>未加引号</a>',
    '<a href="java&#x73;cript:alert(1)">实体编码</a>',
    '<a href="JaVaScRiPt:alert(1)">大小写混淆</a>',
    '<svg><a href="javascript:alert(1)">SVG</a></svg>',
    '<img src=x onerror=alert(1)>',
  ])('移除危险 HTML：%s', (payload) => {
    const safe = mdToSafeHtml(payload)
    const box = document.createElement('div')
    box.innerHTML = safe
    expect(safe.toLowerCase()).not.toContain('javascript:')
    expect(box.querySelector('[onerror],[onclick],[onload]')).toBeNull()
    expect(box.querySelector('svg')).toBeNull()
  })

  it('保留正常 GFM、WikiLink 与安全链接', () => {
    const safe = mdToSafeHtml('| A | B |\n|---|---|\n| 1 | 2 |\n\n[[证据规则]]\n\n[官网](https://example.com)')
    expect(safe).toContain('<table>')
    expect(safe).toContain('data-wiki-link')
    expect(safe).toContain('https://example.com')
  })
})

describe('mdBridge · sanitizeInlineHtml（首页 AI 建议 / FTS5 snippet 行内片段）', () => {
  // 首页 AI 建议文本以模板字面量生成 <strong>，正文部分来自任务名/案件名（跨信任边界）。
  // 审查 P2-1：HomeView/KnowledgeSidebar 将其预计算为 computed。
  it('保留展示型标签 <strong>/<b>，用于高亮', () => {
    const safe = sanitizeInlineHtml('检测到任务<strong>「专利无效答辩」</strong>需要处理')
    expect(safe).toContain('<strong>「专利无效答辩」</strong>')
  })

  it('剥离一切属性与事件（含 onclick/事件属性）', () => {
    const safe = sanitizeInlineHtml('<strong onclick="alert(1)">文本</strong><b style="color:red">加粗</b>')
    const box = document.createElement('div')
    box.innerHTML = safe
    expect(box.querySelector('[onclick],[onload],[style]')).toBeNull()
    expect(safe).toContain('文本')
    expect(safe).toContain('加粗')
  })

  it('移除危险标签（img/script/svg）与事件载荷', () => {
    const safe = sanitizeInlineHtml('片段 <img src=x onerror=alert(1)> <script>alert(1)</script>')
    expect(safe).not.toContain('onerror')
    expect(safe).not.toContain('<script')
    expect(safe).not.toContain('<img')
  })
})

describe('mdBridge · sanitizePreviewHtml（文书模板渲染结果 HTML 预览）', () => {
  // DocumentGenView 的 HTML 预览 tab 走此函数（审查 P2-1：已预计算为 computed）。
  it('保留白名单内标签（table/img/h1）', () => {
    const safe = sanitizePreviewHtml('<h1>标题</h1><table><tr><td>单元格</td></tr></table><img src="asset://x.png" alt="图">')
    expect(safe).toContain('<h1>标题</h1>')
    expect(safe).toContain('<table>')
    expect(safe).toContain('单元格')
  })

  it('移除脚本、事件属性与危险协议链接', () => {
    const safe = sanitizePreviewHtml('<script>alert(1)</script><a href="javascript:alert(1)">点我</a><img src=x onerror=alert(1)>')
    expect(safe).not.toContain('<script')
    expect(safe).not.toContain('javascript:')
    expect(safe).not.toContain('onerror')
  })
})
