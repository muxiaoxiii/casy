import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync, readdirSync } from 'node:fs'

test('command readers do not silently discard SQL row decoding errors', () => {
  for (const file of readdirSync('src-tauri/src/commands').filter(name => name.endsWith('.rs'))) {
    const source = readFileSync(`src-tauri/src/commands/${file}`, 'utf8')
    assert.doesNotMatch(source, /filter_map\(\|r\| r\.ok\(\)\)/, file)
    assert.doesNotMatch(source, /(?:rows|case_iter)\.flatten\(\)/, file)
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
