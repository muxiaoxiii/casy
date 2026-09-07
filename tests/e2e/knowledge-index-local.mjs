import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import http from 'node:http'
import { spawn } from 'node:child_process'
import assert from 'node:assert/strict'

const { chromium } = await import(process.env.CASY_PLAYWRIGHT_MODULE || 'playwright')
const profile = fs.mkdtempSync(path.join(process.env.CASY_QA_DIR || os.tmpdir(), 'knowledge-index-profile-'))
const binary = path.join(process.cwd(), 'src-tauri/target/debug/examples/knowledge_local_bridge')
const requests = [], errors = [], calls = []
let fail = false
const server = http.createServer(async (req, res) => {
  let raw = ''
  for await (const chunk of req) raw += chunk
  const body = JSON.parse(raw)
  requests.push({ url: req.url, body, authorized: req.headers.authorization === 'Bearer synthetic-vector-key' })
  if (fail) { res.writeHead(503); res.end('synthetic-error'); return }
  res.setHeader('Content-Type', 'application/json')
  res.end(JSON.stringify({ data: body.input.map((text, index) => ({ index, embedding: /赔偿金额|损失数额/.test(text) ? [1, 0] : [0, 1] })).reverse() }))
})
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
const base = `http://127.0.0.1:${server.address().port}/v1`
function call(command, args = {}) {
  calls.push(command)
  return new Promise((resolve, reject) => {
    const child = spawn(binary, [], { env: { ...process.env, CASY_TEST_DATA_DIR: profile }, stdio: ['pipe', 'pipe', 'pipe'] })
    let output = '', stderr = ''
    child.stdout.on('data', chunk => { output += chunk })
    child.stderr.on('data', chunk => { stderr += chunk })
    child.on('error', reject)
    child.on('close', code => {
      if (code) return reject(new Error(stderr))
      try { const result = JSON.parse(output); result.ok ? resolve(result.data) : reject(new Error(result.error)) }
      catch (error) { reject(error) }
    })
    child.stdin.end(JSON.stringify({ command, args }))
  })
}
let browser, page
try {
  await call('qa_seed')
  const initialNotes = await call('list_knowledge')
  assert.equal(initialNotes.length, 2000)
  assert(!initialNotes.some(note => note.id === 'qa-long'))
  await call('save_ai_profiles', { config: {
    profiles: [{ id: 'qa-vector', name: '本地验收接口', mode: 'openai', apiUrl: base, model: 'chat-model', hasApiKey: false, apiKey: 'synthetic-vector-key' }],
    activeId: 'qa-vector', dailyLimit: 0, systemPrompt: '合成材料测试', embedding: null,
  } })
  browser = await chromium.launch({ channel: 'chrome', headless: true })
  page = await browser.newPage({ viewport: { width: 1440, height: 1000 } })
  page.on('pageerror', error => errors.push(error.message))
  page.on('console', message => { if (message.type() === 'error' || message.text().startsWith('[Vue warn]')) errors.push(message.text()) })
  await page.exposeFunction('__casyKnowledge', call)
  await page.goto((process.env.CASY_QA_URL || 'http://127.0.0.1:1422/').replace(/#.*$/, '') + '#/cases')
  await page.getByRole('button', { name: '暂时跳过' }).click()
  await page.evaluate(async () => {
    const { tryMockCommand } = await import('/src/core/mockData.ts')
    const native = new Set(['get_ai_profiles', 'save_ai_profiles', 'test_embedding_connection', 'embed_knowledge', 'embed_all_knowledge', 'get_knowledge_index_status', 'cancel_knowledge_index_job', 'search_knowledge_index', 'list_knowledge', 'get_knowledge_with_blocks', 'recover_editor_drafts', 'get_workspace_sync_status'])
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} }
    // Settings mounts its other tabs too; keep those unrelated services inert.
    const inactive = {
      get_holidays_summary: { holidaysCount: 0, workdaysCount: 0, yearRange: null },
      list_folder_templates: [], get_folder_naming_settings: {}, list_reminder_rules: [], get_reminder_log: [],
      start_reminder_engine: null, list_deadline_rules: [], list_smart_rules: [], get_feishu_sync_info: {},
      get_email_monitor_status: { running: false }, list_imap_accounts: [], get_calendar_sync_status: {},
      check_keychain_status: {}, list_mcp_pending_writes: [], list_backups: [], get_settings: {},
      list_case_files: [], list_case_dirs: [], search_tasks: [], search_cases: [], list_projects: [],
    }
    window.__TAURI_INTERNALS__ = {
      invoke: async (command, args = {}) => {
        if (native.has(command)) return window.__casyKnowledge(command, args)
        if (Object.hasOwn(inactive, command)) return inactive[command]
        const result = tryMockCommand(command, args)
        if (result === undefined) throw new Error('Command outside knowledge UI test: ' + command)
        return result
      }, transformCallback: () => 0, unregisterCallback: () => {},
    }
    location.hash = '/knowledge'
  })
  await page.getByRole('button', { name: '知识库检索', exact: true }).click()
  let drawer = page.locator('.knowledge-search-drawer')
  await drawer.getByLabel('知识库检索问题').fill('赔偿金额')
  await drawer.getByRole('button', { name: '检索', exact: true }).click()
  await drawer.locator('.knowledge-hit').filter({ hasText: '125000.25' }).waitFor()
  assert.equal(requests.length, 0)
  await drawer.getByRole('tab', { name: '索引任务' }).click()
  await drawer.getByText('尚未配置向量接口', { exact: true }).waitFor()
  await drawer.getByRole('button', { name: '向量接口', exact: true }).click()
  const settings = page.locator('.ai-settings')
  await settings.locator('.el-switch').click()
  await settings.locator('.el-form-item').filter({ hasText: '向量模型来源' }).locator('.el-select').click()
  await page.getByRole('option', { name: '本地验收接口', exact: true }).click()
  await settings.getByLabel('向量模型 ID', { exact: true }).fill('custom-vector-v1')
  await settings.getByRole('button', { name: '保存并测试向量模型' }).click()
  await settings.getByText('custom-vector-v1：2 维，连接成功', { exact: true }).waitFor()
  assert.equal(requests.length, 1)
  assert(requests[0].authorized)
  await page.evaluate(() => { location.hash = '/knowledge' })
  await page.getByRole('button', { name: '知识库检索', exact: true }).click()
  drawer = page.locator('.knowledge-search-drawer')
  await drawer.getByRole('tab', { name: '索引任务' }).click()
  await drawer.getByRole('button', { name: '更新全部索引' }).click()
  await drawer.locator('.index-job[data-status="queued"]').waitFor()
  await drawer.getByRole('button', { name: '取消索引' }).click()
  await drawer.locator('.index-job[data-status="cancelled"]').waitFor()
  await drawer.getByRole('button', { name: '重试索引' }).click()
  await drawer.locator('.index-job[data-status="queued"]').waitFor()
  fail = true
  await call('qa_process_next')
  await drawer.locator('.index-job[data-status="failed"]').waitFor()
  assert((await drawer.innerText()).includes('HTTP 503'))
  fail = false
  await drawer.getByRole('button', { name: '重试索引' }).click()
  await drawer.locator('.index-job[data-status="queued"]').waitFor()
  await call('qa_process_next')
  await drawer.locator('.index-job[data-status="completed"]').waitFor()
  const status = await call('get_knowledge_index_status')
  assert.equal(status.indexed, 1)
  assert(status.jobs[0].totalChunks > 1)
  assert(requests.some(r => r.body.input.some(text => text.includes('125000.25'))))
  await page.screenshot({ path: path.join(profile, 'index-desktop.png'), animations: 'disabled' })
  await drawer.getByRole('tab', { name: '检索', exact: true }).click()
  await drawer.getByText('混合检索', { exact: true }).click()
  await drawer.getByLabel('知识库检索问题').fill('损失数额')
  await drawer.getByRole('button', { name: '检索', exact: true }).click()
  await drawer.locator('.knowledge-hit').filter({ hasText: '125000.25' }).waitFor()
  assert((await drawer.locator('.knowledge-hit').innerText()).includes('语义'))
  await page.screenshot({ path: path.join(profile, 'search-desktop.png'), animations: 'disabled' })
  await page.setViewportSize({ width: 390, height: 844 })
  await page.screenshot({ path: path.join(profile, 'search-mobile.png'), animations: 'disabled' })
  const box = await drawer.boundingBox()
  assert(box.x >= -1 && box.x + box.width <= 391)
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth + 1), false)
  fail = true
  await drawer.getByLabel('知识库检索问题').fill('赔偿金额')
  await drawer.getByRole('button', { name: '检索', exact: true }).click()
  await drawer.getByText('向量接口返回 HTTP 503', { exact: true }).waitFor()
  await drawer.locator('.knowledge-hit').filter({ hasText: '125000.25' }).waitFor()
  fail = false
  await drawer.getByText('关键词', { exact: true }).first().click()
  await drawer.getByLabel('知识库检索问题').fill('完全不存在的短语')
  await drawer.getByRole('button', { name: '检索', exact: true }).click()
  await drawer.getByText('没有找到相关笔记').waitFor()
  await drawer.getByLabel('知识库检索问题').fill('赔偿金额')
  await drawer.getByRole('button', { name: '检索', exact: true }).click()
  await drawer.locator('.knowledge-hit').click()
  await drawer.waitFor({ state: 'hidden' })
  assert.equal(await page.locator('.title-editor').inputValue(), '第三人赔偿研究')
  // The entry point must remain reachable after closing the drawer on a narrow screen.
  await page.getByRole('navigation', { name: '笔记视图' }).getByRole('button', { name: '笔记', exact: true }).click()
  await page.getByRole('button', { name: '知识库检索', exact: true }).click()
  await drawer.getByRole('tab', { name: '索引任务' }).click()
  await drawer.getByRole('button', { name: '重建索引' }).click()
  await drawer.locator('.index-job[data-status="queued"]').waitFor()
  await page.screenshot({ path: path.join(profile, 'index-mobile.png'), animations: 'disabled' })
  await drawer.getByRole('button', { name: '取消索引' }).click()
  await drawer.locator('.index-job[data-status="cancelled"]').waitFor()
  await call('embed_knowledge', { itemId: status.jobs[0].itemId })
  await call('qa_process_next')
  await page.keyboard.press('Escape')
  await page.evaluate(() => { location.hash = '/cases' })
  await page.getByRole('button', { name: '全局搜索', exact: true }).click()
  const globalSearch = page.getByRole('dialog', { name: '全局搜索', exact: true })
  await globalSearch.getByText('混合检索', { exact: true }).click()
  await globalSearch.getByRole('textbox', { name: '全局检索问题' }).fill('损失数额')
  await globalSearch.locator('.cmdk-item').filter({ hasText: '第三人赔偿研究' }).waitFor()
  assert((await globalSearch.innerText()).includes('语义'))
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth + 1), false)
  await page.screenshot({ path: path.join(profile, 'global-mobile.png'), animations: 'disabled' })
  await page.setViewportSize({ width: 1440, height: 1000 })
  await page.screenshot({ path: path.join(profile, 'global-desktop.png'), animations: 'disabled' })
  await globalSearch.locator('.cmdk-item').filter({ hasText: '第三人赔偿研究' }).click()
  await page.waitForURL(/knowledge\?select=/)
  await page.waitForFunction(() => document.querySelector('.title-editor')?.value === '第三人赔偿研究')
  assert.equal(await page.locator('.title-editor').inputValue(), '第三人赔偿研究')
  const audit = await call('qa_audit')
  assert.deepEqual(audit, { integrity: 'ok', foreignKeyErrors: 0 })
  assert.equal(errors.length, 0, errors.join('\n'))
  assert(requests.every(r => r.url === '/v1/embeddings' && r.authorized && r.body.model === 'custom-vector-v1'))
  fs.writeFileSync(path.join(profile, 'result.json'), JSON.stringify({ profile, audit, status, requests: requests.length, errors, calls }, null, 2))
  console.log(JSON.stringify({ profile, audit, requests: requests.length, errors }, null, 2))
} catch (error) {
  await page?.screenshot({ path: path.join(profile, 'failure.png'), fullPage: true })
  console.error(JSON.stringify({ profile, calls, errors }))
  throw error
} finally {
  await browser?.close()
  await call('save_ai_profiles', { config: { profiles: [], activeId: null, dailyLimit: 0, systemPrompt: '', embedding: null } })
  await new Promise(resolve => server.close(resolve))
}
