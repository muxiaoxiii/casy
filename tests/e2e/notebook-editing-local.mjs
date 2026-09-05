import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { spawn } from 'node:child_process'
import assert from 'node:assert/strict'

const { chromium } = await import(process.env.CASY_PLAYWRIGHT_MODULE || 'playwright')
const profile = fs.mkdtempSync(path.join(process.env.CASY_QA_DIR || os.tmpdir(), 'notebook-profile-'))
const binary = path.join(process.cwd(), 'src-tauri/target/debug/examples/knowledge_local_bridge')
const errors = [], calls = [], timing = {}
let queue = Promise.resolve()
function call(command, args = {}) {
  calls.push(command)
  const result = queue.then(() => new Promise((resolve, reject) => {
    const child = spawn(binary, [], { env: { ...process.env, CASY_TEST_DATA_DIR: profile }, stdio: ['pipe', 'pipe', 'pipe'] })
    let output = '', stderr = ''
    child.stdout.on('data', chunk => { output += chunk })
    child.stderr.on('data', chunk => { stderr += chunk })
    child.on('error', reject)
    child.on('close', code => {
      if (code) return reject(new Error(stderr))
      try { const parsed = JSON.parse(output); parsed.ok ? resolve(parsed.data) : reject(new Error(parsed.error)) }
      catch (error) { reject(error) }
    })
    child.stdin.end(JSON.stringify({ command, args }))
  }))
  queue = result.catch(() => {})
  return result
}
const seeded = await call('qa_seed_editing')
const get = async id => (await call('get_knowledge_with_blocks', { id })).item
const original = (await get('edit-a')).content
const longOriginal = (await get('edit-long')).content
const browser = await chromium.launch({ channel: 'chrome', headless: true })
const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } })
page.on('pageerror', error => errors.push(error.message))
page.on('console', message => { if (message.type() === 'error' || message.text().startsWith('[Vue warn]')) errors.push(message.text()) })
let pauseNext = false, saveStarted = false, releaseSave, failNext = false
await page.exposeFunction('__casyNotebook', async (command, args) => {
  if (command === 'update_knowledge' && failNext) { failNext = false; throw new Error('合成测试：笔记保存失败') }
  if (command === 'update_knowledge' && pauseNext) {
    pauseNext = false
    saveStarted = true
    await new Promise(resolve => { releaseSave = resolve })
  }
  return call(command, args)
})
const note = title => page.locator('.note-card').filter({ has: page.locator('strong').getByText(title, { exact: true }) })
const mode = label => page.locator('.mode-switch').getByRole('button', { name: label, exact: true })
const saved = () => page.locator('.save-state').getByText('已保存', { exact: true }).waitFor()
const replaceSource = async text => {
  const input = page.locator('.cm-content')
  await input.click()
  await input.press('Meta+a')
  await page.keyboard.insertText(text)
}
try {
  const url = new URL(process.env.CASY_QA_URL || 'http://127.0.0.1:1422/')
  url.hash = '/cases'
  await page.goto(url.href)
  await page.getByRole('button', { name: '稍后再填' }).click()
  await page.evaluate(async () => {
    const { tryMockCommand } = await import('/src/core/mockData.ts')
    const native = new Set(['list_knowledge', 'get_knowledge_with_blocks', 'create_knowledge', 'update_knowledge', 'delete_knowledge',
      'list_knowledge_versions', 'diff_knowledge_with_current', 'restore_knowledge_version', 'list_links_for', 'get_backlinks'])
    window.__TAURI_INTERNALS__ = { invoke: async (command, args = {}) => {
      if (native.has(command)) return window.__casyNotebook(command, args)
      if (command === 'get_settings') return {}
      if (command === 'get_lawyer_profile') return { onboarding_completed: true, name: '本地验收' }
      return tryMockCommand(command, args) ?? null
    }, transformCallback: () => 0, unregisterCallback: () => {} }
    location.hash = '/knowledge'
  })
  await page.locator('.title-editor').waitFor()
  await note('乙研究').click()
  await note('甲研究').click()
  assert.equal((await get('edit-a')).content, original)
  await page.locator('.tiptap').click()
  await page.locator('.tiptap p').last().click()
  await page.keyboard.press('Meta+ArrowRight')
  await page.keyboard.insertText('即时切换保留')
  await mode('源码').click()
  assert((await page.locator('.cm-content').innerText()).includes('即时切换保留'))
  await note('甲研究').click()
  assert((await page.locator('.cm-content').innerText()).includes('即时切换保留'))

  pauseNext = true
  await replaceSource('第一次保存')
  await page.locator('.save-button').click()
  for (let attempt = 0; !saveStarted && attempt < 100; attempt++) await new Promise(resolve => setTimeout(resolve, 20))
  assert(saveStarted)
  await replaceSource('保存过程中追加的最后一句 [[乙研究]]')
  await note('乙研究').click()
  assert.equal(await page.locator('.title-editor').inputValue(), '甲研究')
  releaseSave()
  await page.waitForFunction(() => document.querySelector('.title-editor')?.value === '乙研究')
  assert.equal((await get('edit-a')).content, '保存过程中追加的最后一句 [[乙研究]]')
  assert.equal((await get('edit-b')).content, '乙笔记原文')
  const backlinks = await call('get_backlinks', { targetType: 'knowledge', targetId: 'edit-b' })
  assert(backlinks.some(link => link.sourceId === 'edit-a' && link.anchor === 'wiki:乙研究'))
  await note('甲研究').click()
  failNext = true
  await replaceSource('失败后仍保留在编辑器中的正文')
  assert.equal(await page.evaluate(() => {
    const event = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(event)
    return event.defaultPrevented
  }), true)
  await note('乙研究').click()
  await page.getByText('合成测试：笔记保存失败', { exact: true }).first().waitFor()
  assert.equal(await page.locator('.title-editor').inputValue(), '甲研究')
  assert((await page.locator('.cm-content').innerText()).includes('失败后仍保留'))
  await page.locator('.save-button').click()
  await saved()
  assert.equal(await page.evaluate(() => {
    const event = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(event)
    return event.defaultPrevented
  }), false)
  await page.locator('.info-tabs').getByRole('button', { name: '历史', exact: true }).click()
  await page.locator('.version-item').first().click()
  await page.locator('.diff-line.removed').first().waitFor()
  await page.getByRole('button', { name: '恢复此版本', exact: true }).click()
  await page.locator('.el-message-box').getByRole('button', { name: '恢复', exact: true }).click()
  await page.getByText('历史版本已恢复', { exact: true }).waitFor()
  assert.equal((await get('edit-a')).content, original)
  assert((await call('list_knowledge_versions', { itemId: 'edit-a' })).some(version => version.changeReason === 'before_restore' && version.content.includes('失败后仍保留')))
  assert((await page.locator('.cm-content').innerText()).includes('保留换行'))

  await call('update_knowledge', { id: 'edit-b', data: { content: '其他窗口已更新的正文' } })
  await note('乙研究').click()
  await page.waitForFunction(() => document.querySelector('.title-editor')?.value === '乙研究')
  assert((await page.locator('.cm-content').innerText()).includes('其他窗口已更新的正文'))

  const openStart = performance.now()
  await note('长篇材料').click()
  await page.waitForFunction(() => document.querySelector('.title-editor')?.value === '长篇材料')
  timing.sourceOpenMs = Math.round(performance.now() - openStart)
  const editStart = performance.now()
  await page.locator('.cm-content').click()
  await page.locator('.cm-content').press('Meta+End')
  await page.keyboard.insertText('\n\n长文新增尾注-END2')
  await page.locator('.save-button').click()
  await saved()
  timing.sourceEditSaveMs = Math.round(performance.now() - editStart)
  const longSaved = (await get('edit-long')).content
  assert.equal(longSaved, longOriginal + '\n\n长文新增尾注-END2')
  const richStart = performance.now()
  await mode('富文本').click()
  await page.locator('.tiptap h2').first().waitFor()
  timing.richOpenMs = Math.round(performance.now() - richStart)
  assert.equal(await page.locator('.tiptap h2').count(), 3000)
  await page.locator('.tiptap p').last().click()
  await page.keyboard.press('Meta+ArrowRight')
  await page.keyboard.insertText('富文本尾注-END3')
  const richSaveStart = performance.now()
  await page.locator('.save-button').click()
  await saved()
  timing.richSaveMs = Math.round(performance.now() - richSaveStart)
  await page.locator('.version-item').first().waitFor()
  const finalContent = (await get('edit-long')).content
  assert(finalContent.includes('材料第2999节'))
  assert(finalContent.includes('长文新增尾注-END2'))
  assert(finalContent.endsWith('富文本尾注-END3'))
  const exportPath = path.join(profile, 'long-note.md')
  await call('export_knowledge_markdown', { itemId: 'edit-long', outputPath: exportPath })
  assert(fs.readFileSync(exportPath, 'utf8') === finalContent + '\n', 'Markdown export must contain the complete saved text followed by one newline')
  await note('甲研究').click()
  await page.waitForFunction(() => document.querySelectorAll('.el-message').length === 0)
  await page.screenshot({ path: path.join(profile, 'notebook-desktop.png'), animations: 'disabled' })
  await page.setViewportSize({ width: 1320, height: 1000 })
  assert(await page.locator('.editor-toolbar button, .info-tabs button').evaluateAll(buttons => buttons.every(button => {
    const box = button.getBoundingClientRect(); return box.x >= 0 && box.right <= 1321
  })))
  await page.setViewportSize({ width: 390, height: 844 })
  await page.getByRole('navigation', { name: '笔记视图' }).getByRole('button', { name: '正文', exact: true }).click()
  await page.screenshot({ path: path.join(profile, 'notebook-mobile.png'), animations: 'disabled' })
  await page.getByRole('button', { name: '折叠/展开侧栏', exact: true }).click()
  await page.locator('.app-sidebar.mobile-open').waitFor({ state: 'visible' })
  assert.equal(await page.getByRole('button', { name: '折叠/展开侧栏', exact: true }).getAttribute('aria-expanded'), 'true')
  await page.getByRole('button', { name: '关闭导航', exact: true }).click({ position: { x: 380, y: 400 } })
  await page.locator('.app-sidebar.mobile-open').waitFor({ state: 'hidden' })
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth + 1), false)
  assert(await page.locator('.editor-toolbar button').evaluateAll(buttons => buttons.every(button => {
    const box = button.getBoundingClientRect(); return box.x >= 0 && box.right <= 391
  })))
  await page.getByRole('navigation', { name: '笔记视图' }).getByRole('button', { name: '关联与历史', exact: true }).click()
  await page.locator('.version-item').first().waitFor()
  await page.screenshot({ path: path.join(profile, 'notebook-history-mobile.png'), animations: 'disabled' })
  assert.equal(await page.evaluate(() => {
    const tabs = document.querySelector('.info-tabs').getBoundingClientRect()
    const panel = document.querySelector('.history-panel').getBoundingClientRect()
    return panel.top >= tabs.bottom && panel.width >= innerWidth - 2
  }), true)
  await page.setViewportSize({ width: 820, height: 1000 })
  await page.getByRole('navigation', { name: '笔记视图' }).getByRole('button', { name: '正文', exact: true }).click()
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth + 1), false)
  await page.screenshot({ path: path.join(profile, 'notebook-tablet.png'), animations: 'disabled' })
  assert.deepEqual(errors, ['[Casy] update_knowledge failed: 合成测试：笔记保存失败'])
  const audit = await call('qa_audit')
  assert.equal(audit.integrity, 'ok')
  assert.equal(audit.foreignKeyErrors, 0)
  const report = { profile, seeded, timing, nativeCalls: calls.length, audit, errors: [], expectedErrors: errors }
  fs.writeFileSync(path.join(profile, 'verification.json'), JSON.stringify(report, null, 2))
  console.log(JSON.stringify(report))
} catch (error) {
  releaseSave?.()
  await page.screenshot({ path: path.join(profile, 'failure.png') })
  fs.writeFileSync(path.join(profile, 'failure.html'), await page.content())
  console.error('Notebook QA artifacts: ' + profile)
  throw error
} finally { await browser.close() }
