<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useAiSettingsStore } from '../../stores/aiSettings'
import { Cpu, ArrowRight } from '../icons'
import { tauriCallSafe } from '../../core/tauriBridge'

const router = useRouter()
const settingsStore = useAiSettingsStore()

// AI 状态：available / disabled / degraded
const aiStatus = computed(() => {
  return settingsStore.config.activeId ? 'available' : 'disabled'
})

const statusLabel = computed(() => {
  switch (aiStatus.value) {
    case 'available': return 'AI 已配置'
    case 'disabled': return 'AI 已关闭'
    case 'degraded': return 'AI 降级'
    default: return 'AI 未知'
  }
})

const dotClass = computed(() => {
  switch (aiStatus.value) {
    case 'available': return 'dot-available'
    case 'disabled': return 'dot-disabled'
    case 'degraded': return 'dot-degraded'
    default: return 'dot-disabled'
  }
})

// 今日调用次数/配额
const todayUsed = ref(null)
const usageError = ref('')
const dailyLimit = ref(0)
const remaining = computed(() => Math.max(0, dailyLimit.value - todayUsed.value))

// Popover 可见性
const popoverVisible = ref(false)

function goToAISettings() {
  popoverVisible.value = false
  router.push({ name: 'settings', query: { tab: 'ai' } })
}

async function loadUsage() {
  usageError.value = ''
  try {
    const result = await tauriCallSafe('get_ai_usage', {})
    if (result.ok && result.data) {
      todayUsed.value = result.data.usedToday
      dailyLimit.value = result.data.dailyLimit
    } else {
      todayUsed.value = null
      usageError.value = result.error || '调用次数暂不可用'
    }
  } catch {
    todayUsed.value = null
    usageError.value = '调用次数暂不可用'
  }
}
onMounted(loadUsage)
watch(popoverVisible, visible => { if (visible) void loadUsage() })
</script>

<template>
  <el-popover
    v-model:visible="popoverVisible"
    placement="bottom-end"
    :width="220"
    trigger="click"
  >
    <template #reference>
      <button type="button" class="ai-badge" title="AI 调用情况" aria-label="AI 调用情况">
        <span class="badge-dot" :class="dotClass"></span>
        <span class="badge-label">调用情况</span>
      </button>
    </template>

    <div class="ai-popover ui-col">
      <div class="popover-header">
        <div class="popover-status ui-row">
          <span class="popover-dot" :class="dotClass"></span>
          <span class="popover-status-text">{{ statusLabel }}</span>
        </div>
        <el-icon
          class="popover-settings-icon"
          :size="14"
          @click="goToAISettings"
          title="AI 设置"
        >
          <Cpu />
        </el-icon>
      </div>

      <div v-if="aiStatus === 'available'" class="popover-quota">
        <div class="quota-row">
          <span class="quota-label" title="已发出的请求次数，包含失败请求">今日请求</span>
          <span class="quota-value">{{ todayUsed ?? '--' }}</span>
        </div>
        <div class="quota-row">
          <span class="quota-label">剩余配额</span>
          <span class="quota-value" :class="{ 'quota-low': todayUsed !== null && dailyLimit > 0 && remaining <= 5 }">{{ todayUsed === null ? '--' : dailyLimit === 0 ? '不限' : remaining }}</span>
        </div>
        <el-progress
          v-if="todayUsed !== null"
          :percentage="dailyLimit === 0 ? 0 : Math.min(100, (todayUsed / dailyLimit) * 100)"
          :stroke-width="4"
          :show-text="false"
          :color="remaining <= 5 ? '#F59E0B' : '#2563EB'"
        />
        <p v-if="usageError" role="status">{{ usageError }}</p>
      </div>

      <div v-if="aiStatus === 'disabled'" class="popover-hint">
        <p>AI 功能已关闭，当前使用规则版替代方案。</p>
        <el-button size="small" type="primary" text @click="goToAISettings">
          前往开启 <el-icon><ArrowRight /></el-icon>
        </el-button>
      </div>

      <div v-if="aiStatus === 'degraded'" class="popover-hint">
        <p>AI 服务暂时不可用，已自动切换到规则版方案。</p>
        <el-button size="small" type="primary" text @click="goToAISettings">
          查看详情 <el-icon><ArrowRight /></el-icon>
        </el-button>
      </div>
    </div>
  </el-popover>
</template>

<style scoped>
.ai-badge {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  border-radius: 16px;
  background: #F9FAFB;
  border: 1px solid #E5E7EB;
  cursor: pointer;
  transition: all var(--motion-fast)  ease;
  user-select: none;
}

.ai-badge:hover {
  background: #F3F4F6;
  border-color: #D1D5DB;
}

.badge-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.dot-available {
  background: #22C55E;
  box-shadow: 0 0 0 2px rgba(34, 197, 94, 0.2);
}

.dot-disabled {
  background: #D1D5DB;
}

.dot-degraded {
  background: #F59E0B;
  box-shadow: 0 0 0 2px rgba(245, 158, 11, 0.2);
  animation: amber-pulse 2s ease-in-out infinite;
}

@keyframes amber-pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.6; }
}

.badge-label {
  font-size: 12px;
  font-weight: 600;
  color: var(--gray-500);
  letter-spacing: 0.5px;
}

/* Popover 内容：布局走共享 .ui-col，此处只保留 12px 间距 */
.ai-popover { gap: 12px; }

.popover-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.popover-status { gap: 6px; }

.popover-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.popover-dot.dot-available { background: #22C55E; }
.popover-dot.dot-disabled { background: #D1D5DB; }
.popover-dot.dot-degraded { background: #F59E0B; }

.popover-status-text {
  font-size: 13px;
  font-weight: 600;
  color: var(--gray-700);
}

.popover-settings-icon {
  color: #9CA3AF;
  cursor: pointer;
  transition: color var(--motion-fast) ease;
}

.popover-settings-icon:hover {
  color: var(--gray-500);
}

/* 配额区域 */
.popover-quota {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding-top: 8px;
  border-top: 1px solid #F3F4F6;
}

.quota-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.quota-label {
  font-size: 12px;
  color: #9CA3AF;
}

.quota-value {
  font-size: 13px;
  font-weight: 600;
  color: var(--gray-700);
}

.quota-value.quota-low {
  color: #F59E0B;
}

/* 提示区域 */
.popover-hint {
  padding-top: 8px;
  border-top: 1px solid #F3F4F6;
}

.popover-hint p {
  font-size: 12px;
  color: var(--gray-500);
  margin-bottom: 8px;
  line-height: 1.5;
}
</style>
