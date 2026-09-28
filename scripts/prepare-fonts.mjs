// Shared by CI tests and full desktop packaging; fonts are hash-pinned.
import { createHash } from 'node:crypto'
import { createReadStream, createWriteStream } from 'node:fs'
import { mkdir, stat } from 'node:fs/promises'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { pipeline } from 'node:stream/promises'
import { Readable } from 'node:stream'
const project = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const runtime = join(project, 'src-tauri/runtime')
for (const folder of ['fonts', 'licenses']) await mkdir(join(runtime, folder), { recursive: true })
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
// Static outlines are required for consistent Typst SVG/PDF rendering. Keep the existing variable font for other consumers.
await pinnedDownload('fonts/NotoSansCJKsc-Regular.otf', fontRoot + 'Sans/OTF/SimplifiedChinese/NotoSansCJKsc-Regular.otf', 16437364, 'dc15562470b4f842321894787a0d066879ccff8b')
await pinnedDownload('licenses/Noto-OFL.txt', fontRoot + 'Sans/LICENSE', 4301, 'd952d62c065f3f35fb83a173496e90b21525aef3')

