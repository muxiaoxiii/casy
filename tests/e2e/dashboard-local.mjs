import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { spawn } from 'node:child_process'
import assert from 'node:assert/strict'

const { chromium } = await import(process.env.CASY_PLAYWRIGHT_MODULE || 'playwright')
const profile = fs.mkdtempSync(path.join(process.env.CASY_QA_DIR || os.tmpdir(), 'dashboard-profile-'))
const emptyProfile = path.join(profile, 'empty')
const binary = path.join(process.cwd(), 'src-tauri/target/debug/examples/dashboard_local_bridge')
const calls = [], errors = []
let queue = Promise.resolve(), activeProfile = profile, failTrend = false
function call(command, args = {}) {
  const directory = activeProfile
  calls.push(command)
  const pending = queue.then(() => new Promise((resolve, reject) => {
    const child = spawn(binary, [], { env: { ...process.env, CASY_TEST_DATA_DIR: directory }, stdio: ['pipe', 'pipe', 'pipe'] })
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
await call('qa_seed')
const browser = await chromium.launch({ channel: 'chrome', headless: true })
const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } })
page.on('pageerror', error => errors.push(error.message))
page.on('console', message => { if (message.type() === 'error' || message.text().startsWith('[Vue warn]')) errors.push(message.text()) })
await page.exposeFunction('__casyDashboard', (command, args) => {
  if (command === 'get_monthly_task_trend' && failTrend) { failTrend = false; throw new Error('合成测试：趋势暂不可用') }
  return call(command, args)
})
const ready = () => page.locator('.dash-page[aria-busy="false"]').waitFor()
const kpis = () => page.locator('.kv').allTextContents()
const dashboard = async () => { await page.evaluate(() => { location.hash = '/dashboard' }); await ready() }
const noOverflow = async () => assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth + 1), false)
try {
  const url = new URL(process.env.CASY_QA_URL || 'http://127.0.0.1:1422/')
  url.hash = '/cases'
  await page.goto(url.href)
  await page.getByRole('button', { name: '暂时跳过' }).click()
  await page.evaluate(async () => {
    const { tryMockCommand } = await import('/src/core/mockData.ts')
    const native = new Set(['get_monthly_task_trend','get_track_distribution','get_upcoming_hearings','get_today_kpis','case_stats','list_cases','get_case','list_case_hearings','get_case_timeline','get_relations','list_case_persons','list_tasks','create_task','update_task','toggle_task','restore_task','get_calendar_events','get_deadline_warnings','get_holiday_calendar'])
    window.__TAURI_INTERNALS__ = { invoke: async (command, args = {}) => {
      if (native.has(command)) return window.__casyDashboard(command, args)
      if (command.startsWith('ai_')) throw new Error('External AI disabled in dashboard QA')
      if (command === 'list_areas') return []
      if (command === 'get_settings') return {}
      if (command === 'get_lawyer_profile') return { onboarding_completed: true, name: '本地验收' }
      return tryMockCommand(command, args) ?? null
    }, transformCallback: () => 0, unregisterCallback: () => {} }
  })
  await dashboard()
  assert.deepEqual(await kpis(), ['2项','2项','2项','1项'])
  assert.equal(await page.locator('.dc-main').innerText(), '3')
  assert.deepEqual(await page.locator('.hb-value').allTextContents(), ['2','1'])
  assert.equal(await page.locator('.gt-row').count(), 3)
  assert.equal(await page.locator('.trend-wrap tbody tr').count(), 6)
  assert.deepEqual(await page.locator('.trend-wrap tbody tr').last().locator('td').allTextContents(), ['5','1'])
  await noOverflow()
  await page.screenshot({ path: path.join(profile, 'dashboard-desktop.png'), fullPage: true })
  await page.locator('.el-radio-button').filter({ hasText: '12 个月' }).click()
  await ready()
  assert.equal(await page.locator('.trend-wrap tbody tr').count(), 12)
  const chart = page.locator('.trend-wrap svg')
  const rect = await chart.boundingBox()
  await page.mouse.move(rect.x + rect.width * 0.97, rect.y + 50)
  assert.match(await page.locator('.tip-line').textContent(), /新建 5 · 完成 1/)
  await page.setViewportSize({ width: 390, height: 844 })
  await noOverflow()
  assert(await page.locator('.dash-page button').evaluateAll(buttons => buttons.every(button => {
    const b = button.getBoundingClientRect(); return b.x >= 0 && b.right <= 391
  })))
  await page.screenshot({ path: path.join(profile, 'dashboard-mobile.png'), fullPage: true })
  await page.setViewportSize({ width: 1440, height: 1000 })
  failTrend = true
  await page.getByRole('button', { name: '刷新看板' }).click()
  await ready()
  await page.getByRole('alert').filter({ hasText: '任务趋势加载失败' }).waitFor()
  assert.deepEqual(await kpis(), ['2项','2项','2项','1项'])
  await page.getByRole('button', { name: '重试', exact: true }).click()
  await ready()
  assert.equal(await page.getByRole('alert').count(), 0)
  await page.locator('.kpi-card').nth(1).click()
  await page.locator('.lsh-badge').filter({ hasText: '2 项' }).waitFor()
  assert.deepEqual((await page.locator('.task-card .task-name-text').allTextContents()).sort(), ['今天到期任务','今天截止任务'].sort())
  const due = page.locator('.task-card').filter({ hasText: '今天到期任务' }).first()
  await due.getByRole('checkbox').press('Space')
  await page.locator('.lsh-badge').filter({ hasText: '1 项' }).waitFor()
  await page.waitForFunction(() => !document.querySelector('button[aria-label="撤销任务操作"]')?.disabled)
  await dashboard()
  assert.deepEqual(await kpis(), ['2项','1项','2项','1项'])
  await page.locator('.kpi-card').nth(1).click()
  await page.getByRole('button', { name: '撤销任务操作' }).click()
  await page.locator('.lsh-badge').filter({ hasText: '2 项' }).waitFor()
  await dashboard()
  assert.deepEqual(await kpis(), ['2项','2项','2项','1项'])
  await page.locator('.kpi-card').nth(2).click()
  await page.locator('.lsh-badge').filter({ hasText: '2 项' }).waitFor()
  assert.deepEqual((await page.locator('.task-card .task-name-text').allTextContents()).sort(), ['等待法院回执','等待第三人材料'].sort())
  await dashboard()
  await page.getByRole('button', { name: '民事诉讼，2 件，查看案件' }).click()
  await page.waitForFunction(() => document.querySelectorAll('.case-card-title').length === 2)
  assert.deepEqual((await page.locator('.case-card-title').allTextContents()).sort(), ['合成民事案件甲','合成民事案件乙'].sort())
  await dashboard()
  await page.locator('.kpi-card').first().click()
  await page.locator('.day-full-card').waitFor()
  const hearing = page.locator('.day-hour-drop-row').filter({ hasText: '09:00' }).getByRole('button', { name: /第三人参加庭审/ })
  await hearing.waitFor()
  assert.match(await hearing.textContent(), /09:37.*第三人参加庭审.*合成民事案件甲/)
  await page.locator('.day-unscheduled').getByRole('button', { name: /未定时庭审/ }).waitFor()
  await page.locator('.day-hour-drop-row').filter({ hasText: '07:00' }).getByRole('button', { name: /早间材料核对/ }).waitFor()
  await hearing.scrollIntoViewIfNeeded()
  await page.screenshot({ path: path.join(profile, 'calendar-day-desktop.png') })
  await page.setViewportSize({ width: 390, height: 844 })
  await hearing.scrollIntoViewIfNeeded()
  await noOverflow()
  const hearingBounds = await hearing.boundingBox()
  assert(hearingBounds.x >= 0 && hearingBounds.x + hearingBounds.width <= 391)
  await page.screenshot({ path: path.join(profile, 'calendar-day-mobile.png') })
  await hearing.click()
  await page.waitForURL(/#\/cases\/qa-civil/)
  await page.setViewportSize({ width: 1440, height: 1000 })
  await dashboard()
  await page.evaluate(async () => {
    const { casyContext } = await import('/src/core/plugin/context.ts')
    await casyContext.tasks.create({ taskName: '捕获后的新增到期任务', dueDate: new Date().toLocaleDateString('en-CA'), startBucket: 'anytime' })
  })
  await page.waitForFunction(() => document.querySelectorAll('.kv')[1]?.textContent === '3项')
  const state = await call('qa_state')
  assert.deepEqual(state, { integrity: 'ok', foreignKeyErrors: 0 })
  activeProfile = emptyProfile
  await page.getByRole('button', { name: '刷新看板' }).click()
  await ready()
  assert.deepEqual(await kpis(), ['0项','0项','0项','0项'])
  await page.getByText('暂无案件', { exact: true }).waitFor()
  await page.getByText('未来 30 天暂无庭审安排', { exact: true }).waitFor()
  await page.screenshot({ path: path.join(profile, 'dashboard-empty.png'), fullPage: true })
  assert.deepEqual(errors, ['[Casy] get_monthly_task_trend failed: 合成测试：趋势暂不可用'])
  const report = { profile, state, nativeCalls: calls.length, errors: [], expectedErrors: errors }
  fs.writeFileSync(path.join(profile, 'verification.json'), JSON.stringify(report, null, 2))
  console.log(JSON.stringify(report))
} catch (error) {
  await page.screenshot({ path: path.join(profile, 'failure.png'), fullPage: true })
  fs.writeFileSync(path.join(profile, 'failure.html'), await page.content())
  console.error('Dashboard QA artifacts: ' + profile)
  throw error
} finally { await browser.close() }
