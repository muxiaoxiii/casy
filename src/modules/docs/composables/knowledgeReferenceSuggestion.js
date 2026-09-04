import { Extension } from '@tiptap/core'
import { PluginKey } from '@tiptap/pm/state'
import { Suggestion } from '@tiptap/suggestion'
import { casyContext } from '../../../core/plugin/context'

async function getSuggestionItems({ query }) {
  const result = await casyContext.knowledge.search(query)
  if (result.ok && result.data) {
    const items = Array.isArray(result.data) ? result.data : []
    return items.map(item => ({
      label: item.title,
      id: item.id,
      icon: '📚',
      command: ({ editor, range }) => {
        // 插入块级引用或者只是带链接的文本
        editor
          .chain()
          .focus()
          .deleteRange(range)
          .insertContent(`<a href="/knowledge/${item.id}" target="_blank" style="color: var(--c-primary); text-decoration: underline; background: var(--c-primary-light); border-radius: 4px; padding: 2px 4px;">${item.title}</a>`)
          .run()
      },
    }))
  }
  return []
}

export const KnowledgeReferenceSuggestion = Extension.create({
  name: 'knowledgeReferenceSuggestion',
  addOptions() {
    return {
      suggestion: {
        char: '/',
        items: getSuggestionItems,
        render: () => {
          let popup
          
          function renderItems(props) {
            if (!popup) return
            popup.innerHTML = ''
            popup.style.cssText = `
              position: fixed;
              background: white;
              border: 1px solid var(--c-border);
              border-radius: 8px;
              box-shadow: 0 4px 16px rgba(0,0,0,0.12);
              max-height: 240px;
              overflow-y: auto;
              z-index: 9999;
              min-width: 240px;
              padding: 4px;
            `
            
            const rect = props.clientRect?.()
            if (rect) {
              popup.style.top = `${rect.bottom + 4}px`
              popup.style.left = `${rect.left}px`
            }
            
            props.items.forEach((item, index) => {
              const div = document.createElement('div')
              div.style.cssText = `
                padding: 8px 12px;
                cursor: pointer;
                display: flex;
                align-items: center;
                gap: 10px;
                font-size: 14px;
                border-radius: 6px;
                color: var(--c-text);
                ${index === props.selected ? 'background: var(--c-primary-light); color: var(--c-primary); font-weight: 500;' : ''}
              `
              div.textContent = `${item.icon} ${item.label}`
              div.addEventListener('click', () => props.command(item))
              popup.appendChild(div)
            })
            
            if (props.items.length === 0) {
              const div = document.createElement('div')
              div.style.cssText = 'padding: 12px; color: var(--c-text-tertiary); font-size: 13px; text-align: center;'
              div.textContent = '暂无相关知识 (回车确认忽略)'
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
    return [Suggestion({ editor: this.editor, ...this.options.suggestion, pluginKey: new PluginKey('knowledgeReferenceSuggestion') })]
  },
})
