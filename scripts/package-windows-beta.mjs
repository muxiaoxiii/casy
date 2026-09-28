import { execFileSync } from 'node:child_process'
import { cpSync, mkdirSync, readFileSync, readdirSync, writeFileSync, createReadStream } from 'node:fs'
import { resolve, join } from 'node:path'
import { createHash } from 'node:crypto'
import assert from 'node:assert/strict'
assert.equal(process.platform, 'win32')
const { version } = JSON.parse(readFileSync('package.json', 'utf8'))
assert.match(version, /^\d+\.\d+\.\d+-beta\.\d+$/)
const release = resolve('release')
const evidence = resolve(`outputs/release-${version}`)
const installed = resolve('outputs/windows-installed')
for (const path of [release, evidence, installed]) mkdirSync(path, { recursive: true })
const installers = readdirSync('src-tauri/target/release/bundle/nsis').filter(name => name.endsWith('.exe'))
assert.equal(installers.length, 1, 'Expected one NSIS installer')
const installer = resolve('src-tauri/target/release/bundle/nsis', installers[0])
// Validate the installed layout, including the loader DLL beside casy.exe.
execFileSync(installer, ['/S', `/D=${installed}`], { timeout: 300000, stdio: 'inherit' })
for (const script of ['verify-bundle', 'smoke-bundle']) {
  const result = execFileSync(process.execPath, [`scripts/${script}.mjs`, installed, join(evidence, 'ocr')], { encoding: 'utf8', timeout: 900000, maxBuffer: 16 * 1024 * 1024 })
  writeFileSync(join(evidence, `${script}.json`), result)
  process.stdout.write(result)
}
const name = `Casy-${version}-Windows-x64-setup.exe`
const destination = join(release, name)
cpSync(installer, destination)
const hash = createHash('sha256')
for await (const chunk of createReadStream(destination)) hash.update(chunk)
const sha256 = hash.digest('hex')
writeFileSync(`${destination}.sha256`, `${sha256}  ${name}\n`)
writeFileSync(`${destination}.json`, JSON.stringify({ version, sha256, platform: 'win32', arch: 'x64', signing: 'unsigned', revision: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(), validatedAt: new Date().toISOString() }, null, 2) + '\n')
