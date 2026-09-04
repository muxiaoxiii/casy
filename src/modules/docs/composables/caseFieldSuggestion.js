import { Extension } from '@tiptap/core'
import { PluginKey } from '@tiptap/pm/state'
import { Suggestion } from '@tiptap/suggestion'

function getSuggestionItems({ query, editor }) {
  const caseData = editor.storage.caseData || {}

  const fields = [
    { label: '案号', value: caseData.caseNo || '' },
    { label: '案件名称', value: caseData.caseName || '' },
    { label: '客户名称', value: caseData.clientName || '' },
    { label: '我方地位', value: caseData.ourRole || '' },
    { label: '对方名称', value: caseData.opponentName || '' },
    { label: '对方地位', value: caseData.opponentRole || '' },
    { label: '审理机关', value: caseData.court || '' },
    { label: '案由', value: caseData.causeAction || '' },
    { label: '专利名称', value: caseData.patentName || '' },
    { label: '专利申请号', value: caseData.patentAppNo || '' },
    { label: '内部卷号', value: caseData.internalNo || '' },
    { label: '今日日期', value: new Date().toLocaleDateString('zh-CN') },
    { label: '办案人', value: (caseData.attorneys || []).join('、') },
  ]

  return fields
    .filter(f => f.label.includes(query) || f.value.includes(query))
    .map(f => ({
      ...f,
      command: ({ editor, range }) => {
        editor.chain().focus().deleteRange(range).insertContent(f.value).run()
      },
    }))
}

export const CaseFieldSuggestion = Extension.create({
  name: 'caseFieldSuggestion',
  addOptions() {
    return {
      suggestion: {
        char: '{',
        items: getSuggestionItems,
        render: () => {
          let popup, items

          function renderItems(props) {
            if (!popup) return
            items = props.items
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
              min-width: 200px;
            `

            const rect = props.clientRect?.()
            if (rect) {
              popup.style.top = `${rect.bottom + 4}px`
              popup.style.left = `${rect.left}px`
            }

            items.forEach((item, index) => {
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
              div.textContent = item.label
              if (item.value) {
                const span = document.createElement('span')
                span.style.cssText = 'color: #999; font-size: 12px; margin-left: auto;'
                span.textContent = item.value.slice(0, 20)
                div.appendChild(span)
              }
              div.addEventListener('click', () => {
                props.command(item)
              })
              popup.appendChild(div)
            })

            if (items.length === 0) {
              const div = document.createElement('div')
              div.style.cssText = 'padding: 6px 12px; color: #999; font-size: 13px;'
              div.textContent = '无匹配字段'
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
    return [Suggestion({ editor: this.editor, ...this.options.suggestion, pluginKey: new PluginKey('caseFieldSuggestion') })]
  },
})
