// Real OCR from the packaged engine, with no Homebrew/PATH/model overrides.
import { execFileSync } from 'node:child_process'
import { readFileSync, mkdirSync } from 'node:fs'
import { createHash } from 'node:crypto'
import { resolve, join } from 'node:path'
import assert from 'node:assert/strict'
const app = resolve(process.argv[2] || 'src-tauri/target/release/bundle/macos/Casy.app')
const output = resolve(process.argv[3] || 'outputs/release-validation/ocr')
mkdirSync(output, { recursive: true })
const source = resolve('tests/fixtures/ocr-chart.png')
const sha = () => createHash('sha256').update(readFileSync(source)).digest('hex')
const before = sha()
const env = { ...process.env, PATH: process.platform === 'win32' ? `${process.env.SystemRoot}\\System32;${process.env.SystemRoot}` : '/usr/bin:/bin' }
for (const key of Object.keys(env)) if (key.startsWith('CASY_')) delete env[key]
const request = { jobId: 'package-smoke', sourcePath: source, sourceSha256: before, outputDir: output, markdownOnly: false }
const result = JSON.parse(execFileSync(join(app, process.platform === 'win32' ? 'runtime/bin/casy-doc-engine.exe' : 'Contents/Resources/runtime/bin/casy-doc-engine'), ['process'], {
  input: JSON.stringify(request), env, encoding: 'utf8', timeout: 600000, maxBuffer: 16 * 1024 * 1024,
}))
// R-02 起引擎结果只回传页数（页面集合唯一事实源是落盘页 IR）
assert.equal(result.pageCount, 1)
const markdown = readFileSync(result.markdownPath, 'utf8')
assert.match(markdown, /Quarterly\s+Sales/i)
assert.match(markdown, /263/)
assert.match(markdown, /assets\/[a-f0-9]{64}\.png/)
const manifest = JSON.parse(readFileSync(join(output, 'source.assets.json'), 'utf8'))
assert.equal(manifest.version, 2)
assert.ok(Object.keys(manifest.assets).length > 0)
for (const [id, asset] of Object.entries(manifest.assets)) {
  assert.match(id, /^[a-f0-9]{64}\.png$/)
  const bytes = readFileSync(join(output, 'assets', id))
  assert.equal(bytes.length, asset.size)
  assert.equal(createHash('sha256').update(bytes).digest('hex'), asset.sha256)
}
assert.equal(readFileSync(result.searchablePdfPath).subarray(0, 5).toString(), '%PDF-')
assert.equal(sha(), before)
console.log(JSON.stringify({ engine: 'packaged', pages: 1, recognizedText: true, visualCrop: true, searchablePdf: true, sourceUnchanged: true, elapsedMs: result.elapsedMs }))
