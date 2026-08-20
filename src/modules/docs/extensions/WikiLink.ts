import { Mark, mergeAttributes } from '@tiptap/core'

/**
 * 双向链接扩展 [[知识标题]]
 * 设计哲学 §8.2：知识块级化 + 双向链接
 */
export const WikiLink = Mark.create({
  name: 'wikiLink',
  
  addAttributes() {
    return {
      knowledgeId: {
        default: null,
        parseHTML: element => element.getAttribute('data-knowledge-id'),
        renderHTML: attributes => ({
          'data-knowledge-id': attributes.knowledgeId,
        }),
      },
      title: {
        default: '',
        parseHTML: element => element.getAttribute('data-title'),
        renderHTML: attributes => ({
          'data-title': attributes.title,
        }),
      },
    }
  },

  parseHTML() {
    return [
      {
        tag: 'a[data-wiki-link]',
      },
    ]
  },

  renderHTML({ HTMLAttributes }) {
    return ['a', mergeAttributes(HTMLAttributes, { 
      'data-wiki-link': '',
      class: 'wiki-link',
      href: '#',
    }), 0]
  },

  addCommands() {
    return {
      setWikiLink: (knowledgeId, title) => ({ commands }) => {
        return commands.setMark(this.name, { knowledgeId, title })
      },
      unsetWikiLink: () => ({ commands }) => {
        return commands.unsetMark(this.name)
      },
    }
  },
})

export default WikiLink
