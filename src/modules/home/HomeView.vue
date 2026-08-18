<script setup>
import { computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import {
  Calendar,
  Clock,
  Warning,
  Document,
  List,
  Bell,
  Timer,
  RefreshRight,
  CircleCheck,
} from '@element-plus/icons-vue'
import { useCasesStore } from '../../stores/cases'
import { useTasksStore } from '../../stores/tasks'
import { useCalendarStore } from '../../stores/calendar'

const router = useRouter()
const casesStore = useCasesStore()
const tasksStore = useTasksStore()
const calendarStore = useCalendarStore()

const today = new Date().toISOString().split('T')[0]

onMounted(async () => {
  await Promise.all([
    casesStore.loadDashboard(),
    tasksStore.loadTasks(),
    calendarStore.loadEvents(new Date().getFullYear(), new Date().getMonth() + 1),
  ])
})

// ============================================================
// 硬性日程：今天的 hearing/deadline 类型日历事件
// ============================================================
const todayEvents = computed(() => {
  return calendarStore.events
    .filter(e => e.date === today && (e.type === 'hearing' || e.type === 'deadline'))
    .sort((a, b) => (a.title || '').localeCompare(b.title || ''))
})

// ============================================================
// 下一步行动：最紧急的 5 个未完成 action 任务
// ============================================================
const priorityOrder = {
  urgent_important: 0,
  urgent: 1,
  important: 2,
  high: 3,
  normal: 4,
  low: 5,
}

const nextActionTasks = computed(() => {
  return [...tasksStore.nextActions]
    .sort((a, b) => {
      const pa = priorityOrder[a.priority] ?? 4
      const pb = priorityOrder[b.priority] ?? 4
      if (pa !== pb) return pa - pb
      // 有截止日期的优先
      if (a.dueDate && !b.dueDate) return -1
      if (!a.dueDate && b.dueDate) return 1
      if (a.dueDate && b.dueDate) return a.dueDate.localeCompare(b.dueDate)
      return 0
    })
    .slice(0, 5)
})

// ============================================================
// 等待跟进：等待超过 3 天的 waiting 任务
// ============================================================
const waitingOverdueTasks = computed(() => {
  const threeDaysAgo = new Date()
  threeDaysAgo.setDate(threeDaysAgo.getDate() - 3)
  const cutoff = threeDaysAgo.toISOString().split('T')[0]
  return tasksStore.waitingTasks.filter(t => {
    if (!t.followUpDate) return false
    return t.followUpDate <= cutoff
  })
})

// ============================================================
// 逾期任务
// ============================================================
const overdueTasks = computed(() => {
  return tasksStore.pendingTasks
    .filter(t => t.isOverdue === 1)
    .sort((a, b) => (a.dueDate || '').localeCompare(b.dueDate || ''))
})

// ============================================================
// 今日到期任务数
// ============================================================
const dueTodayCount = computed(() => {
  return tasksStore.pendingTasks.filter(t => t.dueDate === today).length
})

// ============================================================
// 最近活动
// ============================================================
const recentActivities = computed(() => {
  return (casesStore.dashboard.recentActivities || []).slice(0, 5)
})

// ============================================================
// 优先级标签
// ============================================================
function priorityLabel(priority) {
  const map = {
    urgent_important: '紧急重要',
    urgent: '紧急',
    important: '重要',
    high: '高',
    normal: '普通',
    low: '低',
  }
  return map[priority] || '普通'
}

function priorityClass(priority) {
  if (priority === 'urgent_important' || priority === 'urgent') return 'tag-red'
  if (priority === 'important' || priority === 'high') return 'tag-amber'
  return 'tag-default'
}

// ============================================================
// 日程类型标签
// ============================================================
function eventTypeLabel(type) {
  return type === 'hearing' ? '开庭/口审' : '期限'
}

function eventTypeClass(type) {
  return type === 'hearing' ? 'tag-red' : 'tag-amber'
}

// ============================================================
// 活动类型图标
// ============================================================
const activityIcons = {
  log: Document,
  hearing: Calendar,
  task: CircleCheck,
}

// ============================================================
// 计算等待天数
// ============================================================
function getWaitingDays(followUpDate) {
  if (!followUpDate) return 0
  const follow = new Date(followUpDate)
  const now = new Date()
  return Math.max(0, Math.ceil((now.getTime() - follow.getTime()) / (1000 * 60 * 60 * 24)))
}

// ============================================================
// 计算逾期天数
// ============================================================
function getOverdueDays(dueDate) {
  if (!dueDate) return 0
  const due = new Date(dueDate)
  const now = new Date()
  return Math.max(0, Math.ceil((now.getTime() - due.getTime()) / (1000 * 60 * 60 * 24)))
}

// ============================================================
// 勾选完成任务
// ============================================================
async function toggleTask(task) {
  await tasksStore.toggleTask(task.id)
}

// ============================================================
// 导航
// ============================================================
function goToCalendar() {
  router.push({ name: 'calendar' })
}

function goToTasks(perspective) {
  tasksStore.activePerspective = perspective || 'today'
  router.push({ name: 'tasks' })
}

function goToCase(id) {
  router.push({ name: 'case-detail', params: { id } })
}
</script>

<template>
  <div class="dashboard">
    <!-- 今日要点统计卡片 -->
    <div class="summary-bar">
      <div class="summary-card" @click="goToCalendar">
        <div class="summary-icon icon-red">
          <el-icon :size="20"><Calendar /></el-icon>
        </div>
        <div class="summary-body">
          <div class="summary-value">{{ todayEvents.length }}</div>
          <div class="summary-label">硬性日程</div>
        </div>
      </div>

      <div class="summary-card" @click="goToTasks('today')">
        <div class="summary-icon icon-amber">
          <el-icon :size="20"><Clock /></el-icon>
        </div>
        <div class="summary-body">
          <div class="summary-value">{{ dueTodayCount }}</div>
          <div class="summary-label">今日到期</div>
        </div>
      </div>

      <div class="summary-card" @click="goToTasks('waiting')">
        <div class="summary-icon icon-amber">
          <el-icon :size="20"><Timer /></el-icon>
        </div>
        <div class="summary-body">
          <div class="summary-value">{{ waitingOverdueTasks.length }}</div>
          <div class="summary-label">等待超时</div>
        </div>
      </div>

      <div class="summary-card" @click="goToTasks('review')">
        <div class="summary-icon icon-default">
          <el-icon :size="20"><RefreshRight /></el-icon>
        </div>
        <div class="summary-body">
          <div class="summary-value">{{ tasksStore.taskStats.review }}</div>
          <div class="summary-label">需回顾</div>
        </div>
      </div>
    </div>

    <!-- 主内容区：两栏布局 -->
    <div class="main-grid">
      <!-- 左栏：硬性日程 -->
      <div class="panel">
        <div class="panel-header">
          <el-icon><Calendar /></el-icon>
          <span>硬性日程（今日）</span>
        </div>
        <div class="panel-body">
          <template v-if="todayEvents.length">
            <div
              v-for="event in todayEvents"
              :key="event.id"
              class="schedule-item"
            >
              <span class="schedule-time" :class="eventTypeClass(event.type)">
                {{ eventTypeLabel(event.type) }}
              </span>
              <span class="schedule-title">{{ event.title }}</span>
              <el-tag
                v-if="event.caseId"
                size="small"
                :type="event.type === 'hearing' ? 'danger' : 'warning'"
                @click.stop="goToCase(event.caseId)"
                class="schedule-case-tag"
              >
                查看案件
              </el-tag>
            </div>
          </template>
          <div v-else class="empty-state">
            <el-icon :size="32" class="empty-icon"><CircleCheck /></el-icon>
            <span>今天没有硬性日程</span>
          </div>
        </div>
      </div>

      <!-- 右栏：下一步行动 -->
      <div class="panel">
        <div class="panel-header">
          <el-icon><List /></el-icon>
          <span>下一步行动</span>
        </div>
        <div class="panel-body">
          <template v-if="nextActionTasks.length">
            <div
              v-for="task in nextActionTasks"
              :key="task.id"
              class="task-item"
            >
              <el-checkbox
                :model-value="false"
                @change="toggleTask(task)"
                class="task-check"
              />
              <div class="task-content">
                <div class="task-name">{{ task.taskName }}</div>
                <div class="task-meta">
                  <span v-if="task.caseId" class="task-case" @click="goToCase(task.caseId)">
                    {{ task.caseId }}
                  </span>
                  <span v-if="task.dueDate" class="task-due" :class="{ 'text-red': task.dueDate < today }">
                    {{ task.dueDate }}
                  </span>
                </div>
              </div>
              <el-tag
                size="small"
                :class="priorityClass(task.priority)"
              >
                {{ priorityLabel(task.priority) }}
              </el-tag>
            </div>
          </template>
          <div v-else class="empty-state">
            <el-icon :size="32" class="empty-icon"><CircleCheck /></el-icon>
            <span>所有任务已完成</span>
          </div>
        </div>
      </div>

      <!-- 等待跟进（有数据才显示） -->
      <div v-if="waitingOverdueTasks.length" class="panel">
        <div class="panel-header">
          <el-icon><Timer /></el-icon>
          <span>等待跟进</span>
          <el-tag size="small" type="warning" class="header-tag">
            {{ waitingOverdueTasks.length }}
          </el-tag>
        </div>
        <div class="panel-body">
          <div
            v-for="task in waitingOverdueTasks"
            :key="task.id"
            class="task-item"
          >
            <div class="task-content">
              <div class="task-name">{{ task.taskName }}</div>
              <div class="task-meta">
                <span v-if="task.waitingFor" class="task-waiting-for">
                  等 {{ task.waitingFor }}
                </span>
                <span class="task-waiting-days">
                  已等 {{ getWaitingDays(task.followUpDate) }} 天
                </span>
                <span v-if="task.caseId" class="task-case" @click="goToCase(task.caseId)">
                  {{ task.caseId }}
                </span>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- 逾期追踪（有数据才显示） -->
      <div v-if="overdueTasks.length" class="panel">
        <div class="panel-header">
          <el-icon><Warning /></el-icon>
          <span>逾期追踪</span>
          <el-tag size="small" type="danger" class="header-tag">
            {{ overdueTasks.length }}
          </el-tag>
        </div>
        <div class="panel-body">
          <div
            v-for="task in overdueTasks"
            :key="task.id"
            class="task-item"
          >
            <el-checkbox
              :model-value="false"
              @change="toggleTask(task)"
              class="task-check"
            />
            <div class="task-content">
              <div class="task-name">{{ task.taskName }}</div>
              <div class="task-meta">
                <span v-if="task.caseId" class="task-case" @click="goToCase(task.caseId)">
                  {{ task.caseId }}
                </span>
              </div>
            </div>
            <span class="overdue-badge">
              逾期 {{ getOverdueDays(task.dueDate) }} 天
            </span>
          </div>
        </div>
      </div>
    </div>

    <!-- 最近活动 -->
    <div class="panel activity-panel">
      <div class="panel-header">
        <el-icon><Bell /></el-icon>
        <span>最近活动</span>
      </div>
      <div class="panel-body">
        <template v-if="recentActivities.length">
          <div
            v-for="act in recentActivities"
            :key="act.id"
            class="activity-item"
            @click="act.caseId && goToCase(act.caseId)"
          >
            <span class="activity-icon-wrap">
              <el-icon :size="14">
                <component :is="activityIcons[act.eventType] || Document" />
              </el-icon>
            </span>
            <span class="activity-time">{{ act.eventDate }}</span>
            <span class="activity-summary">{{ act.eventSummary }}</span>
          </div>
        </template>
        <div v-else class="empty-state">
          <el-icon :size="32" class="empty-icon"><Document /></el-icon>
          <span>暂无最近活动</span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* ============================================================
   视觉 Token
   ============================================================ */
:root {
  --c-primary: #2563EB;
  --c-bg: #FAFAFA;
  --c-surface: #FFFFFF;
  --c-text: #18181B;
  --c-text-secondary: #52525B;
  --c-text-muted: #A1A1AA;
  --c-red: #EF4444;
  --c-amber: #F59E0B;
  --c-green: #10B981;
  --c-purple: #8B5CF6;
  --radius-card: 8px;
  --radius-btn: 6px;
}

/* ============================================================
   Layout
   ============================================================ */
.dashboard {
  max-width: 1200px;
  margin: 0 auto;
  padding: 20px;
  background: var(--c-bg);
  min-height: 100%;
}

/* ============================================================
   Summary Bar
   ============================================================ */
.summary-bar {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 12px;
  margin-bottom: 20px;
}

.summary-card {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 16px;
  background: var(--c-surface);
  border-radius: var(--radius-card);
  cursor: pointer;
  transition: box-shadow 0.2s;
}

.summary-card:hover {
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.06);
}

