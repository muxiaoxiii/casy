import { Extension } from '@tiptap/core'
import { Suggestion } from '@tiptap/suggestion'

// 本地法条数据库（常用专利法条）
const PATENT_LAW = [
  { article: '第2条', title: '发明创造的定义', law: '专利法' },
  { article: '第22条', title: '授予专利权的条件（新颖性、创造性、实用性）', law: '专利法' },
  { article: '第23条', title: '外观设计的授权条件', law: '专利法' },
  { article: '第25条', title: '不授予专利权的客体', law: '专利法' },
  { article: '第26条', title: '说明书和权利要求书', law: '专利法' },
  { article: '第33条', title: '修改不得超范围', law: '专利法' },
  { article: '第42条', title: '专利权期限', law: '专利法' },
  { article: '第45条', title: '无效宣告请求', law: '专利法' },
  { article: '第46条', title: '无效宣告审查决定', law: '专利法' },
  { article: '第47条', title: '无效宣告的效力', law: '专利法' },
  { article: '第59条', title: '保护范围', law: '专利法' },
  { article: '第64条', title: '权利要求的解释', law: '专利法' },
  { article: '第65条', title: '损害赔偿', law: '专利法' },
  { article: '第71条', title: '诉前保全', law: '专利法' },
  // 专利法实施细则
  { article: '第11条', title: '发明人或设计人', law: '专利法实施细则' },
  { article: '第14条', title: '专利申请权和专利权转让', law: '专利法实施细则' },
  { article: '第65条', title: '无效宣告请求的理由', law: '专利法实施细则' },
  { article: '第69条', title: '无效宣告程序', law: '专利法实施细则' },
  // 审查指南
  { article: '第二部分第四章', title: '创造性审查', law: '审查指南' },
  { article: '第二部分第三章', title: '新颖性审查', law: '审查指南' },
  { article: '第四部分第三章', title: '无效宣告请求审查', law: '审查指南' },
]

function getSuggestionItems({ query }) {
  return PATENT_LAW
    .filter(l =>
      l.article.includes(query) ||
      l.title.includes(query) ||
      l.law.includes(query)
    )
    .map(l => ({
      label: `《${l.law}》${l.article} ${l.title}`,
      icon: '📜',
      command: ({ editor, range }) => {
        editor.chain().focus().deleteRange(range).insertContent(`《${l.law}》${l.article}`).run()
      },
    }))
}

export const LegalProvisionSuggestion = Extension.create({
  name: 'legalProvisionSuggestion',
  addOptions() {
    return {
      suggestion: {
        char: '【',
        items: getSuggestionItems,
        render: () => {
          let popup

          function renderItems(props) {
            if (!popup) return
            popup.innerHTML = ''
            popup.style.cssText = `
              position: fixed;
              background: white;
              border: 1px solid #e0e0e0;
              border-radius: 6px;
              box-shadow: 0 4px 12px rgba(0,0,0,0.15);
              max-height: 200px;
              overflow-y: auto;
              z-index: 9999;
              min-width: 280px;
            `

            const rect = props.clientRect?.()
            if (rect) {
              popup.style.top = `${rect.bottom + 4}px`
              popup.style.left = `${rect.left}px`
            }

            props.items.forEach((item, index) => {
              const div = document.createElement('div')
              div.style.cssText = `
                padding: 6px 12px;
                cursor: pointer;
                display: flex;
                align-items: center;
                gap: 8px;
                font-size: 13px;
                ${index === props.selected ? 'background: #ecf5ff; color: #409eff;' : ''}
              `
              div.textContent = `${item.icon} ${item.label}`
              div.addEventListener('click', () => props.command(item))
              popup.appendChild(div)
            })

            if (props.items.length === 0) {
              const div = document.createElement('div')
              div.style.cssText = 'padding: 6px 12px; color: #999; font-size: 13px;'
              div.textContent = '无法条匹配'
              popup.appendChild(div)
            }
          }

          return {
            onStart(props) {
              popup = document.createElement('div')
              document.body.appendChild(popup)
              renderItems(props)
            },
            onUpdate(props) {
              renderItems(props)
            },
            onKeyDown(props) {
              if (props.event.key === 'Escape') {
                popup?.remove()
                popup = null
                return true
              }
              return false
            },
            onExit() {
              popup?.remove()
              popup = null
            },
          }
        },
      },
    }
  },
  addProseMirrorPlugins() {
    return [Suggestion({ editor: this.editor, ...this.options.suggestion })]
  },
})
