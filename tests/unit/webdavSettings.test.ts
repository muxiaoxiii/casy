// @vitest-environment jsdom
import { beforeEach, afterEach, expect, it, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import ElementPlus from 'element-plus'
import WebDAVSettings from '../../src/modules/settings/components/WebDAVSettings.vue'
const mocks = vi.hoisted(() => ({
  backupFull: vi.fn(), restoreFull: vi.fn(),
  store: { webdavUrl: 'https://dav.example.com/casy', webdavUsername: 'user', webdavPassword: '', webdavPassword_configured: true, webdavAutoSync: false },
}))
vi.mock('../../src/core/plugin/context', () => ({ casyContext: { sync: {
  status: vi.fn().mockResolvedValue({ ok: true, data: { connectionState: 'unknown' } }),
  backupFull: mocks.backupFull, restoreFull: mocks.restoreFull,
} } }))
vi.mock('../../src/stores/settings', () => ({ useSettingsStore: () => mocks.store }))
let view: ReturnType<typeof mount>
beforeEach(async () => {
  vi.clearAllMocks()
  view = mount(WebDAVSettings, { attachTo: document.body, global: { plugins: [ElementPlus], stubs: { teleport: true } } })
  await flushPromises()
})
afterEach(() => { view.unmount(); document.body.innerHTML = '' })
const click = async (text: string) => { await view.findAll('button').find(b => b.text() === text)!.trigger('click'); await flushPromises() }
it('shows full data actions and only starts backup after valid matching passwords', async () => {
  await click('备份全部数据')
  await click('加密并备份')
  expect(mocks.backupFull).not.toHaveBeenCalled()
  await view.get('input[aria-label="备份密码"]').setValue('test-password-123')
  await view.get('input[aria-label="再次输入备份密码"]').setValue('test-password-123')
  mocks.backupFull.mockResolvedValue({ ok: true, data: '完整备份成功' })
  await click('加密并备份')
  expect(mocks.backupFull).toHaveBeenCalledWith(mocks.store.webdavUrl, 'user', '', 'test-password-123')
})
it('requires explicit restore confirmation and keeps failure visible without reporting success', async () => {
  await click('恢复全部数据')
  expect(view.text()).toContain('当前案件、任务和设置将被备份中的数据替换')
  expect(mocks.restoreFull).not.toHaveBeenCalled()
  await view.get('input[aria-label="备份密码"]').setValue('wrong-password')
  mocks.restoreFull.mockResolvedValue({ ok: false, error: '备份密码错误或文件损坏' })
  await click('确认覆盖并恢复')
  expect(mocks.restoreFull).toHaveBeenCalledOnce()
  expect(view.get('input[aria-label="备份密码"]').exists()).toBe(true)
  expect(document.body.textContent).toContain('备份密码错误或文件损坏')
})
it('blocks duplicate requests and closing while transfer is pending', async () => {
  await click('备份全部数据')
  await view.get('input[aria-label="备份密码"]').setValue('test-password-123')
  await view.get('input[aria-label="再次输入备份密码"]').setValue('test-password-123')
  let finish!: (value: unknown) => void
  mocks.backupFull.mockImplementation(() => new Promise(resolve => { finish = resolve }))
  await click('加密并备份')
  await click('加密并备份')
  expect(mocks.backupFull).toHaveBeenCalledOnce()
  expect(view.findAll('button').find(b => b.text() === '取消')!.attributes('disabled')).toBeDefined()
  finish({ ok: false, error: '网络中断' }); await flushPromises()
  expect(view.findAll('button').find(b => b.text() === '取消')!.attributes('disabled')).toBeUndefined()
})
