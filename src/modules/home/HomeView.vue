<script setup>
import { ref, computed, onMounted } from 'vue'
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
  Star,
} from '@element-plus/icons-vue'
import { casyContext } from '../../core/plugin/context'
import { useCasesStore } from '../../stores/cases'
import { useTasksStore } from '../../stores/tasks'
import { useCalendarStore } from '../../stores/calendar'
import { useProfileStore } from '../../stores/profile'

const router = useRouter()
const casesStore = useCasesStore()
const tasksStore = useTasksStore()
const calendarStore = useCalendarStore()
const profileStore = useProfileStore()

// 早报问候语：有画像姓名时带称呼（如「早安，王律师 · 每日早报」）
const greeting = computed(() => {
  const name = profileStore.name?.trim()
  return name ? `早安，${name}律师 · 每日早报` : '每日早报'
})

const today = new Date().toISOString().split('T')[0]

// 分级预警（设计哲学 §11.2）
const deadlineWarnings = ref([])

async function loadDeadlineWarnings() {
  const result = await casyContext.calendar.deadlineWarningsWithLevels()
  if (result.ok && result.data) {
    deadlineWarnings.value = result.data
  }
}

onMounted(async () => {
  profileStore.load()
  await Promise.all([
    casesStore.loadDashboard(),
    tasksStore.loadTasks(),
    calendarStore.loadEvents(new Date().getFullYear(), new Date().getMonth() + 1),
    loadDeadlineWarnings(),
    loadBrief(),
    loadHomeRecommendations(),
  ])
})

// ============================================================
// 每日早报（后端规则版 Markdown，失败回退本地拼接）
// ============================================================
const brief = ref(null)
const briefDegraded = ref(false)
const briefLoading = ref(false)

/** get_today_brief 返回 smart_summaries 行；generate_daily_brief_cmd 返回 DailyBrief（markdown 字段） */
function extractBriefContent(data) {
  return data?.content || data?.markdown || ''
}

const briefContent = computed(() => extractBriefContent(brief.value))
const briefTime = computed(() => brief.value?.createdAt || brief.value?.date || '')

async function loadBrief() {
  briefLoading.value = true
  const result = await casyContext.calendar.todayBrief()
  briefLoading.value = false
  if (result.ok && result.data && extractBriefContent(result.data)) {
    brief.value = result.data
    briefDegraded.value = false
  } else {
    brief.value = null
    briefDegraded.value = true
  }
}

async function regenerateBrief() {
  briefLoading.value = true
  const result = await casyContext.calendar.generateDailyBrief()
  briefLoading.value = false
  if (result.ok && result.data && extractBriefContent(result.data)) {
    brief.value = result.data
    briefDegraded.value = false
  } else {
    briefDegraded.value = true
  }
}

/** 极简 Markdown 渲染：标题/列表/加粗（不引入新依赖） */
function renderMarkdown(md) {
  if (!md) return ''
  const esc = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
  const inline = (s) => esc(s).replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
  let html = ''
  let inList = false
  for (const line of md.split('\n')) {
    const t = line.trim()
    if (t.startsWith('- ') || t.startsWith('* ')) {
      if (!inList) { html += '<ul>'; inList = true }
      html += `<li>${inline(t.slice(2))}</li>`
      continue
    }
    if (inList) { html += '</ul>'; inList = false }
    if (!t) continue
    if (t.startsWith('### ')) html += `<h5>${inline(t.slice(4))}</h5>`
    else if (t.startsWith('## ')) html += `<h4>${inline(t.slice(3))}</h4>`
    else if (t.startsWith('# ')) html += `<h3>${inline(t.slice(2))}</h3>`
    else html += `<p>${inline(t)}</p>`
  }
  if (inList) html += '</ul>'
  return html
}

// ============================================================
// 智能推荐：优先消费 get_today_recommendations，失败回退本地排序
// ============================================================
const homeRecommendations = ref([])

