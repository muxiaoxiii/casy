<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { casyContext } from '../../../core/plugin/context'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  ArrowLeft, Edit, Calendar, Finished, Document,
  Folder, Collection, Clock, Warning, Check,
  Plus, More, Connection, Timer, Bell
} from '@element-plus/icons-vue'
import {
  CIVIL_STATUS_LABELS,
  INVALIDATION_STATUS_LABELS,
  ADMIN_STATUS_LABELS,
  CASE_ROUTE_LABELS,
} from '../../../types'

const route = useRoute()
const router = useRouter()

// ============================================================
// 状态
// ============================================================
const caseData = ref(null)
const loading = ref(false)
const tasks = ref([])
const timeline = ref([])
const knowledge = ref([])
const files = ref([])

// 编辑状态
const editingGoal = ref(false)
const goalInput = ref('')

// ============================================================
// 计算属性
// ============================================================
const caseId = computed(() => route.params.id)

const caseTypeLabel = computed(() => {
  const types = {
    computational: '计算型',
    exploratory: '探索型',
    growth: '成长型',
  }
  return types[caseData.value?.caseType] || '探索型'
})

const caseTypeColor = computed(() => {
  const colors = {
    computational: '#409EFF',
    exploratory: '#8B5CF6',
    growth: '#10B981',
  }
  return colors[caseData.value?.caseType] || '#8B5CF6'
})

const trackBadges = computed(() => {
  if (!caseData.value) return []
  
  const badges = []
  const route = caseData.value.caseRoute || ''
  
  // 民事诉讼状态颜色映射
  const civilStatusColors = {
    intake: '#6B7280',
    filed: '#3B82F6',
    pre_hearing: '#F59E0B',
    in_trial: '#8B5CF6',
    settled: '#10B981',
    awaiting_verdict: '#F59E0B',
    verdict_issued: '#EF4444',
    appeal_period: '#F59E0B',
    second_instance: '#8B5CF6',
    second_verdict: '#EF4444',
    retrial: '#8B5CF6',
    enforcement: '#10B981',
    suspended: '#6B7280',
    closed: '#10B981',
  }
  
  // 专利无效状态颜色映射
  const invalidationStatusColors = {
    preparing: '#6B7280',
    filed: '#3B82F6',
    pre_oral: '#F59E0B',
    oral_done: '#8B5CF6',
    awaiting_decision: '#F59E0B',
    decision_issued: '#EF4444',
  }
  
  // 行政诉讼状态颜色映射
  const adminStatusColors = {
    filed: '#3B82F6',
    pre_hearing: '#F59E0B',
    in_trial: '#8B5CF6',
    awaiting_verdict: '#F59E0B',
    verdict_issued: '#EF4444',
    second_instance: '#8B5CF6',
    closed: '#10B981',
  }
  
  if (route.includes('民事诉讼') && caseData.value.civilStatus) {
    badges.push({
      track: '民事诉讼',
      status: caseData.value.civilStatus,
      label: CIVIL_STATUS_LABELS[caseData.value.civilStatus] || caseData.value.civilStatus,
      color: civilStatusColors[caseData.value.civilStatus] || '#6B7280',
    })
  }
  
  if (route.includes('专利无效') && caseData.value.invalidationStatus) {
    badges.push({
      track: '专利无效',
      status: caseData.value.invalidationStatus,
      label: INVALIDATION_STATUS_LABELS[caseData.value.invalidationStatus] || caseData.value.invalidationStatus,
      color: invalidationStatusColors[caseData.value.invalidationStatus] || '#6B7280',
    })
  }
  
  if (route.includes('行政诉讼') && caseData.value.adminStatus) {
    badges.push({
      track: '行政诉讼',
      status: caseData.value.adminStatus,
      label: ADMIN_STATUS_LABELS[caseData.value.adminStatus] || caseData.value.adminStatus,
      color: adminStatusColors[caseData.value.adminStatus] || '#6B7280',
    })
  }
  
  return badges
})

