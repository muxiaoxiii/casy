import fs from 'node:fs'
import path from 'node:path'
import os from 'node:os'
import { spawn } from 'node:child_process'
import assert from 'node:assert/strict'

const { chromium } = await import(process.env.CASY_PLAYWRIGHT_MODULE || 'playwright')
const profile = fs.mkdtempSync(path.join(process.env.CASY_QA_DIR || os.tmpdir(), 'capture-profile-'))
const binary = path.join(process.cwd(), 'src-tauri/target/debug/examples/capture_local_bridge')
const calls = [], errors = []
let queue = Promise.resolve()
function call(command, args = {}) {
  calls.push(command)
  const pending = queue.then(() => new Promise((resolve, reject) => {
    const child = spawn(binary, [], { env: { ...process.env, CASY_TEST_DATA_DIR: profile }, stdio: ['pipe', 'pipe', 'pipe'] })
    let output = '', stderr = ''
    child.stdout.on('data', data => { output += data })
    child.stderr.on('data', data => { stderr += data })
    child.on('error', reject)
    child.on('close', code => {
      if (code) return reject(new Error(stderr))
      try { const result = JSON.parse(output); result.ok ? resolve(result.data) : reject(new Error(result.error)) }
      catch (error) { reject(error) }
    })
    child.stdin.end(JSON.stringify({ command, args }))
  }))
  queue = pending.catch(() => {})
  return pending
}
await call('qa_seed')
const attachment = path.join(profile, 'synthetic-attachment.txt')
fs.writeFileSync(attachment, '合成附件：原文完整保留。\n')
const browser = await chromium.launch({ channel: 'chrome', headless: true })
const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } })
page.on('pageerror', error => errors.push(error.message))
page.on('console', message => { if (message.type() === 'error' || message.text().startsWith('[Vue warn]')) errors.push(message.text()) })
let failFile = true, loseReply = true
await page.exposeFunction('__captureNative', async (command, args) => {
  if (command === 'add_inbox_item' && args.sourceType === 'file' && failFile) {
    failFile = false
    throw new Error('合成测试：附件捕获暂时失败')
  }
  const result = await call(command, args)
  if (command === 'confirm_inbox_action' && args.action === 'create_task' && loseReply) {
    loseReply = false
    throw new Error('合成测试：确认响应丢失')
  }
  return result
})
const dialog = page.locator('.unified-capture-dialog')
async function selectOption(select, label) {
  await select.click()
  await page.locator('.el-select-dropdown:visible').getByText(label, { exact: true }).click()
}
async function openCapture(action, content) {
  await page.locator('.topbar-right .btn-primary').click()
  await dialog.getByRole('textbox', { name: '捕获内容' }).fill(content)
  await selectOption(dialog.locator('.capture-fields .el-select').first(), action)
}
async function closeCapture() {
  await dialog.locator('.capture-footer').getByRole('button', { name: '关闭', exact: true }).click()
  await dialog.waitFor({ state: 'hidden' })
}
const review = () => dialog.getByRole('button', { name: '确认处理', exact: true }).click()
const done = () => dialog.getByRole('heading', { name: '处理完成', exact: true }).waitFor()
try {
  const url = new URL(process.env.CASY_QA_URL || 'http://127.0.0.1:1422/'); url.hash = '/cases'
  await page.goto(url.href)
  await page.getByRole('button', { name: '稍后再填' }).click()
  await page.evaluate(async () => {
    const { tryMockCommand } = await import('/src/core/mockData.ts')
    const native = new Set(['add_inbox_item','list_inbox_items','quick_judge_inbox_item','confirm_inbox_action','list_cases','get_case','list_tasks'])
    window.__TAURI_INTERNALS__ = { invoke: (command, args = {}) => {
      if (native.has(command)) return window.__captureNative(command, args)
      if (command === 'get_settings') return {}
      if (command === 'get_lawyer_profile') return { onboarding_completed: true, name: '合成验收' }
      return tryMockCommand(command, args) ?? null
    }, transformCallback: () => 0, unregisterCallback: () => {} }
  })

  const raw = '2026-09-18 下午三点三十七分提交合成材料'
  await openCapture('任务', raw)
  await selectOption(dialog.locator('.capture-fields .el-select').nth(1), '捕获验收案件')
  await page.evaluate(file => window.dispatchEvent(new CustomEvent('casy:file-drop', { detail: { paths: [file] } })), attachment)
  await dialog.getByRole('button', { name: '继续', exact: true }).click()
  await page.getByText('合成测试：附件捕获暂时失败', { exact: true }).waitFor()
  assert.equal(await dialog.getByRole('textbox', { name: '捕获内容' }).inputValue(), raw)
  await dialog.getByRole('button', { name: '继续', exact: true }).click()
  await dialog.getByRole('textbox', { name: '标题', exact: true }).fill('核对后提交合成材料')
  await review()
  await page.getByText('合成测试：确认响应丢失', { exact: true }).waitFor()
  assert.equal((await call('qa_state')).tasks.length, 1)
  await review(); await done()
  let state = await call('qa_state')
  assert.equal(state.items.length, 2)
  assert.equal(state.tasks.length, 1)
  assert.deepEqual([state.tasks[0].name,state.tasks[0].date,state.tasks[0].time,state.tasks[0].description], ['核对后提交合成材料','2026-09-18','15:37',raw])
  assert(state.tasks[0].source)
  assert(state.items.every(item => item.status === 'filed'))
  assert.equal(state.files.length, 1)
  assert.equal(fs.readFileSync(state.files[0].path, 'utf8'), '合成附件：原文完整保留。\n')
  fs.unlinkSync(attachment)
  assert.equal(fs.readFileSync(state.items.find(item => item.sourceType === 'file').sourcePath, 'utf8'), '合成附件：原文完整保留。\n')
  await closeCapture()

  await openCapture('日程', '2026-09-19 下午两点十五分材料讨论')
  await dialog.getByRole('button', { name: '继续', exact: true }).click()
  await page.waitForFunction(() => document.querySelectorAll('.el-message').length === 0)
  await page.screenshot({ path: path.join(profile, 'capture-desktop.png'), animations: 'disabled' })
  await page.setViewportSize({ width: 390, height: 844 })
  await page.screenshot({ path: path.join(profile, 'capture-mobile.png'), animations: 'disabled' })
  const bounds = await dialog.boundingBox()
  assert(bounds && bounds.x >= 0 && bounds.x + bounds.width <= 391)
  await page.setViewportSize({ width: 360, height: 640 })
  const small = await dialog.boundingBox()
  assert(small && small.x >= 0 && small.x + small.width <= 361 && small.y >= 0 && small.y + small.height <= 641)
  await page.setViewportSize({ width: 390, height: 844 })
  await review(); await done()
  const events = await call('list_calendar_events', { startDate: '2026-09-19', endDate: '2026-09-19' })
  assert.equal(events.length, 1)
  assert.equal(events[0].startTime, '14:15')
  assert.equal(events[0].title, '材料讨论')
  await closeCapture()

  await openCapture('知识', '# 合成研究\n\n完整笔记正文。')
  await dialog.getByRole('button', { name: '继续', exact: true }).click()
  await dialog.getByRole('textbox', { name: '标题', exact: true }).fill('捕获知识验收')
  await review(); await done()
  const notes = await call('list_knowledge', { filter: {} })
  assert(notes.some(note => note.title === '捕获知识验收' && note.content === '# 合成研究\n\n完整笔记正文。'))
  await closeCapture()

  await openCapture('案件', '案件名称：捕获完整案件\n我方当事人：客户甲\n对方当事人：相对方乙\n第三人：第三人甲')
  await dialog.getByRole('button', { name: '继续', exact: true }).click()
  await dialog.getByRole('button', { name: '完善案件信息', exact: true }).click()
  const wizard = page.locator('.case-wizard-drawer')
  await wizard.getByRole('heading', { name: '新建案件', exact: true }).waitFor()
  await wizard.getByRole('button', { name: '当事各方', exact: true }).click()
  const field = name => wizard.locator('.el-form-item').filter({ has: page.locator('.el-form-item__label').getByText(name, { exact: true }) }).locator('input').first()
  assert.equal(await field('第三人 1 名称').inputValue(), '第三人甲')
  await wizard.getByRole('button', { name: '添加第三人', exact: true }).click()
  await field('第三人 2 名称').fill('第三人乙')
  const partyBounds = await field('第三人 2 名称').boundingBox()
  const footerBounds = await wizard.locator('footer').boundingBox()
  assert(partyBounds && footerBounds && partyBounds.y + partyBounds.height <= footerBounds.y)
  await page.waitForFunction(() => document.querySelectorAll('.el-message').length === 0)
  await page.screenshot({ path: path.join(profile, 'capture-case-mobile.png'), animations: 'disabled' })
  await wizard.getByRole('button', { name: '创建案件', exact: true }).click()
  await done()
  const cases = await call('list_cases', { filter: { search: '捕获完整案件' } })
  assert.equal(cases.items.length, 1)
  assert(JSON.parse(cases.items[0].thirdParties).some(party => party.name === '第三人乙'))
  await closeCapture()
  await page.getByText('捕获完整案件', { exact: true }).first().waitFor()
  state = await call('qa_state')
  assert.equal(state.integrity, 'ok'); assert.equal(state.foreignKeyErrors, 0)
  assert.equal(errors.length, 2, JSON.stringify(errors))
  assert(errors.some(error => error.includes('合成测试：附件捕获暂时失败')))
  assert(errors.some(error => error.includes('合成测试：确认响应丢失')))
  const report = { profile, nativeCalls: calls.length, tasks: state.tasks.length, files: state.files.length, inbox: state.items.length, events: events.length, cases: cases.items.length, foreignKeyErrors: state.foreignKeyErrors, integrity: state.integrity, expectedErrors: errors }
  fs.writeFileSync(path.join(profile,'verification.json'),JSON.stringify(report,null,2))
  console.log(JSON.stringify(report))
} catch (error) {
  fs.writeFileSync(path.join(profile,'errors.json'),JSON.stringify(errors,null,2))
  await page.screenshot({ path: path.join(profile,'failure.png') })
  console.error('Capture QA artifacts: '+profile)
  throw error
} finally { await browser.close() }
