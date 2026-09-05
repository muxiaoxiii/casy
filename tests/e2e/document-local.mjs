import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { spawn } from 'node:child_process'
import assert from 'node:assert/strict'

const { chromium } = await import(process.env.CASY_PLAYWRIGHT_MODULE || 'playwright')
const profile = fs.mkdtempSync(path.join(process.env.CASY_QA_DIR || os.tmpdir(), 'document-profile-'))
const binary = path.join(process.cwd(), 'src-tauri/target/debug/examples/document_local_bridge')
const source = process.env.CASY_OCR_QA_SOURCE
assert(source, 'CASY_OCR_QA_SOURCE must point to a synthetic scanned PDF')
function call(command, args = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(binary, [], {
      env: { ...process.env, CASY_TEST_DATA_DIR: profile }, stdio: ['pipe', 'pipe', 'pipe'],
    })
    let output = '', errors = ''
    child.stdout.on('data', chunk => { output += chunk })
    child.stderr.on('data', chunk => { errors += chunk })
    child.on('error', reject)
    child.on('close', code => {
      if (code) return reject(new Error(errors))
      try {
        const result = JSON.parse(output)
        result.ok ? resolve(result.data) : reject(new Error(result.error))
      } catch (error) { reject(error) }
    })
    child.stdin.end(JSON.stringify({ command, args }))
  })
}
await call('qa_seed', { source })
const browser = await chromium.launch({ channel: 'chrome', headless: true })
const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } })
const errors = []
page.on('pageerror', error => errors.push(error.message))
await page.exposeFunction('__casyLocalDocument', call)
let worker
try {
  await page.goto(process.env.CASY_QA_URL || 'http://127.0.0.1:1421/')
  await page.getByRole('button', { name: '稍后再填' }).click()
  await page.evaluate(async () => {
    const { tryMockCommand } = await import('/src/core/mockData.ts')
    const commands = new Set([
      'get_case', 'list_case_files', 'get_document_engine_status', 'queue_document_processing',
      'list_document_jobs', 'retry_document_job', 'cancel_document_job', 'get_file_ocr_text', 'list_case_ocr_states',
    ])
    window.__TAURI_INTERNALS__ = {
      invoke: async (command, args = {}) => {
        if (commands.has(command)) return window.__casyLocalDocument(command, args)
        const result = tryMockCommand(command, args)
        if (result === undefined) throw new Error('Command outside document UI test: ' + command)
        return result
      }, transformCallback: () => 0, unregisterCallback: () => {},
    }
    location.hash = '/files/ocr-case'
  })
  const processButton = page.getByRole('button', { name: '生成可搜索 PDF', exact: true })
  await processButton.click()
  await page.getByRole('button', { name: '取消处理', exact: true }).click()
  await page.locator('.ocr-badge').filter({ hasText: '已取消' }).first().waitFor()
  let jobs = await call('list_document_jobs', { fileId: 'ocr-file' })
  assert.equal(jobs[0].status, 'cancelled')
  await page.getByRole('button', { name: '重试', exact: true }).click()
  await page.getByText('已重新加入处理队列', { exact: true }).waitFor()
  await page.locator('.ocr-badge').filter({ hasText: '已排队' }).first().waitFor()
  jobs = await call('list_document_jobs', { fileId: 'ocr-file' })
  assert.equal(jobs[0].status, 'queued')
  assert.equal(jobs.length, 2)
  const started = Date.now()
  worker = call('qa_process_next')
  await page.waitForFunction(() => document.querySelector('.ocr-badge.processing'), null, { timeout: 60000 })
  await page.screenshot({ path: path.join(profile, 'ocr-running-desktop.png') })
  await page.getByRole('button', { name: '取消处理', exact: true }).click()
  await page.locator('.ocr-badge').filter({ hasText: '已取消' }).first().waitFor()
  await worker
  jobs = await call('list_document_jobs', { fileId: 'ocr-file' })
  assert.equal(jobs[0].status, 'cancelled')
  assert.equal((await call('qa_audit')).pages, 0)
  await page.getByRole('button', { name: '重试', exact: true }).click()
  await page.locator('.ocr-badge').filter({ hasText: '已排队' }).first().waitFor()
  worker = call('qa_process_next')
  await worker
  jobs = await call('list_document_jobs', { fileId: 'ocr-file' })
  assert.equal(jobs[0].status, 'completed', jobs[0].errorMessage)
  const audit = await call('qa_audit')
  assert.equal(audit.ocrStatus, 'completed')
  assert.equal(audit.indexStatus, 'completed')
  assert.equal(audit.category, 'summons')
  assert.equal(audit.pages, 2)
  assert(audit.nodes >= 3)
  assert.equal(audit.integrity, 'ok')
  assert.equal(audit.foreignKeyErrors, 0)
  await page.getByRole('button', { name: '查看文本', exact: true }).click({ timeout: 15000 })
  await page.locator('.ocr-text-view').filter({ hasText: '第三人' }).waitFor()
  await page.getByRole('dialog').waitFor({ state: 'visible' })
  await page.screenshot({ path: path.join(profile, 'ocr-text-desktop.png'), animations: 'disabled' })
  await page.getByRole('dialog').getByRole('button', { name: '关闭', exact: true }).click()
  await page.setViewportSize({ width: 390, height: 844 })
  await page.screenshot({ path: path.join(profile, 'ocr-files-mobile.png') })
  await page.locator('.file-row').first().click()
  await page.getByRole('button', { name: '查看文本', exact: true }).click()
  const dialog = page.getByRole('dialog')
  await dialog.waitFor({ state: 'visible' })
  await page.screenshot({ path: path.join(profile, 'ocr-text-mobile.png'), animations: 'disabled' })
  const bounds = await dialog.boundingBox()
  assert(bounds.x >= 0 && bounds.x + bounds.width <= 391)
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth > innerWidth + 1)
  assert.equal(overflow, false)
  assert.deepEqual(errors, [])
  fs.writeFileSync(path.join(profile, 'verification.json'), JSON.stringify({ audit, jobs, elapsedSeconds: (Date.now() - started) / 1000, errors }, null, 2))
  console.log(JSON.stringify({ profile, audit, elapsedSeconds: (Date.now() - started) / 1000 }))
} catch (error) {
  await page.screenshot({ path: path.join(profile, 'failure.png') })
  console.error('Document QA artifacts: ' + profile)
  throw error
} finally {
  await worker?.catch(() => {})
  await browser.close()
}