const taskStats = computed(() => {
  const total = tasks.value.length
  const completed = tasks.value.filter(t => t.completed).length
  const pending = tasks.value.filter(t => !t.completed).length
  const overdue = tasks.value.filter(t => {
    if (t.completed) return false
    const due = t.dueDate || t.deadline
    return due && due < new Date().toISOString().split('T')[0]
  }).length
  
  return { total, completed, pending, overdue }
})

const nextAction = computed(() => {
  // 找到第一个 blocked=0 的任务
  return tasks.value.find(t => !t.completed && t.blocked === 0 && t.taskType === 'action')
})

// 顺序项目统计
const sequentialTasks = computed(() => {
  return tasks.value.filter(t => t.sequential).sort((a, b) => a.sequenceOrder - b.sequenceOrder)
})

const sequentialTotalCount = computed(() => {
  return sequentialTasks.value.length
})

const sequentialCompletedCount = computed(() => {
  return sequentialTasks.value.filter(t => t.completed).length
})

const sequentialCompletionRate = computed(() => {
  if (sequentialTotalCount.value === 0) return 0
  return Math.round(sequentialCompletedCount.value / sequentialTotalCount.value * 100)
})

// ============================================================
// 案件类型差异化指标（get_case_type_metrics，加载失败静默隐藏）
// ============================================================
const typeMetrics = ref(null)

const metricsCaseType = computed(() => {
  const t = typeMetrics.value?.caseType || typeMetrics.value?.case_type || caseData.value?.caseType || 'generic'
  return ['computational', 'exploratory', 'growth'].includes(t) ? t : 'generic'
})

const metricsTypeLabel = computed(() => {
  const labels = { computational: '计算型', exploratory: '探索型', growth: '成长型', generic: '通用' }
  return labels[metricsCaseType.value]
})

/** 比率字段可能为 0-1 或 0-100，统一转百分比文本 */
function percentText(v) {
  if (v == null) return '—'
  const p = v <= 1 ? v * 100 : v
  return `${Math.round(p)}%`
}

// ============================================================
// 数据加载
// ============================================================
async function loadCaseData() {
  loading.value = true
  await Promise.all([
    loadCase(),
    loadTasks(),
    loadTimeline(),
    loadKnowledge(),
    loadFiles(),
    loadTypeMetrics(),
  ])
  loading.value = false
}

async function loadTypeMetrics() {
  const result = await casyContext.cases.caseTypeMetrics(caseId.value)
  if (result.ok && result.data) {
    typeMetrics.value = result.data
  } else {
    typeMetrics.value = null
  }
}

async function loadCase() {
  const result = await casyContext.cases.get(caseId.value)
  if (result.ok) {
    caseData.value = result.data
    goalInput.value = result.data.caseGoal || ''
  }
}

async function loadTasks() {
  const result = await casyContext.tasks.list({ caseId: caseId.value })
  if (result.ok) {
    tasks.value = result.data || []
  }
}

async function loadTimeline() {
  const result = await casyContext.cases.timeline(caseId.value)
  if (result.ok) {
    timeline.value = result.data || []
  }
}

async function loadKnowledge() {
  // TODO: 加载关联知识
  knowledge.value = []
}

async function loadFiles() {
  const result = await casyContext.files.list(caseId.value)
  if (result.ok) {
    files.value = result.data || []
  }
}

// ============================================================
// 操作
// ============================================================
function goBack() {
  router.push({ name: 'cases' })
}

async function saveGoal() {
  if (!caseData.value) return
  
  const result = await casyContext.cases.update(caseId.value, { caseGoal: goalInput.value })
  
  if (result.ok) {
    caseData.value.caseGoal = goalInput.value
    editingGoal.value = false
    ElMessage.success('已保存')
  }
}

async function toggleTaskComplete(task) {
  const result = await casyContext.tasks.toggle(task.id)
  if (result.ok) {
    task.completed = task.completed ? 0 : 1
    ElMessage.success(task.completed ? '已完成' : '已恢复')
    
    // 如果是顺序项目，解锁下一个任务
    if (task.completed && task.sequential) {
      await unlockNextTask(task)
    }
  }
}

