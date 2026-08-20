/**
 * 同步插件
 * 
 * 将同步功能封装为插件
 */

import type { CasyPlugin, CasyContext, CasyTool } from '../plugin/types'

/** 从设置中读取 WebDAV 凭据（settings 表 snake_case 键） */
async function getWebdavCredentials() {
  const { tauriCallSafe } = await import('../../core/tauriBridge')
  const res = await tauriCallSafe('get_settings', {})
  if (!res.ok || !res.data) return null
  const s = res.data
  const url = s.webdav_url || s.webdavUrl
  const username = s.webdav_username || s.webdavUsername
  const password = s.webdav_password || s.webdavPassword
  if (!url || !username || !password) return null
  return { url, username, password }
}

export class SyncPlugin implements CasyPlugin {
  name = 'sync'
  version = '1.0.0'
  description = '同步模块（WebDAV/飞书）'
  
  async install(ctx: CasyContext): Promise<void> {
    ctx.registerTool(this.createGetSyncStatusTool(ctx))
    ctx.registerTool(this.createTestWebdavConnectionTool(ctx))
    ctx.registerTool(this.createManualSyncPushTool(ctx))
    ctx.registerTool(this.createManualSyncPullTool(ctx))
    
    console.log('SyncPlugin installed')
  }
  
  async uninstall(ctx: CasyContext): Promise<void> {
    ctx.unregisterTool('get_sync_status')
    ctx.unregisterTool('test_webdav_connection')
    ctx.unregisterTool('manual_sync_push')
    ctx.unregisterTool('manual_sync_pull')
    console.log('SyncPlugin uninstalled')
  }
  
  private createGetSyncStatusTool(ctx: CasyContext): CasyTool {
    return {
      name: 'get_sync_status',
      description: '获取同步状态（WebDAV/飞书）',
      category: 'sync',
      parameters: { type: 'object', properties: {} },
      execute: async () => {
        const { tauriCallSafe } = await import('../../core/tauriBridge')
        const result = await tauriCallSafe('get_sync_status', {})
        return result
      },
    }
  }
  
  private createTestWebdavConnectionTool(ctx: CasyContext): CasyTool {
    return {
      name: 'test_webdav_connection',
      description: '测试 WebDAV 连接',
      category: 'sync',
      parameters: { type: 'object', properties: {} },
      execute: async () => {
        const { tauriCallSafe } = await import('../../core/tauriBridge')
        const creds = await getWebdavCredentials()
        if (!creds) return { ok: false, error: '未配置 WebDAV（请在设置中填写 URL/用户名/密码）' }
        const result = await tauriCallSafe('test_webdav_connection', creds)
        return result
      },
    }
  }
  
  private createManualSyncPushTool(ctx: CasyContext): CasyTool {
    return {
      name: 'manual_sync_push',
      description: '手动推送同步',
      category: 'sync',
      parameters: { type: 'object', properties: {} },
      execute: async () => {
        // 需要 L2 确认
        const confirmed = await ctx.requestConfirm({
          level: 'L2',
          title: '确认推送同步',
          message: '确定要将本地数据推送到远程吗？',
          onConfirm: async () => {},
        })
        
        if (!confirmed) {
          return { ok: false, error: '用户取消操作' }
        }
        
        const { tauriCallSafe } = await import('../../core/tauriBridge')
        const creds = await getWebdavCredentials()
        if (!creds) return { ok: false, error: '未配置 WebDAV（请在设置中填写 URL/用户名/密码）' }
        const result = await tauriCallSafe('webdav_push', creds)
        return result
      },
    }
  }
  
  private createManualSyncPullTool(ctx: CasyContext): CasyTool {
    return {
      name: 'manual_sync_pull',
      description: '手动拉取同步',
      category: 'sync',
      parameters: { type: 'object', properties: {} },
      execute: async () => {
        // 需要 L2 确认
        const confirmed = await ctx.requestConfirm({
          level: 'L2',
          title: '确认拉取同步',
          message: '确定要从远程拉取数据吗？这会覆盖本地数据。',
          onConfirm: async () => {},
        })
        
        if (!confirmed) {
          return { ok: false, error: '用户取消操作' }
        }
        
        const { tauriCallSafe } = await import('../../core/tauriBridge')
        const creds = await getWebdavCredentials()
        if (!creds) return { ok: false, error: '未配置 WebDAV（请在设置中填写 URL/用户名/密码）' }
        const result = await tauriCallSafe('webdav_pull', creds)
        return result
      },
    }
  }
}
