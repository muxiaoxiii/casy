import { spawnSync } from 'node:child_process'
import { mkdir, readFile, readdir, copyFile, stat, writeFile } from 'node:fs/promises'
import { dirname, join, resolve } from 'node:path'
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
const entries = []
for (const [id, pkg] of packages) {
  const directory = join(output, id)
  await mkdir(directory, { recursive: true })
  const notices = []
  for (const entry of await readdir(pkg.root, { withFileTypes: true })) {
    if (entry.isFile() && /^(license|copying|notice|copyright|authors)([._-]|$)/i.test(entry.name)) {
      await copyFile(join(pkg.root, entry.name), join(directory, entry.name))
      notices.push(entry.name)
    }
  }
  // Include exact installed sources, including nested license notices.
  const source = join(directory, 'source.tar.gz')
  if (!await exists(source)) run('tar', ['-czf', source, '--exclude=node_modules', '--exclude=target', '--exclude=.git', '-C', pkg.root, '.'])
  const { root, ...entry } = pkg
  entries.push({ ...entry, notices: notices.map(name => `${id}/${name}`), source: `${id}/source.tar.gz` })
}
await writeFile(join(output, 'index.json'), JSON.stringify(entries, null, 2) + '\n')
await writeFile(join(output, 'NOTICE.txt'), 'Casy third-party dependencies\n\nindex.json identifies resolved Cargo and installed production npm packages. Each directory contains available top-level license notices and exact installed package sources, including nested notices. Entries conservatively include dependencies which may be unused by the final binary. Native renderer sources and font/model notices are in the parent directory.\n')
console.log(`Packaged notices and sources for ${entries.length} dependencies`)
