// image-size DoS 补丁回归（GHSA-w3rx-r6r6-pgpr）
// 恶意 ICNS：条目长度为 0 时上游实现死循环；vendored 补丁必须毫秒级抛错。
import { describe, expect, it } from 'vitest'
// 直接引用 vendored 补丁产物（image-size 不再提升到顶层 node_modules，
// 经 overrides 以 file: 链接进 html-to-docx 的依赖树）
// @ts-expect-error 无类型声明的 vendored 包
import imageSize from '../vendor/image-size/dist/index.js'

function maliciousIcns(): Uint8Array {
  const buf = Buffer.alloc(64)
  buf.write('icns', 0, 'ascii')
  buf.writeUInt32BE(1024, 4) // 头部声称文件 1024 字节
  buf.write('ic04', 8, 'ascii') // 合法条目类型
  buf.writeUInt32BE(0, 12) // 条目长度 0 → 上游死循环点
  return new Uint8Array(buf)
}

describe('image-size vendored patch', () => {
  it('拒绝零长度条目的恶意 ICNS，而不是死循环', () => {
    const start = Date.now()
    expect(() => imageSize(maliciousIcns())).toThrow(/Invalid ICNS entry length/)
    expect(Date.now() - start).toBeLessThan(1000)
  })
})
