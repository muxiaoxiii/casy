// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { mount, type VueWrapper } from '@vue/test-utils'
import { nextTick } from 'vue'
import MarkdownWysiwygEditor from '../../src/modules/knowledge/components/MarkdownWysiwygEditor.vue'

let wrapper: VueWrapper | null = null
beforeEach(() => setActivePinia(createPinia()))

afterEach(() => {
  wrapper?.unmount()
  wrapper = null
})

describe('MarkdownWysiwygEditor', () => {
  it('不编辑正文时保留原始 Markdown 格式', async () => {
    const source = '# 标题\n\n* 第一项\n* 第二项\n\n正文  \n换行\n'
    wrapper = mount(MarkdownWysiwygEditor, { props: { modelValue: source } })
    await waitForEditor()
    expect((wrapper.vm as unknown as { flushAndGetMarkdown: () => string }).flushAndGetMarkdown()).toBe(source)
    expect(wrapper.emitted('update:modelValue')).toBeUndefined()
  })
  async function waitForEditor() {
    for (let index = 0; index < 10; index += 1) {
      await new Promise(resolve => setTimeout(resolve, 0))
      await nextTick()
      if (wrapper?.find('.tiptap').exists()) return
    }
  }

  it('立即恢复外部正文时保留权威 Markdown，包括仅格式不同的替换', async () => {
    wrapper = mount(MarkdownWysiwygEditor, { props: { modelValue: '原文' } })
    await waitForEditor()
    const api = wrapper.vm as unknown as { setMarkdown: (value: string) => string; flushAndGetMarkdown: () => string }
    api.setMarkdown('刚编辑的正文')
    await wrapper.setProps({ modelValue: '* 恢复项目\n' })
    expect(api.flushAndGetMarkdown()).toBe('* 恢复项目\n')
    await wrapper.setProps({ modelValue: '- 恢复项目' })
    expect(api.flushAndGetMarkdown()).toBe('- 恢复项目')
  })

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

describe('统一编辑器的实际内容往返',()=>{
 async function open(content:string,extra:Record<string,unknown>={}){
  wrapper=mount(MarkdownWysiwygEditor,{attachTo:document.body,props:{modelValue:content,...extra}})
  for(let i=0;i<15;i++){await new Promise(resolve=>setTimeout(resolve,0));if((wrapper.vm as any).getEditor?.())break}
  return (wrapper.vm as any)
 }
 it('待办勾选及任务标识在编辑、保存、重开后保持',async()=>{
  const api=await open('- [ ] 核对材料\n- [x] 已经完成 <!--casy-task:task-123-->')
  let json=api.getDocumentJson();expect(json.content[0].type).toBe('taskList');expect(json.content[0].content[1].attrs).toMatchObject({checked:true,taskId:'task-123'})
  const ed=api.getEditor();ed.commands.insertContentAt(3,'补充');const saved=api.flushAndGetMarkdown();expect(saved).toContain('- [x]');expect(saved).toContain('<!--casy-task:task-123-->')
  wrapper!.unmount();wrapper=null;const reopened=await open(saved);json=reopened.getDocumentJson();expect(json.content[0].content[1].attrs).toMatchObject({checked:true,taskId:'task-123'})
 })
 it('实际待办 NodeView 有布局标记，勾选后仍保留同级标签与正文容器',async()=>{
  const api=await open('- [ ] 很长的材料核查事项\n    - [x] 子事项')
  const row=wrapper!.find('.tiptap li[data-type="taskItem"]')
  expect(row.exists()).toBe(true)
  expect(row.element.children[0].tagName).toBe('LABEL')
  expect(row.element.children[1].tagName).toBe('DIV')
  await row.find('input[type="checkbox"]').setValue(true)
  expect(row.attributes('data-type')).toBe('taskItem')
  expect(row.attributes('data-checked')).toBe('true')
  expect(api.flushAndGetMarkdown()).toContain('- [x] 很长')
  expect(row.find('div ul[data-type="taskList"] li[data-type="taskItem"]').exists()).toBe(true)
 })
 it('调整图片尺寸后序列化和重开不会恢复原大小',async()=>{
  const api=await open('![截图](data:image/png;base64,AA==)\n\n图片后正文')
  const ed=api.getEditor();let imagePos=0;ed.state.doc.descendants((n:any,p:number)=>{if(n.type.name==='image')imagePos=p});ed.commands.setNodeSelection(imagePos);ed.commands.updateAttributes('image',{width:260});const saved=api.flushAndGetMarkdown();expect(saved).toContain('width="260"')
  wrapper!.unmount();wrapper=null;const reopened=await open(saved);expect(reopened.getDocumentJson().content.find((n:any)=>n.type==='image')).toMatchObject({type:'image',attrs:{width:260}});expect(reopened.getHtml()).toContain('图片后正文')
 })
 it('空白文档的提示是占位属性，不进入正文或导出树',async()=>{
  const api=await open('',{placeholder:'仅供提示，不是正文'})
  expect(api.flushAndGetMarkdown()).toBe('');expect(JSON.stringify(api.getDocumentJson())).not.toContain('仅供提示');expect(api.getHtml()).not.toContain('仅供提示')
 })
 it('文书与笔记使用同一工具栏，列表和表格是独立操作',async()=>{
  const api=await open('<p>文书正文</p>',{contentFormat:'html'})
  expect(wrapper!.find('[aria-label="更多编辑工具"]').exists()).toBe(true)
  expect(wrapper!.find('[aria-label="正文编辑工具"]').exists()).toBe(true)
  api.getEditor().commands.toggleBulletList();expect(api.getDocumentJson().content[0].type).toBe('bulletList')
  expect(api.flushAndGetMarkdown()).toContain('<ul>')
 })
})

describe('富文本属性不会在 Markdown 往返中丢失',()=>{
 it('图片、合并表格、对齐和证据引用使用可保留属性的 Markdown HTML',async()=>{
  const {documentFromContent}=await import('../../src/shared/editor/schema')
  const {htmlToMd}=await import('../../src/shared/markdown/mdBridge')
  const html='<p style="text-align:center">居中标题</p><table><tbody><tr><td colspan="2"><p>合并内容</p></td></tr></tbody></table><p><span data-evidence-link="" data-target-type="file" data-target-id="file-a" data-case-id="case-a" data-anchor="page:2" data-label="原件">原件</span></p>'
  const json=documentFromContent(htmlToMd(html))
  expect(json.content[0].attrs.textAlign).toBe('center');expect(json.content.find((n:any)=>n.type==='table').content[0].content[0].attrs.colspan).toBe(2)
  expect(JSON.stringify(json)).toContain('file-a');expect(JSON.stringify(json)).toContain('page:2')
 })
})
