import { Extension } from '@tiptap/core'
import { Suggestion } from '@tiptap/suggestion'

function getSuggestionItems({ query, editor }) {
  // 从 editor storage 获取所有当事人
  const caseData = editor.storage.caseData || {}
  const allCases = editor.storage.allCases || []

  const parties = new Set()
  // 当前案件的当事人
  if (caseData.clientName) parties.add(caseData.clientName)
  if (caseData.opponentName) parties.add(caseData.opponentName)
  if (caseData.opponentAgent) parties.add(caseData.opponentAgent)
  // 所有案件的当事人
  for (const c of allCases) {
    if (c.clientName) parties.add(c.clientName)
    if (c.opponentName) parties.add(c.opponentName)
  }

  return Array.from(parties)
    .filter(name => name.includes(query))
    .map(name => ({
      label: name,
      icon: '👤',
      command: ({ editor, range }) => {
        editor.chain().focus().deleteRange(range).insertContent(name).run()
      },
    }))
}

export const PartyNameSuggestion = Extension.create({
  name: 'partyNameSuggestion',
  addOptions() {
    return {
      suggestion: {
        char: '@',
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
              min-width: 180px;
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
              div.textContent = '无匹配当事人'
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
