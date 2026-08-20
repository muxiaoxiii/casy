<script setup>
import { ref, computed, onMounted } from 'vue'
import { casyContext } from '../../../core/plugin/context'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Refresh, Filter, View } from '@element-plus/icons-vue'

const loading = ref(false)
const decisions = ref([])
const selectedType = ref('')
const selectedStatus = ref('')
const showDetailDialog = ref(false)
const selectedDecision = ref(null)

// 决策类型选项
const typeOptions = [
  { value: '', label: '全部类型' },
  { value: 'appeal', label: '上诉' },
  { value: 'settle', label: '和解' },
  { value: 'accept', label: '受理' },
  { value: 'refuse', label: '拒绝' },
  { value: 'recommend_today', label: '推荐今日' },
  { value: 'recommend_priority', label: '推荐优先级' },
  { value: 'recommend_estimate', label: '推荐预估' },
  { value: 'recommend_schedule', label: '推荐排期' },
  { value: 'recommend_action', label: '推荐行动' },
  { value: 'recommend_followup', label: '推荐跟进' },
  { value: 'other', label: '其他' },
]

const statusOptions = [
  { value: '', label: '全部状态' },
  { value: 'proposed', label: '待确认' },
  { value: 'confirmed', label: '已确认' },
  { value: 'rejected', label: '已拒绝' },
  { value: 'voided', label: '已作废' },
]

const statusTagType = {
  proposed: 'warning',
  confirmed: 'success',
  rejected: 'danger',
  voided: 'info',
}

const statusLabels = {
  proposed: '待确认',
  confirmed: '已确认',
  rejected: '已拒绝',
  voided: '已作废',
}

// 确认等级颜色
const confirmLevelColors = {
  L1: '#67C23A',
  L2: '#E6A23C',
  L3: '#F56C6C',
}

const confirmLevelLabels = {
  L1: '可读确认',
  L2: '逐项确认',
  L3: '双人复核',
}

// 过滤后的数据
const filteredDecisions = computed(() => {
  let list = decisions.value
  if (selectedType.value) {
    list = list.filter(d => d.decisionType === selectedType.value)
  }
  if (selectedStatus.value) {
    list = list.filter(d => d.status === selectedStatus.value)
  }
  return list
})

// 统计
const stats = computed(() => ({
  total: decisions.value.length,
  proposed: decisions.value.filter(d => d.status === 'proposed').length,
  confirmed: decisions.value.filter(d => d.status === 'confirmed').length,
  rejected: decisions.value.filter(d => d.status === 'rejected').length,
}))

async function loadDecisions() {
  loading.value = true
  // 尝试调用后端命令，如果不存在则使用占位数据
  const result = await casyContext.ai.listDecisions({ limit: 200 })
  if (result.ok) {
    decisions.value = result.data || []
  } else {
    // 后端命令不存在，使用占位数据
    decisions.value = getPlaceholderData()
  }
  loading.value = false
}

function getPlaceholderData() {
  // 占位数据，展示 UI 结构
  return [
    {
      id: 'demo-001',
      entityType: 'case',
      entityId: 'case-001',
      decisionType: 'recommend_priority',
      decision: '建议将案件优先级提升为重要紧急',
      basis: JSON.stringify({ reason: '开庭日期临近', daysLeft: 5 }),
      aiAdvice: '根据案件时间线分析，距离开庭仅剩 5 天，建议优先处理',
      aiModel: 'gpt-4o-mini',
      status: 'proposed',
      createdAt: new Date().toISOString(),
    },
    {
      id: 'demo-002',
      entityType: 'task',
      entityId: 'task-001',
      decisionType: 'recommend_today',
      decision: '建议今日完成证据整理',
      basis: JSON.stringify({ reason: '举证期限将至' }),
      aiAdvice: '举证期限为明日，建议今日完成证据整理和提交',
      aiModel: 'gpt-4o-mini',
      status: 'confirmed',
      confirmedAt: new Date().toISOString(),
      createdAt: new Date(Date.now() - 86400000).toISOString(),
    },
  ]
}

function viewDetail(decision) {
  selectedDecision.value = decision
  showDetailDialog.value = true
}

function formatTime(timeStr) {
  if (!timeStr) return '-'
  const d = new Date(timeStr)
  return d.toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  })
}

