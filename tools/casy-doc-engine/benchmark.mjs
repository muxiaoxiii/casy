import { createHash } from 'node:crypto'
import { createReadStream } from 'node:fs'
import { mkdtemp, writeFile, readFile } from 'node:fs/promises'
import { join, resolve } from 'node:path'
import { spawn, execFileSync } from 'node:child_process'
import assert from 'node:assert/strict'

const [sourceArg, outputRoot] = process.argv.slice(2)
assert(sourceArg && outputRoot && process.env.CASY_DOC_ENGINE, 'Usage: CASY_DOC_ENGINE=... node benchmark.mjs source.pdf /existing/private/output-root')
const sourcePath = resolve(sourceArg)
const outputDir = await mkdtemp(join(resolve(outputRoot), 'ocr-benchmark-'))
async function hash(path) {
  const digest = createHash('sha256')
  for await (const chunk of createReadStream(path)) digest.update(chunk)
  return digest.digest('hex')
}
const sourceSha256 = await hash(sourcePath)
const request = {
  jobId: 'benchmark', sourcePath, sourceSha256, outputDir,
  coordinateModelDir: process.env.CASY_PPOCR_MODEL_DIR,
  paddleModelDir: process.env.CASY_PADDLEOCR_VL_MODEL_DIR,
  ovisModelDir: process.env.CASY_OVISOCR2_MODEL_DIR,
  cjkFontPath: process.env.CASY_OCR_FONT, device: process.env.CASY_OCR_DEVICE || 'cpu',
}
const start = Date.now()
const child = spawn('/usr/bin/time', ['-l', process.env.CASY_DOC_ENGINE, 'process'], {
  env: { ...process.env, RAYON_NUM_THREADS: '2' }, stdio: ['pipe', 'pipe', 'pipe'], detached: true,
})
let stdout = '', stderr = ''
child.stdout.on('data', data => { stdout += data })
child.stderr.on('data', data => { stderr += data })
child.stdin.end(JSON.stringify(request))
const timeout = setTimeout(() => { try { process.kill(-child.pid, 'SIGTERM') } catch {} }, 30 * 60 * 1000)
try {
  const code = await new Promise((resolve, reject) => { child.on('error', reject); child.on('close', resolve) })
  await writeFile(join(outputDir, 'runtime.txt'), stderr)
  assert.equal(code, 0, stderr)
  const result = JSON.parse(stdout)
  assert.equal(result.sourceSha256, sourceSha256)
  assert.equal(await hash(sourcePath), sourceSha256)
  const inputInfo = execFileSync('pdfinfo', [sourcePath], { encoding: 'utf8' })
  const outputInfo = execFileSync('pdfinfo', [result.searchablePdfPath], { encoding: 'utf8' })
  const totalPages = Number(inputInfo.match(/^Pages:\s+(\d+)/m)?.[1])
  assert.equal(result.pages.length, totalPages)
  assert.equal(Number(outputInfo.match(/^Pages:\s+(\d+)/m)?.[1]), totalPages)
  assert.deepEqual(JSON.parse(await readFile(result.pageIrPath, 'utf8')), result.pages)
  await writeFile(join(outputDir, 'result.json'), JSON.stringify(result, null, 2))
  const summary = {
    outputDir, sourceSha256, totalPages, elapsedSeconds: (Date.now() - start) / 1000,
    maxResidentBytes: Number(stderr.match(/(\d+)\s+maximum resident set size/)?.[1]),
    peakFootprintBytes: Number(stderr.match(/(\d+)\s+peak memory footprint/)?.[1]),
    pageStats: result.pages.map(page => ({
      pageNumber: page.pageNumber, textLength: page.plainText.length, markdownLength: page.markdown.length,
      confidence: page.confidence, regions: page.regions.length,
    })),
  }
  await writeFile(join(outputDir, 'benchmark.json'), JSON.stringify(summary, null, 2))
  console.log(JSON.stringify(summary))
} finally {
  clearTimeout(timeout)
}
