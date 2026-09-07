import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { createReadStream, createWriteStream } from 'node:fs'
import { chmod, copyFile, mkdir, readFile, readdir, realpath, stat, writeFile } from 'node:fs/promises'
import { basename, dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { pipeline } from 'node:stream/promises'
import { Readable } from 'node:stream'

const project = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const runtime = join(project, 'src-tauri/runtime')
const run = (command, args, options = {}) => {
  const result = spawnSync(command, args, { cwd: project, encoding: 'utf8', ...options })
  if (result.status !== 0) throw new Error(`${command} failed: ${result.stderr || result.error}`)
  return result.stdout?.trim() || ''
}
const exists = async path => Boolean(await stat(path).catch(() => null))
for (const folder of ['bin', 'lib', 'models', 'fonts', 'licenses']) await mkdir(join(runtime, folder), { recursive: true })

run('cargo', ['build', '--release', '--locked', '--features', 'models', '--manifest-path', 'tools/casy-doc-engine/Cargo.toml'], { stdio: 'inherit' })
const exe = process.platform === 'win32' ? '.exe' : ''
await copyFile(join(project, `tools/casy-doc-engine/target/release/casy-doc-engine${exe}`), join(runtime, `bin/casy-doc-engine${exe}`))
if (process.env.CASY_MODEL_CACHE) {
  const { cp } = await import('node:fs/promises')
  await cp(resolve(process.env.CASY_MODEL_CACHE), join(runtime, 'models/ppocrv6-medium'), { recursive: true })
}
run(process.execPath, ['tools/casy-doc-engine/install-models.mjs', join(runtime, 'models')], { stdio: 'inherit' })

const fontRoot = 'https://raw.githubusercontent.com/notofonts/noto-cjk/f8d157532fbfaeda587e826d4cd5b21a49186f7c/'
async function pinnedDownload(relative, url, size, blob) {
  const path = join(runtime, relative)
  const verify = async () => {
    if ((await stat(path).catch(() => null))?.size !== size) return false
    const hash = createHash('sha1').update(`blob ${size}\0`)
    for await (const chunk of createReadStream(path)) hash.update(chunk)
    return hash.digest('hex') === blob
  }
  if (await verify()) return
  const response = await fetch(url, { signal: AbortSignal.timeout(300000) })
  if (!response.ok) throw new Error(`Download failed: ${url}: ${response.status}`)
  await pipeline(Readable.fromWeb(response.body), createWriteStream(path))
  if (!await verify()) throw new Error(`Checksum mismatch: ${relative}`)
}
await pinnedDownload('fonts/NotoSansCJK-Regular.ttf', fontRoot + 'Sans/Variable/TTF/NotoSansCJKsc-VF.ttf', 36144788, 'e67840913223f5c5db60570ce5bf001b0e079d42')
await pinnedDownload('licenses/Noto-OFL.txt', fontRoot + 'Sans/LICENSE', 4301, 'd952d62c065f3f35fb83a173496e90b21525aef3')

const components = []
run(process.execPath, ['scripts/prepare-notices.mjs'], { stdio: 'inherit' })
await copyFile(join(runtime, 'licenses/packages/rust-oar-ocr-0.9.2/LICENSE'), join(runtime, 'licenses/Paddle-models-Apache-2.0.txt'))
await copyFile(join(project, 'tools/casy-doc-engine/install-models.mjs'), join(runtime, 'licenses/model-sources.mjs'))
if (process.platform === 'darwin') {
  const renderer = process.env.CASY_PDFTOPPM || run('which', ['pdftoppm'])
  const copied = new Map()
  const formulas = new Set()
  const system = path => path.startsWith('/usr/lib/') || path.startsWith('/System/')
  async function copyMachO(source, destination) {
    source = await realpath(source)
    const prior = copied.get(destination)
    if (prior) {
      if (prior !== source) throw new Error(`Conflicting libraries: ${destination}`)
      return
    }
    copied.set(destination, source)
    const cellar = source.match(/\/Cellar\/([^/]+)\/([^/]+)\//)
    if (cellar) formulas.add(cellar[1])
    await copyFile(source, destination)
    await chmod(destination, 0o755)
    spawnSync('codesign', ['--remove-signature', destination], { encoding: 'utf8' })
    const dependencies = run('otool', ['-L', source]).split('\n').slice(1).map(line => line.trim().split(' (')[0])
    const rpaths = [...run('otool', ['-l', source]).matchAll(/cmd LC_RPATH\s+cmdsize \d+\s+path (.*?) \(offset/g)].map(m => m[1])
    for (const dependency of dependencies) {
      if (system(dependency)) continue
      if (basename(destination) === basename(dependency) && destination.endsWith('.dylib')) continue
      let resolved = dependency
      if (dependency.startsWith('@loader_path/')) resolved = join(dirname(source), dependency.slice(13))
      if (dependency.startsWith('@rpath/')) {
        resolved = null
        for (const rpath of rpaths) {
          const candidate = join(rpath.replace('@loader_path', dirname(source)), dependency.slice(7))
          if (await exists(candidate)) { resolved = candidate; break }
        }
        if (!resolved) {
          const candidate = join(dirname(source), '../lib', basename(dependency))
          if (await exists(candidate)) resolved = candidate
        }
      }
      if (!resolved || !await exists(resolved)) throw new Error(`Unresolved dependency: ${source}: ${dependency}`)
      const target = join(runtime, 'lib', basename(dependency))
      await copyMachO(resolved, target)
      const relative = destination.endsWith('.dylib') ? `@loader_path/${basename(target)}` : `@loader_path/../lib/${basename(target)}`
      run('install_name_tool', ['-change', dependency, relative, destination])
    }
    if (destination.endsWith('.dylib')) run('install_name_tool', ['-id', `@loader_path/${basename(destination)}`, destination])
    run('codesign', ['--force', '--sign', '-', destination])
  }
  await copyMachO(renderer, join(runtime, 'bin/pdftoppm'))
  for (const formula of formulas) {
    const prefix = run('brew', ['--prefix', formula])
    const licenseDir = join(runtime, 'licenses', formula)
    await mkdir(licenseDir, { recursive: true })
    const formulaPath = join(prefix, '.brew', `${formula}.rb`)
    const receipt = JSON.parse(await readFile(join(prefix, 'INSTALL_RECEIPT.json'), 'utf8'))
    const installed = JSON.parse(run('/usr/bin/ruby', ['scripts/homebrew-source.rb', formulaPath]))
    await copyFile(formulaPath, join(licenseDir, `${formula}.rb`))
    const source = { url: installed.url?.[0], checksum: installed.sha256?.[0] }
    if (!source.url || !/^[a-f0-9]{64}$/.test(source.checksum || '')) throw new Error(`Missing installed source metadata: ${formula}`)
    const sourcePath = join(licenseDir, basename(new URL(source.url).pathname))
    const validSource = async () => {
      if (!await exists(sourcePath)) return false
      const hash = createHash('sha256')
      for await (const chunk of createReadStream(sourcePath)) hash.update(chunk)
      return hash.digest('hex') === source.checksum
    }
    if (!await validSource()) {
      console.log(`Including source: ${formula}`)
      const urls = [source.url]
      if (source.url.startsWith('https://ftpmirror.gnu.org/')) urls.push(source.url.replace('https://ftpmirror.gnu.org/', 'https://ftp.gnu.org/'))
      let downloaded = false
      for (const url of urls) {
        try {
          run('curl', ['--fail', '--location', '--retry', '2', '--max-time', '180', '--output', sourcePath, url])
          if (await validSource()) { downloaded = true; break }
        } catch { console.warn(`Source mirror unavailable: ${url}`) }
      }
      if (!downloaded) throw new Error(`Unable to download verified source: ${formula}`)
      if (!await validSource()) throw new Error(`Source checksum mismatch: ${formula}`)
    }
    for (const name of await readdir(prefix)) {
      if (/^(COPYING|LICENSE|NOTICE|AUTHORS)/i.test(name) && (await stat(join(prefix, name))).isFile()) await copyFile(join(prefix, name), join(licenseDir, name))
    }
    components.push({ name: formula, version: receipt.source.versions.stable, license: installed.license, source, buildRecipe: `licenses/${formula}/${formula}.rb` })
  }
} else {
  // Other platform builds must supply a complete redistributable renderer tree.
  const distribution = process.env.CASY_RENDERER_DISTRIBUTION
  if (!distribution) throw new Error('CASY_RENDERER_DISTRIBUTION must contain bin, lib and licenses for this platform')
  const { cp } = await import('node:fs/promises')
  await cp(resolve(distribution), runtime, { recursive: true })
}
await copyFile(join(project, 'docs/compliance/LICENSES.md'), join(runtime, 'licenses/Casy-dependencies.md'))
await copyFile(join(project, 'docs/compliance/MinerU-Popo-LICENSE.txt'), join(runtime, 'licenses/MinerU-Popo-LICENSE.txt'))
await copyFile(join(project, 'scripts/prepare-runtime.mjs'), join(runtime, 'licenses/runtime-build.mjs'))
await copyFile(join(project, 'scripts/prepare-notices.mjs'), join(runtime, 'licenses/prepare-notices.mjs'))
await copyFile(join(project, 'scripts/homebrew-source.rb'), join(runtime, 'licenses/homebrew-source.rb'))
await writeFile(join(runtime, 'fonts/fonts.conf'), '<?xml version="1.0"?><!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd"><fontconfig><dir prefix="relative">.</dir><cachedir prefix="xdg">casy/fontconfig</cachedir></fontconfig>\n')
await writeFile(join(runtime, 'licenses/native-components.json'), JSON.stringify(components, null, 2) + '\n')
const files = []
async function inventory(directory, prefix = '') {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const relative = prefix + entry.name
    if (entry.isDirectory()) await inventory(join(directory, entry.name), relative + '/')
    else if (relative !== 'manifest.json') {
      const hash = createHash('sha256')
      for await (const chunk of createReadStream(join(runtime, relative))) hash.update(chunk)
      files.push({ path: relative, bytes: (await stat(join(runtime, relative))).size, sha256: hash.digest('hex') })
    }
  }
}
await inventory(runtime)
await writeFile(join(runtime, 'manifest.json'), JSON.stringify({ platform: process.platform, arch: process.arch, files }, null, 2) + '\n')
const env = { ...process.env, PATH: '/usr/bin:/bin' }
for (const key of ['CASY_DOC_ENGINE', 'CASY_PPOCR_MODEL_DIR', 'CASY_OCR_FONT', 'CASY_PDFTOPPM']) delete env[key]
const probe = JSON.parse(run(join(runtime, `bin/casy-doc-engine${exe}`), ['probe'], { env }))
if (!probe.available) throw new Error(`Bundled engine unavailable: ${JSON.stringify(probe)}`)
console.log(`Runtime verified: ${files.length} files, ${files.reduce((n, f) => n + f.bytes, 0)} bytes`)
