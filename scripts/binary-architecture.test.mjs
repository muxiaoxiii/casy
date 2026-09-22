import test from 'node:test'
import assert from 'node:assert/strict'
import { binaryArchitecture } from './binary-architecture.mjs'

test('Mach-O ARM and Intel are distinguished, including Universal', () => {
  for (const [cpu, arch] of [[0x0100000c,'arm64'],[0x01000007,'x64']]) {
    const data=Buffer.alloc(64);data.writeUInt32BE(0xcffaedfe);data.writeUInt32LE(cpu,4)
    assert.deepEqual(binaryArchitecture(data),{platform:'darwin',architectures:[arch]})
  }
  const fat=Buffer.alloc(64);fat.writeUInt32BE(0xcafebabe);fat.writeUInt32BE(2,4);fat.writeUInt32BE(0x0100000c,8);fat.writeUInt32BE(0x01000007,28)
  assert.deepEqual(binaryArchitecture(fat).architectures,['arm64','x64'])
})
test('PE headers distinguish Windows 32-bit x86 and 64-bit x64', () => {
  for (const [machine,arch] of [[0x14c,'ia32'],[0x8664,'x64'],[0xaa64,'arm64']]) {
    const pe=Buffer.alloc(128);pe.write('MZ');pe.writeUInt32LE(64,0x3c);pe.writeUInt32LE(0x4550,64);pe.writeUInt16LE(machine,68)
    assert.deepEqual(binaryArchitecture(pe),{platform:'win32',architectures:[arch]})
  }
})
test('truncated or corrupt native headers are rejected', () => {
  assert.throws(()=>binaryArchitecture(Buffer.alloc(4)))
  const pe=Buffer.alloc(128);pe.write('MZ');pe.writeUInt32LE(0xfffffffe,0x3c)
  assert.throws(()=>binaryArchitecture(pe))
})