async function unlockNextTask(completedTask) {
  // 找到 sequence_order 大于当前任务的下一个任务
  const nextTask = tasks.value.find(t => 
    t.caseId === caseId.value && 
    t.sequential && 
    t.blocked && 
    t.sequenceOrder > completedTask.sequenceOrder
  )
  
  if (nextTask) {
    await casyContext.tasks.update({
      id: nextTask.id,
      blocked: 0,
    })
    nextTask.blocked = 0
    ElMessage.success(`已解锁：${nextTask.taskName}`)
  }
}

function openTaskEdit(task) {
  router.push({ name: 'tasks', query: { edit: task.id } })
}

function addNewTask() {
  router.push({ name: 'tasks', query: { capture: 'task', caseId: caseId.value } })
}

// ============================================================
// 工具函数
// ============================================================
function formatDate(dateStr) {
  if (!dateStr) return ''
  const date = new Date(dateStr)
  return date.toLocaleDateString('zh-CN', { month: 'short', day: 'numeric' })
}

function isOverdue(task) {
  const due = task.dueDate || task.deadline
  return due && due < new Date().toISOString().split('T')[0]
}

function getTaskTypeLabel(type) {
  const labels = { action: '行动', waiting: '等待', delegated: '委派', someday: '某天' }
  return labels[type] || type
}

function getTaskTypeColor(type) {
  const colors = { action: '#409EFF', waiting: '#E6A23C', delegated: '#909399', someday: '#909399' }
  return colors[type] || '#909399'
}

// 跳转到客户视图
function goToClient(clientName) {
  // 这里假设有一个客户视图路由，实际需要根据项目路由结构来实现
  // 暂时使用简单的alert，后续可以改为router.push
  ElMessage.info(`跳转到客户: ${clientName}`)
  // router.push({ name: 'client', query: { name: clientName } })
}

// ============================================================
// 生命周期
// ============================================================
onMounted(() => {
  loadCaseData()
})

watch(caseId, () => {
  loadCaseData()
})
</script>

