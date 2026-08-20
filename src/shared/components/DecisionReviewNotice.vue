<script setup>
import { ref, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { safeListen } from '../../core/tauriEvents'
import { ElMessage } from 'element-plus'
import { Warning, Close, ArrowRight } from '@element-plus/icons-vue'

const router = useRouter()

const visible = ref(false)
const pendingCount = ref(0)
const receivedAt = ref('')
let unlisten = null

// ============================================================
// 事件监听：后端每日 08:30 检查待复核决策后 emit 'decision:review-due'
// payload: { count, decisions, at }
// ============================================================
async function setupListener() {
  try {
    unlisten = await safeListen('decision:review-due', (event) => {
      const payload = event.payload
      const count = typeof payload === 'object' && payload !== null ? payload.count : 0
      if (!count) return

      pendingCount.value = count
      receivedAt.value = payload?.at || ''
      visible.value = true

      ElMessage({
        message: `有 ${count} 条决策到期待复核`,
        type: 'warning',
        duration: 4000,
      })
    })
  } catch (e) {
    console.warn('[Casy] DecisionReviewNotice 事件监听未建立:', e)
  }
}

function close() {
  visible.value = false
}

function goReview() {
  visible.value = false
  router.push({ name: 'ai', query: { tab: 'decisions' } })
}

onMounted(setupListener)
onUnmounted(() => { if (unlisten) unlisten() })
</script>

<template>
  <Transition name="banner-slide">
    <div v-if="visible" class="review-banner">
      <div class="banner-icon">
        <el-icon :size="18"><Warning /></el-icon>
      </div>

      <div class="banner-body">
        <el-tag type="warning" size="small" effect="dark" round>决策复核</el-tag>
        <span class="banner-text">{{ pendingCount }} 条决策已到复核期，请确认是否仍然有效</span>
        <span class="banner-time" v-if="receivedAt">{{ receivedAt }}</span>
      </div>

      <div class="banner-actions">
        <el-button size="small" type="primary" text @click="goReview">
          前往复核 <el-icon><ArrowRight /></el-icon>
        </el-button>
        <el-button size="small" text @click="close" :icon="Close" />
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.review-banner {
  position: fixed;
  top: 0;
  /* 侧栏折叠态宽 48px（App.vue）；不用 --sidebar-width（theme.css 中为 216px，会错位） */
  left: 48px;
  right: 0;
  z-index: 2000;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 20px;
  background: #FDF6EC;
  border-bottom: 2px solid #B0823A;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
}

.banner-icon {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  color: #B0823A;
}

.banner-body {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  flex-wrap: wrap;
}

.banner-text {
  font-size: 13px;
  font-weight: 600;
  color: #303133;
}

.banner-time {
  font-size: 12px;
  color: #909399;
}

.banner-actions {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
}

/* 过渡动画 */
.banner-slide-enter-active {
  transition: all 0.25s ease-out;
}

.banner-slide-leave-active {
  transition: all 0.2s ease-in;
}

.banner-slide-enter-from {
  transform: translateY(-100%);
  opacity: 0;
}

.banner-slide-leave-to {
  transform: translateY(-100%);
  opacity: 0;
}
</style>
