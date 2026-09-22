import { afterEach, describe, expect, it, vi } from 'vitest'
import { observeChanges } from '../../src/core/observeChanges'
import { validateParams } from '../../src/core/plugin/validateParams'
import { defineTool } from '../../src/core/plugin/defineTool'
import { Service } from '../../src/core/plugin/types'
import { WorkspaceService } from '../../src/core/services/workspace'
import type { CasyContext } from '../../src/core/plugin/types'

function eventBus() {
  const handlers = new Map<string, Set<() => void>>()
  return {
    on(name: string, fn: () => void) { const set = handlers.get(name) || new Set(); set.add(fn); handlers.set(name, set); return () => { set.delete(fn) } },
    emit(name: string) { handlers.get(name)?.forEach(fn => fn()) },
  }
}
afterEach(() => vi.useRealTimers())

describe('connected workspace invalidation', () => {
  it('coalesces writes, queues changes during a read, and cleans up', async () => {
    vi.useFakeTimers()
    const bus = eventBus()
    let resolve!: () => void
    const read = vi.fn().mockImplementationOnce(() => new Promise<void>(r => { resolve = r })).mockResolvedValue(undefined)
    const stop = observeChanges(bus, ['task', 'calendar'], read)
    bus.emit('task:changed'); bus.emit('task:completed'); bus.emit('calendar:changed')
    await vi.advanceTimersByTimeAsync(80)
    expect(read).toHaveBeenCalledTimes(1)
    bus.emit('task:changed'); bus.emit('task:changed')
    resolve()
    await vi.advanceTimersByTimeAsync(0)
    expect(read).toHaveBeenCalledTimes(2)
    stop(); bus.emit('task:changed')
    await vi.advanceTimersByTimeAsync(100)
    expect(read).toHaveBeenCalledTimes(2)
  })
  it('emits only after successful persistence', async () => {
    const bus = eventBus(), changed = vi.fn()
    bus.on('calendar:changed', changed)
    class TestService extends Service { run(ok: boolean) { return this.mutation('calendar', Promise.resolve({ ok })) } }
    const service = new TestService(bus as unknown as CasyContext)
    await service.run(false); expect(changed).not.toHaveBeenCalled()
    await service.run(true); expect(changed).toHaveBeenCalledOnce()
  })
  it('keeps source failures visible while returning usable related data', async () => {
    const ctx = {
      cases: { get: vi.fn().mockResolvedValue({ ok: true, data: { id: 'c1' } }) },
      tasks: { list: vi.fn().mockResolvedValue({ ok: true, data: [{ id: 't1', caseId: 'c1' }] }) },
      calendar: { listEvents: vi.fn().mockResolvedValue({ ok: true, data: [{ id: 'e1', caseId: 'c1' }, { id: 'e2', caseId: 'c2' }] }) },
      knowledge: { list: vi.fn().mockResolvedValue({ ok: false, error: '索引暂不可用' }) },
      docs: { listDrafts: vi.fn().mockResolvedValue({ ok: true, data: [{ id: 'd1', caseId: 'c1', content: '正文', version: 2 }, { id: 'd2', caseId: 'c2' }] }) },
      files: { list: vi.fn().mockResolvedValue({ ok: true, data: [] }) },
    }
    const result = await new WorkspaceService(ctx as unknown as CasyContext).caseContext('c1', '2026-09-13', '2026-10-13')
    expect(result.ok).toBe(true)
    expect(result.data.calendar.map(e => e.id)).toEqual(['e1'])
    expect(result.data.docs).toEqual([{ id: 'd1', caseId: 'c1', version: 2 }])
    expect(result.data.errors).toEqual([{ source: 'knowledge', error: '索引暂不可用' }])
    expect(ctx.tasks.list).toHaveBeenCalledWith({ caseId: 'c1' })
  })
})

describe('tool parameter boundary', () => {
  it('rejects malformed nested arguments before invoking a tool', async () => {
    const execute = vi.fn().mockResolvedValue({ ok: true })
    const tool = defineTool({ name: 'test', category: 'test', description: '', parameters: {
      type: 'object', required: ['data'], properties: { data: { type: 'object', required: ['name'], properties: { name: { type: 'string' }, count: { type: 'integer' } } } },
    }, execute })
    expect((await tool.execute({ data: [] })).ok).toBe(false)
    expect((await tool.execute({ data: { count: 2 } })).ok).toBe(false)
    expect((await tool.execute({ data: { name: '任务', count: 2.5 } })).ok).toBe(false)
    expect(execute).not.toHaveBeenCalled()
    expect((await tool.execute({ data: { name: '任务', count: 2 } })).ok).toBe(true)
    expect(execute).toHaveBeenCalledOnce()
  })
  it('validates arrays and finite numbers while preserving open patch schemas', () => {
    expect(validateParams({ type: 'number' }, NaN)).toBeTruthy()
    expect(validateParams({ type: 'array', items: { type: 'string' } }, ['ok', 5])).toContain('[1]')
    expect(validateParams({ type: 'object' }, { content: null, expectedVersion: 2 })).toBeNull()
  })
})
