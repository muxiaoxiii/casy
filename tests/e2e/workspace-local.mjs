import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { spawn } from 'node:child_process'
import assert from 'node:assert/strict'

const { chromium } = await import(process.env.CASY_PLAYWRIGHT_MODULE || 'playwright')
const profile = fs.mkdtempSync(path.join(process.env.CASY_QA_DIR || os.tmpdir(), 'workspace-ui-'))
const binary = path.join(process.cwd(), 'src-tauri/target/debug/examples/knowledge_local_bridge')
let queue = Promise.resolve()
const errors = [], calls = []
function call(command, args = {}) {
  calls.push(command)
  const result = queue.then(() => new Promise((resolve, reject) => {
    const child = spawn(binary, [], { env: { ...process.env, CASY_TEST_DATA_DIR: profile }, stdio: ['pipe', 'pipe', 'pipe'] })
    let out = '', err = ''
    child.stdout.on('data', data => { out += data }); child.stderr.on('data', data => { err += data })
    child.on('error', reject)
    child.on('exit', code => {
      if (code) return reject(new Error(err || out))
      try { const result = JSON.parse(out); result.ok ? resolve(result.data) : reject(new Error(result.error)) } catch (e) { reject(e) }
    })
    child.stdin.end(JSON.stringify({ command, args }))
  }))
  queue = result.catch(() => {})
  return result
}
const seeded = await call('qa_seed_workspace')
const browser = await chromium.launch({ channel: 'chrome', headless: true })
const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } })
page.on('pageerror', error => errors.push(error.message))
await page.exposeFunction('__workspaceCall', call)
try {
  const url = new URL(process.env.CASY_QA_URL || 'http://127.0.0.1:1424/'); url.hash = '/cases'
  await page.goto(url.href)
  await page.getByRole('dialog', { name: '个人设置' }).waitFor()
  assert.equal(await page.getByText('计算型', { exact: true }).count(), 0)
  assert.equal(await page.getByText('提醒通道', { exact: true }).count(), 0)
  await page.getByRole('button', { name: '暂时跳过' }).click()
  await page.evaluate(async () => {
    const { tryMockCommand } = await import('/src/core/mockData.ts')
    const native = new Set(['list_knowledge','get_knowledge_with_blocks','update_knowledge','list_knowledge_versions','list_links_for','get_backlinks','save_editor_recovery','recover_editor_drafts','list_workspace_sources','get_workspace_document','get_workspace_sync_status','import_pageindex_to_knowledge','get_settings','save_settings'])
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} }
    window.__TAURI_INTERNALS__ = { invoke: async (command, args = {}) => {
      if (native.has(command)) return window.__workspaceCall(command, args)
      if (command === 'list_cases') return { items: [{ id: 'workspace-case', caseName: '本地同步验收案' }], total: 1 }
      if (command === 'get_email_monitor_status') return { running: false, accountCount: 0 }
      if (command === 'get_lawyer_profile') return { onboarding_completed: true, name: '本地验收' }
      return tryMockCommand(command, args) ?? null
    }, transformCallback: () => 0, unregisterCallback: () => {} }
    location.hash = '/knowledge'
  })
  await page.locator('.case-scope').click()
  await page.locator('.el-select-dropdown__item').filter({ hasText: '本地同步验收案' }).click()
  await page.keyboard.press('Escape')
  await page.locator('.source-markdown-preview h1').filter({ hasText: '多语种证据' }).waitFor()
  assert((await page.locator('.source-markdown-preview').innerText()).includes('Prüfung français 日本語'))
  const colors = await page.locator('.source-markdown-preview > blockquote').evaluateAll(nodes => nodes.map(node => ({ color: getComputedStyle(node).borderLeftColor, width: getComputedStyle(node).borderLeftWidth, inner: [...node.querySelectorAll('blockquote')].map(q => [getComputedStyle(q).borderLeftWidth, getComputedStyle(q).marginLeft, getComputedStyle(q).paddingLeft]) })))
  assert.deepEqual(colors.map(c => c.color), ['rgb(152, 107, 104)','rgb(105, 129, 111)','rgb(98, 124, 148)','rgb(154, 133, 87)'])
  assert(colors.every(c => c.width === '2px' && c.inner.every(s => s.every(v => v === '0px'))))
  await page.screenshot({ path: path.join(profile, 'case-markdown-desktop.png'), animations: 'disabled' })
  const sources = await call('list_workspace_sources', { caseIds: [seeded.caseId] })
  const originalId = sources[0].fileId
  const renamed = path.join(path.dirname(seeded.path), '外部改名后的证据.md')
  fs.renameSync(seeded.path, renamed)
  await call('qa_sync_workspace')
  await page.locator('.note-card').filter({ hasText: '外部改名后的证据.md' }).waitFor({ timeout: 20000 })
  assert.equal((await call('list_workspace_sources', { caseIds: [seeded.caseId] }))[0].fileId, originalId)
  await page.getByRole('button', { name: '编辑知识快照' }).click()
  await page.locator('.tiptap').waitFor()
  await page.locator('.tiptap blockquote p').getByText('来源一', { exact: true }).click()
  await page.getByRole('button', { name: '引用来源', exact: true }).click()
  await page.getByText('来源 3', { exact: true }).click()
  await page.locator('.mode-switch').getByRole('button', { name: '源码', exact: true }).click()
  assert((await page.locator('.cm-content').innerText()).includes('> > > 来源一'))
  await page.locator('.mode-switch').getByRole('button', { name: '富文本', exact: true }).click()
  assert.equal(await page.locator('.tiptap > blockquote').filter({ hasText: '来源一' }).locator('blockquote').count(), 2)
  await page.locator('.save-button').click()
  await page.locator('.save-state').getByText('已保存', { exact: true }).waitFor()
  await page.screenshot({ path: path.join(profile, 'quotes-rich-desktop.png'), animations: 'disabled' })
  await page.setViewportSize({ width: 390, height: 844 })
  await page.getByRole('navigation', { name: '笔记视图' }).getByRole('button', { name: '正文', exact: true }).click()
  await page.screenshot({ path: path.join(profile, 'quotes-rich-mobile.png'), animations: 'disabled' })
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth))
  await page.setViewportSize({ width: 1440, height: 1000 })
  await page.evaluate(() => { location.hash = '/settings' })
  await page.getByText('常规设置', { exact: true }).filter({ visible: true }).click()
  const switches = page.locator('.workspace-settings [role="switch"]')
  assert.equal(await switches.count(), 6)
  await page.locator('.workspace-settings .el-switch__core').first().click()
  await page.getByRole('button', { name: '保存设置', exact: true }).click()
  await page.getByText('通用设置已保存', { exact: true }).waitFor()
  assert.equal((await call('get_settings')).workspace_sync.register, false)
  await page.screenshot({ path: path.join(profile, 'sync-settings.png'), animations: 'disabled' })
  const audit = await call('qa_audit')
  assert.deepEqual(audit, { integrity: 'ok', foreignKeyErrors: 0 })
  assert.deepEqual(errors, [])
  fs.writeFileSync(path.join(profile, 'verification.json'), JSON.stringify({ profile, colors, audit, calls, errors }, null, 2))
  console.log(JSON.stringify({ profile, audit, calls: calls.length, errors }))
} catch (error) {
  await page.screenshot({ path: path.join(profile, 'failure.png') }); console.error(profile); throw error
} finally { await browser.close() }
