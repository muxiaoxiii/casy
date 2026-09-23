// Reproducible local validation package; no publishing or Developer ID claims.
import { execFileSync } from 'node:child_process'
import { readFileSync, mkdirSync, mkdtempSync, symlinkSync, rmSync, writeFileSync, createReadStream } from 'node:fs'
import { createHash } from 'node:crypto'
import { resolve, join } from 'node:path'
import { tmpdir } from 'node:os'
import assert from 'node:assert/strict'
const run = (command, args, options = {}) => execFileSync(command, args, { stdio: 'inherit', ...options })
assert.equal(process.platform, 'darwin', 'This validation distribution targets macOS')
const { version } = JSON.parse(readFileSync('package.json', 'utf8'))
const app = resolve('src-tauri/target/release/bundle/macos/Casy.app')
const release = resolve('release')
const labelArg = process.argv.find(arg => arg.startsWith('--label='))
const label = labelArg ? labelArg.slice('--label='.length) : ''
assert.match(label, /^[a-z0-9-]*$/, 'Package label must contain lowercase letters, numbers or hyphens')
const evidence = resolve(`outputs/release-${version}${label ? `-${label}` : ''}`)
mkdirSync(release, { recursive: true }); mkdirSync(evidence, { recursive: true })
if (!process.argv.includes('--skip-build')) run('npm', ['run', 'build:desktop', '--', '--bundles', 'app'])
const actualVersion = execFileSync('/usr/libexec/PlistBuddy', ['-c', 'Print :CFBundleShortVersionString', join(app, 'Contents/Info.plist')], { encoding: 'utf8' }).trim()
assert.equal(actualVersion, version)
for (const script of ['verify-bundle', 'smoke-bundle']) {
  const output = execFileSync(process.execPath, [`scripts/${script}.mjs`, app, join(evidence, 'ocr')], { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 })
  writeFileSync(join(evidence, `${script}.json`), output)
  process.stdout.write(output)
}
const name = `Casy-${version}-production-validation${label ? `-${label}` : ''}-macOS-${process.arch}.dmg`
const dmg = join(release, name)
const staging = mkdtempSync(join(tmpdir(), 'casy-package-'))
try {
  run('ditto', [app, join(staging, 'Casy.app')])
  symlinkSync('/Applications', join(staging, 'Applications'))
  run('hdiutil', ['create', '-volname', `Casy ${version}`, '-srcfolder', staging, '-format', 'UDZO', dmg])
  run('hdiutil', ['verify', dmg])
} finally { rmSync(staging, { recursive: true, force: true }) }
const hash = createHash('sha256')
for await (const chunk of createReadStream(dmg)) hash.update(chunk)
const sha256 = hash.digest('hex')
writeFileSync(`${dmg}.sha256`, `${sha256}  ${name}\n`)
const revision = execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim()
const dirty = Boolean(execFileSync('git', ['status', '--porcelain'], { encoding: 'utf8' }).trim())
writeFileSync(`${dmg}.json`, JSON.stringify({ version, label, revision, dirty, sha256, arch: process.arch, minimumMacOS: '26.0', signing: 'ad-hoc', notarized: false, validatedAt: new Date().toISOString() }, null, 2) + '\n')
console.log(`Validated package: ${dmg}\nSHA256: ${sha256}`)
