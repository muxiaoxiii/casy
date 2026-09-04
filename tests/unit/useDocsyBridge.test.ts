// @vitest-environment node
import { describe, it, expect, vi, beforeEach } from 'vitest'

/**
 * 可控 deferred mock：按调用顺序暴露 resolver，
 * 让测试能精确控制多个渲染请求的完成顺序，覆盖竞态交错。
 */
const h = vi.hoisted(() => {
  const calls: { resolve: (v: unknown) => void; reject: (e: unknown) => void }[] = []
  const tauriCallSafe = vi.fn(() => {
    return new Promise((resolve, reject) => {
      calls.push({ resolve, reject })
    })
  })
  return { calls, tauriCallSafe }
})

vi.mock('../../src/core/tauriBridge', () => ({
  tauriCallSafe: h.tauriCallSafe,
}))

import { useDocsyBridge } from '../../src/modules/docs/composables/useDocsyBridge'

function bridge() {
  return useDocsyBridge()
}

describe('useDocsyBridge · renderTemplate 竞态守卫', () => {
  beforeEach(() => {
    h.calls.length = 0
    h.tauriCallSafe.mockClear()
  })

  it('最新请求写入 renderResult 并释放 renderLoading', async () => {
    const { renderResult, renderLoading, renderTemplate } = bridge()

    const p = renderTemplate('tpl-1', 'case-1')
    // 请求进行中：忙碌态应保持
    expect(renderLoading.value).toBe(true)

    h.calls[0].resolve({ ok: true, data: 'html-1' })
    const result = await p

    expect(result.stale).toBe(false)
    expect(renderResult.value).toBe('html-1')
    expect(renderLoading.value).toBe(false)
  })

  it('旧请求先返回：不覆盖最新选择，也不提前关闭 spinner', async () => {
    const { renderResult, renderLoading, renderTemplate } = bridge()

    const p1 = renderTemplate('tpl-1', 'case-1') // 请求 #1
    const p2 = renderTemplate('tpl-2', 'case-2') // 请求 #2（最新）
    expect(renderLoading.value).toBe(true)

    // 旧请求后到（在时间上先返回），但它已是 stale
    h.calls[0].resolve({ ok: true, data: 'html-1' })
    const r1 = await p1

    expect(r1.stale).toBe(true)
    // 旧请求不得写入结果、不得提前复位忙碌态（最新请求仍在途）
    expect(renderResult.value).toBe(null)
    expect(renderLoading.value).toBe(true)

    // 最新请求完成
    h.calls[1].resolve({ ok: true, data: 'html-2' })
    const r2 = await p2

    expect(r2.stale).toBe(false)
    expect(renderResult.value).toBe('html-2')
    expect(renderLoading.value).toBe(false)
  })

  it('最新请求先返回，旧请求后到：不得覆盖结果、不得悬挂/复位忙碌态', async () => {
    const { renderResult, renderLoading, renderTemplate } = bridge()

    const p1 = renderTemplate('tpl-1', 'case-1') // 请求 #1
    const p2 = renderTemplate('tpl-2', 'case-2') // 请求 #2（最新）

    // 最新请求先完成
    h.calls[1].resolve({ ok: true, data: 'html-2' })
    const r2 = await p2

    expect(r2.stale).toBe(false)
    expect(renderResult.value).toBe('html-2')
    expect(renderLoading.value).toBe(false)

    // 旧请求随后完成：应被判 stale，不改结果、不把忙碌态改回 true（即不悬挂）
    h.calls[0].resolve({ ok: true, data: 'html-1' })
    const r1 = await p1

    expect(r1.stale).toBe(true)
    expect(renderResult.value).toBe('html-2')
    expect(renderLoading.value).toBe(false)
  })

  it('clearRenderResult 使在途请求失效并复位状态', async () => {
    const { renderResult, renderLoading, renderTemplate, clearRenderResult } = bridge()

    const p1 = renderTemplate('tpl-1', 'case-1')
    clearRenderResult()

    expect(renderResult.value).toBe(null)
    expect(renderLoading.value).toBe(false)

    // 在途请求到货后变为 stale，不得写回结果
    h.calls[0].resolve({ ok: true, data: 'html-1' })
    const r1 = await p1

    expect(r1.stale).toBe(true)
    expect(renderResult.value).toBe(null)
    expect(renderLoading.value).toBe(false)
  })

  it('渲染失败：最新请求写入 error 并释放忙碌态', async () => {
    const { renderResult, error, renderLoading, renderTemplate } = bridge()

    const p = renderTemplate('tpl-1', 'case-1')
    h.calls[0].resolve({ ok: false, error: 'boom' })
    const result = await p

    expect(result.stale).toBe(false)
    expect(renderResult.value).toBe(null)
    expect(error.value).toBe('boom')
    expect(renderLoading.value).toBe(false)
  })
})
