import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { spawn } from 'node:child_process'
import assert from 'node:assert/strict'

const { chromium } = await import(process.env.CASY_PLAYWRIGHT_MODULE || 'playwright')
const profile = fs.mkdtempSync(path.join(process.env.CASY_QA_DIR || os.tmpdir(), 'file-lifecycle-profile-'))
const binary = path.join(process.cwd(), 'src-tauri/target/debug/examples/document_local_bridge')
const errors = [], calls = [], opened = []
let selectedPaths = []
async function call(command, args = {}) {
  calls.push(command)
  if (command === 'plugin:dialog|open') return selectedPaths
  if (command === 'open_file_with_default') {
    assert(fs.statSync(args.path).isFile())
    opened.push(args.path)
    return null
  }
  return new Promise((resolve, reject) => {
    const child = spawn(binary, [], { env: { ...process.env, CASY_TEST_DATA_DIR: profile }, stdio: ['pipe', 'pipe', 'pipe'] })
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
const source = path.join(profile, '案件材料.md')
const content = '# 案件材料\n\n第三人的赔偿金额为125000.25元。\n'
fs.writeFileSync(source, content)
selectedPaths = [source]
const caseData = await call('create_case', { data: { caseName: '卷宗操作验收', clientName: '本地合成客户', track: 'civil_tort' } })
const caseId = caseData.id
const list = () => call('list_case_files', { caseId })
const browser = await chromium.launch({ channel: 'chrome', headless: true })
const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } })
page.on('pageerror', error => errors.push(error.message))
page.on('console', message => { if (message.type() === 'error' || message.text().startsWith('[Vue warn]')) errors.push(message.text()) })
await page.exposeFunction('__casyFiles', call)
try {
  await page.goto(process.env.CASY_QA_URL || 'http://127.0.0.1:1422/')
  await page.getByRole('button', { name: '暂时跳过' }).click()
  await page.evaluate(async caseId => {
    const { tryMockCommand } = await import('/src/core/mockData.ts')
    const native = new Set(['get_case', 'list_case_files', 'list_removed_case_files', 'list_case_dirs', 'list_case_document_jobs',
      'get_document_engine_status', 'list_case_ocr_states', 'create_case_subdir', 'import_files_to_case', 'apply_case_file_renames',
      'move_case_files', 'set_case_file_category', 'delete_case_file', 'restore_case_file', 'open_file_with_default', 'plugin:dialog|open'])
    window.__TAURI_INTERNALS__ = { invoke: async (command, args = {}) => {
      if (native.has(command)) return window.__casyFiles(command, args)
      const result = tryMockCommand(command, args)
      if (result === undefined) throw new Error('Command outside file UI test: ' + command)
      return result
    }, transformCallback: () => 0, unregisterCallback: () => {} }
    location.hash = '/files/' + caseId
  }, caseId)
  await page.getByRole('button', { name: '新建文件夹', exact: true }).click()
  await page.locator('.el-message-box input').fill('庭审材料')
  await page.locator('.el-message-box').getByRole('button', { name: '新建', exact: true }).click()
  await page.locator('.directory-picker').getByText('庭审材料', { exact: true }).waitFor()
  await page.getByRole('navigation', { name: '文件分类' }).getByRole('button', { name: /证据材料/ }).click()
  await page.getByRole('button', { name: '上传文件', exact: true }).click()
  await page.locator('.file-row').filter({ hasText: '案件材料.md' }).waitFor()
  let file = (await list())[0]
  const id = file.id
  assert.equal(file.category, 'evidence')
  assert(file.filePath.includes('/庭审材料/'))
  assert.equal(fs.readFileSync(file.filePath, 'utf8'), content)
  assert.equal(fs.readFileSync(source, 'utf8'), content)
  await page.getByRole('button', { name: '重命名', exact: true }).click()
  await page.locator('.el-message-box input').fill('庭审证据')
  await page.locator('.el-message-box').getByRole('button', { name: '重命名', exact: true }).click()
  await page.locator('.file-row').filter({ hasText: '庭审证据.md' }).waitFor()
  assert(!fs.existsSync(file.filePath))
  file = (await list())[0]
  assert.equal(file.id, id)
  await page.locator('.file-inspector .el-select__wrapper').click()
  await page.getByRole('option', { name: '提交文件', exact: true }).click()
  await page.getByRole('navigation', { name: '文件分类' }).getByRole('button', { name: /全部文件/ }).click()
  await page.locator('.file-row').filter({ hasText: '庭审证据.md' }).click()
  assert.equal((await list())[0].category, 'submitted')
  await page.getByRole('button', { name: '移动到文件夹', exact: true }).click()
  const move = page.getByRole('dialog', { name: '移动文件', exact: true })
  await move.locator('.el-select').click()
  const dirs = await call('list_case_dirs', { caseId })
  await page.getByRole('option', { name: dirs[0].name, exact: true }).click()
  await move.getByRole('button', { name: '移动', exact: true }).click()
  await move.waitFor({ state: 'hidden' })
  await page.locator('.file-row').filter({ hasText: '庭审证据.md' }).waitFor()
  assert(!fs.existsSync(file.filePath))
  file = (await list())[0]
  assert.equal(path.dirname(file.filePath), dirs[0].absolutePath)
  await page.getByRole('button', { name: '打开文件', exact: true }).click()
  assert.equal(opened[0], file.filePath)
  await page.getByRole('button', { name: '移除登记', exact: true }).click()
  await page.locator('.el-message-box').getByRole('button', { name: '移除登记', exact: true }).click()
  await page.locator('.file-row').waitFor({ state: 'hidden' })
  assert(fs.existsSync(file.filePath))
  assert.equal((await list()).length, 0)
  await page.getByRole('navigation', { name: '文件分类' }).getByRole('button', { name: /已移除登记/ }).click()
  await page.locator('.file-row').filter({ hasText: '庭审证据.md' }).click()
  await page.getByRole('button', { name: '恢复登记', exact: true }).click()
  await page.getByRole('button', { name: '打开文件', exact: true }).waitFor()
  assert.equal((await list())[0].id, id)
  assert.equal(fs.readFileSync(file.filePath, 'utf8'), content)
  const secondSource = path.join(profile, '补充证据.md')
  fs.writeFileSync(secondSource, '第二份合成材料')
  selectedPaths = [secondSource, secondSource]
  await page.getByRole('button', { name: '上传文件', exact: true }).click()
  await page.locator('.file-row').filter({ hasText: '补充证据.md' }).waitFor()
  assert.equal((await list()).length, 2)
  await page.locator('.selection-actions .el-checkbox').click()
  assert.equal(await page.getByRole('checkbox', { name: '选择全部文件', exact: true }).isChecked(), true)
  await page.locator('.selection-actions').getByRole('button', { name: '移动', exact: true }).click()
  await move.locator('.el-select').click()
  await page.getByRole('option', { name: '庭审材料', exact: true }).click()
  await move.getByRole('button', { name: '移动', exact: true }).click()
  await move.waitFor({ state: 'hidden' })
  const movedFiles = await list()
  assert(movedFiles.every(file => file.filePath.includes('/庭审材料/')))
  assert.equal(fs.readFileSync(movedFiles.find(file => file.id === id).filePath, 'utf8'), content)
  await page.waitForFunction(() => document.querySelectorAll('.el-message').length === 0)
  await page.screenshot({ path: path.join(profile, 'files-desktop.png'), animations: 'disabled' })
  await page.setViewportSize({ width: 390, height: 844 })
  await page.screenshot({ path: path.join(profile, 'files-mobile.png'), animations: 'disabled' })
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth + 1), false)
  assert(await page.locator('.file-row').evaluateAll(rows => rows.every(row => {
    const bounds = row.getBoundingClientRect()
    return bounds.x >= 0 && bounds.right <= 391
  })))
  assert.deepEqual(errors, [])
  const audit = await call('qa_file_audit')
  assert.equal(audit.integrity, 'ok')
  assert.equal(audit.foreignKeyErrors, 0)
  const report = { profile, caseId, fileId: id, audit, errors, nativeCalls: calls.length }
  fs.writeFileSync(path.join(profile, 'verification.json'), JSON.stringify(report, null, 2))
  console.log(JSON.stringify(report))
} catch (error) {
  await page.screenshot({ path: path.join(profile, 'failure.png') })
  fs.writeFileSync(path.join(profile, 'failure.html'), await page.content())
  console.error('File QA artifacts: ' + profile)
  throw error
} finally { await browser.close() }