<template>
  <div class="case-detail" v-loading="loading">
    <!-- 顶部导航 -->
    <div class="detail-header">
      <el-button @click="goBack" :icon="ArrowLeft" text>返回案件列表</el-button>
    </div>

    <!-- 案件概要 -->
    <div class="case-summary" v-if="caseData">
      <div class="summary-main">
        <div class="summary-title">
          <h1>{{ caseData.caseName }}</h1>
          <el-tag v-if="caseData.caseNo" size="small">{{ caseData.caseNo }}</el-tag>
        </div>
        
        <div class="summary-meta">
          <span class="meta-item clickable" @click="goToClient(caseData.clientName)">
            <el-icon><Folder /></el-icon>
            {{ caseData.clientName }}
          </span>
          <span class="meta-item" v-if="caseData.court">
            <el-icon><Connection /></el-icon>
            {{ caseData.court }}
          </span>
          <span class="meta-item case-type" :style="{ color: caseTypeColor }">
            <el-icon><Collection /></el-icon>
            {{ caseTypeLabel }}
          </span>
        </div>
        
        <!-- 轨道徽章 -->
        <div class="track-badges" v-if="trackBadges.length > 0">
          <div 
            v-for="badge in trackBadges" 
            :key="badge.track"
            class="track-badge"
            :style="{ borderColor: badge.color, color: badge.color }"
          >
            {{ badge.track }}: {{ badge.label }}
          </div>
        </div>
      </div>
    </div>

    <!-- 项目总览 -->
    <div class="section" v-if="caseData">
      <div class="section-header">
        <h2>项目总览</h2>
      </div>
      
      <div class="project-overview">
        <!-- 案件目标 -->
        <div class="overview-item goal">
          <div class="item-header">
            <span class="item-label">案件目标</span>
            <el-button 
              v-if="!editingGoal" 
              text 
              size="small" 
              @click="editingGoal = true"
            >
              编辑
            </el-button>
          </div>
          
          <div v-if="editingGoal" class="goal-edit">
            <el-input
              v-model="goalInput"
              placeholder="30字内概括案件目标..."
              maxlength="30"
              show-word-limit
              @keyup.enter="saveGoal"
            />
            <div class="goal-actions">
              <el-button size="small" @click="editingGoal = false">取消</el-button>
              <el-button size="small" type="primary" @click="saveGoal">保存</el-button>
            </div>
          </div>
          
          <div v-else class="goal-display">
            {{ caseData.caseGoal || '点击编辑设置案件目标' }}
          </div>
        </div>
        
        <!-- 任务统计 -->
        <div class="overview-item stats">
          <div class="item-label">任务统计</div>
          <div class="stats-grid">
            <div class="stat">
              <span class="stat-value">{{ taskStats.total }}</span>
              <span class="stat-label">总计</span>
            </div>
            <div class="stat">
              <span class="stat-value">{{ taskStats.pending }}</span>
              <span class="stat-label">待办</span>
            </div>
            <div class="stat">
              <span class="stat-value">{{ taskStats.completed }}</span>
              <span class="stat-label">完成</span>
            </div>
            <div class="stat" v-if="taskStats.overdue > 0">
              <span class="stat-value overdue">{{ taskStats.overdue }}</span>
              <span class="stat-label">逾期</span>
            </div>
          </div>
        </div>
        
        <!-- 进度环 -->
        <div class="overview-item progress">
          <div class="item-label">项目进度</div>
          <div class="progress-ring">
            <el-progress
              type="circle"
              :percentage="taskStats.total > 0 ? Math.round(taskStats.completed / taskStats.total * 100) : 0"
              :width="80"
              :stroke-width="8"
              color="#4C8067"
            />
          </div>
        </div>

        <!-- 案件类型差异化指标（加载失败静默隐藏） -->
        <div class="overview-item type-metrics" v-if="typeMetrics">
          <div class="item-label">{{ metricsTypeLabel }}指标</div>
          <div class="metrics-rows">
            <template v-if="metricsCaseType === 'computational'">
              <div class="metric-row">
                <span class="metric-label">期限内按时完成率</span>
                <span class="metric-value">{{ percentText(typeMetrics.onTimeRate) }}</span>
              </div>
              <div class="metric-row">
                <span class="metric-label">当前逾期</span>
                <span class="metric-value" :class="{ 'metric-danger': (typeMetrics.overdueCount || 0) > 0 }">
                  {{ typeMetrics.overdueCount ?? 0 }}
                </span>
              </div>
            </template>
            <template v-else-if="metricsCaseType === 'exploratory'">
              <div class="metric-row">
                <span class="metric-label">近 90 天阶段推进</span>
                <span class="metric-value">{{ typeMetrics.trackTransitions90d ?? 0 }} 次</span>
              </div>
              <div class="metric-row">
                <span class="metric-label">顺序项目解锁进度</span>
                <span class="metric-value">{{ typeMetrics.blockedResolved ?? 0 }}/{{ typeMetrics.blockedTotal ?? 0 }}</span>
              </div>
            </template>
            <template v-else-if="metricsCaseType === 'growth'">
              <div class="metric-row">
                <span class="metric-label">近 30 天活跃天数</span>
                <span class="metric-value">{{ typeMetrics.activeDays30d ?? 0 }} 天</span>
              </div>
              <div class="metric-row">
                <span class="metric-label">连续无活动</span>
                <span class="metric-value" :class="{ 'metric-warn': (typeMetrics.inactiveStreakDays || 0) > 3 }">
                  {{ typeMetrics.inactiveStreakDays ?? 0 }} 天
                </span>
              </div>
            </template>
            <template v-else>
              <div class="metric-row">
                <span class="metric-label">完成率</span>
                <span class="metric-value">{{ percentText(typeMetrics.completionRate) }}</span>
              </div>
              <div class="metric-row">
                <span class="metric-label">逾期</span>
                <span class="metric-value" :class="{ 'metric-danger': (typeMetrics.overdueCount || 0) > 0 }">
                  {{ typeMetrics.overdueCount ?? 0 }}
                </span>
              </div>
            </template>
          </div>
        </div>
      </div>
    </div>

    <!-- 下一步行动 -->
    <div class="section next-action-section" v-if="nextAction">
      <div class="section-header">
        <h2>下一步行动</h2>
        <el-button 
          type="success" 
          size="small" 
          @click.stop="toggleTaskComplete(nextAction)"
          class="complete-action-btn"
        >
          <el-icon><Check /></el-icon>
          完成
        </el-button>
      </div>
      
      <div class="next-action-card" @click="openTaskEdit(nextAction)">
        <div class="action-check" @click.stop="toggleTaskComplete(nextAction)">
          <el-icon color="#C0C4CC"><CircleCheck /></el-icon>
        </div>
        
        <div class="action-content">
          <div class="action-title">{{ nextAction.taskName }}</div>
          <div class="action-meta">
            <span v-if="nextAction.dueDate || nextAction.deadline" class="meta-item" :class="{ overdue: isOverdue(nextAction) }">
              <el-icon><Calendar /></el-icon>
              {{ formatDate(nextAction.dueDate || nextAction.deadline) }}
            </span>
            <span v-if="nextAction.estimatedMinutes" class="meta-item">
              <el-icon><Timer /></el-icon>
              {{ nextAction.estimatedMinutes }}分钟
            </span>
            <span v-if="nextAction.context" class="meta-item context">
              @{{ nextAction.context }}
            </span>
          </div>
        </div>
        
        <el-icon color="#A1A1AA"><ArrowRight /></el-icon>
      </div>
    </div>

    <!-- 顺序项目列表 -->
    <div class="section" v-if="tasks.filter(t => t.sequential).length > 0">
      <div class="section-header">
        <h2>项目流程</h2>
        <div class="sequential-progress">
          <el-progress 
            :percentage="sequentialCompletionRate" 
            :stroke-width="8"
            :show-text="false"
            color="#67C23A"
          />
          <span class="progress-text">{{ sequentialCompletedCount }}/{{ sequentialTotalCount }}</span>
        </div>
        <el-button text size="small" @click="addNewTask">
          <el-icon><Plus /></el-icon>
          添加步骤
        </el-button>
      </div>
      
      <div class="sequential-tasks">
        <div
          v-for="task in tasks.filter(t => t.sequential).sort((a, b) => a.sequenceOrder - b.sequenceOrder)"
          :key="task.id"
          :class="['sequential-task', { 
            completed: task.completed, 
            blocked: task.blocked,
            current: !task.completed && !task.blocked 
          }]"
        >
          <div class="task-check" @click="toggleTaskComplete(task)">
            <el-icon v-if="task.completed" color="#67C23A"><Check /></el-icon>
            <el-icon v-else-if="task.blocked" color="#C0C4CC"><Lock /></el-icon>
            <el-icon v-else color="#C0C4CC"><CircleCheck /></el-icon>
          </div>
          
          <div class="task-content" @click="openTaskEdit(task)">
            <span class="task-name">{{ task.taskName }}</span>
            <span v-if="task.blocked" class="blocked-hint">等待前置步骤完成</span>
          </div>
          
          <div class="task-status">
            <el-tag v-if="task.completed" type="success" size="small">已完成</el-tag>
            <el-tag v-else-if="task.blocked" type="info" size="small">已锁定</el-tag>
            <el-tag v-else type="primary" size="small">进行中</el-tag>
          </div>
        </div>
      </div>
    </div>

    <!-- 三轨状态 -->
    <div class="section" v-if="trackBadges.length > 0">
      <div class="section-header">
        <h2>案件状态</h2>
      </div>
      
      <div class="track-status">
        <div 
          v-for="badge in trackBadges" 
          :key="badge.track"
          class="track-card"
        >
          <div class="track-header" :style="{ borderBottomColor: badge.color }">
            <span class="track-name">{{ badge.track }}</span>
          </div>
          
          <div class="track-body">
            <div class="status-badge" :style="{ backgroundColor: badge.color + '20', color: badge.color }">
              {{ badge.label }}
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- 关联资源 -->
    <div class="section">
      <div class="section-header">
        <h2>关联资源</h2>
      </div>
      
      <div class="resources-grid">
        <!-- 任务 -->
        <div class="resource-card" @click="router.push({ name: 'tasks', query: { caseId: caseId } })">
          <el-icon :size="24" color="#E6A23C"><Finished /></el-icon>
          <div class="resource-info">
            <span class="resource-count">{{ taskStats.pending }}</span>
            <span class="resource-label">待办任务</span>
          </div>
        </div>
        
        <!-- 文件 -->
        <div class="resource-card">
          <el-icon :size="24" color="#409EFF"><Folder /></el-icon>
          <div class="resource-info">
            <span class="resource-count">{{ files.length }}</span>
            <span class="resource-label">案卷文件</span>
          </div>
        </div>
        
        <!-- 知识 -->
        <div class="resource-card">
          <el-icon :size="24" color="#8B5CF6"><Collection /></el-icon>
          <div class="resource-info">
            <span class="resource-count">{{ knowledge.length }}</span>
            <span class="resource-label">关联知识</span>
          </div>
        </div>
      </div>
    </div>

    <!-- 动态轨迹 -->
    <div class="section">
      <div class="section-header">
        <h2>动态轨迹</h2>
      </div>
      
      <div class="timeline" v-if="timeline.length > 0">
        <div 
          v-for="event in timeline.slice(0, 10)" 
          :key="event.id"
          class="timeline-item"
        >
          <div class="timeline-dot" :style="{ backgroundColor: event.color || '#409EFF' }"></div>
          <div class="timeline-content">
            <div class="timeline-title">{{ event.eventSummary }}</div>
            <div class="timeline-date">{{ formatDate(event.eventDate) }}</div>
          </div>
        </div>
      </div>
      
      <div v-else class="empty-timeline">
        <el-icon :size="32" color="#C0C4CC"><Clock /></el-icon>
        <p>暂无动态记录</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.case-detail {
  padding: 20px;
  max-width: 1200px;
  margin: 0 auto;
}

