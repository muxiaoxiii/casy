// 准备 OCR 识别模型档位（PP-OCRv6 tiny / small / medium）。
//
// 模型源：mgchao/SnowShotOCR（ModelScope），与 Snow Shot 同一份 sha256 校验清单。
// 落盘布局（引擎与父进程按此解析）：
//   src-tauri/runtime/models/ocr/<tier>/det.onnx
//   src-tauri/runtime/models/ocr/<tier>/rec.onnx
//   src-tauri/runtime/models/ocr/<tier>/dict.txt
//
// 用法：
//   node scripts/prepare-ocr-models.mjs            # 默认 small（约 30MB）
//   node scripts/prepare-ocr-models.mjs --tier medium
//   node scripts/prepare-ocr-models.mjs --all      # 三个档位全部准备
//
// 档位说明（多语言实测）：
//   small  与 medium 使用同一份 18709 字符字库——日/韩/法/德/中/英覆盖一致，约 30MB。
//   medium 质量最高，约 132MB，按需下载。
//   tiny   字库仅 6905 字符：缺少全部日文假名与约 1 万生僻汉字（法/德/韩/中/英在），
//          仅建议纯中文/英文场景使用，约 6MB。
import { createHash } from 'node:crypto'
import { readFile, mkdir, rename, rm, stat } from 'node:fs/promises'
import { join, dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { execFileSync } from 'node:child_process'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const BASE = 'https://www.modelscope.cn/models/mgchao/SnowShotOCR/resolve/master'

// [源目录, 目标名, 字节数, sha256]
const TIERS = {
  tiny: [
    ['PP-OCRv6/tiny', 'det.onnx', 1829618, 'f42c0fbd294d95eac1a550e131b277dac97462c8025fa4b6c3cec1b7894bd3d5', 'PP-OCRv6_det_tiny.onnx'],
    ['PP-OCRv6/tiny', 'rec.onnx', 4489813, 'e16e242de5937ad92609223f19bc2aff3727ee40b095f996907c24749bad251b', 'PP-OCRv6_rec_tiny.onnx'],
    ['PP-OCRv6/tiny', 'dict.txt', 27156, 'c5cbe34ef40c29c4df07ed012bf96569cb69a2d2a01a07027e9f13cb832bd9cd', 'ppocrv6_tiny_dict.txt'],
  ],
  small: [
    ['PP-OCRv6/small', 'det.onnx', 9929594, '090f04abcd9d9a7498bc4ebf677e4cb9bdce1fe4197ddb7e529f1ef44e1ff94f', 'PP-OCRv6_det_small.onnx'],
    ['PP-OCRv6/small', 'rec.onnx', 21234383, '6f327246b50388f3c176ae304bd95767ea6dc0c9ae92153ef8cbe210b3c14884', 'PP-OCRv6_rec_small.onnx'],
    ['PP-OCRv6/small', 'dict.txt', 74947, 'b5f2bfe2bdd9448429e3e82b51c789775d9b42f2403d082b00662eb77e401c5d', 'ppocrv6_dict.txt'],
  ],
  medium: [
    ['PP-OCRv6/medium', 'det.onnx', 62119454, '92078b7355007ccfffcd4c8cd441a3afd4538904d06881b29a155e1e679907c2', 'PP-OCRv6_det_medium.onnx'],
    ['PP-OCRv6/medium', 'rec.onnx', 76629984, 'eef444829dbbe18d7fea59a3f6eb75647518d2b3a9568d27c92e42940204894b', 'PP-OCRv6_rec_medium.onnx'],
    ['PP-OCRv6/medium', 'dict.txt', 74947, 'b5f2bfe2bdd9448429e3e82b51c789775d9b42f2403d082b00662eb77e401c5d', 'ppocrv6_dict.txt'],
  ],
}

const args = process.argv.slice(2)
const tierIndex = args.indexOf('--tier')
const requested = tierIndex >= 0 ? args[tierIndex + 1] : 'small'
const tiers = args.includes('--all') ? Object.keys(TIERS) : [requested]
for (const tier of tiers) {
  if (!TIERS[tier]) throw new Error(`未知档位: ${tier}（可用: ${Object.keys(TIERS).join(', ')}）`)
}

const cache = join(root, 'src-tauri/target/runtime-cache/ocr-models')
await mkdir(cache, { recursive: true })

async function sha256(path) {
  return createHash('sha256').update(await readFile(path)).digest('hex')
}

for (const tier of tiers) {
  const destination = join(root, 'src-tauri/runtime/models/ocr', tier)
  await mkdir(destination, { recursive: true })
  for (const [sourceDir, targetName, size, hash, sourceName] of TIERS[tier]) {
    const target = join(destination, targetName)
    const existing = await stat(target).then((s) => s.size, () => -1)
    if (existing === size && (await sha256(target)) === hash) {
      console.log(`[ocr-models] ${tier}/${targetName} 已就绪`)
      continue
    }
    const archive = join(cache, sourceName)
    const cached = await stat(archive).then((s) => s.size, () => -1)
    if (!(cached === size && (await sha256(archive)) === hash)) {
      execFileSync('curl', ['--fail', '--location', '--retry', '2', '--max-time', '600',
        '--output', archive, `${BASE}/${sourceDir}/${sourceName}`], { stdio: 'inherit' })
    }
    if (!((await stat(archive).then((s) => s.size, () => -1)) === size && (await sha256(archive)) === hash)) {
      throw new Error(`[ocr-models] ${sourceName} 校验失败（大小或 sha256 不匹配）`)
    }
    await rename(archive, target)
    console.log(`[ocr-models] ${tier}/${targetName} 已安装（${(size / 1048576).toFixed(1)}MB）`)
  }
}

const installed = tiers.includes('tiny')
  ? '\n[ocr-models] 注意：tiny 档字库不含日文假名，日文文档请使用 small/medium。'
  : ''
console.log(`[ocr-models] 完成：${tiers.join(', ')}${installed}`)
