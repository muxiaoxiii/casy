<script setup>
import { onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useCasesStore } from '../../stores/cases.js'

const router = useRouter()
const casesStore = useCasesStore()

onMounted(async () => {
  await casesStore.loadDashboard()
})

const trackLabels = {
  patent_invalidation: '专利无效',
  admin_litigation: '行政诉讼',
  civil_tort: '民事侵权',
  other: '其他',
}

const activityIcons = {
  log: '📝',
  hearing: '📅',
  task: '📌',
}

const activityColors = {
  log: '#6b7280',
  hearing: '#3b82f6',
  task: '#8b5cf6',
}

function goToCases(filter) {
  if (filter) {
    casesStore.filter.track = filter
  }
  router.push({ name: 'cases' })
}

function goToCase(id) {
  router.push({ name: 'case-detail', params: { id } })
}

function urgencyClass(urgency) {
  if (urgency === 'red') return 'urgency-red'
  if (urgency === 'yellow') return 'urgency-yellow'
  return 'urgency-green'
}

function urgencyLabel(urgency) {
  if (urgency === 'red') return '紧急'
  if (urgency === 'yellow') return '即将到期'
  return '正常'
}
</script>

<template>
  <div class="dashboard">
    <!-- 顶部统计卡片 -->
    <div class="stat-cards">
      <el-card class="stat-card" shadow="hover" @click="goToCases(null)">
        <div class="stat-value">{{ casesStore.dashboard.activeCount }}</div>
        <div class="stat-label">活跃案件</div>
      </el-card>
      <el-card class="stat-card" shadow="hover" @click="goToCases(null)">
        <div class="stat-value total">{{ casesStore.dashboard.totalCount }}</div>
        <div class="stat-label">全部案件</div>
      </el-card>
      <el-card class="stat-card" shadow="hover" @click="goToCases(null)">
        <div class="stat-value closed">{{ casesStore.dashboard.closedCount }}</div>
        <div class="stat-label">已完结</div>
      </el-card>
    </div>

    <div class="dashboard-row">
      <!-- 期限预警 -->
      <el-card class="dashboard-card warnings-card">
        <template #header>
          <div class="card-header">
            <span>⚠️ 期限预警</span>
            <el-tag size="small" type="danger" v-if="casesStore.dashboard.deadlineWarnings?.length">
              {{ casesStore.dashboard.deadlineWarnings.length }} 项
            </el-tag>
          </div>
        </template>
        <div v-if="casesStore.dashboard.deadlineWarnings?.length" class="warnings-list">
          <div
            v-for="w in casesStore.dashboard.deadlineWarnings"
            :key="w.caseId + w.ruleName"
            class="warning-item"
            :class="urgencyClass(w.urgency)"
            @click="goToCase(w.caseId)"
          >
            <div class="warning-left">
              <span class="warning-urgency">{{ urgencyLabel(w.urgency) }}</span>
              <span class="warning-days">{{ w.daysLeft }}天</span>
            </div>
            <div class="warning-right">
              <div class="warning-rule">{{ w.ruleName }}</div>
              <div class="warning-meta">
                <span class="warning-case">{{ w.caseName }}</span>
                <span class="warning-date">截止 {{ w.dueDate }}</span>
              </div>
            </div>
          </div>
        </div>
        <el-empty v-else description="暂无期限预警" :image-size="60" />
      </el-card>

      <!-- 最近 7 天活动 -->
      <el-card class="dashboard-card activities-card">
        <template #header>
          <div class="card-header">
            <span>📋 最近 7 天活动</span>
          </div>
        </template>
        <div v-if="casesStore.dashboard.recentActivities?.length" class="activities-list">
          <div
            v-for="(act, idx) in casesStore.dashboard.recentActivities"
            :key="idx"
            class="activity-item"
            @click="act.caseId && goToCase(act.caseId)"
          >
            <span class="activity-icon" :style="{ color: activityColors[act.eventType] || '#999' }">
              {{ activityIcons[act.eventType] || '📄' }}
            </span>
            <div class="activity-content">
              <div class="activity-title">{{ act.title }}</div>
              <div class="activity-meta">
                <span>{{ act.caseName }}</span>
                <span class="activity-date">{{ act.eventDate }}</span>
              </div>
            </div>
          </div>
        </div>
        <el-empty v-else description="暂无最近活动" :image-size="60" />
      </el-card>
    </div>

    <!-- 案件分布 -->
    <el-card class="dashboard-card">
      <template #header>
        <div class="card-header">
          <span>📈 案件分布（按轨道）</span>
        </div>
      </template>
      <div v-if="casesStore.dashboard.byTrack?.length" class="distribution-list">
        <div
          v-for="[track, count] in casesStore.dashboard.byTrack"
          :key="track"
          class="distribution-item"
          @click="goToCases(track)"
        >
          <span class="dist-label">{{ trackLabels[track] || track }}</span>
          <div class="dist-bar-container">
            <div
              class="dist-bar"
              :style="{ width: `${(count / casesStore.dashboard.totalCount) * 100}%` }"
            />
          </div>
          <span class="dist-count">{{ count }}件</span>
        </div>
      </div>
      <el-empty v-else description="暂无数据" :image-size="60" />
    </el-card>

    <!-- 快捷操作 -->
    <el-card class="dashboard-card quick-actions">
      <template #header>
        <div class="card-header">
          <span>🚀 快捷操作</span>
        </div>
      </template>
      <div class="actions-grid">
        <el-button @click="router.push({ name: 'cases', query: { action: 'create' } })">
          ➕ 新建案件
        </el-button>
        <el-button @click="router.push({ name: 'cases' })">
          📋 查看案件
        </el-button>
        <el-button @click="router.push({ name: 'tasks' })">
          📌 任务管理
        </el-button>
        <el-button @click="router.push({ name: 'inbox' })">
          📥 收件箱
        </el-button>
        <el-button @click="router.push({ name: 'calendar' })">
          📅 日历
        </el-button>
        <el-button @click="router.push({ name: 'settings' })">
          ⚙️ 设置
        </el-button>
      </div>
    </el-card>
  </div>
