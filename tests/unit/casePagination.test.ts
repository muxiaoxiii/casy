import { expect, it, vi } from 'vitest'
import { CasesService } from '../../src/core/services/cases'
vi.mock('../../src/core/tauriBridge', () => ({ tauriCallSafe: vi.fn() }))
it('relation candidates include all pages and reject partial reads', async () => {
  const service = Object.create(CasesService.prototype) as CasesService
  const list = vi.spyOn(service, 'list')
  list.mockResolvedValueOnce({ ok: true, data: { items: [{ id: 'first' }] as any, total: 2, page: 1, perPage: 1 } })
    .mockResolvedValueOnce({ ok: true, data: { items: [{ id: 'second' }] as any, total: 2, page: 2, perPage: 1 } })
  expect((await service.listAll()).data?.map(c => c.id)).toEqual(['first', 'second'])
  expect(list).toHaveBeenNthCalledWith(2, { page: 2, perPage: 200 })
  list.mockResolvedValueOnce({ ok: true, data: { items: [{ id: 'first' }] as any, total: 2, page: 1, perPage: 1 } })
    .mockResolvedValueOnce({ ok: false, error: 'failed second page' })
  expect(await service.listAll()).toEqual({ ok: false, error: 'failed second page' })
})
