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
const env = { ...process.env, PATH: '/usr/bin:/bin' }
for (const key of Object.keys(env)) if (key.startsWith('CASY_')) delete env[key]
const request = { jobId: 'package-smoke', sourcePath: source, sourceSha256: before, outputDir: output, markdownOnly: false }
const result = JSON.parse(execFileSync(join(app, 'Contents/Resources/runtime/bin/casy-doc-engine'), ['process'], {
  input: JSON.stringify(request), env, encoding: 'utf8', timeout: 600000, maxBuffer: 16 * 1024 * 1024,
}))
assert.equal(result.pages.length, 1)
const markdown = readFileSync(result.markdownPath, 'utf8')
assert.match(markdown, /Quarterly\s+Sales/i)
assert.match(markdown, /263/)
assert.match(markdown, /data:image\//) // Internal IR intentionally embeds the visual crop; export externalizes it.
assert.equal(readFileSync(result.searchablePdfPath).subarray(0, 5).toString(), '%PDF-')
assert.equal(sha(), before)
console.log(JSON.stringify({ engine: 'packaged', pages: 1, recognizedText: true, visualCrop: true, searchablePdf: true, sourceUnchanged: true, elapsedMs: result.elapsedMs }))
