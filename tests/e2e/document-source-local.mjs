import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { spawn } from 'node:child_process'
import assert from 'node:assert/strict'
const { chromium }=await import(process.env.CASY_PLAYWRIGHT_MODULE||'playwright')
const profile=fs.mkdtempSync(path.join(process.env.CASY_QA_DIR||os.tmpdir(),'source-view-profile-'))
const binary=path.resolve('src-tauri/target/debug/examples/document_local_bridge')
const source=process.env.CASY_OCR_QA_SOURCE
assert(source,'Provide the synthetic multilingual PDF with a cross-page word')
const calls=[], errors=[]
function call(command,args={}) {
  calls.push(command)
  return new Promise((resolve,reject)=>{
    const child=spawn(binary,[],{env:{...process.env,CASY_TEST_DATA_DIR:profile},stdio:['pipe','pipe','pipe']})
    let stdout='',stderr=''
    child.stdout.on('data',c=>stdout+=c);child.stderr.on('data',c=>stderr+=c)
    child.on('error',reject)
    child.on('close',code=>{
      if(code)return reject(new Error(stderr))
      try{const result=JSON.parse(stdout);result.ok?resolve(result.data):reject(new Error(result.error))}catch(e){reject(e)}
    })
    child.stdin.end(JSON.stringify({command,args}))
  })
}
await call('qa_seed',{source})
await call('queue_document_processing',{fileId:'ocr-file'})
await call('qa_process_next')
const job=(await call('list_document_jobs',{fileId:'ocr-file'}))[0]
assert.equal(job.status,'completed',job.errorMessage)
const hits=await call('search_document_passages',{query:'Schadensersatz',scope:['ocr-file']})
const cross=hits.find(h=>h.locations.some(l=>l.pageNumber===2))
assert(cross,'Cross-page hit missing')
assert(cross.locations.every(l=>l.bbox&&l.regionIndex!==null))
const browser=await chromium.launch({channel:'chrome',headless:true})
const page=await browser.newPage({viewport:{width:1440,height:1000}})
page.on('pageerror',e=>errors.push(e.message))
await page.exposeFunction('__documentSourceCall',call)
try {
  await page.goto(process.env.CASY_QA_URL||'http://127.0.0.1:1424/')
  await page.getByRole('button',{name:'暂时跳过'}).click()
  await page.evaluate(async()=>{
    const {tryMockCommand}=await import('/src/core/mockData.ts')
    const commands=new Set(['get_case','list_case_files','list_removed_case_files','list_case_dirs','list_case_document_jobs',
      'get_document_engine_status','list_document_jobs','get_file_ocr_text','list_case_ocr_states','search_document_passages','get_document_page','correct_document_region'])
    window.__TAURI_INTERNALS__={invoke:async(command,args={})=>{
      if(commands.has(command))return window.__documentSourceCall(command,args)
      if(command==='get_settings')return {}
      if(command==='get_lawyer_profile')return {onboarding_completed:true,name:'Local QA'}
      const result=tryMockCommand(command,args)
      if(result===undefined)throw new Error('Command outside source QA: '+command)
      return result
    },transformCallback:()=>0,unregisterCallback:()=>{}}
    location.hash='/files/ocr-case'
  })
  await page.getByRole('button',{name:'卷宗检索',exact:true}).click()
  const drawer=page.locator('.reasoning-drawer')
  await drawer.getByPlaceholder('关键词或案情问题').fill('Schadensersatz')
  await drawer.getByRole('button',{name:'检索',exact:true}).click()
  const hit=drawer.locator('.passage').filter({hasText:'第 1、2 页'})
  await hit.getByRole('button',{name:'定位原文'}).click()
  const dialog=page.locator('.document-source-dialog')
  const image=dialog.locator('.source-page-image img')
  await image.waitFor()
  await image.evaluate(img=>img.decode())
  assert(await dialog.locator('.source-region.marked').count()>0)
  const pixels=await image.evaluate(img=>{
    const canvas=document.createElement('canvas');canvas.width=img.naturalWidth;canvas.height=img.naturalHeight
    const context=canvas.getContext('2d');context.drawImage(img,0,0)
    const data=context.getImageData(0,0,canvas.width,canvas.height).data
    let dark=0;for(let i=0;i<data.length;i+=4)if(data[i]<180)dark++
    return {width:canvas.width,height:canvas.height,dark}
  })
  assert(pixels.width>500&&pixels.dark>1000)
  await dialog.getByRole('button',{name:'第 2 页',exact:true}).click()
  await dialog.locator('.source-text-lines').filter({hasText:'ersatz'}).waitFor()
  assert(await dialog.locator('.source-region.marked').count()>0)
  await dialog.getByRole('button',{name:'放大',exact:true}).click()
  await dialog.getByRole('button',{name:'缩小',exact:true}).click()
  await dialog.locator('.source-text-lines button').filter({hasText:'ersatz'}).click()
  assert.equal(await dialog.locator('.source-region.selected').count(),1)
  await dialog.getByText('Markdown',{exact:true}).click()
  await dialog.locator('pre').filter({hasText:'ersatz'}).waitFor()
  await page.screenshot({path:path.join(profile,'source-desktop.png'),animations:'disabled'})
  await page.setViewportSize({width:390,height:844})
  await page.screenshot({path:path.join(profile,'source-mobile.png'),animations:'disabled'})
  const bounds=await dialog.boundingBox()
  assert(bounds.x>=0&&bounds.x+bounds.width<=391)
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth+1),false)
  await page.setViewportSize({width:1440,height:1000})
  await dialog.getByRole('button',{name:'校订选中区域'}).click()
  await dialog.getByRole('textbox',{name:'校订文字'}).fill('ersatz Prüfung - 校订测试')
  await dialog.getByRole('button',{name:'保存校订'}).click()
  await dialog.locator('pre').filter({hasText:'校订测试'}).waitFor({timeout:120000})
  const revisedJobs=await call('list_document_jobs',{fileId:'ocr-file'})
  const revised=revisedJobs.find(j=>j.engine==='paddle-onnx-corrected'&&j.status==='completed')
  assert(revised,'Correction did not create an immutable completed revision')
  const oldPage=await call('get_document_page',{fileId:'ocr-file',jobId:job.id,pageNumber:2})
  assert(!oldPage.markdown.includes('校订测试'),'Old revision was overwritten')
  const correctedHits=await call('search_document_passages',{query:'校订测试',scope:['ocr-file']})
  assert(correctedHits.some(h=>h.jobId===revised.id&&h.locations.some(l=>l.pageNumber===2)))
  await assert.rejects(call('correct_document_region',{fileId:'ocr-file',jobId:job.id,pageNumber:2,regionIndex:0,expectedText:oldPage.regions[0].text,text:'stale correction'}),/OCR_VERSION_CHANGED/)
  await page.screenshot({path:path.join(profile,'source-corrected.png'),animations:'disabled'})
  // Change only the isolated copy to prove old coordinates are never used on a new source.
  const localSource=path.join(profile,'source',path.basename(source))
  fs.appendFileSync(localSource,'\n% changed test copy\n')
  await dialog.getByRole('button',{name:'上一页',exact:true}).click()
  await dialog.getByText(/SOURCE_CHANGED/).waitFor()
  assert.equal(await dialog.locator('.source-page-image img').count(),0)
  const audit=await call('qa_audit')
  assert.equal(audit.integrity,'ok');assert.equal(audit.foreignKeyErrors,0)
  assert.deepEqual(errors,[])
  fs.writeFileSync(path.join(profile,'verification.json'),JSON.stringify({audit,job,hits,pixels,calls,errors},null,2))
  console.log(JSON.stringify({profile,audit,pixels,commands:calls.length}))
}catch(e){await page.screenshot({path:path.join(profile,'failure.png')});console.error(profile);throw e}
finally{await browser.close()}