async function loadHomeRecommendations() {
  const result = await casyContext.calendar.todayRecommendations()
  if (result.ok && result.data?.recommendations?.length) {
    homeRecommendations.value = result.data.recommendations.slice(0, 3)
  }
}

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
    <!-- 每日早报 AI 横幅（设计哲学 §11.3，后端规则版 Markdown） -->
    <div class="ai-banner">
      <span class="ai-dot" :style="{ background: briefDegraded ? '#B0823A' : '#4C8067' }"></span>
      <div class="ai-banner-content">
        <strong>{{ greeting }}</strong>
        <template v-if="briefDegraded">
          <span class="ai-banner-summary">
            昨日完成 {{ tasksStore.taskStats?.completed || 0 }} 项 ·
            今日 {{ todayEvents.length }} 场硬性日程 ·
            {{ waitingOverdueTasks.length }} 条等待超 3 天 ·
            {{ overdueTasks.length }} 项逾期
          </span>
          <span class="ai-banner-degraded">AI/报表不可用，已显示本地汇总</span>
        </template>
        <span v-else-if="brief?.title" class="ai-banner-summary">{{ brief.title }}</span>
        <span v-else class="ai-banner-summary">早报生成中…</span>
      </div>
      <el-button
        size="small"
        text
        :loading="briefLoading"
        @click="regenerateBrief"
      >
        重新生成
      </el-button>
      <span class="ai-banner-time">{{ briefDegraded ? '本地汇总' : (briefTime || '规则版') }}</span>
    </div>

    <!-- 早报正文（Markdown 渲染） -->
    <div v-if="!briefDegraded && briefContent" class="brief-body" v-html="renderMarkdown(briefContent)"></div>

    <!-- 分级预警横幅 R1-R4（设计哲学 §11.2） -->
    <div v-if="deadlineWarnings.length > 0" class="deadline-warnings">
      <div
        v-for="w in deadlineWarnings.slice(0, 4)"
        :key="w.deadlineId"
        class="warning-item"
        :class="'level-' + w.level.toLowerCase()"
        @click="goToCase(w.caseId)"
      >
        <span class="warning-dot" :style="{ background: w.levelColor }" />
        <span class="warning-level">{{ w.level }}</span>
        <span class="warning-msg">{{ w.message }}</span>
        <span class="warning-case">{{ w.caseName }}</span>
      </div>
    </div>

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

    <!-- 智能推荐（优先后端推荐，回退本地规则排序 · 设计哲学 §11.6） -->
    <div class="panel reco-panel">
      <div class="panel-header">
        <el-icon><Star /></el-icon>
        <span>智能推荐 · 今日 {{ homeRecommendations.length || nextActionTasks.length }} 件事</span>
        <span class="reco-badge">{{ homeRecommendations.length ? '推荐引擎' : '规则排序' }}</span>
      </div>
      <div class="panel-body">
        <template v-if="homeRecommendations.length">
          <div
            v-for="(rec, idx) in homeRecommendations"
            :key="rec.taskId"
            class="reco-item"
            @click="router.push({ name: 'tasks', query: { edit: rec.taskId } })"
          >
            <span class="reco-order">{{ idx + 1 }}</span>
            <div class="reco-content">
              <div class="reco-title">{{ rec.taskName }}</div>
              <div class="reco-why">
                {{ rec.reason }}
                <template v-if="rec.estimatedMinutes"> · 预估 {{ rec.estimatedMinutes }} 分钟</template>
              </div>
            </div>
          </div>
        </template>
        <template v-else>
          <div
            v-for="(task, idx) in nextActionTasks.slice(0, 3)"
            :key="task.id"
            class="reco-item"
            @click="router.push({ name: 'tasks', query: { edit: task.id } })"
          >
            <span class="reco-order">{{ idx + 1 }}</span>
            <div class="reco-content">
              <div class="reco-title">{{ task.taskName }}</div>
              <div class="reco-why">
                {{ task.caseId ? `关联：${task.caseId}` : '无案件关联' }}
                {{ task.dueDate ? ` · 截止 ${task.dueDate}` : '' }}
              </div>
            </div>
          </div>
        </template>
        <div v-if="!homeRecommendations.length && nextActionTasks.length === 0" class="empty-state" style="padding: 16px">
          <span>暂无推荐</span>
        </div>
      </div>
    </div>

    <!-- 底部统计一行（退居角落，不是主角） -->
    <div class="statline">
      <div class="st">
        <span class="sv">{{ casesStore.cases.length }}</span>
        <span class="sk">活跃案件</span>
      </div>
      <div class="st">
        <span class="sv">{{ waitingOverdueTasks.length }}</span>
        <span class="sk">等待中</span>
      </div>
      <div class="st">
        <span class="sv">{{ tasksStore.taskStats?.completed || 0 }}</span>
        <span class="sk">已完成</span>
      </div>
      <div class="st">
        <span class="sv" style="color: #B4554F">{{ overdueTasks.length }}</span>
        <span class="sk">逾期</span>
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
  --c-primary: #3E5C9A;
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