/* 顶部导航 */
.detail-header {
  margin-bottom: 20px;
}

/* 案件概要 */
.case-summary {
  background: #FFFFFF;
  border-radius: 8px;
  padding: 20px;
  margin-bottom: 20px;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
}

.summary-title {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 12px;
}

.summary-title h1 {
  margin: 0;
  font-size: 20px;
  font-weight: 600;
  color: #18181B;
}

.summary-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 16px;
  margin-bottom: 12px;
  font-size: 13px;
  color: #52525B;
}

.meta-item {
  display: flex;
  align-items: center;
  gap: 4px;
}

.case-type {
  font-weight: 500;
}

.track-badges {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.track-badge {
  padding: 4px 12px;
  border-radius: 4px;
  border: 1px solid;
  font-size: 12px;
  font-weight: 500;
}

/* 区块通用 */
.section {
  background: #FFFFFF;
  border-radius: 8px;
  padding: 20px;
  margin-bottom: 20px;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
}

.section-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
}

.section-header h2 {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  color: #18181B;
}

/* 项目总览 */
.project-overview {
  display: grid;
  grid-template-columns: 1fr 1fr 1fr;
  gap: 20px;
}

.overview-item {
  padding: 16px;
  background: #FAFAFA;
  border-radius: 8px;
}

