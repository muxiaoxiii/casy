import { describe, expect, it, vi } from 'vitest'
import { WorkspacePlugin } from '../../src/core/plugins/workspace-plugin'
import { SettingsPlugin } from '../../src/core/plugins/settings-plugin'
import type { CasyContext, CasyTool } from '../../src/core/plugin/types'

describe('workspace data access', () => {
  it('reads long drafts in complete, non-overlapping windows with source versions', async () => {
    const tools = new Map<string, CasyTool>()
    const content = '文书内容'.repeat(2100)
    const ctx = { registerTool: (tool: CasyTool) => tools.set(tool.name, tool), docs: { getDraft: vi.fn().mockResolvedValue({ ok: true, data: { id: 'd1', title: '文书', content, version: 4 } }) } } as unknown as CasyContext
    new WorkspacePlugin().install(ctx)
    const tool = tools.get('get_draft')!
    const first = (await tool.execute({ id: 'd1' })).data as { content: string; nextOffset: number; version: number }
    const second = (await tool.execute({ id: 'd1', offset: first.nextOffset })).data as { content: string; nextOffset: null }
    expect(first.version).toBe(4)
    expect(first.content + second.content).toBe(content)
    expect(second.nextOffset).toBeNull()
    expect((await tool.execute({ id: 'd1', offset: -1 })).ok).toBe(false)
  })
  it('exposes preferences without including connection secrets', async () => {
    const tools = new Map<string, CasyTool>()
    const ctx = { registerTool: (tool: CasyTool) => tools.set(tool.name, tool), settings: { get: vi.fn().mockResolvedValue({ ok: true, data: { theme: 'dark', locale: 'zh-CN', apiKey: 'secret', smtp_password: 'secret', imap: { password: 'secret' } } }) } } as unknown as CasyContext
    await new SettingsPlugin().install(ctx)
    expect(await tools.get('get_settings')!.execute({})).toEqual({ ok: true, data: { theme: 'dark', locale: 'zh-CN' } })
  })
})
