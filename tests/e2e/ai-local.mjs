import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import http from 'node:http'
import { spawn } from 'node:child_process'
import assert from 'node:assert/strict'

const { chromium } = await import(process.env.CASY_PLAYWRIGHT_MODULE || 'playwright')
const profile = fs.mkdtempSync(path.join(process.env.CASY_QA_DIR || os.tmpdir(), 'ai-profile-'))
const binary = path.join(process.cwd(), 'src-tauri/target/debug/examples/ai_local_bridge')
function call(command, args = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(binary, [], { env: { ...process.env, CASY_TEST_DATA_DIR: profile }, stdio: ['pipe', 'pipe', 'pipe'] })
    let output = '', errors = ''
    child.stdout.on('data', d => { output += d })
    child.stderr.on('data', d => { errors += d })
    child.on('error', reject)
    child.on('close', code => {
      if (code) return reject(new Error(errors))
      const result = JSON.parse(output)
      result.ok ? resolve(result.data) : reject(new Error(result.error))
    })
    child.stdin.end(JSON.stringify({ command, args }))
  })
}
const requests = []
let credentialAccounts = []
let failNext = false
const server = http.createServer((req, res) => {
  let body = ''
  req.on('data', chunk => { body += chunk })
  req.on('end', () => {
    requests.push({ url: req.url, authorization: req.headers.authorization, body: JSON.parse(body) })
    res.setHeader('Content-Type', 'application/json')
    if (failNext) { failNext = false; res.writeHead(401); res.end('synthetic-key-echo'); return }
    res.end(JSON.stringify({ choices: [{ message: { content: `Response from ${JSON.parse(body).model}` } }] }))
  })
})
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
const baseUrl = `http://127.0.0.1:${server.address().port}`
const browser = await chromium.launch({ channel: 'chrome', headless: true })
const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } })
const pageErrors = []
page.on('pageerror', error => pageErrors.push(error.message))
await call('get_ai_profiles')
await page.exposeFunction('__casyLocalAi', call)
try {
  await page.goto(process.env.CASY_QA_URL || 'http://127.0.0.1:1421/')
  await page.getByRole('button', { name: '稍后再填' }).click()
  await page.evaluate(async () => {
    const { tryMockCommand } = await import('/src/core/mockData.ts')
    const commands = new Set(['get_ai_profiles', 'save_ai_profiles', 'test_ai_profile', 'ai_chat', 'get_ai_config'])
    window.__TAURI_INTERNALS__ = { invoke: async (cmd, args = {}) => {
      if (commands.has(cmd)) return window.__casyLocalAi(cmd, args)
      const mock = tryMockCommand(cmd, args)
      if (mock === undefined) throw new Error('Command not included in isolated AI test')
      return mock
    },
      transformCallback: () => 0, unregisterCallback: () => {} }
    location.hash = '/settings'
  })
  await page.locator('.settings-sidebar .nav-item').filter({ hasText: /AI/ }).click()
  const settings = page.locator('.ai-settings')
  const field = label => settings.locator('.el-form-item').filter({ has: page.locator('label').filter({ hasText: new RegExp('^' + label + '$') }) }).locator('input,textarea').first()
  await settings.getByRole('button', { name: '添加配置', exact: true }).click()
  await field('配置名称').fill('Primary API')
  await field('API 基础地址').fill(baseUrl + '/first/v1/')
  await field('API Key').fill('synthetic-key-a')
  await field('模型 ID').fill('model-a')
  await settings.getByRole('button', { name: '测试连接', exact: true }).click()
  await settings.getByText('连接成功：model-a', { exact: true }).waitFor()
  await settings.getByRole('button', { name: '保存配置', exact: true }).click()
  await page.getByText('AI 配置已保存', { exact: true }).waitFor()
  await settings.getByRole('button', { name: '添加配置', exact: true }).click()
  await field('配置名称').fill('Secondary API')
  await field('API 基础地址').fill(baseUrl + '/second/v1')
  await field('API Key').fill('synthetic-key-b')
  await field('模型 ID').fill('model-b')
  await settings.getByRole('button', { name: '保存配置', exact: true }).click()
  await page.waitForFunction(() => document.querySelector('.ai-settings input[autocomplete="new-password"]')?.value === '')
  let saved = await call('get_ai_profiles')
  assert.equal(saved.profiles.length, 2)
  assert(saved.profiles.every(p => p.hasApiKey && !p.apiKey))
  const [first, second] = saved.profiles
  saved.activeId = second.id
  saved.systemPrompt = 'Synthetic system preference'
  await call('save_ai_profiles', { config: saved })
  const chat = await call('ai_chat', { messages: [{ role: 'user', content: 'Synthetic question' }] })
  assert.equal(chat.content, 'Response from model-b')
  assert.equal(requests.at(-1).authorization, 'Bearer synthetic-key-b')
  assert.equal(requests.at(-1).url, '/second/v1/chat/completions')
  assert(requests.at(-1).body.messages.some(m => m.content === 'Synthetic system preference'))
  await call('ai_chat', { profileId: first.id, messages: [{ role: 'user', content: 'Second question' }] })
  assert.equal(requests.at(-1).authorization, 'Bearer synthetic-key-a')
  assert.equal(requests.at(-1).url, '/first/v1/chat/completions')
  const before = requests.length
  await assert.rejects(call('ai_chat', { apiUrl: baseUrl + '/wrong', messages: [] }), /接口地址/)
  assert.equal(requests.length, before)
  await settings.getByRole('tab', { name: 'Primary API', exact: true }).click()
  await field('模型 ID').fill('model-a-edited')
  await settings.getByRole('button', { name: '保存配置', exact: true }).click()
  await page.waitForTimeout(500)
  const edited = await call('ai_chat', { profileId: first.id, messages: [{ role: 'user', content: 'Edited model' }] })
  assert.equal(edited.content, 'Response from model-a-edited')
  assert.equal(requests.at(-1).authorization, 'Bearer synthetic-key-a')
  failNext = true
  await settings.getByRole('button', { name: '测试连接', exact: true }).click()
  await settings.locator('.el-alert--error').filter({ hasText: 'HTTP 401' }).waitFor()
  assert(!(await settings.innerText()).includes('synthetic-key-echo'))
  await page.locator('.el-message').first().waitFor({ state: 'hidden' })
  await page.screenshot({ path: path.join(profile, 'ai-settings-desktop.png') })
  await page.setViewportSize({ width: 390, height: 844 })
  await page.screenshot({ path: path.join(profile, 'ai-settings-mobile.png') })
  const publicConfig = await call('get_ai_config')
  assert.equal(publicConfig.apiKey, null)
  const allSettings = await call('get_settings')
  assert(!('ai_profiles_v1' in allSettings) && !('ai_api_key' in allSettings))
  assert(!JSON.stringify(allSettings).includes('synthetic-key'))
  const storage = await call('qa_ai_storage')
  assert.equal(storage.leakedSettings, 0)
  credentialAccounts = Object.values(storage.credentialAccounts)
  assert.equal(credentialAccounts.length, 2)
  const bounds = await settings.boundingBox()
  assert(bounds.x >= 0 && bounds.x + bounds.width <= 391)
  assert.deepEqual(pageErrors, [])
  console.log(JSON.stringify({ profile, requests: requests.length, credentialIsolation: 'passed', nativeRestart: 'passed', pageErrors }, null, 2))
} catch (error) {
  await page.screenshot({ path: path.join(profile, 'ai-failure.png') })
  console.error(JSON.stringify({ profile, alerts: await page.locator('.el-alert').allTextContents() }))
  throw error
} finally {
  await call('save_ai_profiles', { config: { profiles: [], activeId: null, dailyLimit: 50, systemPrompt: '' } })
  for (const account of credentialAccounts) assert.equal(await call('qa_ai_credential_exists', { account }), false)
  await browser.close()
  await new Promise(resolve => server.close(resolve))
}
