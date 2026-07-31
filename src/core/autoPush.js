/**
 * 飞书自动推送 composable
 *
 * 提供数据变更通知功能，在本地数据变更后自动触发飞书推送（5秒防抖）
 */

import { ref } from 'vue'
import { tauriCallSafe } from './tauriBridge.js'

// 自动推送状态
const autoPushEnabled = ref(false)
const autoPushStatus = ref({
  enabled: false,
  pending: false,
  hasTimer: false,
  configured: false,
})

/**
 * 加载自动推送状态
 */
export async function loadAutoPushStatus() {
  const result = await tauriCallSafe('get_feishu_auto_push_status')
  if (result.ok) {
    autoPushStatus.value = result.data
    autoPushEnabled.value = result.data.enabled
  }
  return result
}

/**
 * 设置自动推送开关
 */
export async function setAutoPushEnabled(enabled) {
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
export async function notifyDataChange() {
  // 异步触发，不阻塞调用方
  try {
    await tauriCallSafe('trigger_feishu_push')
  } catch (e) {
    // 静默失败，不影响主流程
    console.debug('飞书自动推送通知失败:', e)
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
