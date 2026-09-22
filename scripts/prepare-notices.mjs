import { spawnSync } from 'node:child_process'
import { mkdir, readFile, readdir, copyFile, stat, writeFile, rm } from 'node:fs/promises'
import { dirname, join, resolve } from 'node:path'
import { createHash } from 'node:crypto'
import { sourceArchiveRequired } from './license-policy.mjs'
import { fileURLToPath } from 'node:url'

const project = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const output = join(project, 'src-tauri/runtime/licenses/packages')
const run = (command, args) => {
  const result = spawnSync(command, args, { cwd: project, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 })
  if (result.status !== 0) throw new Error(`${command}: ${result.stderr}`)
  return result.stdout
}
const exists = async path => Boolean(await stat(path).catch(() => null))
await mkdir(output, { recursive: true })
const packages = new Map()
const target = run('rustc', ['-vV']).match(/^host: (.+)$/m)[1]
for (const manifest of ['src-tauri/Cargo.toml', 'tools/casy-doc-engine/Cargo.toml']) {
  const flags = manifest.startsWith('tools/') ? ['--features', 'models'] : []
  const metadata = JSON.parse(run('cargo', ['metadata', '--locked', '--offline', '--format-version', '1', '--filter-platform', target, '--manifest-path', manifest, ...flags]))
  const resolved = new Set(metadata.resolve.nodes.map(node => node.id))
  for (const pkg of metadata.packages) {
    if (!pkg.source || !resolved.has(pkg.id)) continue
    packages.set(`rust-${pkg.name}-${pkg.version}`, { name: pkg.name, version: pkg.version, ecosystem: 'cargo', license: pkg.license, repository: pkg.repository, root: dirname(pkg.manifest_path) })
  }
}
const lock = JSON.parse(await readFile(join(project, 'package-lock.json'), 'utf8'))
for (const [location, pkg] of Object.entries(lock.packages)) {
  if (!location || pkg.dev) continue
  const root = join(project, location)
  if (!await exists(join(root, 'package.json'))) continue
  const installed = JSON.parse(await readFile(join(root, 'package.json'), 'utf8'))
  packages.set(`npm-${installed.name.replaceAll('/', '_')}-${installed.version}`, { name: installed.name, version: installed.version, ecosystem: 'npm', license: installed.license, repository: installed.repository, root })
}
for (const entry of await readdir(output, { withFileTypes: true })) {
  if (entry.isDirectory() && /^(rust-|npm-)/.test(entry.name) && !packages.has(entry.name))
    await rm(join(output, entry.name), { recursive: true, force: true })
}
const cache = join(project,'src-tauri/target/runtime-cache/license-sources')
await mkdir(cache,{recursive:true})
async function copyNotices(root, destination, relative='') {
  const copied=[]
  for(const entry of await readdir(join(root,relative),{withFileTypes:true})) {
    if(['node_modules','target','.git'].includes(entry.name))continue
    const path=join(relative,entry.name)
    if(entry.isDirectory())copied.push(...await copyNotices(root,destination,path))
    else if(/(^|[._-])(licen[cs]e|copying|notice|copyright|authors)([._-]|$)/i.test(entry.name)) {
      await mkdir(dirname(join(destination,path)),{recursive:true})
      await copyFile(join(root,path),join(destination,path));copied.push(path)
    }
  }
  return copied
}
const entries = []
for (const [id, pkg] of packages) {
  const directory = join(output, id)
  await mkdir(directory, { recursive: true })
  const notices = await copyNotices(pkg.root,directory)
  if (pkg.name === '@excalidraw/excalidraw') {
    if (pkg.version !== '0.18.1') throw new Error('Review the vendored Excalidraw license for the new version')
    await copyFile(join(project, 'vendor/excalidraw/LICENSE'), join(directory, 'LICENSE'))
    if (!notices.includes('LICENSE')) notices.push('LICENSE')
  }
  const bundledSource = join(directory, 'source.tar.gz')
  const required = sourceArchiveRequired(pkg.license) || notices.length === 0
  const source = required ? bundledSource : join(cache,`${id}.tar.gz`)
  if(!required && await exists(bundledSource)) {
    await copyFile(bundledSource,source)
    await rm(bundledSource)
  }
  if (!await exists(source)) run('tar', ['-czf', source, '--exclude=node_modules', '--exclude=target', '--exclude=.git', '-C', pkg.root, '.'])
  const sourceSha256=createHash('sha256').update(await readFile(source)).digest('hex')
  const { root, ...entry } = pkg
  entries.push({ ...entry, notices: notices.map(name => `${id}/${name}`), source: required ? `${id}/source.tar.gz` : null, sourceDelivery: required ? 'bundled' : 'build-cache', sourceReason: sourceArchiveRequired(pkg.license) ? 'license-requires-conservative-delivery' : notices.length === 0 ? 'no-standalone-notice' : 'permissive-with-notices', sourceSha256, sourceUrl: pkg.ecosystem==='cargo' ? `https://crates.io/api/v1/crates/${pkg.name}/${pkg.version}/download` : `https://www.npmjs.com/package/${pkg.name}/v/${pkg.version}` })
}
await writeFile(join(output, 'index.json'), JSON.stringify(entries, null, 2) + '\n')
await writeFile(join(output, 'NOTICE.txt'), 'Casy third-party dependencies\n\nindex.json identifies resolved Cargo and installed production npm packages. Each directory contains available license notices, including nested notices. Copyleft, custom, unknown license packages and packages without standalone notices retain exact installed source archives. Recognized permissive-license source archives remain in the build cache; index.json records their version, upstream source and archive hash. Entries conservatively include dependencies which may be unused by the final binary. Native renderer sources and font/model notices are in the parent directory.\n')
console.log(`Packaged notices and sources for ${entries.length} dependencies`)
