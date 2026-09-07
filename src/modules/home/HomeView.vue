<script setup>
import { ref, computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import {
  CaretRight,
  ChatDotRound,
  Message,
  Document,
  ArrowRight,
  Clock,
  Warning,
  Finished,
  Refresh,
  Reading,
  Suitcase,
  Folder,
  CopyDocument,
  Printer,
  Close,
  Calendar,
  Compass,
  Check,
  Promotion,
  Plus,
  Open,
  DataAnalysis,
  Opportunity,
  Opportunity as Sparkles,
} from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'
import { casyContext } from '../../core/plugin/context'
import { useCasesStore } from '../../stores/cases'
import { useTasksStore } from '../../stores/tasks'
import { useCalendarStore } from '../../stores/calendar'
import { useProfileStore } from '../../stores/profile'
import { useInboxStore } from '../../stores/inbox'
import { useSettingsStore } from '../../stores/settings'
import BriefingModal from '../../shared/components/BriefingModal.vue'
// 审查 P0-3：rec.text 的 <strong> 由模板生成，但任务名/案件名来自用户或同步数据，渲染前必须消毒
import { sanitizeInlineHtml } from '../../shared/markdown/mdBridge'
import { daysUntil } from '../../shared/utils/date'

const { t } = useI18n()
const router = useRouter()
const casesStore = useCasesStore()
const tasksStore = useTasksStore()
const calendarStore = useCalendarStore()
const profileStore = useProfileStore()
const inboxStore = useInboxStore()
const settingsStore = useSettingsStore()

// ============================================================
// 日期与问候语
// ============================================================
const timeFilter = ref('today') // 'today' | 'week'

const dateDisplay = computed(() => {
  const now = new Date()
  const month = now.getMonth() + 1
  const date = now.getDate()
  const dayNames = ['周日', '周一', '周二', '周三', '周四', '周五', '周六']
  return `${month}月${date}日 ${dayNames[now.getDay()]}`
})

const fullDateDisplay = computed(() => {
  return new Date().toLocaleDateString('zh-CN', { year: 'numeric', month: 'long', day: 'numeric', weekday: 'long' })
})

function localDateKey(date = new Date()) {
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${year}-${month}-${day}`
}

const today = localDateKey()

const currentHour = new Date().getHours()
const currentDay = new Date().getDay()
const isMorning = computed(() => currentHour < 14) // Before 14:00 is Morning Planning, after is Evening Summary

const isWeekStart = computed(() => {
  const d = new Date()
  const todayDay = d.getDay() === 0 ? 7 : d.getDay()
  
  // 判断本周剩余几天是否还有工作日
  let hasWorkdayLeftThisWeek = false
  for (let i = todayDay; i <= 7; i++) {
    const checkDate = new Date(d)
    checkDate.setDate(d.getDate() + (i - todayDay))
    if (calendarStore.isWorkday(localDateKey(checkDate))) {
      hasWorkdayLeftThisWeek = true
      break
    }
  }
  
  // 如果本周已经没有工作日了，或者是周一到周三，视为新的一周的开始（计划期）
  if (!hasWorkdayLeftThisWeek) return true
  return todayDay >= 1 && todayDay <= 3
})

const weeklyDateRange = computed(() => {
  const d = new Date()
  const todayDay = d.getDay() === 0 ? 7 : d.getDay()
  
  // 节假日顺延机制：寻找最近的有效工作周
  let hasWorkdayLeftThisWeek = false
  for (let i = todayDay; i <= 7; i++) {
    const checkDate = new Date(d)
    checkDate.setDate(d.getDate() + (i - todayDay))
    if (calendarStore.isWorkday(localDateKey(checkDate))) {
      hasWorkdayLeftThisWeek = true
      break
    }
  }

  // 确定基准日期
  const baseDate = new Date(d)
  if (!hasWorkdayLeftThisWeek) {
    // 顺延到下周一
    baseDate.setDate(d.getDate() + (8 - todayDay))
  }
  
  const day = baseDate.getDay()
  const diffToMonday = day === 0 ? -6 : 1 - day
  
  const start = new Date(baseDate)
  start.setDate(baseDate.getDate() + diffToMonday)
  
  const end = new Date(start)
  end.setDate(start.getDate() + 6)
  
  return `${localDateKey(start).replace(/-/g, '.')} - ${localDateKey(end).replace(/-/g, '.')}`
})

onMounted(async () => {
  await Promise.all([
    profileStore.load(),
    settingsStore.load(),
    casesStore.loadDashboard(),
    tasksStore.loadTasks(),
    casesStore.loadCases(),
    inboxStore.loadItems(),
    calendarStore.loadEvents(new Date().getFullYear(), new Date().getMonth() + 1),
    loadBrief(),
  ])
})

// ============================================================
// 早报与周报弹窗控制
// ============================================================
const showDailyModal = ref(false)
const showWeeklyModal = ref(false)
const brief = ref(null)
const briefDegraded = ref(false)
const briefLoading = ref(false)

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
    ElMessage.success('早报已重新生成')
  } else {
    briefDegraded.value = true
  }
}

// ============================================================
// 下一步行动 (Focused Next Action)
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
      if (a.dueDate && !b.dueDate) return -1
      if (!a.dueDate && b.dueDate) return 1
      if (a.dueDate && b.dueDate) return a.dueDate.localeCompare(b.dueDate)
      return 0
    })
})

const primaryNextAction = computed(() => {
  return nextActionTasks.value[0] || null
})

const displayNextAction = computed(() => {
  if (primaryNextAction.value) return primaryNextAction.value
  return { taskName: '暂无可立即执行的任务', description: '', caseCode: '' }
})

function openNextActionMatter(task) {
  if (!task) return
  if (task.id) {
    router.push({ name: 'tasks', query: { edit: task.id } })
  } else {
    router.push({ name: 'tasks' })
  }
}

// ============================================================
// 1. 硬日程排期 (Hard Schedule - 法庭庭审 / 绝限日程)
// ============================================================
const hardScheduleItems = computed(() => {
  const events = calendarStore.events || []
  
  const targetDateStr = today

  const targetEvents = events.filter(e => e.date === targetDateStr || e.startDate === targetDateStr)

  return targetEvents.map(e => ({
    id: e.id,
    time: e.allDay ? '全天' : e.startTime || e.time || '时间待定',
    court: e.location || '',
    title: e.title || e.name || '未命名日程',
    track: e.track || ({ hearing: '庭审', deadline: '期限', deadline_red: '紧急期限', deadline_yellow: '期限', deadline_green: '期限', task: '任务', other: '日程', event: '日程' }[e.eventType || e.type] || '日程'),
    judge: e.judge || '',
    risk: e.priority === 'high' || e.priority === 'urgent',
    timeRemaining: '今日排期'
  }))
})

// ============================================================
// 2. 今日承诺事项 (Today's Commitments)
// ============================================================
// 案件 id → 案件名 映射：任务只存 caseId，需要从已加载案件里解析真实案件名，
// 避免在卡片上直接暴露原始 UUID。
const caseNameMap = computed(() => {
  const map = new Map()
  for (const c of casesStore.cases || []) {
    if (c.id && c.caseName) map.set(c.id, c.caseName)
  }
  return map
})

function resolveCaseName(task) {
  if (!task?.caseId) return task?.caseName || ''
  // 降级链（审查 P1-4）：caseNameMap → task.caseName → '重点在办案件'。
  // 任务关联的案件可能不在已加载的首页 50 条内，此时 caseNameMap 未命中就返回空串
  // 会让卡片案件名整行消失（配合 v-if="c.caseName"），必须保留兜底。
  return caseNameMap.value.get(task.caseId) || task.caseName || '重点在办案件'
}

const todayCommitments = computed(() => {
  const list = tasksStore.pendingTasks.filter(t => t.startBucket === 'today' || t.dueDate === today)
  return list.map(t => ({
    id: t.id,
    title: t.taskName,
    caseName: resolveCaseName(t),
    category: t.category || (t.caseId ? '案件' : '个人'),
    overdue: t.dueDate && t.dueDate < today,
    completed: t.status === 'completed' || t.status === 'done',
    task: t,
  }))
})

async function toggleCommitment(item) {
  if (pendingCompletions.value.has(item.id)) return
  pendingCompletions.value.add(item.id)
  try { await tasksStore.toggleTask(item.id) }
  finally { pendingCompletions.value.delete(item.id) }
}
const pendingCompletions = ref(new Set())

// ============================================================
// 3. 等待跟进监视器 (Waitlist Items)
// ============================================================
const waitlistItems = computed(() => {
  const waitingTasks = tasksStore.waitingTasks || []
  return [...waitingTasks].sort((a, b) => (a.followUpDate || '9999').localeCompare(b.followUpDate || '9999')).slice(0, 4).map(t => ({
    id: t.id,
    title: t.taskName,
    elapsedText: followUpLabel(t.followUpDate),
    overdue: t.followUpDate && t.followUpDate < today,
    submittedText: t.waitingFor || '等待反馈',
    expectedText: t.followUpDate ? `跟进日 ${t.followUpDate}` : '未设置跟进日',
  }))
})
function followUpLabel(date) {
  const days = daysUntil(date)
  if (days == null) return '待跟进'
  if (days < 0) return `超期 ${Math.abs(days)} 天`
  return days === 0 ? '今日跟进' : `${days} 天后跟进`
}

// ============================================================
// 4. 精力负荷与容量 (Energy & Capacity)
// ============================================================
const totalEstimatedMinutes = computed(() => {
  const todayTasksList = tasksStore.pendingTasks.filter(t => t.startBucket === 'today' || t.dueDate === today)
  return todayTasksList.reduce((sum, t) => sum + Math.max(0, t.estimatedMinutes || 0), 0)
})

const committedHours = computed(() => {
  const hours = (totalEstimatedMinutes.value / 60).toFixed(1)
  return hours
})

const unestimatedCount = computed(() => todayCommitments.value.filter(c => !c.task.estimatedMinutes).length)
const totalCapacityHours = computed(() => Math.max(0, profileStore.work_hours.end_hour - profileStore.work_hours.start_hour))
const capacityPercent = computed(() => {
  if (!totalCapacityHours.value) return 0
  return Math.max(0, Math.round(Number(committedHours.value) / totalCapacityHours.value * 100))
})

const freeSpaceHours = computed(() => {
  const free = Math.max(0, totalCapacityHours.value - Number(committedHours.value)).toFixed(1)
  return `${free}h`
})

// ============================================================
// 5. 红线与统计指标
// ============================================================
const redlineItems = computed(() => {
    return tasksStore.pendingTasks
      .filter(t => t.dueDate && (t.dueDate === today || t.dueDate < today))
      .map(t => ({
        id: t.id,
        title: t.taskName,
        caseTitle: resolveCaseName(t) || '重点案件',
        timeText: t.dueDate < today ? '已逾期' : '今日到期',
      }))
})

const waitingCount = computed(() => tasksStore.waitingTasks.length || 0)
const expiringCount = computed(() => redlineItems.value.length || 0)
const hardCount = computed(() => hardScheduleItems.value.length || 0)

const completedTasksCount = computed(() => tasksStore.tasks.filter(t => t.completed === 1).length)
const totalCases = computed(() => casesStore.cases?.length || 0)

// 动态智能建议（基于真实任务、绝限与日历数据聚合）
const dynamicRecommendations = computed(() => {
  const recs = []
  const urgentTasks = tasksStore.pendingTasks.filter(t => t.dueDate && (t.dueDate <= today || t.priority === 'urgent_important'))
  if (urgentTasks.length > 0) {
    const t = urgentTasks[0]
    recs.push({
      id: `rec-urgent-${t.id}`,
      text: `检测到任务<strong>「${t.taskName}」</strong>（${resolveCaseName(t) || '重点专案'}）需要处理，建议优先安排推进。`,
      actionLabel: '前往办理',
      targetTask: t,
    })
  }

  const waitingOverdue = tasksStore.waitingTasks.filter(t => t.followUpDate && t.followUpDate <= today)
  if (waitingOverdue.length > 0) {
    const w = waitingOverdue[0]
    recs.push({
      id: `rec-wait-${w.id}`,
      text: `外部等待事项<strong>「${w.taskName}」</strong>已到跟进时间，建议核实对方反馈。`,
      actionLabel: '跟进处理',
      targetTask: w,
    })
  }

  if (hardScheduleItems.value.length > 0) {
    const h = hardScheduleItems.value[0]
    recs.push({
      id: `rec-hearing-${h.id}`,
      text: `排期提醒：<strong>「${h.title}」</strong>（${h.time} · ${h.court}），建议核对出庭卷宗。`,
      actionLabel: '查看日历',
      targetTask: null,
    })
  }

  if (recs.length === 0) {
    recs.push({
      id: 'rec-clear',
      text: '当前暂无紧急到期任务，各项诉讼事项平稳进行中。可整理卷宗或沉淀知识至专属智库。',
      actionLabel: '前往智库',
      targetTask: null,
    })
  }

  return recs.slice(0, 2)
})

// 审查 P2-1：模板 v-html 里直接调用 sanitizeInlineHtml(rec.text) 会在每次重渲染都重跑 DOMPurify。
// 这里预计算 safeText，仅在推荐列表变化时消毒一次（与 KnowledgeSidebar 的 safeSnippet 同范式）。
const safeRecommendations = computed(() =>
  dynamicRecommendations.value.map(rec => ({ ...rec, safeText: sanitizeInlineHtml(rec.text) }))
)

// AI 自我进化反馈记录
async function handleRecAction(rec, decision) {
  if (decision === 'accept') {
    if (rec.targetTask) {
      openNextActionMatter(rec.targetTask)
    } else if (rec.id.includes('hearing')) {
      router.push('/calendar')
    } else {
      router.push('/knowledge')
    }
  }
  const decisionRes = await casyContext.ai.recordDecision({
    entityType: 'recommendation',
    entityId: rec.id,
    decisionType: 'recommend_today',
    decision: decision,
  })
  // 后端记录失败时不再虚假提示已采纳/已忽略
  if (!decisionRes.ok) return ElMessage.error(decisionRes.error || '记录决策失败')
  ElMessage.success(decision === 'accept' ? '已采纳建议' : '已忽略该建议')
}
</script>

<template>
  <div class="today-page-container">
    <!-- ═══ 顶部日期、状态指标与报表入口 ═══ -->
    <header class="today-hero-header">
      <div class="header-left">
        <h1 class="header-date-title">{{ fullDateDisplay }}</h1>
        <div class="header-status-chips">
          <span class="status-chip chip-risk">
            <span class="dot dot-risk"></span>
            <span>今日日程 {{ hardCount }}</span>
          </span>
          <span class="status-chip chip-warn">
            <span class="dot dot-warn"></span>
            <span>到期与逾期 {{ expiringCount }}</span>
          </span>
          <span class="status-chip chip-info">
            <span class="dot dot-info"></span>
            <span>等待跟进 {{ waitingCount }}</span>
          </span>
        </div>
      </div>

      <div class="header-actions-group">
        <!-- 今日早报按钮 -->
        <button
          class="btn-report-trigger"
          title="打开今日秩序早报/晚报"
          @click="showDailyModal = true"
        >
          <el-icon :size="16"><Reading /></el-icon>
          <span>{{ isMorning ? t('home.greeting_morning') : t('home.greeting_evening') }}</span>
        </button>

        <!-- 周报/周复盘按钮 -->
        <button
          class="btn-report-trigger"
          title="打开每周复盘与综合分析"
          @click="showWeeklyModal = true"
        >
          <el-icon :size="16"><DataAnalysis /></el-icon>
          <span>{{ isWeekStart ? t('home.greeting_week_start') : t('home.greeting_week_end') }}</span>
        </button>

        <!-- 查看日历 -->
        <button
          class="btn-report-trigger sub"
          title="前往排期日历"
          @click="router.push('/calendar')"
        >
          <el-icon :size="16"><Calendar /></el-icon>
          <span>{{ t('home.calendar_shortcut') }}</span>
        </button>
      </div>
    </header>

    <!-- ═══ 主内容区：双栏 8 + 4 架构 (Stitch v4.5) ═══ -->
    <div class="today-grid-layout">
      <!-- ── 左侧 8 栏：核心办案执行流 ── -->
      <div class="main-column-left">
        <!-- 1. Hard Schedule (法庭开庭与硬日程) -->
        <section class="section-block">
          <div class="section-title-row">
            <div class="title-with-icon">
              <el-icon class="icon-primary"><Compass /></el-icon>
              <h2 class="sec-heading">{{ t('home.hard_schedule') }}</h2>
            </div>
            <span class="count-badge">{{ t('home.hard_schedule_count', { count: hardScheduleItems.length }) }}</span>
          </div>

          <div class="hard-schedule-card-list">
            <div
              v-for="item in hardScheduleItems"
              :key="item.id"
              class="hard-schedule-row group"
            >
              <div class="time-col">
                <span class="time-text">{{ item.time }}</span>
                <span class="time-sub" :class="{ 'text-risk': item.risk }">
                  {{ item.timeRemaining || item.court }}
                </span>
              </div>

              <div class="info-col">
                <div class="title-line">
                  <span v-if="item.risk" class="dot-indicator risk"></span>
                  <span v-else class="dot-indicator safe"></span>
                  <strong class="matter-title">{{ item.title }}</strong>
                </div>
                <div class="meta-line">
                  <span class="track-tag">{{ item.track }}</span>
                  <span v-if="item.court || item.judge" class="judge-text">{{ [item.court, item.judge].filter(Boolean).join(' · ') }}</span>
                </div>
              </div>

              <div class="action-col">
                <button
                  class="btn-icon-jump"
                  title="查看详情"
                  @click="router.push('/calendar')"
                >
                  <el-icon><Open /></el-icon>
                </button>
              </div>
            </div>
          </div>
          <p v-if="!hardScheduleItems.length" class="section-empty">今日暂无日程</p>
        </section>

        <!-- 2. Today's Commitments (今日承诺 / 待办事项) -->
        <section class="section-block">
          <div class="section-title-row">
            <div class="title-with-icon">
              <el-icon class="icon-primary"><Finished /></el-icon>
              <h2 class="sec-heading">{{ t('home.today_commitments') }}</h2>
            </div>
            <span class="count-badge">{{ t('home.commitments_unresolved', { count: todayCommitments.filter(c => !c.completed).length }) }}</span>
          </div>

          <div class="commitments-list">
            <div
              v-for="c in todayCommitments"
              :key="c.id"
              class="commitment-item"
              :class="{ 'is-completed': c.completed, 'is-overdue': c.overdue && !c.completed }"
            >
              <input
                type="checkbox"
                class="custom-chk"
                :checked="c.completed"
                :aria-label="`完成任务：${c.title}`"
                :disabled="pendingCompletions.has(c.id)"
                @click.stop="toggleCommitment(c)"
              />
              <div class="commitment-info">
                <div class="commitment-title-row">
                  <button type="button" class="c-title task-open" @click="openNextActionMatter(c.task)">{{ c.title }}</button>
                  <span v-if="c.overdue && !c.completed" class="badge-overdue">已逾期</span>
                </div>
                <span v-if="c.caseName" class="c-case">{{ c.caseName }}</span>
              </div>
              <span class="c-category-chip" :class="c.category.toLowerCase()">
                {{ c.category }}
              </span>
            </div>
          </div>
          <p v-if="!todayCommitments.length" class="section-empty">今日承诺事项已清空</p>
        </section>

        <!-- 3. Waitlist (等待跟进 / 外部回执) -->
        <section class="section-block">
          <div class="section-title-row">
            <div class="title-with-icon">
              <el-icon class="icon-discovery"><Clock /></el-icon>
              <h2 class="sec-heading">{{ t('home.waitlist') }}</h2>
            </div>
            <span class="count-badge">{{ t('home.waitlist_count', { count: waitlistItems.length }) }}</span>
          </div>

          <div class="waitlist-grid">
            <button type="button"
              v-for="w in waitlistItems"
              :key="w.id"
              class="waitlist-card"
              @click="router.push({ name: 'tasks', query: { edit: w.id } })"
            >
              <div class="wl-left-accent"></div>
              <div class="wl-content">
                <div class="wl-head">
                  <strong class="wl-title">{{ w.title }}</strong>
                  <span class="wl-elapsed" :class="{ 'text-risk': w.overdue }">{{ w.elapsedText }}</span>
                </div>
                <div class="wl-foot">
                  <span>{{ w.submittedText }}</span>
                  <span>{{ w.expectedText }}</span>
                </div>
              </div>
            </button>
          </div>
          <p v-if="!waitlistItems.length" class="section-empty">暂无等待跟进事项</p>
        </section>
      </div>

      <!-- ── 右侧 4 栏：重点聚焦、智能建议与精力容量 ── -->
      <div class="side-column-right">
        <!-- 1. Focused Next Action 卡片 (高对比度深蓝) -->
        <div class="next-action-card">
          <div class="na-header">
            <span class="na-kicker">下一步行动</span>
            <span class="na-code" v-if="displayNextAction.caseCode">{{ displayNextAction.caseCode }}</span>
          </div>

          <h3 class="na-title">{{ displayNextAction.taskName }}</h3>
          <p v-if="displayNextAction.description" class="na-desc">{{ displayNextAction.description }}</p>

          <div class="na-actions">
            <button
              class="btn-na-open"
              @click="openNextActionMatter(displayNextAction)"
            >
              <el-icon><Promotion /></el-icon>
              <span>{{ primaryNextAction ? '查看任务' : '查看任务列表' }}</span>
            </button>
          </div>
        </div>

        <!-- 2. Smart Recommendations (智能建议 / AI) -->
        <div class="smart-recs-card">
          <div class="recs-head">
            <div class="recs-title-left">
              <el-icon class="icon-sparkle"><Sparkles /></el-icon>
              <span class="recs-title">{{ t('home.smart_recs') }}</span>
            </div>
            <span class="recs-ai-tag">待办提醒</span>
          </div>

          <div class="recs-body">
            <div
              v-for="rec in safeRecommendations"
              :key="rec.id"
              class="rec-item"
            >
              <span class="rec-dot"></span>
              <div class="rec-content">
                <p v-html="rec.safeText"></p>
                <div class="rec-actions">
                  <button class="btn-rec-action accept" @click="handleRecAction(rec, 'accept')">{{ rec.actionLabel }}</button>
                  <button class="btn-rec-action dismiss" @click="handleRecAction(rec, 'reject')">忽略</button>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- 3. 精力负荷与容量 (Energy & Capacity) -->
        <div class="capacity-meter-card">
          <div class="cap-head">
            <span class="cap-title">今日任务预估</span>
            <span class="cap-metric-val">{{ committedHours }} / {{ totalCapacityHours }}h</span>
          </div>

          <div class="cap-bar-track">
            <div
              class="cap-bar-fill"
              :style="{ width: `${Math.min(100, capacityPercent)}%` }"
              :class="{ 'cap-over': capacityPercent > 85 }"
            ></div>
          </div>
          <p v-if="unestimatedCount" class="estimate-note">{{ unestimatedCount }} 项任务未填写预估时长</p>

          <div class="cap-foot-info">
            <span class="cap-free">{{ t('home.capacity_free') }}<strong>{{ freeSpaceHours }}</strong></span>
            <span class="cap-percent">{{ t('home.capacity_percent', { percent: capacityPercent }) }}</span>
          </div>
        </div>
      </div>
    </div>

    <!-- ═══ 弹出式早报模态框 ═══ -->
    <BriefingModal
      v-model:visible="showDailyModal"
      type="daily"
      :style-variant="settingsStore.daily_brief_style || 'gazette'"
      :content="briefContent"
      :loading="briefLoading"
      :next-action="displayNextAction"
      :redlines="redlineItems"
      :hearings="hardScheduleItems"
      :metrics="{
        committedHours,
        freeSpaceHours,
        waitingCount,
        completedCount: completedTasksCount,
        totalCases: totalCases,
      }"
      @regenerate="regenerateBrief"
    />

    <!-- ═══ 弹出式周报模态框 ═══ -->
    <BriefingModal
      v-model:visible="showWeeklyModal"
      type="weekly"
      :style-variant="settingsStore.weekly_report_style || 'dossier'"
      :date-range="weeklyDateRange"
      :next-action="displayNextAction"
      :redlines="redlineItems"
      :hearings="hardScheduleItems"
      :metrics="{
        committedHours,
        freeSpaceHours,
        waitingCount,
        completedCount: completedTasksCount,
        totalCases: totalCases,
      }"
    />
  </div>
</template>

<style scoped>
/* ═══════════════════════════════════════════════════════════
   Today View (Stitch UI v4.5 Layout & Styling)
   ═══════════════════════════════════════════════════════════ */
.today-page-container {
  max-width: 1280px;
  margin: 0 auto;
  padding: 24px 32px 48px;
  display: flex;
  flex-direction: column;
  gap: 28px;
}

/* ── 顶部日期与操作栏 ── */
.today-hero-header {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  border-bottom: 1px solid var(--c-border);
  padding-bottom: 20px;
  gap: 20px;
  flex-wrap: wrap;
}

.header-left {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.header-date-title {
  font-size: 24px;
  font-weight: 700;
  color: var(--c-text-heading);
  letter-spacing: 0;
  margin: 0;
}

.header-status-chips {
  display: flex;
  align-items: center;
  gap: 16px;
  font-family: var(--font-mono);
  font-size: 11.5px;
  color: var(--c-text-secondary);
}

.status-chip {
  display: flex;
  align-items: center;
  gap: 6px;
}

.dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
}
.dot-risk { background: var(--status-risk); }
.dot-warn { background: var(--status-warning); }
.dot-info { background: var(--status-discovery); }

.header-actions-group {
  display: flex;
  align-items: center;
  gap: 10px;
}

.btn-report-trigger {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 7px 14px;
  border-radius: var(--c-radius-md);
  border: 1px solid var(--c-border);
  background: var(--c-bg-card);
  color: var(--c-text);
  font-size: 12.5px;
  font-weight: 600;
  cursor: pointer;
  box-shadow: var(--shadow-sm);
  transition: all var(--motion-fast);
}

.btn-report-trigger:hover {
  border-color: var(--c-primary);
  color: var(--c-primary);
  background: var(--c-primary-light);
}

.btn-report-trigger.sub {
  color: var(--c-text-secondary);
}

/* ── 主栅格布局 (8 + 4 架构) ── */
.today-grid-layout {
  display: grid;
  grid-template-columns: 1fr;
  gap: 28px;
}

@container (min-width: 880px) {
  .today-grid-layout {
    grid-template-columns: minmax(0, 1.7fr) minmax(260px, 1fr);
  }
}

/* ── 左栏 ── */
.main-column-left {
  display: flex;
  flex-direction: column;
  gap: 28px;
}

.section-block {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.section-title-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.title-with-icon {
  display: flex;
  align-items: center;
  gap: 8px;
}

.icon-primary { color: var(--c-primary); font-size: 18px; }
.icon-discovery { color: var(--status-discovery); font-size: 18px; }

.sec-heading {
  font-size: 15px;
  font-weight: 700;
  color: var(--c-text-heading);
  margin: 0;
}

.count-badge {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--slate-gray-light);
  background: var(--c-bg-subtle);
  padding: 2px 8px;
  border-radius: 4px;
}

/* 1. Hard Schedule */
.hard-schedule-card-list {
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-xl);
  overflow: hidden;
  box-shadow: var(--shadow-sm);
}

.hard-schedule-row {
  display: flex;
  align-items: center;
  padding: 14px 18px;
  border-bottom: 1px solid var(--c-border-light);
  transition: background var(--motion-fast);
}

.hard-schedule-row:last-child {
  border-bottom: none;
}

.hard-schedule-row:hover {
  background: var(--c-bg-hover);
}

.time-col {
  width: 90px;
  display: flex;
  flex-direction: column;
  padding-right: 14px;
  border-right: 1px solid var(--c-border-light);
  flex-shrink: 0;
}

.time-text {
  font-family: var(--font-mono);
  font-size: 12px;
  font-weight: 700;
  color: var(--c-text);
}

.time-sub {
  font-size: 11px;
  color: var(--slate-gray-light);
  margin-top: 2px;
}

.text-risk {
  color: var(--status-risk) !important;
  font-weight: 600;
}

.info-col {
  flex: 1;
  padding-left: 16px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.title-line {
  display: flex;
  align-items: center;
  gap: 8px;
}

.dot-indicator {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex-shrink: 0;
}

.dot-indicator.risk { background: var(--status-risk); }
.dot-indicator.safe { background: var(--status-warning); }

.matter-title {
  font-size: 13.5px;
  font-weight: 600;
  color: var(--c-text-heading);
}

.meta-line {
  display: flex;
  align-items: center;
  gap: 8px;
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--slate-gray-light);
}

.track-tag {
  background: var(--c-primary-light);
  color: var(--c-primary);
  padding: 1px 6px;
  border-radius: 3px;
  font-size: 10.5px;
  font-weight: 600;
}

.action-col {
  margin-left: 8px;
}

.btn-icon-jump {
  background: transparent;
  border: none;
  color: var(--slate-gray-light);
  cursor: pointer;
  padding: 6px;
  border-radius: 4px;
  transition: all var(--motion-fast);
}

.btn-icon-jump:hover {
  background: var(--c-bg-subtle);
  color: var(--c-primary);
}

/* 2. Today's Commitments */
.commitments-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.commitment-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 14px;
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  box-shadow: var(--shadow-sm);
  cursor: pointer;
  transition: all var(--motion-fast);
}

.commitment-item:hover {
  border-color: var(--c-border-strong);
}

.commitment-item.is-overdue {
  background: color-mix(in srgb, var(--status-risk) 6%, var(--c-bg-card));
  border-color: color-mix(in srgb, var(--status-risk) 30%, var(--c-border));
  box-shadow: var(--shadow-sm), inset 3px 0 0 0 var(--status-risk);
}

.commitment-item.is-completed .c-title {
  text-decoration: line-through;
  color: var(--slate-gray-light);
}

.custom-chk {
  width: 16px;
  height: 16px;
  accent-color: var(--c-primary);
  cursor: pointer;
}

.commitment-info {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.commitment-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.c-title {
  font-size: 13px;
  font-weight: 500;
  color: var(--c-text);
}

.badge-overdue {
  font-family: var(--font-mono);
  font-size: 9.5px;
  font-weight: 700;
  background: var(--status-risk);
  color: #fff;
  padding: 1px 5px;
  border-radius: 3px;
}

.c-case {
  font-size: 11.5px;
  color: var(--slate-gray-light);
}

.c-category-chip {
  font-family: var(--font-mono);
  font-size: 10px;
  padding: 2px 7px;
  border-radius: 4px;
  background: var(--c-bg-subtle);
  color: var(--slate-gray-light);
}

.c-category-chip.client {
  background: var(--c-primary-light);
  color: var(--c-primary);
  font-weight: 600;
}

/* 3. Waitlist */
.waitlist-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
  gap: 12px;
}

.waitlist-card {
  font: inherit;
  text-align: left;
  cursor: pointer;
  position: relative;
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  padding: 12px 14px;
  box-shadow: var(--shadow-sm);
  overflow: hidden;
}

.wl-left-accent {
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 3px;
  background: var(--status-discovery);
}

.wl-content {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding-left: 4px;
}

.wl-head {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
}

.wl-title {
  font-size: 12.5px;
  font-weight: 600;
  color: var(--c-text-heading);
}

.wl-elapsed {
  font-family: var(--font-mono);
  font-size: 10.5px;
  color: var(--slate-gray-light);
}

.wl-progress-track {
  height: 4px;
  background: var(--c-bg-page);
  border-radius: 2px;
  overflow: hidden;
  margin: 2px 0;
}

.wl-progress-fill {
  height: 100%;
  background: var(--status-discovery);
  border-radius: 2px;
}

.wl-foot {
  display: flex;
  justify-content: space-between;
  font-family: var(--font-mono);
  font-size: 10px;
  color: var(--slate-gray-light);
}

/* ── 右栏 (4 栏) ── */
.side-column-right {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

/* 1. Next Action Card (深蓝高对比度) */
.next-action-card {
  background: var(--c-primary);
  color: var(--c-primary-contrast, #ffffff);
  border-radius: var(--c-radius-xl);
  padding: 20px;
  box-shadow: var(--shadow-md);
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.na-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.na-kicker {
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0;
  opacity: 0.85;
}

.na-code {
  font-family: var(--font-mono);
  font-size: 10.5px;
  background: rgba(255, 255, 255, 0.15);
  padding: 1px 6px;
  border-radius: 4px;
}

.na-title {
  font-size: 15px;
  font-weight: 700;
  margin: 0;
  line-height: 1.4;
  color: #ffffff;
}

.na-desc {
  font-size: 12px;
  line-height: 1.5;
  opacity: 0.85;
  margin: 0;
  color: #ffffff;
}

.na-actions {
  margin-top: 4px;
}

.btn-na-open {
  width: 100%;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 8px 14px;
  background: #ffffff;
  color: var(--c-primary);
  border: none;
  border-radius: var(--c-radius-md);
  font-size: 12px;
  font-weight: 700;
  cursor: pointer;
  transition: all var(--motion-fast);
  box-shadow: var(--shadow-sm);
}

.btn-na-open:hover {
  background: #f1f5f9;
  transform: translateY(-1px);
}

/* 2. Smart Recommendations */
.smart-recs-card {
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-xl);
  padding: 16px;
  box-shadow: var(--shadow-sm);
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.recs-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-bottom: 8px;
  border-bottom: 1px dashed var(--c-border-light);
}

.recs-title-left {
  display: flex;
  align-items: center;
  gap: 6px;
}

.icon-sparkle {
  color: var(--status-success);
  font-size: 16px;
}

.recs-title {
  font-size: 12.5px;
  font-weight: 700;
  color: var(--c-text-heading);
}

.recs-ai-tag {
  font-family: var(--font-mono);
  font-size: 9.5px;
  font-weight: 700;
  padding: 1px 5px;
  border-radius: 3px;
  background: var(--c-primary-light);
  color: var(--c-primary);
}

.recs-body {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.rec-item {
  display: flex;
  gap: 12px;
  align-items: flex-start;
}

.rec-content {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.rec-content p {
  font-size: 12px;
  line-height: 1.5;
  color: var(--c-text-regular);
  margin: 0;
}

.rec-actions {
  display: flex;
  gap: 8px;
}

.btn-rec-action {
  font-size: 11px;
  padding: 4px 10px;
  border-radius: 4px;
  border: 1px solid var(--c-border);
  background: var(--c-bg-card);
  cursor: pointer;
  transition: all var(--motion-fast);
}

.btn-rec-action.accept {
  color: var(--c-primary);
  border-color: var(--c-primary);
}

.btn-rec-action.accept:hover {
  background: var(--c-primary-light);
}

.btn-rec-action.dismiss:hover {
  background: var(--c-bg-hover);
}

.rec-dot {
  width: 6px;
  height: 5px;
  border-radius: 50%;
  background: var(--status-success);
  margin-top: 6px;
  flex-shrink: 0;
}

.rec-item p {
  font-size: 12px;
  line-height: 1.5;
  color: var(--c-text-regular);
  margin: 0;
}

/* 3. Energy & Capacity */
.capacity-meter-card {
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-xl);
  padding: 16px;
  box-shadow: var(--shadow-sm);
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.cap-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.cap-title {
  font-size: 12.5px;
  font-weight: 700;
  color: var(--c-text-heading);
}

.cap-metric-val {
  font-family: var(--font-mono);
  font-size: 12px;
  font-weight: 700;
  color: var(--c-primary);
}

.cap-bar-track {
  height: 6px;
  background: var(--c-bg-page);
  border-radius: 3px;
  overflow: hidden;
}

.cap-bar-fill {
  height: 100%;
  background: var(--c-primary);
  border-radius: 3px;
  transition: width 0.4s ease;
}

.cap-bar-fill.cap-over {
  background: var(--status-warning);
}
.section-empty { padding: 20px 0; color: var(--c-text-secondary); font-size: 13px; border-bottom: 1px solid var(--c-border-light); }
.task-open { border: 0; background: transparent; padding: 0; text-align: left; cursor: pointer; font: inherit; color: inherit; }
.task-open:hover { color: var(--c-primary); }
.commitment-info, .info-col, .main-column-left, .side-column-right { min-width: 0; }
.matter-title, .c-title, .wl-title, .na-title { overflow-wrap: anywhere; }
.title-line, .meta-line, .commitment-title-row, .wl-head, .wl-foot, .cap-foot-info { flex-wrap: wrap; }
.wl-head, .wl-foot { gap: 8px; }
.estimate-note { margin: 0; font-size: 12px; color: var(--c-warning); }
.next-action-card { background: var(--c-primary-light); color: var(--c-text); border: 1px solid var(--c-border); box-shadow: none; border-left: 3px solid var(--c-primary); border-radius: 8px; }
.na-title { color: var(--c-text-heading); }
.na-desc { color: var(--c-text-regular); opacity: 1; }
.na-kicker { color: var(--c-primary); font-size: 12px; opacity: 1; }
.btn-na-open { background: var(--c-primary); color: var(--c-primary-contrast); }
.btn-na-open:hover { background: var(--c-primary-hover); }
.smart-recs-card, .capacity-meter-card { background: transparent; border: 0; border-top: 1px solid var(--c-border); border-radius: 0; box-shadow: none; padding: 16px 0; }
@media (max-width: 600px) {
  .today-page-container { padding: 20px 16px 32px; gap: 24px; }
  .header-date-title { font-size: 20px; }
  .header-status-chips, .header-actions-group { flex-wrap: wrap; gap: 10px; }
  .btn-report-trigger { padding: 7px 10px; }
  .hard-schedule-row { padding: 12px; }
  .time-col { width: 64px; padding-right: 8px; }
  .info-col { padding-left: 10px; }
  .waitlist-grid { grid-template-columns: minmax(0, 1fr); }
}

.cap-foot-info {
  display: flex;
  justify-content: space-between;
  font-size: 11px;
  color: var(--slate-gray-light);
}

.cap-free strong {
  color: var(--c-text);
}
</style>
