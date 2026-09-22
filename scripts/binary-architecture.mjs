import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const machCpu = new Map([[7, 'ia32'], [0x01000007, 'x64'], [12, 'arm'], [0x0100000c, 'arm64']])
const peCpu = new Map([[0x014c, 'ia32'], [0x8664, 'x64'], [0xaa64, 'arm64']])

/** Inspect file headers, not the build machine or a filename such as "universal". */
export function binaryArchitecture(bytes) {
  assert(bytes.length >= 64, 'Truncated native binary')
  const magic = bytes.readUInt32BE(0)
  if ([0xfeedface, 0xfeedfacf, 0xcefaedfe, 0xcffaedfe].includes(magic)) {
    const cpu = [0xcefaedfe, 0xcffaedfe].includes(magic) ? bytes.readUInt32LE(4) : bytes.readUInt32BE(4)
    assert(machCpu.has(cpu), 'Unknown Mach-O CPU')
    return { platform: 'darwin', architectures: [machCpu.get(cpu)] }
  }
  if ([0xcafebabe, 0xcafebabf].includes(magic)) {
    const count = bytes.readUInt32BE(4)
    const stride = magic === 0xcafebabf ? 32 : 20
    assert(count > 0 && count <= 16 && bytes.length >= 8 + count * stride, 'Invalid Universal Mach-O header')
    const architectures = Array.from({ length: count }, (_, i) => machCpu.get(bytes.readUInt32BE(8 + i * stride)))
    assert(architectures.every(Boolean), 'Unknown Universal Mach-O CPU')
    return { platform: 'darwin', architectures }
  }
  if (bytes.toString('ascii', 0, 2) === 'MZ') {
    const offset = bytes.readUInt32LE(0x3c)
    assert(offset <= bytes.length - 6 && bytes.readUInt32LE(offset) === 0x00004550, 'Invalid PE header')
    const cpu = peCpu.get(bytes.readUInt16LE(offset + 4))
    assert(cpu, 'Unknown Windows CPU')
    return { platform: 'win32', architectures: [cpu] }
  }
  throw new Error('Unrecognized native binary format')
}

export async function assertBinaryTarget(path, platform, arch) {
  const actual = binaryArchitecture(await readFile(path))
  assert.equal(actual.platform, platform, `Wrong native platform: ${path}`)
  assert(actual.architectures.includes(arch), `Wrong native architecture (${actual.architectures.join(',')} instead of ${arch}): ${path}`)
  return actual
}
