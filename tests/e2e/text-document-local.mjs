import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { spawn, execFileSync } from 'node:child_process'
import assert from 'node:assert/strict'

const { chromium } = await import(process.env.CASY_PLAYWRIGHT_MODULE || 'playwright')
const profile = fs.mkdtempSync(path.join(process.env.CASY_QA_DIR || os.tmpdir(), 'text-document-profile-'))
const binary = path.join(process.cwd(), 'src-tauri/target/debug/examples/document_local_bridge')
const opened = [], calls = [], errors = [], compatibilityFailures = []
function call(command, args = {}) {
  calls.push(command)
  if (command === 'open_file_with_default') {
    assert(fs.statSync(args.path).isFile())
    opened.push(args.path)
    return Promise.resolve(null)
  }
  return new Promise((resolve, reject) => {
    const child = spawn(binary, [], {
      env: { ...process.env, CASY_TEST_DATA_DIR: profile }, stdio: ['pipe', 'pipe', 'pipe'],
    })
    let output = '', stderr = ''
    child.stdout.on('data', chunk => { output += chunk })
    child.stderr.on('data', chunk => { stderr += chunk })
    child.on('error', reject)
    child.on('close', code => {
      if (code) return reject(new Error(stderr))
      try {
        const result = JSON.parse(output)
        result.ok ? resolve(result.data) : reject(new Error(result.error))
      } catch (error) { reject(error) }
    })
    child.stdin.end(JSON.stringify({ command, args }))
  })
}
const source = path.join(profile, '案件约定.md')
const text = '# 案件约定\n\n第三人乙公司的赔偿金额为125000.25元。\n\n付款期限为判决生效之日起十五日。\n'
fs.writeFileSync(source, text)
await call('qa_seed', { source })
// textutil only generates synthetic fixtures; parsing uses the Rust pipeline.
if (process.platform === 'darwin') {
  for (const ext of ['doc', 'docx']) {
    const word = path.join(profile, `证据.${ext}`)
    execFileSync('/usr/bin/textutil', ['-convert', ext, '-output', word, source])
    await call('qa_seed', { source: word, fileId: ext })
    await call('queue_document_processing', { fileId: ext })
    await call('qa_process_next')
    const job = (await call('list_document_jobs', { fileId: ext }))[0]
    if (ext === 'doc' && job.status === 'failed') {
      assert(job.errorMessage.includes('DOCUMENT_PARSE:'))
      compatibilityFailures.push({ format: ext, error: job.errorMessage })
      continue
    }
    assert.equal(job.status, 'completed', job.errorMessage)
    assert(fs.readFileSync(job.markdownPath, 'utf8').includes('125000.25'))
  }
}
const browser = await chromium.launch({ channel: 'chrome', headless: true })
const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } })
page.on('pageerror', error => errors.push(error.message))
page.on('console', message => {
  if (message.type() === 'error' || message.text().startsWith('[Vue warn]')) errors.push(message.text())
})
await page.exposeFunction('__casyTextDocument', call)
try {
  const url = new URL(process.env.CASY_QA_URL || 'http://127.0.0.1:1421/')
  url.hash = '/cases'
  await page.goto(url.href)
  await page.getByRole('button', { name: '暂时跳过' }).click()
  await page.locator('.cases-topbar').waitFor()
  await page.evaluate(async () => {
    const { tryMockCommand } = await import('/src/core/mockData.ts')
    const native = new Set([
      'get_case', 'list_case_files', 'list_removed_case_files', 'list_case_dirs', 'list_case_document_jobs', 'get_document_engine_status', 'queue_document_processing',
      'list_document_jobs', 'retry_document_job', 'cancel_document_job', 'get_file_ocr_text', 'list_case_ocr_states',
      'search_document_passages', 'get_document_page', 'list_knowledge_document_sources', 'import_pageindex_to_knowledge',
      'list_knowledge', 'get_knowledge_with_blocks', 'open_file_with_default',
    ])
    window.__TAURI_INTERNALS__ = {
      invoke: async (command, args = {}) => {
        if (native.has(command)) return window.__casyTextDocument(command, args)
        if (command === 'get_settings') return {}
        if (command === 'get_lawyer_profile') return { onboarding_completed: true, name: '本地验收' }
        const result = tryMockCommand(command, args)
        if (result === undefined) throw new Error('Command outside text document UI test: ' + command)
        return result
      }, transformCallback: () => 0, unregisterCallback: () => {},
    }
    location.hash = '/files/ocr-case'
  })
  await page.locator('.file-row').filter({ hasText: '案件约定.md' }).click()
  await page.getByRole('button', { name: '提取正文并索引', exact: true }).click()
  await page.locator('.ocr-badge').filter({ hasText: '已排队' }).first().waitFor()
  const started = Date.now()
  await call('qa_process_next')
  await page.getByRole('button', { name: '打开 Markdown 备份', exact: true }).click()
  assert.equal(fs.readFileSync(opened[0], 'utf8'), text)
  const job = (await call('list_document_jobs', { fileId: 'ocr-file' }))[0]
  assert.equal(job.searchablePdfPath, null)
  await page.getByRole('button', { name: '卷宗检索', exact: true }).click()
  const drawer = page.locator('.reasoning-drawer')
  if (process.platform === 'darwin') {
    await drawer.locator('.el-select').click()
    await page.getByRole('option', { name: '证据.docx', exact: true }).click()
    await page.getByRole('option', { name: '证据.doc', exact: true }).click()
    await drawer.getByRole('heading', { name: '卷宗检索' }).click()
  }
  await drawer.getByPlaceholder('关键词或案情问题').fill('第三人的赔偿金额是多少')
  await drawer.getByRole('button', { name: '检索', exact: true }).click()
  await drawer.locator('.passage').filter({ hasText: '125000.25' }).first().waitFor()
  assert.equal(await drawer.locator('.passage').count(), 1)
  assert((await drawer.innerText()).includes('第 1 段'))
  await drawer.getByRole('button', { name: '定位原文' }).first().click()
  const sourceDialog=page.locator('.document-source-dialog')
  await sourceDialog.locator('pre').filter({hasText:'125000.25'}).waitFor()
  assert.equal(await sourceDialog.locator('.source-page-image').count(),0)
  await sourceDialog.getByRole('button',{name:'关闭此对话框',exact:true}).click()
  await page.screenshot({ path: path.join(profile, 'source-search-desktop.png'), animations: 'disabled' })
  await page.setViewportSize({ width: 390, height: 844 })
  await page.screenshot({ path: path.join(profile, 'source-search-mobile.png'), animations: 'disabled' })
  const bounds = await drawer.boundingBox()
  assert(bounds.x >= 0 && bounds.x + bounds.width <= 391)
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth + 1), false)
  await drawer.getByPlaceholder('关键词或案情问题').fill('不存在的专利技术')
  await drawer.getByRole('button', { name: '检索', exact: true }).click()
  await drawer.getByText('没有找到相关原文').waitFor()
  await drawer.getByRole('button', { name: '关闭', exact: true }).click()
  await page.setViewportSize({ width: 1440, height: 1000 })
  await page.evaluate(() => { location.hash = '/knowledge' })
  await page.locator('.info-tabs').getByRole('button', { name: '沉淀', exact: true }).click()
  const item = page.locator('.source-item').filter({ hasText: '案件约定.md' })
  await item.getByRole('button', { name: '沉淀', exact: true }).click()
  await item.getByRole('button', { name: '打开', exact: true }).waitFor()
  const sources = await call('list_knowledge_document_sources')
  const knowledgeId = sources.find(s => s.fileId === 'ocr-file').importedKnowledgeId
  assert(knowledgeId)
  const note = await call('get_knowledge_with_blocks', { id: knowledgeId })
  assert(JSON.stringify(note).includes('125000.25'))
  await page.locator('.tiptap').filter({ hasText: '125000.25' }).waitFor()
  assert(await page.locator('.md-wysiwyg-toolbar button').evaluateAll(buttons => buttons.every(button => button.getBoundingClientRect().height < 40)))
  await page.screenshot({ path: path.join(profile, 'knowledge-import-desktop.png'), animations: 'disabled' })
  assert(!calls.includes('reasoning_search'))
  assert.deepEqual(errors, [])
  const audit = await call('qa_audit')
  assert.equal(audit.foreignKeyErrors, 0)
  const report = { profile, job, audit, elapsedSeconds: (Date.now() - started) / 1000, errors, compatibilityFailures, opened, nativeCalls: calls.length }
  fs.writeFileSync(path.join(profile, 'verification.json'), JSON.stringify(report, null, 2))
  console.log(JSON.stringify(report))
} catch (error) {
  await page.screenshot({ path: path.join(profile, 'failure.png') })
  fs.writeFileSync(path.join(profile, 'failure.html'), await page.content())
  console.error('Text document QA artifacts: ' + profile)
  throw error
} finally {
  await browser.close()
}
