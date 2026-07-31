import { defineStore } from 'pinia'
import { tauriCallSafe } from '../core/tauriBridge.js'

export const useInboxStore = defineStore('inbox', {
  state: () => ({
    items: [],
    loading: false,
    processing: false,
  }),

  getters: {
    pendingItems: (state) => state.items.filter((i) => i.status === 'pending'),
    filedItems: (state) => state.items.filter((i) => i.status === 'filed'),
    pendingCount: (state) => state.items.filter((i) => i.status === 'pending').length,
  },

  actions: {
    async loadItems(status = null) {
      this.loading = true
      const result = await tauriCallSafe('list_inbox_items', { status })
      if (result.ok) {
        this.items = result.data || []
      }
      this.loading = false
    },

    async addItem({ sourceType, contentText, sourcePath, title }) {
      this.processing = true
      const result = await tauriCallSafe('add_inbox_item', {
        sourceType,
        contentText: contentText || null,
        sourcePath: sourcePath || null,
        title: title || null,
      })
      this.processing = false
      if (result.ok) {
        await this.loadItems()
      }
      return result
    },

    async processItem(id) {
      this.processing = true
      const result = await tauriCallSafe('process_inbox_item', { id })
      this.processing = false
      if (result.ok) {
        await this.loadItems()
      }
      return result
    },

    async fileItem(itemId, caseId, category) {
      const result = await tauriCallSafe('file_inbox_item', {
        itemId,
        caseId,
        category,
      })
      if (result.ok) {
        await this.loadItems()
      }
      return result
    },

    async dismissItem(id) {
      const result = await tauriCallSafe('dismiss_inbox_item', { id })
      if (result.ok) {
        await this.loadItems()
      }
      return result
    },
  },
})
