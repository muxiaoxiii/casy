<script setup>
import { ref, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { safeListen } from '../../core/tauriEvents'
import { Warning, Close, ArrowRight } from '@element-plus/icons-vue'

const router = useRouter()

const visible = ref(false)
const banner = ref(null) // { level, message, caseName, type, color }
let unlisten = null

// ============================================================
// 解析提醒消息
// ============================================================
function parseMessage(message) {
  const result = { caseName: '', type: '', dueDate: '', daysLeft: null }
  const lines = message.split('\n')
  for (const line of lines) {
    if (line.startsWith('案件:')) result.caseName = line.replace('案件:', '').trim()
    if (line.startsWith('期限:')) result.type = line.replace('期限:', '').trim()
    if (line.startsWith('任务:')) result.type = line.replace('任务:', '').trim()
    if (line.startsWith('庭审:')) result.type = line.replace('庭审:', '').trim()
    if (line.startsWith('截止日期:')) result.dueDate = line.replace('截止日期:', '').trim()
    if (line.startsWith('剩余:')) {
      const m = line.match(/-?\d+/)
      if (m) result.daysLeft = parseInt(m[0])
    }
  }
  return result
}

function classifyLevel(daysLeft) {
  if (daysLeft === null) return null
  if (daysLeft < 0) return 'R4'
  if (daysLeft === 0) return 'R3'
  if (daysLeft <= 1) return 'R2'
  return null // R1 不触发横幅
}

// ============================================================
// 事件监听
// ============================================================
async function setupListener() {
  try {
    unlisten = await safeListen('reminder:triggered', (event) => {
      const payload = event.payload
      const msg = typeof payload === 'string' ? payload : payload?.message
      if (!msg) return

      const parsed = parseMessage(msg)
      const level = classifyLevel(parsed.daysLeft)

      // 仅 R2/R3 显示横幅，R4 也会显示（逾期更紧急）
      if (level === 'R2' || level === 'R3' || level === 'R4') {
        const colorMap = {
          R2: { bg: '#FDF6EC', border: '#E6A23C', text: '#E6A23C', label: '提醒' },
          R3: { bg: '#FEF0F0', border: '#F56C6C', text: '#F56C6C', label: '强提醒' },
          R4: { bg: '#FDE2E2', border: '#C00000', text: '#C00000', label: '逾期' },
        }
        banner.value = {
          level,
          message: msg,
          ...parsed,
          ...colorMap[level],
        }
        visible.value = true

        // R3/R4 不自动关闭，R2 10 秒后关闭
        if (level === 'R2') {
          setTimeout(() => {
            if (banner.value?.level === 'R2') {
              visible.value = false
            }
          }, 10000)
        }
      }
    })
  } catch (e) {
    console.warn('[Casy] ReminderBanner 事件监听未建立:', e)
  }
}

function close() {
  visible.value = false
}

function goDetail() {
  visible.value = false
  router.push({ name: 'reminder' })
}

onMounted(setupListener)
onUnmounted(() => { if (unlisten) unlisten() })
</script>

<template>
  <Transition name="banner-slide">
    <div
      v-if="visible && banner"
      class="reminder-banner"
      :style="{
        background: banner.bg,
        borderBottom: `2px solid ${banner.border}`,
      }"
    >
      <div class="banner-icon" :style="{ color: banner.text }">
        <el-icon :size="18"><Warning /></el-icon>
      </div>

      <div class="banner-body">
        <el-tag
          :type="banner.level === 'R3' || banner.level === 'R4' ? 'danger' : 'warning'"
          size="small"
          effect="dark"
          round
        >
          {{ banner.label }}
        </el-tag>
        <span class="banner-case">{{ banner.caseName || '未知案件' }}</span>
        <span class="banner-type" v-if="banner.type">— {{ banner.type }}</span>
        <span class="banner-date" v-if="banner.dueDate">
          截止 {{ banner.dueDate }}
          <template v-if="banner.daysLeft !== null">
            （{{ banner.daysLeft < 0 ? `逾期 ${Math.abs(banner.daysLeft)} 天` : banner.daysLeft === 0 ? '今天到期' : `剩余 ${banner.daysLeft} 天` }}）
          </template>
        </span>
      </div>

      <div class="banner-actions">
        <el-button size="small" type="primary" text @click="goDetail">
          查看详情 <el-icon><ArrowRight /></el-icon>
        </el-button>
        <el-button size="small" text @click="close" :icon="Close" />
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.reminder-banner {
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
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
}

.banner-icon {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  animation: pulse-icon 1.5s infinite;
}

@keyframes pulse-icon {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}

.banner-body {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  flex-wrap: wrap;
}

.banner-case {
  font-size: 13px;
  font-weight: 600;
  color: #303133;
}

.banner-type {
  font-size: 13px;
  color: #606266;
}

.banner-date {
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
