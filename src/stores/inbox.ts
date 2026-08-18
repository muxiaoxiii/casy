import { defineStore } from 'pinia'
import { tauriCallSafe } from '../core/tauriBridge'
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
      const result = await tauriCallSafe<InboxItem[]>('list_inbox_items', { status })
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
    }): Promise<ReturnType<typeof tauriCallSafe<InboxItem>>> {
      this.processing = true
      const result = await tauriCallSafe<InboxItem>('add_inbox_item', {
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

    async processItem(id: string): Promise<ReturnType<typeof tauriCallSafe<unknown>>> {
      this.processing = true
      const result = await tauriCallSafe<unknown>('process_inbox_item', { id })
      this.processing = false
      if (result.ok) {
        await this.loadItems()
      }
      return result
    },

    async fileItem(itemId: string, caseId: string, category: string): Promise<ReturnType<typeof tauriCallSafe<void>>> {
      const result = await tauriCallSafe<void>('file_inbox_item', {
        itemId,
        caseId,
        category,
      })
      if (result.ok) {
        await this.loadItems()
      }
      return result
    },

    async dismissItem(id: string): Promise<ReturnType<typeof tauriCallSafe<void>>> {
      const result = await tauriCallSafe<void>('dismiss_inbox_item', { id })
      if (result.ok) {
        await this.loadItems()
      }
      return result
    },
  },
})