/* ============================================================
   智能推荐
   ============================================================ */
.reco-panel {
  margin-bottom: 0;
}

.reco-badge {
  margin-left: auto;
  font-size: 11px;
  color: var(--c-text-muted);
  font-weight: 400;
}

.reco-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 9px 10px;
  border-radius: 6px;
  border: 1px solid #E0E3E9;
  margin-bottom: 8px;
  background: var(--c-surface);
  cursor: pointer;
  transition: all 0.15s;
}

.reco-item:hover {
  border-color: #C3CFE3;
  background: #EDF1F8;
}

.reco-order {
  width: 20px;
  height: 20px;
  border-radius: 6px;
  background: #EDF1F8;
  color: #3E5C9A;
  font-size: 12px;
  font-weight: 700;
  display: grid;
  place-items: center;
  flex-shrink: 0;
}

.reco-content {
  flex: 1;
  min-width: 0;
}

.reco-title {
  font-size: 13px;
  font-weight: 500;
  color: #1F2430;
}

.reco-why {
  font-size: 11px;
  color: #9BA2AF;
  margin-top: 1px;
}

/* ============================================================
   底部统计一行
   ============================================================ */
.statline {
  display: flex;
  gap: 0;
  background: var(--c-surface);
  border: 1px solid #E0E3E9;
  border-radius: 8px;
  overflow: hidden;
  margin-top: 16px;
}

.st {
  flex: 1;
  padding: 12px 16px;
  border-right: 1px solid #E0E3E9;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.st:last-child {
  border-right: none;
}

.sv {
  font-size: 20px;
  font-weight: 700;
  letter-spacing: -0.3px;
  color: #1F2430;
}

.sk {
  font-size: 11px;
  color: #9BA2AF;
}

/* ============================================================
   分级预警横幅 R1-R4
   ============================================================ */
.deadline-warnings {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-bottom: 14px;
}

.warning-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 14px;
  border-radius: 6px;
  font-size: 12.5px;
  cursor: pointer;
  transition: all 0.15s;
}

.warning-item.level-r1 {
  background: #F6F7F9;
  color: #4B5160;
}

.warning-item.level-r2 {
  background: #F7F1E3;
  color: #7A5B24;
}

.warning-item.level-r3,
.warning-item.level-r4 {
  background: #F6EDEC;
  color: #B4554F;
}

.warning-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex-shrink: 0;
}

.warning-level {
  font-weight: 600;
  font-size: 11px;
  min-width: 20px;
}

.warning-msg {
  flex: 1;
}

.warning-case {
  font-size: 11px;
  color: #9BA2AF;
  flex-shrink: 0;
}

/* ============================================================
   每日早报 AI 横幅
   ============================================================ */
.ai-banner {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 14px;
  background: var(--c-surface);
  border: 1px solid #E0E3E9;
  border-radius: 8px;
  padding: 10px 16px;
  font-size: 12.5px;
  color: #4B5160;
}

.ai-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: #4C8067;
  flex-shrink: 0;
}

.ai-banner-content {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.ai-banner-content strong {
  color: #1F2430;
  font-weight: 600;
}

.ai-banner-summary {
  color: #9BA2AF;
}

.ai-banner-time {
  font-size: 11px;
  color: #9BA2AF;
  flex-shrink: 0;
}

.ai-banner-degraded {
  font-size: 11px;
  color: #B0823A;
  background: #F7F1E3;
  border: 1px solid #E4D3A8;
  border-radius: 4px;
  padding: 1px 8px;
}

/* 早报正文（Markdown 渲染） */
.brief-body {
  margin: -6px 0 14px;
  background: var(--c-surface);
  border: 1px solid #E0E3E9;
  border-radius: 8px;
  padding: 12px 16px;
  font-size: 12.5px;
  color: #4B5160;
  max-height: 220px;
  overflow-y: auto;
  line-height: 1.7;
}

.brief-body :deep(h3),
.brief-body :deep(h4),
.brief-body :deep(h5) {
  margin: 8px 0 4px;
  font-size: 13px;
  font-weight: 600;
  color: #1F2430;
}

.brief-body :deep(p) {
  margin: 2px 0;
}

.brief-body :deep(ul) {
  margin: 2px 0;
  padding-left: 18px;
}
</style>