.summary-icon {
  width: 40px;
  height: 40px;
  border-radius: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.icon-red {
  background: #FEF2F2;
  color: var(--c-red);
}

.icon-amber {
  background: #FFFBEB;
  color: var(--c-amber);
}

.icon-default {
  background: #F4F4F5;
  color: var(--c-text-muted);
}

.summary-value {
  font-size: 24px;
  font-weight: 700;
  color: var(--c-text);
  line-height: 1;
}

.summary-label {
  font-size: 12px;
  color: var(--c-text-muted);
  margin-top: 2px;
}

/* ============================================================
   Main Grid (2 columns)
   ============================================================ */
.main-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
  margin-bottom: 16px;
}

/* ============================================================
   Panel
   ============================================================ */
.panel {
  background: var(--c-surface);
  border-radius: var(--radius-card);
  overflow: hidden;
}

.panel-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 14px 16px;
  font-size: 14px;
  font-weight: 600;
  color: var(--c-text);
  border-bottom: 1px solid #F4F4F5;
}

.panel-header .el-icon {
  color: var(--c-text-muted);
}

.header-tag {
  margin-left: auto;
}

.panel-body {
  padding: 8px 0;
  max-height: 320px;
  overflow-y: auto;
}

/* ============================================================
   Schedule Items
   ============================================================ */
