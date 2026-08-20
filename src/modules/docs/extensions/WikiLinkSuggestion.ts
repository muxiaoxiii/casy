import { Extension } from '@tiptap/core'
import { Plugin, PluginKey } from '@tiptap/pm/state'
import { Decoration, DecorationSet } from '@tiptap/pm/view'

/**
 * [[ ]] 自动补全建议插件
 * 设计哲学 §8.2：双向链接体验
 */
export interface WikiLinkSuggestionOptions {
  /** 搜索知识条目的函数 */
  search: (query: string) => Promise<Array<{ id: string; title: string; category?: string }>>
  /** 选中后的回调 */
  onSelect: (item: { id: string; title: string }) => void
  /** 触发字符 */
  trigger: string
  /** 最小触发长度 */
  minQueryLength: number
}

export const WikiLinkSuggestion = Extension.create<WikiLinkSuggestionOptions>({
  name: 'wikiLinkSuggestion',

  addOptions() {
    return {
      search: async () => [],
      onSelect: () => {},
      trigger: '[[',
      minQueryLength: 1,
    }
  },

  addProseMirrorPlugins() {
    const options = this.options
    const key = new PluginKey('wikiLinkSuggestion')

    return [
      new Plugin({
        key,
        state: {
          init() {
            return {
              active: false,
              query: '',
              items: [] as Array<{ id: string; title: string; category?: string }>,
              pos: null as number | null,
              selectedIndex: 0,
              loading: false,
            }
          },
          apply(tr, value) {
            const meta = tr.getMeta(key)
            if (meta) {
              return { ...value, ...meta }
            }
            // 如果文档改变且不是由本插件触发的，检查是否需要关闭
            if (tr.docChanged && !tr.getMeta('wikiLinkSelect')) {
              const newValue = { ...value }
              // 检查光标位置是否还在 [[ 后面
              const sel = tr.selection
              if (sel.empty) {
                const textBefore = tr.doc.textBetween(
                  Math.max(0, sel.from - 20),
                  sel.from,
                  ' ',
                  ' '
                )
                const match = textBefore.match(/\[\[([^\]]*)$/)
                if (match) {
                  newValue.active = true
                  newValue.query = match[1]
                } else {
                  newValue.active = false
                  newValue.query = ''
                  newValue.items = []
                }
              }
              return newValue
            }
            return value
          },
        },

        props: {
          handleKeyDown(view, event) {
            const state = key.getState(view.state)
            if (!state?.active) return false

            switch (event.key) {
              case 'ArrowDown':
                event.preventDefault()
                view.dispatch(
                  view.state.tr.setMeta(key, {
                    selectedIndex: (state.selectedIndex + 1) % Math.max(state.items.length, 1),
                  })
                )
                return true
              case 'ArrowUp':
                event.preventDefault()
                view.dispatch(
                  view.state.tr.setMeta(key, {
                    selectedIndex:
                      (state.selectedIndex - 1 + Math.max(state.items.length, 1)) %
                      Math.max(state.items.length, 1),
                  })
                )
                return true
              case 'Enter':
              case 'Tab':
                event.preventDefault()
                if (state.items[state.selectedIndex]) {
                  options.onSelect(state.items[state.selectedIndex])
                  // 关闭建议
                  view.dispatch(
                    view.state.tr
                      .setMeta(key, { active: false, items: [], query: '' })
                      .setMeta('wikiLinkSelect', true)
                  )
                }
                return true
              case 'Escape':
                view.dispatch(view.state.tr.setMeta(key, { active: false, items: [], query: '' }))
                return true
            }
            return false
          },

          decorations(state) {
            const pluginState = key.getState(state)
            if (!pluginState?.active || pluginState.items.length === 0) return null

            const sel = state.selection
            if (!sel.empty) return null

            // 在光标位置显示建议列表的标记
            const deco = Decoration.widget(sel.from, () => {
              const container = document.createElement('div')
              container.className = 'wiki-link-suggestions'
              container.style.cssText = `
                position: absolute;
                background: white;
                border: 1px solid #e4e7ed;
                border-radius: 4px;
                box-shadow: 0 2px 12px rgba(0,0,0,0.1);
                max-height: 200px;
                overflow-y: auto;
                z-index: 1000;
                min-width: 200px;
              `

              pluginState.items.forEach((item, index) => {
                const div = document.createElement('div')
                div.className = `wiki-link-item ${index === pluginState.selectedIndex ? 'selected' : ''}`
                div.style.cssText = `
                  padding: 8px 12px;
                  cursor: pointer;
                  display: flex;
                  align-items: center;
                  gap: 8px;
                  ${index === pluginState.selectedIndex ? 'background: #f5f7fa;' : ''}
                `
                div.innerHTML = `
                  <span style="font-weight: 500;">${item.title}</span>
                  ${item.category ? `<span style="font-size: 12px; color: #999; margin-left: auto;">${item.category}</span>` : ''}
                `
                div.addEventListener('click', () => {
                  options.onSelect(item)
                })
                div.addEventListener('mouseenter', () => {
                  view.dispatch(
                    state.tr.setMeta(key, { selectedIndex: index })
                  )
                })
                container.appendChild(div)
              })

              return container
            })

            return DecorationSet.create(state.doc, [deco])
          },
        },
      }),
    ]
  },
})

export default WikiLinkSuggestion
