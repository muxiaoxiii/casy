// 准备向量模型（默认 multilingual-e5-small-int8，384 维，约 135MB 含 tokenizer）。
//
// 模型源：Hugging Face（Xenova/multilingual-e5-small 的量化 ONNX + intfloat 官方
// tokenizer），sha256 校验。e5-base（768 维，约 281MB）为可选档，用 --tier base 下载。
//
// 落盘布局（引擎与父进程按此解析）：
//   src-tauri/runtime/models/embedding-e5-small/{model_int8.onnx,tokenizer.json}
//   src-tauri/runtime/models/embedding-e5-base/{model_int8.onnx,tokenizer.json}
//
// 用法：
//   node scripts/prepare-embed-models.mjs            # 默认 e5-small
//   node scripts/prepare-embed-models.mjs --tier base
import { createHash } from 'node:crypto'
import { readFile, mkdir, rename, stat } from 'node:fs/promises'
import { join, dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { execFileSync } from 'node:child_process'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const HF = 'https://huggingface.co'

// [目标名, 字节数, sha256, 下载 URL]
const TIERS = {
  small: [
    ['model_int8.onnx', 118308185, 'f80102d3f2a1229f387d3c81909990d8945513e347b0eab049f7de3c6f98c193',
      `${HF}/Xenova/multilingual-e5-small/resolve/main/onnx/model_quantized.onnx`],
    ['tokenizer.json', 17082730, '0b44a9d7b51c3c62626640cda0e2c2f70fdacdc25bbbd68038369d14ebdf4c39',
      `${HF}/intfloat/multilingual-e5-small/resolve/main/tokenizer.json`],
  ],
}

const args = process.argv.slice(2)
const tierIndex = args.indexOf('--tier')
const tier = tierIndex >= 0 ? args[tierIndex + 1] : 'small'
if (!TIERS[tier]) {
  throw new Error(`未知向量模型档位: ${tier}（可用: ${Object.keys(TIERS).join(', ')}；e5-base 为旧默认，随历史包分发）`)
}

const destination = join(root, 'src-tauri/runtime/models', `embedding-e5-${tier}`)
await mkdir(destination, { recursive: true })
const cache = join(root, 'src-tauri/target/runtime-cache/embed-models')
await mkdir(cache, { recursive: true })

async function sha256(path) {
  return createHash('sha256').update(await readFile(path)).digest('hex')
}

for (const [name, size, hash, url] of TIERS[tier]) {
  const target = join(destination, name)
  const existing = await stat(target).then((s) => s.size, () => -1)
  if (existing === size && (await sha256(target)) === hash) {
    console.log(`[embed-models] e5-${tier}/${name} 已就绪`)
    continue
  }
  const archive = join(cache, `e5-${tier}-${name}`)
  const cached = await stat(archive).then((s) => s.size, () => -1)
  if (!(cached === size && (await sha256(archive)) === hash)) {
    execFileSync('curl', ['--fail', '--location', '--retry', '2', '--max-time', '600',
      '--output', archive, url], { stdio: 'inherit' })
  }
  if (!((await stat(archive).then((s) => s.size, () => -1)) === size && (await sha256(archive)) === hash)) {
    throw new Error(`[embed-models] ${name} 校验失败（大小或 sha256 不匹配）`)
  }
  await rename(archive, target)
  console.log(`[embed-models] e5-${tier}/${name} 已安装（${(size / 1048576).toFixed(1)}MB）`)
}
console.log(`[embed-models] 完成：e5-${tier}`)