.item-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
}

.item-label {
  font-size: 12px;
  color: #A1A1AA;
  margin-bottom: 8px;
}

.goal-display {
  font-size: 14px;
  color: #18181B;
  min-height: 40px;
}

.goal-edit {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.goal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

/* 统计 */
.stats-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 12px;
}

.stat {
  text-align: center;
}

.stat-value {
  display: block;
  font-size: 20px;
  font-weight: 600;
  color: #18181B;
}

.stat-value.overdue {
  color: #F56C6C;
}

.stat-label {
  font-size: 11px;
  color: #A1A1AA;
}

/* 进度环 */
.progress-ring {
  display: flex;
  justify-content: center;
}

/* 案件类型差异化指标 */
.metrics-rows {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.metric-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 13px;
}

.metric-label {
  color: #52525B;
}

.metric-value {
  font-weight: 600;
  color: #18181B;
}

.metric-danger {
  color: #B4554F;
}

.metric-warn {
  color: #B0823A;
}

/* 下一步行动 */
.next-action-section {
  background: #EFF6FF;
}

.next-action-card {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 16px;
  background: #FFFFFF;
  border-radius: 8px;
  cursor: pointer;
  transition: box-shadow 0.2s;
}

.next-action-card:hover {
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

.action-check {
  cursor: pointer;
}

.action-content {
  flex: 1;
}

.action-title {
  font-size: 14px;
  font-weight: 500;
  color: #18181B;
  margin-bottom: 4px;
}

.action-meta {
  display: flex;
  gap: 12px;
  font-size: 12px;
  color: #A1A1AA;
}

/* 顺序项目 */
.sequential-tasks {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.sequential-task {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px;
  background: #FAFAFA;
  border-radius: 8px;
  transition: all 0.2s;
}

.sequential-task.current {
  background: #EFF6FF;
  border-left: 3px solid #409EFF;
}

.sequential-task.blocked {
  opacity: 0.6;
}

.sequential-task.completed {
  opacity: 0.5;
}

.task-check {
  cursor: pointer;
}

.task-content {
  flex: 1;
  cursor: pointer;
}

.task-name {
  font-size: 14px;
  color: #18181B;
}

.blocked-hint {
  display: block;
  font-size: 12px;
  color: #A1A1AA;
  margin-top: 2px;
}

/* 轨道状态 */
.track-status {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 16px;
}

.track-card {
  border: 1px solid #E4E7ED;
  border-radius: 8px;
  overflow: hidden;
}

.track-header {
  padding: 12px;
  border-bottom: 2px solid;
  background: #FAFAFA;
}

.track-name {
  font-size: 14px;
  font-weight: 500;
  color: #18181B;
}

.track-body {
  padding: 16px;
}

.status-badge {
  display: inline-block;
  padding: 4px 12px;
  border-radius: 4px;
  font-size: 13px;
  font-weight: 500;
}

/* 关联资源 */
.resources-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 16px;
}

.resource-card {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 16px;
  background: #FAFAFA;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s;
}

.resource-card:hover {
  background: #F4F4F5;
}

.resource-info {
  display: flex;
  flex-direction: column;
}

.resource-count {
  font-size: 18px;
  font-weight: 600;
  color: #18181B;
}

.resource-label {
  font-size: 12px;
  color: #A1A1AA;
}

/* 动态轨迹 */
.timeline {
  position: relative;
  padding-left: 20px;
}

.timeline::before {
  content: '';
  position: absolute;
  left: 6px;
  top: 0;
  bottom: 0;
  width: 2px;
  background: #E4E7ED;
}

.timeline-item {
  position: relative;
  padding-bottom: 16px;
  padding-left: 16px;
}

.timeline-dot {
  position: absolute;
  left: -14px;
  top: 4px;
  width: 10px;
  height: 10px;
  border-radius: 50%;
}

.timeline-title {
  font-size: 13px;
  color: #18181B;
  margin-bottom: 2px;
}

.timeline-date {
  font-size: 12px;
  color: #A1A1AA;
}

.empty-timeline {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 40px;
  color: #A1A1AA;
}

.empty-timeline p {
  margin: 8px 0 0;
  font-size: 13px;
}

/* 新增样式 */
.clickable {
  cursor: pointer;
  transition: color 0.2s;
}

.clickable:hover {
  color: #409EFF;
  text-decoration: underline;
}

.complete-action-btn {
  font-weight: 500;
}

.sequential-progress {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 1;
  margin: 0 16px;
}

.progress-text {
  font-size: 12px;
  color: #6B7280;
  white-space: nowrap;
}

.context {
  color: #6B7280;
  background: #F3F4F6;
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 12px;
}
</style>
