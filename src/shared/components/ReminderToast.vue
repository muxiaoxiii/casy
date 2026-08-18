<script setup>
import { ref, onMounted, onUnmounted } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { ElMessage } from 'element-plus'
import { Bell, Check, Clock } from '@element-plus/icons-vue'

const visible = ref(false)
const queue = ref([]) // 提醒队列（后端可能一次触发多条）
const current = ref(null)
let unlisten = null

function showNext() {
  if (queue.value.length === 0) {
    visible.value = false
    current.value = null
    return
  }
  current.value = queue.value.shift()
  visible.value = true
  // 自动关闭（8 秒后如有下一条继续展示）
  setTimeout(() => {
    if (queue.value.length > 0) {
      showNext()
    } else {
      visible.value = false
      current.value = null
    }
  }, 8000)
}

async function setupListener() {
  try {
    unlisten = await listen('reminder:triggered', (event) => {
      const payload = event.payload
      const msg = typeof payload === 'string' ? payload : payload?.message
      if (!msg) return
      queue.value.push({ message: msg, at: payload?.at || '' })
      if (!visible.value) showNext()
      // 同时给一条系统级提示（不阻塞）
      ElMessage({
        message: '🔔 ' + msg.split('\n')[0],
        duration: 4000,
      })
    })
  } catch (e) {
    console.warn('[Casy] 提醒事件监听未建立:', e)
  }
}

function snooze() {
  visible.value = false
  ElMessage.info('已稍后提醒（本地）')
}
function dismiss() {
  visible.value = false
  current.value = null
}

onMounted(setupListener)
onUnmounted(() => { if (unlisten) unlisten() })
</script>

<template>
  <!-- 提醒触发面板：右上角浮层 -->
  <Transition name="reminder-pop">
    <div v-if="visible && current" class="reminder-toast">
      <div class="rt-head">
        <el-icon class="rt-icon"><Bell /></el-icon>
        <span class="rt-title">Casy 期限提醒</span>
        <span class="rt-time">{{ current.at || '' }}</span>
      </div>
      <div class="rt-body">{{ current.message }}</div>
      <div class="rt-actions">
        <el-button size="small" @click="snooze">
          <el-icon><Clock /></el-icon> 稍后提醒
        </el-button>
        <el-button size="small" type="primary" @click="dismiss">
          <el-icon><Check /></el-icon> 知道了
        </el-button>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.reminder-toast {
  position: fixed;
  top: 56px;
  right: 20px;
  width: 340px;
  background: #fff;
  border: 1px solid #dcdfe6;
  border-left: 3px solid #e6a23c;
  border-radius: 8px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.12);
  z-index: 3000;
  padding: 14px 16px;
}
.rt-head { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; }
.rt-icon { color: #e6a23c; font-size: 16px; }
.rt-title { font-weight: 600; font-size: 13px; flex: 1; }
.rt-time { font-size: 11px; color: #909399; }
.rt-body {
  font-size: 13px;
  color: #303133;
  line-height: 1.7;
  white-space: pre-line;
  background: #fafafa;
  border-radius: 6px;
  padding: 10px;
  margin-bottom: 10px;
}
.rt-actions { display: flex; justify-content: flex-end; gap: 8px; }
.reminder-pop-enter-active { transition: all 0.18s ease; }
.reminder-pop-leave-active { transition: all 0.15s ease; }
.reminder-pop-enter-from { opacity: 0; transform: translateX(20px); }
.reminder-pop-leave-to { opacity: 0; transform: translateX(20px); }
</style>
