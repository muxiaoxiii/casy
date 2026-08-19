<script setup>
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useSettingsStore } from '../../stores/settings'
import { Cpu, ArrowRight } from '@element-plus/icons-vue'

const router = useRouter()
const settingsStore = useSettingsStore()

// AI 状态：available / disabled / degraded
const aiStatus = computed(() => {
  const mode = settingsStore.ai_mode
  if (mode === 'none') return 'disabled'
  if (mode === 'local' || mode === 'remote') return 'available'
  return 'disabled'
})

const statusLabel = computed(() => {
  switch (aiStatus.value) {
    case 'available': return 'AI 可用'
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
const todayUsed = ref(0)
const dailyLimit = computed(() => settingsStore.ai_daily_limit || 50)
const remaining = computed(() => Math.max(0, dailyLimit.value - todayUsed.value))

// Popover 可见性
const popoverVisible = ref(false)

function goToAISettings() {
  popoverVisible.value = false
  router.push({ name: 'settings', query: { tab: 'ai' } })
}

function handleClick() {
  popoverVisible.value = !popoverVisible.value
}

onMounted(async () => {
  // 获取今日 AI 调用次数（如果后端支持）
  try {
    const { tauriCallSafe } = await import('../../core/tauriBridge')
    const result = await tauriCallSafe('get_ai_usage_today', {})
    if (result.ok && result.data) {
      todayUsed.value = result.data.used || 0
    }
  } catch {
    // 后端未实现时静默失败
  }
})
</script>

<template>
  <el-popover
    v-model:visible="popoverVisible"
    placement="bottom-end"
    :width="220"
    trigger="click"
  >
    <template #reference>
      <div class="ai-badge" :title="statusLabel" @click="handleClick">
        <span class="badge-dot" :class="dotClass"></span>
        <span class="badge-label">AI</span>
      </div>
    </template>

    <div class="ai-popover">
      <div class="popover-header">
        <div class="popover-status">
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
          <span class="quota-label">今日已用</span>
          <span class="quota-value">{{ todayUsed }}</span>
        </div>
        <div class="quota-row">
          <span class="quota-label">剩余配额</span>
          <span class="quota-value" :class="{ 'quota-low': remaining <= 5 }">{{ remaining }}</span>
        </div>
        <el-progress
          :percentage="Math.min(100, (todayUsed / dailyLimit) * 100)"
          :stroke-width="4"
          :show-text="false"
          :color="remaining <= 5 ? '#F59E0B' : '#2563EB'"
        />
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
  transition: all 0.15s ease;
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
  color: #6B7280;
  letter-spacing: 0.5px;
}

/* Popover 内容 */
.ai-popover {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.popover-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.popover-status {
  display: flex;
  align-items: center;
  gap: 6px;
}

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
  color: #374151;
}

.popover-settings-icon {
  color: #9CA3AF;
  cursor: pointer;
  transition: color 0.15s ease;
}

.popover-settings-icon:hover {
  color: #6B7280;
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
  color: #374151;
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
  color: #6B7280;
  margin-bottom: 8px;
  line-height: 1.5;
}
</style>
