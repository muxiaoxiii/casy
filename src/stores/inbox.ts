import { defineStore } from 'pinia'
import { casyContext } from '../core/plugin/context'
import type { InboxItem, InboxStatus } from '../types'

interface InboxState {
  items: InboxItem[]
  loading: boolean
  processing: boolean
}

export const useInboxStore = defineStore('inbox', {
  state: (): InboxState => ({
    items: [],
    loading: false,
    processing: false,
  }),

  getters: {
    pendingItems: (state): InboxItem[] =>
      state.items.filter((i) => i.status === 'pending'),

    filedItems: (state): InboxItem[] =>
      state.items.filter((i) => i.status === 'filed'),

    pendingCount: (state): number =>
      state.items.filter((i) => i.status === 'pending').length,
  },

  actions: {
    async loadItems(status: InboxStatus | null = null): Promise<void> {
      this.loading = true
      const result = await casyContext.inbox.list(status || undefined)
      if (result.ok && result.data) {
        this.items = result.data
      }
      this.loading = false
    },

    async addItem({
      sourceType,
      contentText,
      sourcePath,
      title,
    }: {
      sourceType: string
      contentText?: string
      sourcePath?: string
      title?: string
    }): Promise<{ ok: boolean; data?: InboxItem; error?: string }> {
      this.processing = true
      const result = await casyContext.inbox.add(sourceType, contentText, sourcePath)
      this.processing = false
      if (result.ok) {
        await this.loadItems()
      }
      return result
    },

    async processItem(id: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
      this.processing = true
      const result = await casyContext.inbox.process(id)
      this.processing = false
      if (result.ok) {
        await this.loadItems()
      }
      return result
    },

    async fileItem(itemId: string, caseId: string, category: string): Promise<{ ok: boolean; error?: string }> {
      const result = await casyContext.inbox.file(itemId, caseId, category)
      if (result.ok) {
        await this.loadItems()
      }
      return result
    },

    async dismissItem(id: string): Promise<{ ok: boolean; error?: string }> {
      const result = await casyContext.inbox.dismiss(id)
      if (result.ok) {
        await this.loadItems()
      }
      return result
    },
  },
})