.schedule-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 16px;
  transition: background 0.15s;
}

.schedule-item:hover {
  background: #FAFAFA;
}

.schedule-time {
  font-size: 12px;
  padding: 2px 8px;
  border-radius: 4px;
  font-weight: 500;
  flex-shrink: 0;
}

.tag-red {
  background: #FEF2F2;
  color: var(--c-red);
}

.tag-amber {
  background: #FFFBEB;
  color: var(--c-amber);
}

.tag-default {
  background: #F4F4F5;
  color: var(--c-text-muted);
}

.schedule-title {
  flex: 1;
  font-size: 13px;
  color: var(--c-text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.schedule-case-tag {
  cursor: pointer;
  flex-shrink: 0;
}

/* ============================================================
   Task Items
   ============================================================ */
.task-item {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 10px 16px;
  transition: background 0.15s;
}

.task-item:hover {
  background: #FAFAFA;
}

.task-check {
  flex-shrink: 0;
  margin-top: 2px;
}

.task-content {
  flex: 1;
  min-width: 0;
}

.task-name {
  font-size: 13px;
  color: var(--c-text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.task-meta {
  display: flex;
  gap: 8px;
  margin-top: 3px;
  font-size: 12px;
  color: var(--c-text-muted);
  flex-wrap: wrap;
}

.task-case {
  cursor: pointer;
  color: var(--c-primary);
}

.task-case:hover {
  text-decoration: underline;
}

.task-due {
  color: var(--c-text-muted);
}

.task-due.text-red {
  color: var(--c-red);
}

.task-waiting-for {
  color: var(--c-text-secondary);
}

.task-waiting-days {
  color: var(--c-amber);
}

/* ============================================================
   Overdue Badge
   ============================================================ */
.overdue-badge {
  font-size: 12px;
  padding: 2px 8px;
  border-radius: 4px;
  background: #FEF2F2;
  color: var(--c-red);
  font-weight: 500;
  flex-shrink: 0;
  white-space: nowrap;
}

/* ============================================================
   Activity Panel
   ============================================================ */
.activity-panel {
  margin-bottom: 0;
}

.activity-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 16px;
  cursor: pointer;
  transition: background 0.15s;
}

.activity-item:hover {
  background: #FAFAFA;
}

.activity-icon-wrap {
  width: 24px;
  height: 24px;
  border-radius: 50%;
  background: #F4F4F5;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--c-text-muted);
  flex-shrink: 0;
}

.activity-time {
  font-size: 12px;
  color: var(--c-text-muted);
  flex-shrink: 0;
  min-width: 48px;
}

.activity-summary {
  font-size: 13px;
  color: var(--c-text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* ============================================================
   Empty State
   ============================================================ */
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 32px 16px;
  color: var(--c-text-muted);
  font-size: 13px;
  gap: 8px;
}

.empty-icon {
  color: var(--c-green);
}
</style>
