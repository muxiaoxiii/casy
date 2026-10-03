import { spawnSync } from 'node:child_process'
import { mkdtempSync, readdirSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
const profile = mkdtempSync(join(tmpdir(), 'casy-test-suite-'))
console.log(`Isolated test profile: ${profile}`)
const npm = process.platform === 'win32' ? 'npm.cmd' : 'npm'
const steps = [
  [npm, ['run', 'typecheck']],
  [npm, ['run', 'test:unit', '--', '--maxWorkers=2']],
  [process.execPath, ['--test', ...readdirSync('scripts').filter(name => name.endsWith('.test.mjs')).map(name => `scripts/${name}`)]],
  ['cargo', ['test', '--locked'], 'src-tauri'],
  ['cargo', ['test', '--locked', '--features', 'models'], 'tools/casy-doc-engine'],
]
for (const [command, args, cwd] of steps) {
  const result = spawnSync(command, args, {
    cwd, stdio: 'inherit', env: { ...process.env, CASY_TEST_DATA_DIR: profile },
    shell: process.platform === 'win32' && command === npm,
  })
  if (result.error || result.status !== 0) {
    console.error(result.error || `Test step failed: ${command} (exit ${result.status}, signal ${result.signal})`)
    process.exit(result.status || 1)
  }
}
