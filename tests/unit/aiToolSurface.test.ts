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