</template>

<style scoped>
.dashboard {
  max-width: 1200px;
  margin: 0 auto;
}

.stat-cards {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 16px;
  margin-bottom: 20px;
}

.stat-card {
  cursor: pointer;
  text-align: center;
  transition: transform 0.2s;
}

.stat-card:hover {
  transform: translateY(-2px);
}

.stat-value {
  font-size: 36px;
  font-weight: bold;
  color: #67c23a;
}

.stat-value.total {
  color: #409eff;
}

.stat-value.closed {
  color: #909399;
}

.stat-label {
  color: #666;
  margin-top: 4px;
}

.dashboard-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
  margin-bottom: 20px;
}

.dashboard-card {
  margin-bottom: 20px;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-weight: 500;
}

/* 期限预警 */
.warnings-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-height: 400px;
  overflow-y: auto;
}

.warning-item {
  display: flex;
  gap: 12px;
  padding: 10px 12px;
  border-radius: 6px;
  cursor: pointer;
  transition: background 0.2s;
  border-left: 4px solid transparent;
}

.warning-item:hover {
  background: #f5f7fa;
}

.warning-item.urgency-red {
  border-left-color: #f56c6c;
  background: #fef0f0;
}

.warning-item.urgency-yellow {
  border-left-color: #e6a23c;
  background: #fdf6ec;
}

.warning-item.urgency-green {
  border-left-color: #67c23a;
  background: #f0f9eb;
}

.warning-left {
  display: flex;
  flex-direction: column;
  align-items: center;
  min-width: 60px;
  gap: 2px;
}

.warning-urgency {
  font-size: 11px;
  padding: 1px 6px;
  border-radius: 4px;
  color: #fff;
  font-weight: 500;
}

.urgency-red .warning-urgency {
  background: #f56c6c;
}

.urgency-yellow .warning-urgency {
  background: #e6a23c;
}

.urgency-green .warning-urgency {
  background: #67c23a;
}

.warning-days {
  font-size: 18px;
  font-weight: bold;
  color: #333;
}

.warning-right {
  flex: 1;
  min-width: 0;
}

.warning-rule {
  font-size: 13px;
  font-weight: 500;
}

.warning-meta {
  display: flex;
  gap: 12px;
  margin-top: 4px;
  font-size: 12px;
  color: #999;
}

.warning-case {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 最近活动 */
.activities-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 400px;
  overflow-y: auto;
}

.activity-item {
  display: flex;
  gap: 10px;
  padding: 8px;
  border-radius: 6px;
  cursor: pointer;
  transition: background 0.2s;
}

.activity-item:hover {
  background: #f5f7fa;
}

.activity-icon {
  font-size: 16px;
  flex-shrink: 0;
  width: 24px;
  text-align: center;
}

.activity-content {
  flex: 1;
  min-width: 0;
}

.activity-title {
  font-size: 13px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.activity-meta {
  display: flex;
  justify-content: space-between;
  margin-top: 2px;
  font-size: 12px;
  color: #999;
}

.activity-date {
  flex-shrink: 0;
}

/* 分布 */
.distribution-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.distribution-item {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  padding: 4px 0;
}

.distribution-item:hover {
  opacity: 0.8;
}

.dist-label {
  width: 80px;
  font-size: 13px;
  text-align: right;
  flex-shrink: 0;
}

.dist-bar-container {
  flex: 1;
  height: 20px;
  background: #f0f0f0;
  border-radius: 4px;
  overflow: hidden;
}

.dist-bar {
  height: 100%;
  background: #409eff;
  border-radius: 4px;
  transition: width 0.3s;
}

.dist-count {
  width: 40px;
  font-size: 13px;
  color: #666;
  flex-shrink: 0;
}

/* 快捷操作 */
.quick-actions .actions-grid {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
}

.quick-actions .el-button {
  flex: 0 0 auto;
}
</style>
