import { createHash } from 'node:crypto'
import { createReadStream, createWriteStream } from 'node:fs'
import { mkdir, rename, rm, stat } from 'node:fs/promises'
import { resolve, join } from 'node:path'
import { Readable } from 'node:stream'
import { pipeline } from 'node:stream/promises'

const root = process.argv[2]
if (!root) throw new Error('Usage: node install-models.mjs /absolute/model-directory')
const selected = process.argv[3] || 'medium'
if (!['medium', '--dry-run'].includes(selected)) throw new Error('VL profiles are paused; use the PP-OCRv6 medium default.')
const det = 'https://huggingface.co/PaddlePaddle/PP-OCRv6_medium_det_onnx/resolve/61323801669c338b7891481ec7bac61ce31b576a/'
const rec = 'https://huggingface.co/PaddlePaddle/PP-OCRv6_medium_rec_onnx/resolve/50c7eacafc52fa7bcf4194e8cd08e46f8558504b/'
const assets = [
  ['embedding-e5-base/model_int8.onnx', 'https://huggingface.co/Xenova/multilingual-e5-base/resolve/1ec9243030a27d1a115d5c340572074c125b58b2/onnx/model_int8.onnx', 'sha256', '9ddfd8b45086dabc59a7e1bb00463225dace8954962418b240840f2153bc87da', 278184162],
  ['embedding-e5-base/tokenizer.json', 'https://huggingface.co/Xenova/multilingual-e5-base/resolve/1ec9243030a27d1a115d5c340572074c125b58b2/tokenizer.json', 'sha256', '62c24cdc13d4c9952d63718d6c9fa4c287974249e16b7ade6d5a85e7bbb75626', 17082660],
  ['ppocrv6-medium/det.onnx', det + 'inference.onnx', 'sha256', 'eb13b44b25bb36f89528b68720af8a61d9cf381176107f465db1757b65d086e1', 62032837],
  ['ppocrv6-medium/rec.onnx', rec + 'inference.onnx', 'sha256', '9c09abf0957f7968c7586464b7397b84ad2387a0497a351af40e9acc71b673ba', 76554979],
  ['ppocrv6-medium/det.yml', det + 'inference.yml', 'sha1', '1c5c05809877e4c7385f899019fff0ac9017ca80', 886],
  ['ppocrv6-medium/rec.yml', rec + 'inference.yml', 'sha1', 'c53a96fcd315a86cb4748d2746f3d90941e1c6d8', 150580],
  ['layout/pp-doclayout_plus-l.onnx', 'https://huggingface.co/PaddlePaddle/PP-DocLayout_plus-L_onnx/resolve/feb74619326f634e0e883218598096a3733ad9f7/inference.onnx', 'sha256', '77afb2caa74dd13240d087d2eced91d7fcd2caebd16006a0a66162fc8707ff0e', 129736329],
  ['layout/inference.yml', 'https://huggingface.co/PaddlePaddle/PP-DocLayout_plus-L_onnx/resolve/feb74619326f634e0e883218598096a3733ad9f7/inference.yml', 'sha1', '9a236587eae068a1e7906fd158f712dc41400563', 1838],
]
console.log('OCR, layout and multilingual E5 embedding assets: ' + assets.reduce((sum, asset) => sum + asset[4], 0) + ' bytes. Licenses: Apache-2.0 (Paddle), MIT (E5).')
if (selected === '--dry-run') process.exit(0)

async function verified(path, algorithm, expected, size) {
  if ((await stat(path).catch(() => null))?.size !== size) return false
  const hash = createHash(algorithm)
  if (algorithm === 'sha1') hash.update('blob ' + size + '\0')
  for await (const chunk of createReadStream(path)) hash.update(chunk)
  return hash.digest('hex') === expected
}

for (const [relative, url, algorithm, expected, size] of assets) {
  const path = resolve(root, relative)
  await mkdir(join(path, '..'), { recursive: true })
  if (await verified(path, algorithm, expected, size)) {
    console.log('Verified ' + relative)
    continue
  }
  const temporary = path + '.partial'
  try {
    console.log('Downloading ' + relative + ' (' + size + ' bytes)')
    const response = await fetch(url, { signal: AbortSignal.timeout(1800000) })
    if (!response.ok || !response.body) throw new Error('Download HTTP ' + response.status)
    await pipeline(Readable.fromWeb(response.body), createWriteStream(temporary))
    if (!await verified(temporary, algorithm, expected, size)) throw new Error('Checksum mismatch: ' + relative)
    await rename(temporary, path)
  } finally {
    await rm(temporary, { force: true })
  }
}
console.log('Verified model assets: ' + resolve(root))