function getTypeLabel(type) {
  const opt = typeOptions.find(o => o.value === type)
  return opt ? opt.label : type
}

function parseBasis(basis) {
  if (!basis) return null
  try {
    return typeof basis === 'string' ? JSON.parse(basis) : basis
  } catch {
    return null
  }
}

// ============================================================
// 待复核决策（get_pending_decision_reviews / mark_decision_reviewed / run_recursive_check）
// ============================================================
const pendingReviews = ref([])
const pendingLoading = ref(false)
const reviewBusyId = ref(null)              // 正在写回复核结果的决策 id
const recursiveCheckingId = ref(null)       // 正在递归核对的决策 id
const recursiveResults = ref({})            // id -> { consistent, gaps, source }

async function loadPendingReviews() {
  pendingLoading.value = true
  const result = await casyContext.ai.pendingDecisionReviews()
  if (result.ok) {
    pendingReviews.value = result.data || []
  }
  pendingLoading.value = false
}

/** 复核：仍有效 / 已失效（失效需填说明） */
async function markReviewed(item, stillValid) {
  let note = null
  if (!stillValid) {
    try {
      const { value } = await ElMessageBox.prompt(
        '请填写失效说明（将追加到决策依据，决策将被作废）',
        '标记已失效',
        {
          confirmButtonText: '确认作废',
          cancelButtonText: '取消',
          inputPlaceholder: '如：案件已和解，该决策不再适用',
          inputValidator: (v) => (v && v.trim() ? true : '请填写失效说明'),
        }
      )
      note = value.trim()
    } catch {
      return // 用户取消
    }
  }

  reviewBusyId.value = item.id
  const result = await casyContext.ai.markDecisionReviewed(item.id, stillValid, note)
  reviewBusyId.value = null

  if (result.ok) {
    ElMessage.success(stillValid ? '已标记为仍有效' : '决策已作废')
    pendingReviews.value = pendingReviews.value.filter(d => d.id !== item.id)
    delete recursiveResults.value[item.id]
    loadDecisions()
  } else {
    ElMessage.error(result.error || '复核操作失败')
  }
}

/** L3 递归核对：AI 核对决策与案件状态的一致性，失败时降级为规则核对 */
async function runRecursiveCheck(item) {
  recursiveCheckingId.value = item.id
  const result = await casyContext.ai.runRecursiveCheck(item.id)
  recursiveCheckingId.value = null

  if (result.ok && result.data) {
    recursiveResults.value = { ...recursiveResults.value, [item.id]: result.data }
    if (!result.data.consistent) {
      ElMessage.warning(`发现 ${result.data.gaps?.length || 0} 处不一致，请人工确认`)
    }
  } else {
    ElMessage.error(result.error || '递归核对失败，请稍后重试')
  }
}

function basisSummary(basis) {
  const parsed = parseBasis(basis)
  if (!parsed) return basis || ''
  if (parsed.reason) return String(parsed.reason)
  return JSON.stringify(parsed)
}

onMounted(() => {
  loadDecisions()
  loadPendingReviews()
})
</script>

