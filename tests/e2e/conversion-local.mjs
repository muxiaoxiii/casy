import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { spawn } from 'node:child_process'
import assert from 'node:assert/strict'
const { chromium } = await import(process.env.CASY_PLAYWRIGHT_MODULE || 'playwright')
const profile = fs.mkdtempSync(path.join(process.env.CASY_QA_DIR || os.tmpdir(), 'conversion-ui-'))
const source = path.join(profile, '多语种证据.txt'), invalid = path.join(profile, '损坏文件.docx'), output = path.join(profile, 'output')
const pdf = process.env.CASY_CONVERSION_TEST_PDF
const sources = [source, invalid, ...(pdf ? [pdf] : [])]
fs.mkdirSync(output)
fs.writeFileSync(source, '# 证据\n\nPrüfung français 日本語\n\n| 项目 | 金额 |\n|---|---|\n| 赔偿 | 100 |')
fs.writeFileSync(invalid, 'invalid zip')
const calls = [], errors = []
let queue = Promise.resolve()
function call(command, args = {}) {
  calls.push(command)
  const result = queue.then(() => new Promise((resolve, reject) => {
    const child = spawn('src-tauri/target/debug/examples/knowledge_local_bridge', [], { env: { ...process.env, CASY_TEST_DATA_DIR: profile }, stdio: ['pipe','pipe','pipe'] })
    let out = '', err = ''
    child.stdout.on('data', data => { out += data }); child.stderr.on('data', data => { err += data })
    child.on('error', reject)
    child.on('exit', code => { if (code) return reject(new Error(err)); try { const value = JSON.parse(out); value.ok ? resolve(value.data) : reject(new Error(value.error)) } catch(e) { reject(e) } })
    child.stdin.end(JSON.stringify({ command, args }))
  }))
  queue = result.catch(() => {})
  return result
}
const markdown = '# 跨页证据表\n\n| 编号 | 内容 |\n|---|---|\n' + Array.from({length:80},(_,i)=>`| ${i+1} | 证据内容 ${i+1} Prüfung français 日本語 |`).join('\n') + '\n\n>> 第二来源\n'
await call('create_knowledge', { data: { title:'跨页证据表', content:markdown, category:'reference' } })
const repair = path.join(profile, 'valid-source.docx')
await call('export_edited_docx', { title:'修复样本',outputPath:repair,document:{type:'doc',content:[{type:'paragraph',content:[{type:'text',text:'第二层引用来源'}]}]} })
const browser = await chromium.launch({channel:'chrome',headless:true})
const page = await browser.newPage({viewport:{width:1440,height:1000}})
page.on('pageerror', e => errors.push(e.message))
await page.exposeFunction('__conversionCall', call)
try {
  await page.addInitScript(() => localStorage.setItem('casy_onboarding_dismissed', '1'))
  const url = new URL(process.env.CASY_QA_URL || 'http://127.0.0.1:1424/'); url.hash = '/cases'
  await page.goto(url.href)
  await page.evaluate(async ({sources,output}) => {
    const { tryMockCommand } = await import('/src/core/mockData.ts')
    const native = new Set(['convert_file_to_markdown','list_knowledge','get_knowledge_with_blocks','list_knowledge_versions','list_links_for','get_backlinks','save_editor_recovery','export_edited_docx'])
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener:()=>{} }
    window.__TAURI_INTERNALS__ = { invoke:async(command,args={})=>{
      if (native.has(command)) return window.__conversionCall(command,args)
      if (command==='plugin:dialog|open') return args.options?.directory ? output : sources
      if (command==='plugin:dialog|save') return `${output}/edited-table.docx`
      if (command==='recover_editor_drafts') return []
      return tryMockCommand(command,args) ?? null
    },transformCallback:()=>0,unregisterCallback:()=>{} }
  },{sources,output})
  assert.equal(await page.locator('.topbar-right').getByRole('button',{name:'新建任务'}).count(),0)
  await page.getByRole('button',{name:'文件转换',exact:true}).click()
  const dialog = page.getByRole('dialog',{name:'文件转换',exact:true})
  await dialog.getByRole('button',{name:'选择文件',exact:true}).click()
  await dialog.getByRole('button',{name:'输出目录',exact:true}).click()
  await dialog.getByRole('button',{name:'转换为 Markdown',exact:true}).click()
  await dialog.getByRole('button',{name:'重试',exact:true}).waitFor()
  assert.equal(fs.readFileSync(path.join(output,'多语种证据.md'),'utf8'),fs.readFileSync(source,'utf8'))
  await dialog.locator('button.is-loading').waitFor({state:'hidden',timeout:180000})
  if (pdf) {
    await dialog.getByRole('button',{name:'定位 Markdown',exact:true}).nth(1).waitFor({timeout:180000})
    assert(fs.readFileSync(path.join(output,path.parse(pdf).name+'.md'),'utf8').includes('<!-- page 1 -->'))
  }
  assert.equal(await dialog.getByRole('button',{name:'定位 Markdown',exact:true}).count(),pdf ? 2 : 1)
  await page.screenshot({path:path.join(profile,'conversion-desktop.png'),animations:'disabled'})
  fs.writeFileSync(invalid, fs.readFileSync(repair))
  await dialog.getByRole('button',{name:'重试',exact:true}).click()
  await dialog.getByRole('button',{name:'转换为 Markdown',exact:true}).click()
  await dialog.locator('.conversion-count').filter({hasText:`${sources.length} / ${sources.length}`}).waitFor({timeout:30000})
  assert(fs.readFileSync(path.join(output,'损坏文件.md'),'utf8').includes('第二层引用来源'))
  await page.setViewportSize({width:390,height:844})
  await page.screenshot({path:path.join(profile,'conversion-mobile.png'),animations:'disabled'})
  assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth))
  await dialog.getByRole('button',{name:'关闭',exact:true}).click()
  await page.getByRole('button',{name:'文件转换',exact:true}).click()
  await dialog.waitFor()
  await dialog.getByRole('button',{name:'关闭',exact:true}).click()
  await page.setViewportSize({width:1440,height:1000})
  await page.evaluate(()=>{location.hash='/knowledge'})
  await page.locator('.tiptap table').waitFor()
  assert.equal(await page.locator('.tiptap table tr').count(),81)
  await page.getByRole('button',{name:'导出',exact:true}).click()
  await page.getByRole('menuitem',{name:'Word 文档（.docx）'}).click()
  await page.getByRole('button',{name:'完成',exact:true}).click()
  assert(fs.statSync(path.join(output,'edited-table.docx')).size>0)
  const audit = await call('qa_audit')
  assert.deepEqual(audit,{integrity:'ok',foreignKeyErrors:0})
  assert.deepEqual(errors,[])
  fs.writeFileSync(path.join(profile,'verification.json'),JSON.stringify({profile,audit,calls,errors},null,2))
  console.log(JSON.stringify({profile,audit,calls:calls.length,errors}))
} catch(e) {
  await page.screenshot({path:path.join(profile,'failure.png')})
  console.error(profile,errors)
  throw e
} finally { await browser.close() }
