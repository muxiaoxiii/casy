/**
 * 飞书自动推送 composable
 *
 * 提供数据变更通知功能，在本地数据变更后自动触发飞书推送（5秒防抖）
 */

import { ref } from 'vue'
import { tauriCallSafe } from './tauriBridge'
import type { TauriResult } from '../types'

interface AutoPushStatus {
  enabled: boolean
  pending: boolean
  hasTimer: boolean
  configured: boolean
}

// 自动推送状态
const autoPushEnabled = ref(false)
const autoPushStatus = ref<AutoPushStatus>({
  enabled: false,
  pending: false,
  hasTimer: false,
  configured: false,
})

/**
 * 加载自动推送状态
 */
export async function loadAutoPushStatus(): Promise<TauriResult<AutoPushStatus>> {
  const result = await tauriCallSafe('get_feishu_auto_push_status', {})
  if (result.ok && result.data) {
    autoPushStatus.value = result.data
    autoPushEnabled.value = result.data.enabled
  }
  return result
}

/**
 * 设置自动推送开关
 */
export async function setAutoPushEnabled(enabled: boolean): Promise<TauriResult<string>> {
  const result = await tauriCallSafe('set_feishu_auto_push', { enabled })
  if (result.ok) {
    autoPushEnabled.value = enabled
    await loadAutoPushStatus()
  }
  return result
}

/**
 * 通知数据变更（触发5秒防抖推送）
 * 在任何本地数据变更后调用此函数
 */
export async function notifyDataChange(): Promise<void> {
  // 异步触发，不阻塞调用方
  try {
    const result = await tauriCallSafe('trigger_feishu_push')
    // 失败可见性（审查 P1-13 残留）：推送失败不能完全静默，至少留痕；
    // 这里不弹用户提示——每次数据变更都会走到此处， toast 会刷屏。
    if (!result.ok) console.warn('[Casy] 飞书自动推送通知失败:', result.error || '未知错误')
  } catch (e) {
    console.warn('[Casy] 飞书自动推送通知失败:', e)
  }
}

/**
 * 飞书自动推送 composable
 */
export function useAutoPush() {
  return {
    autoPushEnabled,
    autoPushStatus,
    loadAutoPushStatus,
    setAutoPushEnabled,
    notifyDataChange,
  }
}
