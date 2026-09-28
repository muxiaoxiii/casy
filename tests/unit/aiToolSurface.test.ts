import { describe, expect, it, vi } from 'vitest'
import { CasyContextImpl } from '../../src/core/plugin/context'

describe('AI tool surface (P0)', () => {
  it('filters disabled tools and applies write approval policy', async () => {
    const ctx = new CasyContextImpl()
    const execute = vi.fn(async () => ({ ok: true, data: 1 }))
    ctx.registerTool({
      name: 'demo_write',
      description: 'demo',
      category: 'test',
      parameters: { type: 'object', properties: {} },
      policy: { write: true, level: 'L1' },
      execute,
    })
    ctx.registerTool({
      name: 'demo_read',
      description: 'demo read',
      category: 'test',
      parameters: { type: 'object', properties: {} },
      policy: { write: false, level: 'L1' },
      execute: async () => ({ ok: true, data: 2 }),
    })
    expect(ctx.getTools().map(t => t.name).sort()).toEqual(['demo_read', 'demo_write'])
    ctx.setToolPolicy({ disabled: ['demo_read'], writeApproval: { demo_write: 'always_reject' } })
    expect(ctx.getTools().map(t => t.name)).toEqual(['demo_write'])
    const blocked = await ctx.executeTool('demo_write', {}, { origin: 'ai' })
    expect(blocked.ok).toBe(false)
    expect(String(blocked.error)).toContain('禁止')
    const gone = await ctx.executeTool('demo_read', {})
    expect(gone.ok).toBe(false)
    ctx.setToolPolicy({ writeApproval: { demo_write: 'always_approve' } })
    const allowed = await ctx.executeTool('demo_write', {}, { origin: 'ai' })
    expect(allowed.ok).toBe(true)
  })
})

it('declares read/write behavior for every registered business tool', async () => {
  const plugins = await import('../../src/core/plugins')
  const { WorkspacePlugin } = await import('../../src/core/plugins/workspace-plugin')
  const ctx = new CasyContextImpl()
  for (const Plugin of [...Object.values(plugins), WorkspacePlugin]) await ctx.use(new Plugin())
  expect(ctx.getRegisteredTools().length).toBeGreaterThan(40)
  expect(ctx.getRegisteredTools().filter(t => typeof t.policy?.write !== 'boolean').map(t => t.name)).toEqual([])
  for (const tool of ctx.getRegisteredTools().filter(t => t.policy?.write)) {
    ctx.setToolPolicy({ writeApproval: { [tool.name]: 'always_reject' } })
    expect(await ctx.executeTool(tool.name, {}, { origin: 'ai' })).toMatchObject({ ok: false, error: expect.stringContaining('禁止写入') })
  }
})

it('reads prefixed knowledge and draft citations without confusing the two sources', async () => {
  const { KnowledgePlugin } = await import('../../src/core/plugins/knowledge-plugin')
  const tools: any[] = []
  const knowledge = { getWithBlocks: vi.fn(async () => ({ ok: true, data: { item: { id: 'k', content: '知识正文' } } })) }
  const docs = { getDraft: vi.fn(async () => ({ ok: true, data: { id: 'd', title: '文书', content: '字'.repeat(7000) } })) }
  await new KnowledgePlugin().install({ registerTool: tool => tools.push(tool), knowledge, docs } as any)
  const read = tools.find(t => t.name === 'read_document')
  expect(await read.execute({ id: 'draft:d', offset: 6000 })).toMatchObject({ ok: true, data: { citation: 'draft:d', nextOffset: null, item: { content: '字'.repeat(1000) } } })
  expect(knowledge.getWithBlocks).not.toHaveBeenCalled()
  expect(await read.execute({ id: 'knowledge:k' })).toMatchObject({ ok: true, data: { citation: 'knowledge:k' } })
  expect(knowledge.getWithBlocks).toHaveBeenCalledWith('k')
})
