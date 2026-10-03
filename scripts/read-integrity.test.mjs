import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync, readdirSync } from 'node:fs'

test('command readers do not silently discard SQL row decoding errors', () => {
  for (const dir of ['commands', 'ai', 'db']) {
  for (const file of readdirSync(`src-tauri/src/${dir}`).filter(name => name.endsWith('.rs'))) {
    const source = readFileSync(`src-tauri/src/${dir}/${file}`, 'utf8')
    assert.doesNotMatch(source, /filter_map\(\|r\| r\.ok\(\)\)/, file)
    assert.doesNotMatch(source, /(?:rows|case_iter)\.flatten\(\)/, file)
  }
  }
})

test('regular CI keeps Windows native test preparation and loader gates', () => {
  const source = readFileSync('.github/workflows/ci.yml', 'utf8')
  assert.match(source, /os: \[ubuntu-latest, macos-latest, windows-2025\]/)
  assert.match(source, /run: \.\.\/scripts\/prepare-windows-renderer\.ps1/)
  assert.match(source, /OPENSSL_STATIC=1/)
  assert.match(source, /GITHUB_PATH "\$pwd\/runtime\/zvec"/)
  assert.match(source, /run: cargo test --locked --tests --no-run/)
  assert.match(source, /run: python scripts\/diagnose-windows-loader\.py/)
})

test('durability flushes do not use read-only file handles on Windows', () => {
  // Windows FlushFileBuffers needs write access, so a read-only handle fails with
  // "Access is denied (os error 5)". This previously scanned four hand-picked files and one
  // chained spelling, so the same defect could reappear anywhere else unnoticed. Walk the whole
  // backend and reject every read-only open that is flushed on the same expression.
  const walk = dir => readdirSync(dir, { withFileTypes: true }).flatMap(entry => {
    const full = `${dir}/${entry.name}`
    if (entry.isDirectory()) return walk(full)
    return entry.name.endsWith('.rs') ? [full] : []
  })
  const readOnlyOpen = /(?:std::)?(?:fs::)?File::open\([^\n]*?\)[^\n]*?\.(?:sync_all|sync_data|flush)\(\)/
  const scanned = walk('src-tauri/src')
  assert.ok(scanned.length > 40, `expected to scan the whole backend, saw ${scanned.length} files`)
  for (const file of scanned) {
    assert.doesNotMatch(readFileSync(file, 'utf8'), readOnlyOpen, file)
  }
})
