/**
 * 设置插件
 * 
 * 将设置功能封装为插件
 */

import type { CasyPlugin, CasyContext, CasyTool } from '../plugin/types'

export class SettingsPlugin implements CasyPlugin {
  name = 'settings'
  version = '1.0.0'
  description = '设置模块'
  
  async install(ctx: CasyContext): Promise<void> {
    ctx.registerTool(this.createGetSettingsTool(ctx))
    ctx.registerTool(this.createSaveSettingsTool(ctx))
    ctx.registerTool(this.createConfigureAiTool(ctx))
    
    console.log('SettingsPlugin installed')
  }
  
  async uninstall(ctx: CasyContext): Promise<void> {
    ctx.unregisterTool('get_settings')
    ctx.unregisterTool('save_settings')
    ctx.unregisterTool('configure_ai')
    console.log('SettingsPlugin uninstalled')
  }
  
  private createGetSettingsTool(ctx: CasyContext): CasyTool {
    return {
      name: 'get_settings',
      description: '获取设置',
      category: 'settings',
      parameters: { type: 'object', properties: {} },
      execute: async () => {
        const { tauriCallSafe } = await import('../../core/tauriBridge')
        const result = await tauriCallSafe('get_settings', {})
        return result
      },
    }
  }
  
  private createSaveSettingsTool(ctx: CasyContext): CasyTool {
    return {
      name: 'save_settings',
      description: '保存设置',
      category: 'settings',
      parameters: {
        type: 'object',
        properties: {
          data: { type: 'object', description: '设置数据' },
        },
        required: ['data'],
      },
      execute: async (params) => {
        const { tauriCallSafe } = await import('../../core/tauriBridge')
        const result = await tauriCallSafe('save_settings', { data: params.data })
        return result
      },
    }
  }
  
  private createConfigureAiTool(ctx: CasyContext): CasyTool {
    return {
      name: 'configure_ai',
      description: '配置 AI 后端',
      category: 'settings',
      parameters: {
        type: 'object',
        properties: {
          mode: { 
            type: 'string', 
            enum: ['ollama', 'openai', 'noop'],
            description: 'AI 模式' 
          },
          endpoint: { type: 'string', description: 'API 端点' },
          apiKey: { type: 'string', description: 'API Key' },
          model: { type: 'string', description: '模型名称' },
        },
        required: ['mode'],
      },
      execute: async (params) => {
        const { tauriCallSafe } = await import('../../core/tauriBridge')
        const result = await tauriCallSafe('configure_ai', {
          mode: params.mode,
          endpoint: params.endpoint,
          api_key: params.apiKey,
          model: params.model,
        })
        return result
      },
    }
  }
}
