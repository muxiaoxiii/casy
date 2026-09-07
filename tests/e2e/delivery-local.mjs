import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { spawn } from 'node:child_process'
import assert from 'node:assert/strict'

const { chromium } = await import(process.env.CASY_PLAYWRIGHT_MODULE || 'playwright')
const profile=fs.mkdtempSync(path.join(process.env.CASY_QA_DIR||os.tmpdir(),'delivery-ui-'))
const archive=path.join(path.dirname(profile),path.basename(profile)+'.casy')
const binary=path.resolve('src-tauri/target/debug/examples/delivery_local_bridge')
const errors=[],calls=[]
let queue=Promise.resolve(),failNext=false
function call(command,args={}) {
  calls.push(command)
  const result=queue.then(()=>new Promise((resolve,reject)=>{
    const child=spawn(binary,[],{env:{...process.env,CASY_TEST_DATA_DIR:profile},stdio:['pipe','pipe','pipe']})
    let output='',stderr=''
    child.stdout.on('data',value=>output+=value);child.stderr.on('data',value=>stderr+=value)
    child.on('error',reject);child.on('close',code=>{
      if(code)return reject(new Error(stderr))
      try { const value=JSON.parse(output);value.ok?resolve(value.data):reject(new Error(value.error)) } catch(error){reject(error)}
    })
    child.stdin.end(JSON.stringify({command,args}))
  }))
  queue=result.catch(()=>{})
  return result
}
const first=await call('create_draft',{title:'交付文书甲',content:'<p>原始正文</p>'})
const second=await call('create_draft',{title:'交付文书乙',content:'<p>乙正文</p>'})
const browser=await chromium.launch({channel:'chrome',headless:true})
const page=await browser.newPage({viewport:{width:1440,height:1000}})
page.on('pageerror',error=>errors.push(error.message))
await page.exposeFunction('__deliveryCall',(command,args)=>{
  if(command==='update_draft'&&failNext){failNext=false;throw new Error('合成测试：文书保存失败')}
  return call(command,args)
})
try {
  await page.goto(process.env.CASY_QA_URL||'http://127.0.0.1:1424/')
  await page.getByRole('button',{name:'稍后再填'}).click()
  await page.evaluate(async archive=>{
    const {tryMockCommand}=await import('/src/core/mockData.ts')
    const native=new Set(['list_backups','export_full_backup','import_full_backup','list_drafts','get_draft','create_draft','update_draft','save_editor_recovery','recover_editor_drafts'])
    window.__TAURI_INTERNALS__={invoke:async(command,args={})=>{
      if(native.has(command))return window.__deliveryCall(command,args)
      if(command==='plugin:dialog|save'||command==='plugin:dialog|open')return archive
      if(command==='get_settings')return {}
      if(command==='get_email_monitor_status')return {running:false,accountCount:0}
      if(command==='get_lawyer_profile')return {onboarding_completed:true,name:'Local QA'}
      return tryMockCommand(command,args)??null
    },transformCallback:()=>0,unregisterCallback:()=>{}}
    location.hash='/docs'
  },archive)
  const draft=title=>page.locator('.draft-item').filter({hasText:title})
  await draft('交付文书甲').click()
  await page.locator('.notion-title-input').fill('交付文书甲已修改')
  await page.locator('.tiptap').fill('末尾追加的文书内容')
  await draft('交付文书乙').click()
  await page.waitForFunction(()=>document.querySelector('.notion-title-input')?.value==='交付文书乙')
  assert((await call('get_draft',{id:first.id})).content.includes('末尾追加'))
  assert.equal((await call('get_draft',{id:first.id})).title,'交付文书甲已修改')
  failNext=true
  await page.locator('.tiptap').fill('保存失败仍保留的内容')
  await draft('交付文书甲已修改').click()
  await page.getByText('合成测试：文书保存失败',{exact:true}).first().waitFor()
  assert.equal(await page.locator('.notion-title-input').inputValue(),'交付文书乙')
  await draft('交付文书甲已修改').click()
  await page.waitForFunction(()=>document.querySelector('.notion-title-input')?.value==='交付文书甲已修改')
  assert((await call('get_draft',{id:second.id})).content.includes('保存失败仍保留'))
  await page.screenshot({path:path.join(profile,'drafts.png')})
  await page.evaluate(()=>{location.hash='/write'})
  await page.locator('.title-input input').fill('写作页离开保存')
  await page.locator('.tiptap').fill('切换页面前输入的最后一句')
  await page.evaluate(()=>{location.hash='/settings'})
  await page.getByText('数据备份',{exact:true}).filter({visible:true}).click()
  const writing=(await call('list_drafts')).find(draft=>draft.title==='写作页离开保存')
  assert(writing?.content.includes('切换页面前输入的最后一句'))
  await page.getByRole('button',{name:'导出完整备份',exact:true}).click()
  const dialog=page.getByRole('dialog',{name:'导出完整备份',exact:true})
  await dialog.locator('input').nth(0).fill('delivery-ui-password')
  await dialog.locator('input').nth(1).fill('different-password')
  await dialog.getByRole('button',{name:'加密并导出'}).click()
  await page.getByText('密码至少 10 个字符，两次输入必须一致',{exact:true}).waitFor()
  assert(!fs.existsSync(archive))
  await dialog.locator('input').nth(1).fill('delivery-ui-password')
  await dialog.getByRole('button',{name:'加密并导出'}).click()
  await page.getByText('完整加密备份已保存',{exact:true}).waitFor({timeout:60000})
  assert(fs.statSync(archive).size>1000)
  await call('update_draft',{id:first.id,content:'备份之后的修改'})
  await page.getByRole('button',{name:'从完整备份恢复'}).click()
  const restore=page.getByRole('dialog',{name:'恢复完整备份',exact:true})
  await restore.locator('input').fill('wrong-password')
  await restore.getByRole('button',{name:'恢复数据'}).click()
  await page.locator('.el-message--error').filter({hasText:/密码|解密|decrypt/}).waitFor({timeout:60000})
  assert.equal((await call('get_draft',{id:first.id})).content,'备份之后的修改')
  await restore.locator('input').fill('delivery-ui-password')
  await page.screenshot({path:path.join(profile,'backup-dialog.png')})
  await restore.getByRole('button',{name:'恢复数据'}).click()
  await page.getByRole('button',{name:'重新加载',exact:true}).waitFor({timeout:60000})
  assert((await call('get_draft',{id:first.id})).content.includes('末尾追加'))
  const audit=await call('qa_audit')
  assert.deepEqual(audit,{integrity:'ok',foreignKeyErrors:0})
  assert.deepEqual(errors,[])
  fs.writeFileSync(path.join(profile,'verification.json'),JSON.stringify({audit,calls,errors},null,2))
  console.log(JSON.stringify({profile,archive,audit,commands:calls.length}))
}catch(error){await page.screenshot({path:path.join(profile,'failure.png')});console.error(profile);throw error}
finally{await browser.close()}
