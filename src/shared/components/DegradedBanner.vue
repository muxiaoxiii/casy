<script setup>
import { ref, computed, watch } from 'vue'
import { Warning, Close, RefreshRight } from '@element-plus/icons-vue'

const props = defineProps({
  /** 降级原因 */
  reason: { type: String, default: '' },
  /** 替代方案描述 */
  alternative: { type: String, default: '' },
  /** 功能名称（用于 localStorage key） */
  feature: { type: String, default: 'global' },
  /** 是否可重试 */
  retryable: { type: Boolean, default: true },
})

const emit = defineEmits(['retry', 'close'])

const storageKey = computed(() => `casy_degraded_dismissed_${props.feature}`)

const visible = ref(true)

// 初始化时检查是否已关闭
function checkDismissed() {
  const dismissed = localStorage.getItem(storageKey.value)
  if (dismissed) {
    const dismissedTime = parseInt(dismissed, 10)
    // 24 小时内不再显示
    if (Date.now() - dismissedTime < 24 * 60 * 60 * 1000) {
      visible.value = false
    } else {
      localStorage.removeItem(storageKey.value)
    }
  }
}

checkDismissed()

function handleClose() {
  visible.value = false
  localStorage.setItem(storageKey.value, String(Date.now()))
  emit('close')
}

function handleRetry() {
  emit('retry')
}
</script>

<template>
  <Transition name="banner-slide">
    <div v-if="visible" class="degraded-banner">
      <div class="banner-icon">
        <el-icon :size="16"><Warning /></el-icon>
      </div>

      <div class="banner-body">
        <span class="banner-title">AI 功能降级</span>
        <span v-if="reason" class="banner-reason">{{ reason }}</span>
        <span v-if="alternative" class="banner-alt">
          当前使用：{{ alternative }}
        </span>
      </div>

      <div class="banner-actions">
        <el-button
          v-if="retryable"
          size="small"
          type="warning"
          text
          @click="handleRetry"
        >
          <el-icon class="retry-icon"><RefreshRight /></el-icon>
          重试
        </el-button>
        <el-button
          size="small"
          text
          @click="handleClose"
          :icon="Close"
        />
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.degraded-banner {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 10px 16px;
  background: #FEF3C7;
  border-left: 3px solid #F59E0B;
  border-radius: 0 6px 6px 0;
  margin-bottom: 12px;
}

.banner-icon {
  flex-shrink: 0;
  color: #D97706;
  margin-top: 1px;
}

.banner-body {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.banner-title {
  font-size: 13px;
  font-weight: 600;
  color: #92400E;
}

.banner-reason {
  font-size: 12px;
  color: #A16207;
  line-height: 1.4;
}

.banner-alt {
  font-size: 12px;
  color: #A16207;
  line-height: 1.4;
}

.banner-actions {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
}

.retry-icon {
  margin-right: 2px;
}

/* 过渡动画 */
.banner-slide-enter-active {
  transition: all 0.25s ease-out;
}

.banner-slide-leave-active {
  transition: all 0.2s ease-in;
}

.banner-slide-enter-from {
  opacity: 0;
  transform: translateY(-8px);
}

.banner-slide-leave-to {
  opacity: 0;
  transform: translateY(-8px);
}
</style>
