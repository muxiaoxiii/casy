import { mkdtemp,readFile,writeFile } from 'node:fs/promises'
import { createHash } from 'node:crypto'
import { join,resolve } from 'node:path'
import { tmpdir } from 'node:os'
import { spawn } from 'node:child_process'
import assert from 'node:assert/strict'

const playwright=await import(process.env.CASY_PLAYWRIGHT_MODULE||'playwright')
const {chromium}=playwright.default||playwright
const directory=await mkdtemp(join(process.env.CASY_QA_DIR||tmpdir(),'layout-qa-'))
const browser=await chromium.launch({channel:'chrome',headless:true})
const page=await browser.newPage()
const paragraphs=Array.from({length:7},(_,i)=>`<p>Argument ${i+1}. The original evidence establishes the date of service. The court reviews each claim and its supporting documents.</p>`).join('')
const pages=Array.from({length:4},(_,i)=>`<section><header>Evidence review and legal analysis</header><h1>Record ${i+1}</h1><main><article><h2>LEFT COLUMN START</h2>${paragraphs}<p>LEFT COLUMN END</p></article><article><h2>RIGHT COLUMN START</h2>${paragraphs}<p>RIGHT COLUMN END</p></article></main><footer>Evidence review - ${i+1}</footer></section>`).join('')
const source=join(directory,'two-column.pdf')
try {
  await page.setContent(`<style>@page{size:A4;margin:0}*{box-sizing:border-box}body{margin:0;color:#111;font:12px Arial}section{width:210mm;height:297mm;padding:15mm;page-break-after:always;position:relative}header{border-bottom:1px solid #444;padding-bottom:8px;font-size:11px}h1{font-size:22px}main{display:grid;grid-template-columns:1fr 1fr;gap:28px}h2{font-size:14px}p{line-height:1.6;margin:0 0 14px}footer{position:absolute;bottom:10mm;left:15mm}</style>${pages}`)
  await page.pdf({path:source,preferCSSPageSize:true,printBackground:true})
  await page.screenshot({path:join(directory,'fixture.png')})
} finally { await browser.close() }
const digest=buffer=>createHash('sha256').update(buffer).digest('hex')
const sourceSha256=digest(await readFile(source))
const engine=process.env.CASY_DOC_ENGINE||resolve('src-tauri/runtime/bin/casy-doc-engine')
const outputDir=join(directory,'ocr')
const start=Date.now()
const result=await new Promise((resolve,reject)=>{
  const child=spawn(engine,['process'],{env:{...process.env,PATH:'/usr/bin:/bin',RAYON_NUM_THREADS:'2'},stdio:['pipe','pipe','pipe']})
  let stdout='',stderr=''
  child.stdout.on('data',chunk=>stdout+=chunk);child.stderr.on('data',chunk=>stderr+=chunk)
  child.on('error',reject);child.on('close',code=>{
    if(code)return reject(new Error(stderr))
    try{resolve(JSON.parse(stdout))}catch(error){reject(error)}
  })
  child.stdin.end(JSON.stringify({jobId:'layout-qa',sourcePath:source,sourceSha256,outputDir}))
})
await writeFile(join(directory,'result.json'),JSON.stringify(result,null,2))
assert.equal(result.pages.length,4)
for(const page of result.pages){
  const text=page.plainText.toUpperCase()
  for(const marker of ['LEFT COLUMN START','LEFT COLUMN END','RIGHT COLUMN START','RIGHT COLUMN END'])assert(text.includes(marker),`${marker} absent on page ${page.pageNumber}: ${directory}`)
  assert(text.indexOf('LEFT COLUMN END')<text.indexOf('RIGHT COLUMN START'),`Columns interleaved on page ${page.pageNumber}: ${directory}`)
  const layout=JSON.parse(await readFile(join(outputDir,`page-${page.pageNumber}.layout.json`),'utf8'))
  assert.equal(layout.model,'pp-doclayout-plus-l')
  assert(layout.blocks.length>2)
  assert.deepEqual(layout.blocks.map(block=>block.readingOrder),layout.blocks.map((_,index)=>index))
  assert.deepEqual(page.layout,layout)
  assert(page.timing.totalMs>=page.timing.ocrMs)
}
const progress=JSON.parse(await readFile(join(outputDir,'progress.json'),'utf8'))
assert.equal(progress.phase,'completed')
assert.equal(progress.currentPage,4)
assert.equal(progress.totalPages,4)
assert(result.elapsedMs>0)
assert.equal(digest(await readFile(source)),sourceSha256)
console.log(JSON.stringify({directory,pages:result.pages.length,elapsedSeconds:(Date.now()-start)/1000,sourceSha256}))
