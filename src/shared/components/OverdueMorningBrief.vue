<script setup>
import { ref, onMounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { tauriCallSafe } from '../../core/tauriBridge'
import { todayLocalISO } from '../utils/date'
import { Warning, Timer, Bell, Calendar, ArrowRight, Check } from '@element-plus/icons-vue'

const router = useRouter()
const route = useRoute()
const visible = ref(false)
const loading = ref(false)
const briefError = ref(false)

// 早报数据
const brief = ref({
  overdueDeadlines: 0,   // 逾期期限数量
  overdueTasks: 0,       // 逾期任务数量
  dueTodayTasks: 0,      // 今日到期任务
  todayHearings: 0,      // 今日开庭/口审
})

// ============================================================
// 加载早报数据
// ============================================================
async function loadBrief() {
  loading.value = true
  briefError.value = false
  try {
    // 获取提醒日志并统计
    const res = await tauriCallSafe('get_reminder_log', { limit: 200 })
    if (res.ok && res.data) {
      let overdueDeadlines = 0
      let overdueTasks = 0
      let dueTodayTasks = 0
      let todayHearings = 0

      for (const entry of res.data) {
        const msg = entry.message || ''
        const daysMatch = msg.match(/剩余:\s*(-?\d+)/)
        const days = daysMatch ? parseInt(daysMatch[1]) : null

        if (msg.includes('期限:') || msg.includes('deadline')) {
          if (days !== null && days < 0) overdueDeadlines++
          if (days === 0) dueTodayTasks++
        }
        if (msg.includes('任务:') || msg.includes('task')) {
          if (days !== null && days < 0) overdueTasks++
          if (days === 0) dueTodayTasks++
        }
        if (msg.includes('庭审:') || msg.includes('hearing')) {
          if (days !== null && days <= 0) todayHearings++
        }
      }

      brief.value = { overdueDeadlines, overdueTasks, dueTodayTasks, todayHearings }
    } else {
      // 不伪造业务数字：命令失败即进入明确错误态，绝不向律师展示虚构的逾期数量。
      briefError.value = true
      brief.value = { overdueDeadlines: 0, overdueTasks: 0, dueTodayTasks: 0, todayHearings: 0 }
    }
  } catch (e) {
    console.warn('[Casy] 早报数据加载失败:', e)
    briefError.value = true
    brief.value = { overdueDeadlines: 0, overdueTasks: 0, dueTodayTasks: 0, todayHearings: 0 }
  }
  loading.value = false
}

// ============================================================
// 是否今日首次打开（localStorage 记录）
// ============================================================
function shouldShow() {
  const today = todayLocalISO()
  const lastShown = localStorage.getItem('casy_morning_brief_date')
  return lastShown !== today
}

function markShown() {
  const today = todayLocalISO()
  localStorage.setItem('casy_morning_brief_date', today)
}

// ============================================================
// 操作
// ============================================================
function dismiss() {
  visible.value = false
  markShown()
}

function goReminder() {
  visible.value = false
  markShown()
  router.push({ name: 'reminder' })
}

watch(() => route.path, (path) => {
  if (path !== '/') visible.value = false
})

// ============================================================
// 初始化
// ============================================================
onMounted(async () => {
  // 早报只属于 Today 工作台，不能在设置、收件箱等业务页上形成全局阻断层。
  if (route.path !== '/') return
  if (shouldShow()) {
    await loadBrief()
    // 加载失败：打开 dialog 展示明确错误态（绝不用虚构数字），供用户点"查看详情"去提醒中心；
    // 不标记"已展示"，明天/下次重试。
    if (briefError.value) {
      visible.value = true
      return
    }
    // 有逾期或今日到期才显示
    const { overdueDeadlines, overdueTasks, dueTodayTasks, todayHearings } = brief.value
    if (overdueDeadlines > 0 || overdueTasks > 0 || dueTodayTasks > 0 || todayHearings > 0) {
      visible.value = true
    } else {
      markShown()
    }
  }
})
</script>

<template>
  <el-dialog
    v-model="visible"
    title=""
    width="420px"
    :close-on-click-modal="false"
    :show-close="false"
    class="morning-brief-dialog"
  >
    <!-- 自定义头部 -->
    <template #header>
      <div class="brief-header">
        <div class="brief-icon">
          <el-icon :size="24" color="var(--c-warning)"><Bell /></el-icon>
        </div>
        <div>
          <h3 class="brief-title">早安，今日概览</h3>
          <p class="brief-subtitle">{{ new Date().toLocaleDateString('zh-CN', { month: 'long', day: 'numeric', weekday: 'long' }) }}</p>
        </div>
      </div>
    </template>

    <!-- 统计卡片（加载失败时展示明确错误态，不伪造数字） -->
    <div v-if="briefError" class="brief-error">
      <el-icon color="var(--c-danger)"><Warning /></el-icon>
      <span>早报数据加载失败，请到提醒中心查看</span>
    </div>
    <div v-else class="brief-stats" v-loading="loading">
      <div class="brief-stat" v-if="brief.overdueDeadlines > 0">
        <div class="stat-icon overdue">
          <el-icon :size="20"><Warning /></el-icon>
        </div>
        <div class="stat-info">
          <div class="stat-num">{{ brief.overdueDeadlines }}</div>
          <div class="stat-text">逾期期限</div>
        </div>
      </div>

      <div class="brief-stat" v-if="brief.overdueTasks > 0">
        <div class="stat-icon overdue">
          <el-icon :size="20"><Timer /></el-icon>
        </div>
        <div class="stat-info">
          <div class="stat-num">{{ brief.overdueTasks }}</div>
          <div class="stat-text">逾期任务</div>
        </div>
      </div>

      <div class="brief-stat" v-if="brief.dueTodayTasks > 0">
        <div class="stat-icon today">
          <el-icon :size="20"><Timer /></el-icon>
        </div>
        <div class="stat-info">
          <div class="stat-num">{{ brief.dueTodayTasks }}</div>
          <div class="stat-text">今日到期</div>
        </div>
      </div>

      <div class="brief-stat" v-if="brief.todayHearings > 0">
        <div class="stat-icon hearing">
          <el-icon :size="20"><Calendar /></el-icon>
        </div>
        <div class="stat-info">
          <div class="stat-num">{{ brief.todayHearings }}</div>
          <div class="stat-text">今日开庭</div>
        </div>
      </div>
    </div>

    <!-- 提示语 -->
    <div class="brief-tip" v-if="brief.overdueDeadlines > 0 || brief.overdueTasks > 0">
      <el-icon color="var(--c-danger)"><Warning /></el-icon>
      <span>有 {{ brief.overdueDeadlines + brief.overdueTasks }} 项已逾期，请尽快处理</span>
    </div>

    <!-- 底部按钮 -->
    <template #footer>
      <div class="brief-footer">
        <el-button @click="dismiss">
          <el-icon><Check /></el-icon> 知道了
        </el-button>
        <el-button type="primary" @click="goReminder">
          查看详情 <el-icon><ArrowRight /></el-icon>
        </el-button>
      </div>
    </template>
  </el-dialog>
</template>

<style scoped>
.brief-header {
  display: flex;
  align-items: center;
  gap: 12px;
}

.brief-icon {
  width: 44px;
  height: 44px;
  border-radius: 12px;
  background: var(--bg-warning-weak);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.brief-title {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  color: var(--c-text);
}

.brief-subtitle {
  margin: 2px 0 0;
  font-size: 12px;
  color: var(--gray-400);
}

/* 统计区域 */
.brief-stats {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 12px;
  margin: 16px 0;
  min-height: 80px;
}

.brief-stat {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 14px;
  border-radius: 8px;
  background: var(--c-bg-subtle);
  border: 1px solid var(--c-border);
}

.stat-icon {
  width: 40px;
  height: 40px;
  border-radius: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.stat-icon.overdue {
  background: var(--bg-risk-weak);
  color: var(--c-danger);
}

.stat-icon.today {
  background: var(--bg-warning-weak);
  color: var(--c-warning);
}

.stat-icon.hearing {
  background: var(--c-primary-light);
  color: var(--c-primary);
}

.stat-num {
  font-size: 22px;
  font-weight: 700;
  color: var(--c-text);
  line-height: 1;
}

.stat-text {
  font-size: 12px;
  color: var(--gray-400);
  margin-top: 2px;
}

/* 加载失败态 */
.brief-error {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  min-height: 96px;
  padding: 16px;
  border-radius: 8px;
  background: var(--bg-risk-weak);
  border: 1px solid var(--c-danger-light);
  font-size: 13px;
  color: var(--c-danger);
}

/* 提示语 */
.brief-tip {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 10px 14px;
  border-radius: 6px;
  background: var(--bg-risk-weak);
  font-size: 13px;
  color: var(--c-danger);
  margin-bottom: 8px;
}

/* 底部 */
.brief-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}
</style>
