<script setup lang="ts">
import { computed } from 'vue'
import { Warning, Loading, Refresh, Check, InfoFilled, Connection } from '../icons'

export type DataState = 'loading' | 'empty' | 'degraded' | 'error' | 'verified'

const props = defineProps<{
  state: DataState
  emptyText?: string
  errorText?: string
  lastVerifiedAt?: string
  degradedText?: string
}>()

const emit = defineEmits<{
  (e: 'retry'): void
}>()

const showSlot = computed(() => {
  return props.state === 'verified' || props.state === 'degraded'
})
</script>

<template>
  <div class="state-feedback-wrapper">
    <!-- State 1: Loading -->
    <div v-if="state === 'loading'" class="state-panel loading-state" role="status" aria-live="polite">
      <el-icon class="spin-icon" :size="32"><Loading /></el-icon>
      <p class="state-text">正在加载数据...</p>
    </div>

    <!-- State 2: Empty -->
    <div v-else-if="state === 'empty'" class="state-panel empty-state">
      <el-icon :size="48"><InfoFilled /></el-icon>
      <p class="state-text">{{ emptyText || '当前暂无待办事项' }}</p>
    </div>

    <!-- State 4: Error -->
    <div v-else-if="state === 'error'" class="state-panel error-state" role="alert">
      <el-icon class="error-icon" :size="48"><Warning /></el-icon>
      <p class="state-text">{{ errorText || '数据加载失败' }}</p>
      <button class="btn-retry" @click="emit('retry')">
        <el-icon><Refresh /></el-icon> 重试
      </button>
    </div>

    <!-- State 3 / State 5 (Render Data Slot) -->
    <template v-if="showSlot">
      <!-- 降级/离线提示栏 -->
      <div v-if="state === 'degraded'" class="status-banner degraded-banner">
        <el-icon><Connection /></el-icon>
        <span class="banner-text">{{ degradedText || '网络离线，正显示本地缓存。' }}</span>
        <span v-if="lastVerifiedAt" class="banner-time">上次同步: {{ lastVerifiedAt }}</span>
      </div>

      <!-- Verified Clean 提示栏 (可选) -->
      <div v-if="state === 'verified' && lastVerifiedAt" class="status-banner verified-banner">
        <el-icon><Check /></el-icon>
        <span class="banner-text">数据已核验。</span>
        <span class="banner-time">核验时间: {{ lastVerifiedAt }}</span>
      </div>

      <div class="data-container">
        <slot></slot>
      </div>
    </template>
  </div>
</template>

<style scoped>
.state-feedback-wrapper {
  display: flex;
  flex-direction: column;
  height: 100%;
  width: 100%;
}

.state-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 40px 20px;
  color: var(--c-text-secondary);
  text-align: center;
  background: var(--c-bg-card);
  border-radius: 8px;
  border: 1px dashed var(--c-border);
}

.spin-icon {
  animation: spin 2s linear infinite;
  color: var(--c-primary);
  margin-bottom: 12px;
}

@keyframes spin {
  0% { transform: rotate(0deg); }
  100% { transform: rotate(360deg); }
}

.empty-state {
  color: var(--c-text-tertiary);
}

.error-state .error-icon {
  color: var(--c-danger);
  margin-bottom: 16px;
}

.state-text {
  font-size: 14px;
  margin: 12px 0 0 0;
}

.btn-retry {
  margin-top: 16px;
  background: var(--c-bg-page);
  border: 1px solid var(--c-border);
  border-radius: 4px;
  padding: 6px 16px;
  display: flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
  color: var(--c-text-primary);
  transition: all 0.2s;
}

.btn-retry:hover {
  background: var(--c-primary-light);
  color: var(--c-primary);
  border-color: var(--c-primary);
}

.status-banner {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 12px;
  font-size: 12px;
  border-radius: 6px;
  margin-bottom: 12px;
}

.degraded-banner {
  background-color: var(--c-warning-light);
  color: var(--c-warning-dark);
  border: 1px solid var(--c-warning);
}

.verified-banner {
  background-color: var(--c-success-light);
  color: var(--c-success-dark);
  border: 1px solid var(--c-success);
}

.banner-time {
  margin-left: auto;
  opacity: 0.8;
  font-family: var(--font-mono);
}

.data-container {
  flex: 1;
  display: flex;
  flex-direction: column;
}
</style>
