// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest'
import { mount, type VueWrapper } from '@vue/test-utils'
import { nextTick } from 'vue'
import MarkdownWysiwygEditor from '../../src/modules/knowledge/components/MarkdownWysiwygEditor.vue'

let wrapper: VueWrapper | null = null

afterEach(() => {
  wrapper?.unmount()
  wrapper = null
})

describe('MarkdownWysiwygEditor', () => {
  async function waitForEditor() {
    for (let index = 0; index < 10; index += 1) {
      await new Promise(resolve => setTimeout(resolve, 0))
      await nextTick()
      if (wrapper?.find('.tiptap').exists()) return
    }
  }

  it('载入并 flush 时保留 Markdown 图片', async () => {
    wrapper = mount(MarkdownWysiwygEditor, {
      attachTo: document.body,
      props: {
        modelValue: '图片前\n\n![证据截图](asset://local/evidence.png "原件")\n\n图片后',
      },
    })
    await waitForEditor()

    const image = wrapper.find('.tiptap img')
    expect(image.exists()).toBe(true)
    expect(image.attributes('src')).toBe('asset://local/evidence.png')
    expect(image.attributes('alt')).toBe('证据截图')

    const markdown = (wrapper.vm as unknown as { flushAndGetMarkdown: () => string })
      .flushAndGetMarkdown()
    expect(markdown).toContain('![证据截图](asset://local/evidence.png "原件")')
  })

  it('flushAndGetMarkdown 返回当前编辑器内容而不是旧 prop', async () => {
    wrapper = mount(MarkdownWysiwygEditor, {
      attachTo: document.body,
      props: { modelValue: '旧内容' },
    })
    await waitForEditor()

    const api = wrapper.vm as unknown as {
      setMarkdown: (value: string) => string
      flushAndGetMarkdown: () => string
    }
    api.setMarkdown('新内容')
    expect(api.flushAndGetMarkdown()).toBe('新内容')
  })

  it('为原生导出和目录提供当前结构化文档', async () => {
    wrapper = mount(MarkdownWysiwygEditor, {
      attachTo: document.body,
      props: { modelValue: '# 一级标题\n\n正文\n\n## 二级标题' },
    })
    await waitForEditor()

    const api = wrapper.vm as unknown as {
      getDocumentJson: () => { type: string; content?: Array<{ type: string }> }
      getOutline: () => Array<{ id: string; level: number; text: string }>
    }
    const documentJson = api.getDocumentJson()
    const outline = api.getOutline()

    expect(documentJson.type).toBe('doc')
    expect(documentJson.content?.some(node => node.type === 'heading')).toBe(true)
    expect(outline.map(item => [item.level, item.text])).toEqual([
      [1, '一级标题'],
      [2, '二级标题'],
    ])
  })
})
