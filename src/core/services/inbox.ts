import { Service } from '../plugin/types'
import { tauriCallSafe } from '../tauriBridge'
import type { InboxItem } from '../../types'

/** 收件箱服务：ctx.inbox */
export class InboxService extends Service {
  static inject: string[] = []

  async list(status?: string): Promise<{ ok: boolean; data?: InboxItem[]; error?: string }> {
    return tauriCallSafe<InboxItem[]>('list_inbox_items', { status: status || 'all' })
  }

  async add(sourceType: string, contentText?: string, sourcePath?: string): Promise<{ ok: boolean; data?: string; error?: string }> {
    return tauriCallSafe<string>('add_inbox_item', {
      sourceType,
      contentText: contentText ?? null,
      sourcePath: sourcePath ?? null,
    })
  }

  async process(id: string): Promise<{ ok: boolean; data?: unknown; error?: string }> {
    return tauriCallSafe<unknown>('process_inbox_item', { id })
  }

  async file(id: string, caseId: string, category?: string): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe<void>('file_inbox_item', {
      itemId: id,
      caseId,
      category: category || '',
    })
  }

  async dismiss(id: string): Promise<{ ok: boolean; error?: string }> {
    return tauriCallSafe<void>('dismiss_inbox_item', { id })
  }
}
