import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { spawn } from 'node:child_process'
import assert from 'node:assert/strict'

const { chromium } = await import(process.env.CASY_PLAYWRIGHT_MODULE || 'playwright')
const profile = fs.mkdtempSync(path.join(process.env.CASY_QA_DIR || os.tmpdir(), 'tasks-profile-'))
const binary = path.join(process.cwd(), 'src-tauri/target/debug/examples/intake_local_bridge')
const errors = [], calls = []
let queue = Promise.resolve()
function call(command, args = {}) {
  calls.push(command)
  const pending = queue.then(() => new Promise((resolve, reject) => {
    const child = spawn(binary, [], { env: { ...process.env, CASY_TEST_DATA_DIR: profile }, stdio: ['pipe', 'pipe', 'pipe'] })
    let stdout = '', stderr = ''
    child.stdout.on('data', chunk => { stdout += chunk })
    child.stderr.on('data', chunk => { stderr += chunk })
    child.on('error', reject)
    child.on('close', code => {
      if (code) return reject(new Error(stderr))
      try { const result = JSON.parse(stdout); result.ok ? resolve(result.data) : reject(new Error(result.error)) }
      catch (error) { reject(error) }
    })
    child.stdin.end(JSON.stringify({ command, args }))
  }))
  queue = pending.catch(() => {})
  return pending
}
const list = () => call('list_tasks', { filter: {} })
await list()
const browser = await chromium.launch({ channel: 'chrome', headless: true })
const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } })
page.on('pageerror', error => errors.push(error.message))
page.on('console', message => { if (message.type() === 'error' || message.text().startsWith('[Vue warn]')) errors.push(message.text()) })
let failSave = false
await page.exposeFunction('__casyTasks', async (command, args) => {
  if (command === 'update_task' && failSave) { failSave = false; throw new Error('合成测试：保存暂时失败') }
  return call(command, args)
})
try {
  const url = new URL(process.env.CASY_QA_URL || 'http://127.0.0.1:1422/')
  url.hash = '/cases'
  await page.goto(url.href)
  await page.getByRole('button', { name: '暂时跳过' }).click()
  await page.evaluate(async () => {
    const { tryMockCommand } = await import('/src/core/mockData.ts')
    const native = new Set(['list_tasks', 'create_task', 'update_task', 'toggle_task', 'snooze_task', 'delete_task', 'restore_task', 'list_cases'])
    window.__TAURI_INTERNALS__ = { invoke: async (command, args = {}) => {
      if (native.has(command)) return window.__casyTasks(command, args)
      if (command === 'list_areas') return []
      if (command === 'get_settings') return {}
      if (command === 'get_lawyer_profile') return { onboarding_completed: true, name: '本地验收' }
      return tryMockCommand(command, args) ?? null
    }, transformCallback: () => 0, unregisterCallback: () => {} }
    location.hash = '/tasks'
  })
  await page.locator('.stitch-tasks-workspace').getByRole('button', { name: '新建任务', exact: true }).click()
  const drawer = page.locator('.el-drawer')
  const field = label => drawer.locator('.form-item').filter({ has: page.locator('label').filter({ hasText: new RegExp('^' + label + '$') }) }).locator('input,select,textarea').first()
  await drawer.getByLabel('任务名称', { exact: true }).fill('核对关联案件期限')
  await drawer.getByLabel('开始安排', { exact: true }).selectOption('anytime')
  await field('开始日期 \\(Do When\\)').fill('2026-11-25')
  await field('截止日期 \\(Deadline\\)').fill('2026-11-30')
  await drawer.getByLabel('截止时间', { exact: true }).fill('09:30')
  await drawer.getByLabel('重复', { exact: true }).selectOption('monthly:31')
  await drawer.getByLabel('下次回顾', { exact: true }).fill('2026-11-28')
  await field('预估工时 \\(分钟\\)').fill('0')
  await drawer.getByRole('button', { name: '创建任务', exact: true }).click()
  await drawer.waitFor({ state: 'hidden' })
  let tasks = await list()
  assert.equal(tasks.length, 1)
  const id = tasks[0].id
  assert.equal(tasks[0].caseId, null)
  assert.equal(tasks[0].areaId, null)
  assert.equal(tasks[0].dueTime, '09:30')
  assert.equal(tasks[0].estimatedMinutes, 0)
  const row = () => page.locator('.task-card').filter({ hasText: '核对关联案件期限' }).first()
  await row().locator('.task-content').click()
  await field('备注与案情要点').fill('合成记录：核对三方送达及关联案期限')
  failSave = true
  await drawer.getByRole('button', { name: '保存修改', exact: true }).click()
  await page.getByText('合成测试：保存暂时失败', { exact: true }).waitFor()
  assert(await drawer.isVisible())
  assert.equal(await field('备注与案情要点').inputValue(), '合成记录：核对三方送达及关联案期限')
  await drawer.getByRole('button', { name: '保存修改', exact: true }).click()
  await drawer.waitFor({ state: 'hidden' })
  assert.equal((await list())[0].dueTime, '09:30')
  await row().getByRole('checkbox').press('Space')
  await page.locator('.lsh-badge').filter({ hasText: '1 项' }).waitFor()
  await page.waitForFunction(() => document.querySelector('.task-card .deadline')?.textContent.includes('12'))
  tasks = await list()
  assert.equal(tasks.length, 2)
  assert.equal(tasks.find(task => task.id === id).completed, 1)
  assert.equal(tasks.find(task => task.id !== id).dueDate, '2026-12-31')
  await page.getByRole('button', { name: '撤销任务操作', exact: true }).click()
  await page.waitForFunction(() => document.querySelector('.task-card .deadline')?.textContent.includes('11'))
  tasks = await list()
  assert.equal(tasks.length, 1)
  assert.equal(tasks[0].completed, 0)
  await row().getByRole('button', { name: '任务操作', exact: true }).click()
  await page.getByRole('menuitem', { name: '稍后：明天', exact: true }).click()
  await page.getByText('已计划到明天', { exact: true }).waitFor()
  tasks = await list()
  assert.equal(tasks[0].dueDate, '2026-11-30')
  assert.equal(tasks[0].deadline, '2026-11-30')
  assert.notEqual(tasks[0].startDate, '2026-11-25')
  await page.getByRole('button', { name: '撤销任务操作', exact: true }).click()
  await page.waitForFunction(() => document.querySelector('button[aria-label="撤销任务操作"]')?.disabled)
  assert.equal((await list())[0].startDate, '2026-11-25')
  await row().locator('.task-content').click()
  await drawer.getByLabel('任务类型', { exact: true }).selectOption('waiting')
  await drawer.getByLabel('等待对象', { exact: true }).fill('第三人代理人')
  await drawer.getByLabel('跟进日期', { exact: true }).fill('2020-01-01')
  await drawer.getByLabel('下次回顾', { exact: true }).fill('2020-01-01')
  await page.waitForFunction(() => document.querySelectorAll('.el-message').length === 0)
  await page.screenshot({ path: path.join(profile, 'task-editor-desktop.png'), animations: 'disabled' })
  await page.setViewportSize({ width: 390, height: 844 })
  await drawer.getByLabel('下次回顾', { exact: true }).scrollIntoViewIfNeeded()
  await page.screenshot({ path: path.join(profile, 'task-editor-mobile.png'), animations: 'disabled' })
  assert(await drawer.locator('input,select,textarea').evaluateAll(inputs => inputs.every(input => {
    const box = input.getBoundingClientRect(); return box.x >= 0 && box.right <= 391
  })))
  await drawer.getByRole('button', { name: '保存修改', exact: true }).click()
  await drawer.waitFor({ state: 'hidden' })
  const tab = name => page.locator('.perspective-tab-pill').filter({ has: page.locator('.tab-title').getByText(name, { exact: true }) })
  await tab('随时行动').click()
  assert.equal(await page.locator('.lsh-badge').innerText(), '0 项')
  await tab('等待追踪').click()
  assert.equal(await page.locator('.lsh-badge').innerText(), '1 项')
  await tab('待回顾').click()
  await page.getByRole('button', { name: '已回顾', exact: true }).click()
  await page.locator('.lsh-badge').filter({ hasText: '0 项' }).waitFor()
  tasks = await list()
  assert.equal(tasks[0].nextReviewDate, null)
  assert(tasks[0].lastReviewDate)
  await tab('全部待办').click()
  await page.waitForFunction(() => document.querySelectorAll('.el-message').length === 0)
  assert(await page.locator('.header-actions button, .header-actions input').evaluateAll(controls => controls.every(control => {
    const box = control.getBoundingClientRect(); return box.x >= 0 && box.right <= 391
  })))
  await page.screenshot({ path: path.join(profile, 'tasks-mobile.png'), animations: 'disabled' })
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth + 1), false)
  await page.setViewportSize({ width: 1440, height: 1000 })
  await page.screenshot({ path: path.join(profile, 'tasks-desktop.png'), animations: 'disabled' })
  assert.deepEqual(errors, ['[Casy] update_task failed: 合成测试：保存暂时失败'])
  const audit = await call('qa_integrity')
  assert.equal(audit.integrity, 'ok')
  assert.equal(audit.foreignKeyErrors, 0)
  const report = { profile, audit, nativeCalls: calls.length, errors: [], expectedErrors: errors }
  fs.writeFileSync(path.join(profile, 'verification.json'), JSON.stringify(report, null, 2))
  console.log(JSON.stringify(report))
} catch (error) {
  await page.screenshot({ path: path.join(profile, 'failure.png') })
  fs.writeFileSync(path.join(profile, 'failure.html'), await page.content())
  console.error('Task QA artifacts: ' + profile)
  throw error
} finally { await browser.close() }
