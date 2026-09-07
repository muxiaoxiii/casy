import { createHash } from 'node:crypto'
import { readFile, writeFile, mkdir, copyFile, mkdtemp, rm, readdir } from 'node:fs/promises'
import { join, dirname, resolve } from 'node:path'
import { tmpdir } from 'node:os'
import { fileURLToPath } from 'node:url'
import { execFileSync } from 'node:child_process'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const releases = {
  'darwin-arm64': ['osx-arm64.tar.gz', 'ac909c57e084bb39f7f98ef89b28db322df88348254158eedefa9c00e2c34dfc'],
  'linux-x64': ['linux-amd64.tar.gz', 'db9472ef2146b8f435b45b47a644eb7a75eb84b9946334debfcfc21112cec7f9'],
  'linux-arm64': ['linux-arm64.tar.gz', '9a2a2867fb6ddb53212029b7f50eac288ad86bebfccb6ea6d52b3f35c4e026a8'],
  'win32-x64': ['windows-amd64.zip', '549f0893a376c8e5b4cf1e685ef55ca37f41a39f5e016632ac1f762ad703d551'],
}
const release = releases[`${process.platform}-${process.arch}`]
if (!release) throw new Error('No pinned Zvec SDK for this platform; provide ZVEC_LIB_DIR with a compatible v0.7.0 SDK')
const destination = join(root, 'src-tauri/runtime/zvec')
await mkdir(destination, { recursive: true })
const archive = join(destination, 'sdk-' + release[0])
const valid = async () => createHash('sha256').update(await readFile(archive).catch(() => Buffer.alloc(0))).digest('hex') === release[1]
if (!await valid()) {
  execFileSync('curl', ['--fail', '--location', '--retry', '2', '--max-time', '180', '--output', archive,
    `https://github.com/alibaba/zvec/releases/download/v0.7.0/zvec-sdk-${release[0]}`], { stdio: 'inherit' })
}
if (!await valid()) throw new Error('Zvec SDK checksum mismatch')
const temporary = await mkdtemp(join(tmpdir(), 'casy-zvec-sdk-'))
try {
  execFileSync('tar', ['-xf', archive, '-C', temporary])
  async function collect(directory) {
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const source = join(directory, entry.name)
      if (entry.isDirectory()) await collect(source)
      else if (/^(libzvec_c_api\.(dylib|so)|zvec_c_api\.(dll|lib))$/.test(entry.name) || /^(LICENSE|NOTICE)/.test(entry.name)) {
        await copyFile(source, join(destination, entry.name))
      }
    }
  }
  await collect(temporary)
} finally { await rm(temporary, { recursive: true, force: true }) }
if (process.platform === 'darwin') {
  const library = join(destination, 'libzvec_c_api.dylib')
  execFileSync('codesign', ['--force', '--sign', '-', library])
}
const licenseHash = '43070e2d4e532684de521b885f385d0841030efa2b1a20bafb76133a5e1379c1'
let license = await readFile(join(destination, 'LICENSE')).catch(() => Buffer.alloc(0))
if (createHash('sha256').update(license).digest('hex') !== licenseHash) {
  const response = await fetch('https://raw.githubusercontent.com/alibaba/zvec/v0.7.0/LICENSE', { signal: AbortSignal.timeout(30000) })
  if (!response.ok) throw new Error(`Zvec license download failed: ${response.status}`)
  license = Buffer.from(await response.arrayBuffer())
  if (createHash('sha256').update(license).digest('hex') !== licenseHash) throw new Error('Zvec license checksum mismatch')
  await writeFile(join(destination, 'LICENSE'), license)
}
await copyFile(fileURLToPath(import.meta.url), join(destination, 'sdk-source.mjs'))
// The verified archive also records the complete redistributable SDK and its notices.
console.log(`Zvec 0.7.0 verified: ${destination}`)
