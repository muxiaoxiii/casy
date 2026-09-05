import fs from 'node:fs'
import path from 'node:path'
import { spawn } from 'node:child_process'
import assert from 'node:assert/strict'

const { chromium } = await import(process.env.CASY_PLAYWRIGHT_MODULE || 'playwright')
const privateDir = process.env.CASY_QA_DIR
assert(privateDir, 'CASY_QA_DIR is required')
const root = process.cwd()
const snapshotPath = path.join(privateDir,'snapshot.json')
const snapshot = JSON.parse(fs.readFileSync(snapshotPath,'utf8'))
const profile = fs.mkdtempSync(path.join(privateDir,'ui-profile-'))
let queue = Promise.resolve()
function call(command,args={}) {
  const task = queue.then(()=>new Promise((resolve,reject)=>{
    const child = spawn(path.join(root,'src-tauri/target/debug/examples/intake_local_bridge'),[],{
      env:{...process.env,CASY_TEST_DATA_DIR:profile},stdio:['pipe','pipe','pipe'],
    })
    let stdout='',stderr=''
    child.stdout.on('data',chunk=>stdout+=chunk)
    child.stderr.on('data',chunk=>stderr+=chunk)
    child.on('error',reject)
    child.on('close',code=>{
      if(code) return reject(new Error(stderr))
      const result=JSON.parse(stdout)
      if(!result.ok) return reject(new Error(result.error))
      resolve(result.data)
    })
    child.stdin.end(JSON.stringify({command,args}))
  }))
  queue=task.catch(()=>{})
  return task
}
await call('list_cases',{filter:{}})
const browser=await chromium.launch({channel:'chrome',headless:true})
const page=await browser.newPage({viewport:{width:1440,height:1000}})
const errors=[]
page.on('pageerror',e=>errors.push(e.message))
let failNextCreate=false
let createdId=''
await page.exposeFunction('__casyLocal',async(command,args)=>{
  if(command==='create_case' && failNextCreate) { failNextCreate=false; throw new Error('测试：数据库暂时不可用') }
  const data=await call(command,args)
  if(command==='create_case') createdId=data.id
  return data
})
try {
  const url = new URL(process.env.CASY_QA_URL || 'http://127.0.0.1:1421/')
  url.hash = '/cases'
  await page.goto(url.href)
  await page.getByRole('button',{name:'稍后再填'}).click()
  await page.evaluate(async()=>{
    const {tryMockCommand}=await import('/src/core/mockData.ts')
    const commands=new Set(['create_case','update_case','get_case','list_cases','search_cases','get_relations','list_case_hearings','get_case_timeline','list_tasks','list_case_persons','list_case_files','feishu_import_snapshot','get_case_source_records','get_imported_asset'])
    window.__TAURI_INTERNALS__={
      invoke:async(command,args={})=>commands.has(command)?window.__casyLocal(command,args):tryMockCommand(command,args)??null,
      transformCallback:()=>0, unregisterCallback:()=>{},
    }
  })
  await page.getByRole('button',{name:'批量导入',exact:true}).click()
  const importer=page.locator('.case-import-dialog')
  await importer.locator('input[type=file]').setInputFiles(snapshotPath)
  await importer.getByRole('button',{name:'写入本地 Casy',exact:true}).click()
  await importer.getByRole('heading',{name:'本地导入完成'}).waitFor({timeout:120000})
  await page.screenshot({path:path.join(privateDir,'import-report.png')})
  await importer.locator('.el-dialog__headerbtn').click()
  const listing=await call('list_cases',{filter:{perPage:200}})
  assert.equal(listing.total,59)
  await page.getByRole('button',{name:'新建案件',exact:true}).click()
  const drawer=page.locator('.case-wizard-drawer')
  const field=label=>drawer.locator('.el-form-item').filter({has:page.locator('label').filter({hasText:new RegExp('^'+label+'$')})}).locator('input,textarea').first()
  await field('案件名称').fill('本地交互测试案件')
  await field('案号').fill('（2026）测试民初001号')
  await drawer.getByRole('button',{name:'当事各方',exact:true}).click()
  await field('我方当事人').fill('本地测试委托人')
  await field('对方当事人').fill('本地测试相对人')
  await drawer.getByRole('button',{name:'添加第三人',exact:true}).click()
  await field('第三人 1 名称').fill('本地测试第三人')
  await field('代理人').fill('第三人代理人')
  await field('联系方式').fill('010-12345678')
  await page.screenshot({path:path.join(privateDir,'intake-native-desktop.png')})
  await page.setViewportSize({width:390,height:844})
  await page.waitForTimeout(500)
  const bounds=await drawer.boundingBox()
  assert(bounds.x>=-1 && bounds.width<=391,JSON.stringify(bounds))
  await page.screenshot({path:path.join(privateDir,'intake-native-mobile.png')})
  await field('第三人 1 名称').scrollIntoViewIfNeeded()
  const partyBounds = await field('第三人 1 名称').boundingBox()
  const footerBounds = await drawer.locator('footer').boundingBox()
  assert(partyBounds.x >= 0 && partyBounds.x + partyBounds.width <= 391)
  assert(partyBounds.y + partyBounds.height <= footerBounds.y)
  await page.screenshot({path:path.join(privateDir,'intake-third-parties-mobile.png')})
  await page.setViewportSize({width:1440,height:1000})
  await drawer.getByRole('button',{name:'日期与期限',exact:true}).click()
  await field('立案日期').fill('2026-09-01')
  await field('立案日期').press('Tab')
  await drawer.getByRole('button',{name:'费用与备注',exact:true}).click()
  await drawer.locator('.el-form-item').filter({hasText:'标的额（元）'}).locator('input').fill('100000.50')
  await drawer.locator('.el-form-item').filter({hasText:'律师费（元）'}).locator('input').fill('20000')
  await field('备注').fill('本地端到端校验')
  await drawer.getByRole('button',{name:'办案节点',exact:true}).click()
  await drawer.getByRole('button',{name:'添加庭审',exact:true}).click()
  await field('开庭时间').fill('2026-09-20 09:30:00')
  await field('开庭时间').press('Tab')
  await field('开庭地点').fill('第五法庭')
  await drawer.getByRole('tab',{name:'办案日志',exact:true}).click()
  await drawer.getByRole('button',{name:'添加办案日志',exact:true}).click()
  await field('事件概述').fill('收到补充材料')
  await field('发生时间').fill('2026-09-01 10:00:00')
  await field('发生时间').press('Tab')
  await drawer.getByRole('tab',{name:'任务',exact:true}).click()
  await drawer.getByRole('button',{name:'添加任务',exact:true}).click()
  await field('任务名称').fill('核对证据清单')
  await drawer.getByRole('tab',{name:'官方联系人',exact:true}).click()
  await drawer.getByRole('button',{name:'添加官方联系人',exact:true}).click()
  await field('姓名').fill('本地测试法官')
  await field('具体联系方式').fill('010-87654321')
  await drawer.getByRole('button',{name:'关联案件',exact:true}).click()
  await drawer.locator('.el-form-item').filter({has:page.locator('label').filter({hasText:/^关联案件$/})}).locator('.el-select').click()
  await page.locator('.el-select-dropdown:visible .el-select-dropdown__item').first().click()
  await drawer.getByRole('button',{name:'添加关联',exact:true}).click()
  failNextCreate=true
  await drawer.getByRole('button',{name:'创建案件',exact:true}).click()
  await drawer.getByText('测试：数据库暂时不可用',{exact:true}).waitFor()
  assert(await drawer.isVisible())
  await drawer.getByRole('button',{name:'基本信息',exact:true}).click()
  assert.equal(await field('案件名称').inputValue(),'本地交互测试案件')
  await drawer.getByRole('button',{name:'创建案件',exact:true}).click()
  await drawer.waitFor({state:'hidden',timeout:30000})
  const saved=await call('get_case',{id:createdId})
  assert.equal(saved.filingDate,'2026-09-01')
  assert.equal(saved.caseAmount,'100000.50')
  assert.equal(JSON.parse(saved.thirdParties)[0].name,'本地测试第三人')
  assert.equal((await call('get_relations',{caseId:createdId})).length,1)
  assert.equal((await call('list_case_hearings',{caseId:createdId}))[0].hearingDate,'2026-09-20 09:30:00')
  assert.equal((await call('list_tasks',{filter:{caseId:createdId}}))[0].taskName,'核对证据清单')
  assert.equal((await call('list_case_persons',{caseId:createdId}))[0].person.name,'本地测试法官')
  assert((await call('get_case_timeline',{caseId:createdId})).some(e=>e.title==='收到补充材料'))
  await page.evaluate(id=>{location.hash='/cases/'+encodeURIComponent(id)},createdId)
  await page.getByText('全量属性与字段',{exact:true}).click()
  await page.getByText('本地测试第三人',{exact:true}).waitFor()
  await page.screenshot({path:path.join(privateDir,'saved-case.png')})
  const attributes = page.locator('.attributes')
  await attributes.getByRole('button',{name:'编辑',exact:true}).click()
  await attributes.getByRole('button',{name:'添加第三人',exact:true}).click()
  const attributeField = label => attributes.locator('.el-form-item').filter({has:page.locator('label').filter({hasText:new RegExp('^'+label+'$')})}).locator('input,textarea').first()
  await attributeField('第三人 2 名称').fill('后续补录第三人')
  await attributeField('标的额（元）').fill('125000.25')
  await attributes.getByRole('button',{name:'保存',exact:true}).click()
  await attributes.getByRole('button',{name:'编辑',exact:true}).waitFor()
  const edited = await call('get_case',{id:createdId})
  assert.equal(JSON.parse(edited.thirdParties)[1].name,'后续补录第三人')
  assert.equal(edited.caseAmount,'125000.25')
  await page.getByText('项目总览',{exact:true}).click()
  await page.getByRole('button',{name:'新建关联案',exact:true}).click()
  const relatedDrawer=page.locator('.case-wizard-drawer')
  await relatedDrawer.locator('.el-form-item').filter({has:page.locator('label').filter({hasText:/^案件名称$/})}).locator('input').fill('本地新建关联案测试')
  const parentId=createdId
  await relatedDrawer.getByRole('button',{name:'创建案件',exact:true}).click()
  await relatedDrawer.waitFor({state:'hidden',timeout:30000})
  const relationship=await call('get_relations',{caseId:createdId})
  assert(relationship.some(r=>r.caseId===parentId))
  const sharedTable=snapshot.tables.find(t=>t.name==='任务管理')
  const shared=sharedTable.records.find(r=>(r.fields['关联项目']||[]).some(v=>v.record_ids?.length>1))
  const sharedCases=shared.fields['关联项目'][0].record_ids.map(r=>'feishu:'+snapshot.appToken+':'+snapshot.tables[0].table_id+':'+r)
  const taskId='feishu:'+snapshot.appToken+':'+sharedTable.table_id+':'+shared.record_id
  const before=(await call('list_tasks',{filter:{caseId:sharedCases[0]}})).find(t=>t.id===taskId).completed
  await call('toggle_task',{id:taskId})
  for(const caseId of sharedCases) {
    const task=(await call('list_tasks',{filter:{caseId}})).find(t=>t.id===taskId)
    assert.equal(task.completed,before?0:1)
  }
  const sources=await call('get_case_source_records',{caseId:listing.items[0].id})
  assert(sources.length>0)
  const asset=snapshot.assets.find(a=>a.contentBase64)
  const local=await call('get_imported_asset',{source:snapshot.appToken,fileToken:asset.fileToken})
  assert.equal(local.contentBase64,asset.contentBase64)
  const audit=await call('qa_integrity')
  assert.equal(audit.integrity,'ok')
  assert.equal(audit.foreignKeyErrors,0)
  assert.equal(audit.assets,17)
  assert(audit.verifiedFiles>=17)
  console.log(JSON.stringify({profile,createdId,total:59,audit,pageErrors:errors},null,2))
  assert.deepEqual(errors,[])
} catch(error) {
  await page.screenshot({path:path.join(privateDir,'failure-ui.png')})
  console.error(JSON.stringify({alerts:await page.locator('.el-alert').allTextContents(),pageErrors:errors,profile}))
  throw error
} finally { await browser.close() }