<template>
  <div class="decisions-view">
    <!-- 统计卡片 -->
    <div class="stats-row">
      <el-card shadow="never" class="stat-card">
        <div class="stat-value">{{ stats.total }}</div>
        <div class="stat-label">总决策</div>
      </el-card>
      <el-card shadow="never" class="stat-card stat-warning">
        <div class="stat-value">{{ stats.proposed }}</div>
        <div class="stat-label">待确认</div>
      </el-card>
      <el-card shadow="never" class="stat-card stat-success">
        <div class="stat-value">{{ stats.confirmed }}</div>
        <div class="stat-label">已确认</div>
      </el-card>
      <el-card shadow="never" class="stat-card stat-danger">
        <div class="stat-value">{{ stats.rejected }}</div>
        <div class="stat-label">已拒绝</div>
      </el-card>
    </div>

    <!-- 待复核决策 -->
    <el-card v-if="pendingLoading || pendingReviews.length > 0" shadow="never" class="review-card" v-loading="pendingLoading">
      <template #header>
        <div class="review-header">
          <strong>待复核决策</strong>
          <el-tag v-if="pendingReviews.length > 0" type="warning" size="small">{{ pendingReviews.length }} 条到期</el-tag>
        </div>
      </template>

      <div v-for="item in pendingReviews" :key="item.id" class="review-item">
        <div class="review-main">
          <div class="review-title-row">
            <el-tag size="small" type="info">{{ item.entityType }}</el-tag>
            <span class="review-type">{{ getTypeLabel(item.decisionType) }}</span>
            <span class="review-due">复核到期：{{ item.reviewDue || '-' }}</span>
          </div>
          <div class="review-decision">{{ item.decision }}</div>
          <div v-if="item.basis" class="review-basis">依据：{{ basisSummary(item.basis) }}</div>

          <!-- 递归核对结果 -->
          <div v-if="recursiveResults[item.id]" class="recursive-result">
            <template v-if="recursiveResults[item.id].consistent">
              <span class="recursive-ok">核对一致（{{ recursiveResults[item.id].source === 'ai' ? 'AI' : '规则' }}核对）</span>
            </template>
            <template v-else>
              <span class="recursive-warn">
                发现缺口（{{ recursiveResults[item.id].source === 'ai' ? 'AI' : '规则' }}核对）：
              </span>
              <ul class="gap-list">
                <li v-for="(gap, i) in recursiveResults[item.id].gaps" :key="i">{{ gap }}</li>
              </ul>
            </template>
          </div>
        </div>

        <div class="review-actions">
          <el-button
            size="small"
            :loading="recursiveCheckingId === item.id"
            :disabled="reviewBusyId === item.id"
            @click="runRecursiveCheck(item)"
          >递归核对</el-button>
          <el-button
            size="small"
            type="success"
            :loading="reviewBusyId === item.id"
            :disabled="recursiveCheckingId === item.id"
            @click="markReviewed(item, true)"
          >仍有效</el-button>
          <el-button
            size="small"
            type="danger"
            :loading="reviewBusyId === item.id"
            :disabled="recursiveCheckingId === item.id"
            @click="markReviewed(item, false)"
          >已失效</el-button>
        </div>
      </div>
    </el-card>

    <!-- 筛选工具栏 -->
    <div class="filter-bar">
      <div class="filter-left">
        <el-icon><Filter /></el-icon>
        <el-select v-model="selectedType" size="small" style="width: 140px;">
          <el-option v-for="opt in typeOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
        </el-select>
        <el-select v-model="selectedStatus" size="small" style="width: 120px;">
          <el-option v-for="opt in statusOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
        </el-select>
      </div>
      <el-button :icon="Refresh" size="small" @click="loadDecisions">刷新</el-button>
    </div>

    <!-- 数据表格 -->
    <el-table
      :data="filteredDecisions"
      v-loading="loading"
      stripe
      size="small"
      style="width: 100%;"
      @row-click="viewDetail"
      row-class-name="clickable-row"
    >
      <el-table-column prop="entityType" label="实体类型" width="90">
        <template #default="{ row }">
          <el-tag size="small" type="info">{{ row.entityType }}</el-tag>
        </template>
      </el-table-column>
      <el-table-column prop="decisionType" label="决策类型" width="130">
        <template #default="{ row }">
          {{ getTypeLabel(row.decisionType) }}
        </template>
      </el-table-column>
      <el-table-column prop="decision" label="决策内容" min-width="200" show-overflow-tooltip />
      <el-table-column prop="status" label="状态" width="100">
        <template #default="{ row }">
          <el-tag :type="statusTagType[row.status]" size="small">
            {{ statusLabels[row.status] || row.status }}
          </el-tag>
        </template>
      </el-table-column>
      <el-table-column prop="aiModel" label="AI 模型" width="120" />
      <el-table-column prop="createdAt" label="创建时间" width="140">
        <template #default="{ row }">
          {{ formatTime(row.createdAt) }}
        </template>
      </el-table-column>
      <el-table-column label="操作" width="60" fixed="right">
        <template #default="{ row }">
          <el-button :icon="View" size="small" text @click.stop="viewDetail(row)" />
        </template>
      </el-table-column>
    </el-table>

    <!-- 空状态 -->
    <div v-if="!loading && filteredDecisions.length === 0" class="empty-state">
      <el-empty description="暂无决策记录" />
    </div>

    <!-- 详情对话框 -->
    <el-dialog v-model="showDetailDialog" title="决策详情" width="600">
      <div v-if="selectedDecision" class="detail-content">
        <el-descriptions :column="2" border size="small">
          <el-descriptions-item label="ID" :span="2">
            <span class="monospace">{{ selectedDecision.id }}</span>
          </el-descriptions-item>
          <el-descriptions-item label="实体类型">{{ selectedDecision.entityType }}</el-descriptions-item>
          <el-descriptions-item label="实体 ID">
            <span class="monospace">{{ selectedDecision.entityId }}</span>
          </el-descriptions-item>
          <el-descriptions-item label="决策类型">{{ getTypeLabel(selectedDecision.decisionType) }}</el-descriptions-item>
          <el-descriptions-item label="状态">
            <el-tag :type="statusTagType[selectedDecision.status]" size="small">
              {{ statusLabels[selectedDecision.status] || selectedDecision.status }}
            </el-tag>
          </el-descriptions-item>
          <el-descriptions-item label="决策内容" :span="2">
            {{ selectedDecision.decision }}
          </el-descriptions-item>
          <el-descriptions-item label="AI 建议" :span="2">
            {{ selectedDecision.aiAdvice || '-' }}
          </el-descriptions-item>
          <el-descriptions-item label="AI 模型">{{ selectedDecision.aiModel || '-' }}</el-descriptions-item>
          <el-descriptions-item label="创建时间">{{ formatTime(selectedDecision.createdAt) }}</el-descriptions-item>
        </el-descriptions>

        <!-- 决策依据 -->
        <div v-if="selectedDecision.basis" class="basis-section">
          <h4>决策依据</h4>
          <pre class="basis-json">{{ parseBasis(selectedDecision.basis) ? JSON.stringify(parseBasis(selectedDecision.basis), null, 2) : selectedDecision.basis }}</pre>
        </div>
      </div>
      <template #footer>
        <el-button @click="showDetailDialog = false">关闭</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.decisions-view {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.stats-row {
  display: flex;
  gap: 12px;
}

.stat-card {
  flex: 1;
  text-align: center;
  padding: 4px 0;
}

.stat-card :deep(.el-card__body) {
  padding: 12px;
}

.stat-value {
  font-size: 24px;
  font-weight: 600;
  color: #303133;
}

.stat-label {
  font-size: 12px;
  color: #909399;
  margin-top: 4px;
}

.stat-success .stat-value { color: #67C23A; }
.stat-danger .stat-value { color: #F56C6C; }
.stat-warning .stat-value { color: #E6A23C; }

.filter-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.filter-left {
  display: flex;
  align-items: center;
  gap: 8px;
  color: #909399;
}

.monospace {
  font-family: monospace;
  font-size: 12px;
}

.clickable-row {
  cursor: pointer;
}

.empty-state {
  padding: 40px 0;
}

.detail-content {
  max-height: 500px;
  overflow-y: auto;
}

.basis-section {
  margin-top: 16px;
}

.basis-section h4 {
  margin: 0 0 8px 0;
  font-size: 14px;
  color: #303133;
}

.basis-json {
  background: #F5F7FA;
  padding: 12px;
  border-radius: 4px;
  font-family: monospace;
  font-size: 12px;
  color: #606266;
  overflow-x: auto;
  margin: 0;
}

/* 待复核决策 */
.review-card :deep(.el-card__body) {
  padding: 8px 16px;
}

.review-header {
  display: flex;
  align-items: center;
  gap: 12px;
}

.review-item {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 16px;
  padding: 10px 0;
  border-bottom: 1px solid #F3F4F6;
}

.review-item:last-child {
  border-bottom: none;
}

.review-main {
  flex: 1;
  min-width: 0;
}

.review-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 4px;
}

.review-type {
  font-size: 12px;
  color: #909399;
}

.review-due {
  font-size: 12px;
  color: #B0823A;
}

.review-decision {
  font-size: 13px;
  color: #303133;
  line-height: 1.5;
}

.review-basis {
  font-size: 12px;
  color: #909399;
  margin-top: 2px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.recursive-result {
  margin-top: 6px;
  font-size: 12px;
  line-height: 1.6;
}

.recursive-ok {
  color: #4C8067;
}

.recursive-warn {
  color: #B0823A;
}

.gap-list {
  margin: 2px 0 0;
  padding-left: 18px;
  color: #B0823A;
}

.review-actions {
  display: flex;
  gap: 4px;
  flex-shrink: 0;
}

.review-actions .el-button + .el-button {
  margin-left: 0;
}
</style>
