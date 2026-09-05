import { createHash } from 'node:crypto'
import { createReadStream, createWriteStream } from 'node:fs'
import { mkdir, rename, rm, stat } from 'node:fs/promises'
import { resolve, join } from 'node:path'
import { Readable } from 'node:stream'
import { pipeline } from 'node:stream/promises'

const root = process.argv[2]
if (!root) throw new Error('Usage: node install-models.mjs /absolute/model-directory')
const selected = process.argv[3] || 'paddle'
if (!['paddle', 'ovis', 'all'].includes(selected)) throw new Error('Model must be paddle, ovis or all')
const revision = '1fc9221b7823a371d6e97f92d527cc847e24e107'
const ovis = 'https://huggingface.co/ATH-MaaS/OvisOCR2/resolve/' + revision + '/'
const ppocr = 'https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/'
const paddle = 'https://huggingface.co/PaddlePaddle/PaddleOCR-VL/resolve/7fa00a8c55b735ba51ba49a9058f3f9c57a99a11/'
const assets = [
  ['ppocr/det.onnx', ppocr + 'pp-ocrv5_mobile_det.onnx', 'sha256', '1eb7b4f7ab657ebd1c66d5f79bca7497f29768a2e3c15e52daecbba1a8e4a039', 4826518],
  ['ppocr/rec.onnx', ppocr + 'pp-ocrv5_mobile_rec.onnx', 'sha256', '243a0f06d826761323e9045e9b113ab2c191c3aa50565585e628300b8eda0224', 16562373],
  ['ppocr/dict.txt', ppocr + 'ppocrv5_dict.txt', 'sha256', 'd1979e9f794c464c0d2e0b70a7fe14dd978e9dc644c0e71f14158cdf8342af1b', 74012],
  ['ovisocr2/config.json', ovis + 'config.json', 'sha1', '715f0448b9d38103211f0ad88bbb4d6e4f4be8c9', 2907],
  ['ovisocr2/preprocessor_config.json', ovis + 'preprocessor_config.json', 'sha1', '2ea84a437d448ff71b08df68fdd949d5cc4ebb64', 390],
  ['ovisocr2/tokenizer.json', ovis + 'tokenizer.json', 'sha256', '5f9e4d4901a92b997e463c1f46055088b6cca5ca61a6522d1b9f64c4bb81cb42', 12807982],
  ['ovisocr2/model.safetensors', ovis + 'model.safetensors', 'sha256', '9270560288656ece5cb3a6989001afcf5af8d223bceed4a423c33a008861d009', 1706030496],
  ['ovisocr2/LICENSE', ovis + 'LICENSE', 'sha1', 'f938136e3adacfd92be087f6e113b5d6d97f678f', 11544],
  ['paddleocr-vl/config.json', paddle + 'config.json', 'sha1', 'c54711466ae75457f8e57b909a49dae570bfa5c5', 2059],
  ['paddleocr-vl/preprocessor_config.json', paddle + 'preprocessor_config.json', 'sha1', '039dc0b08e9940654682d5415c2178e3275644d7', 641],
  ['paddleocr-vl/tokenizer.json', paddle + 'tokenizer.json', 'sha256', 'f90f04fd8e5eb6dfa380f37d10c87392de8438dccb6768a2486b5a96ee76dba6', 11187679],
  ['paddleocr-vl/model.safetensors', paddle + 'model.safetensors', 'sha256', '3085f1042e184f68f8a412aa0f64f2c4b8562989598bbfba326aaa11fc685de8', 1917255968],
  ['paddleocr-vl/LICENSE', paddle + 'LICENSE', 'sha1', '0491a00e80b424eb709078b57e35d8b83ffee985', 11376],
]

async function verified(path, algorithm, expected, size) {
  if ((await stat(path).catch(() => null))?.size !== size) return false
  const hash = createHash(algorithm)
  if (algorithm === 'sha1') hash.update('blob ' + size + '\0')
  for await (const chunk of createReadStream(path)) hash.update(chunk)
  return hash.digest('hex') === expected
}

for (const [relative, url, algorithm, expected, size] of assets) {
  if (relative.startsWith('ovisocr2/') && selected !== 'ovis' && selected !== 'all') continue
  if (relative.startsWith('paddleocr-vl/') && selected === 'ovis') continue
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
