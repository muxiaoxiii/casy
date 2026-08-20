<script setup>
import { ref, computed, onMounted } from 'vue'
import { casyContext } from '../../../core/plugin/context'
import { ElMessage } from 'element-plus'
import { Refresh, View, Filter } from '@element-plus/icons-vue'

const loading = ref(false)
const aiRuns = ref([])
const selectedPurpose = ref('')
const selectedStatus = ref('')
const showDetailDialog = ref(false)
const selectedRun = ref(null)

// 筛选选项
const purposeOptions = [
  { value: '', label: '全部用途' },
  { value: 'document_classify', label: '文档分类' },
  { value: 'info_extract', label: '信息提取' },
  { value: 'summary', label: '摘要生成' },
  { value: 'writing_suggestion', label: '写作建议' },
  { value: 'inbox_analyze', label: '收件箱分析' },
]

const statusOptions = [
  { value: '', label: '全部状态' },
  { value: 'completed', label: '已完成' },
  { value: 'failed', label: '失败' },
  { value: 'running', label: '运行中' },
  { value: 'pending', label: '等待中' },
]

const statusTagType = {
  completed: 'success',
  failed: 'danger',
  running: 'warning',
  pending: 'info',
}

const statusLabels = {
  completed: '已完成',
  failed: '失败',
  running: '运行中',
  pending: '等待中',
}

// 过滤后的数据
const filteredRuns = computed(() => {
  let runs = aiRuns.value
  if (selectedPurpose.value) {
    runs = runs.filter(r => r.purpose === selectedPurpose.value)
  }
  if (selectedStatus.value) {
    runs = runs.filter(r => r.status === selectedStatus.value)
  }
  return runs
})

// 统计
const stats = computed(() => ({
  total: aiRuns.value.length,
  completed: aiRuns.value.filter(r => r.status === 'completed').length,
  failed: aiRuns.value.filter(r => r.status === 'failed').length,
  running: aiRuns.value.filter(r => r.status === 'running').length,
}))

async function loadRuns() {
  loading.value = true
  const result = await casyContext.ai.runHistory(200)
  if (result.ok) {
    aiRuns.value = result.data || []
  } else {
    ElMessage.error(result.error || '加载失败')
  }
  loading.value = false
}

function viewDetail(run) {
  selectedRun.value = run
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
    second: '2-digit',
  })
}

function getPurposeLabel(purpose) {
  const opt = purposeOptions.find(o => o.value === purpose)
  return opt ? opt.label : purpose
}

function truncateHash(hash) {
  if (!hash) return '-'
  return hash.length > 16 ? hash.substring(0, 16) + '...' : hash
}

onMounted(() => {
  loadRuns()
})
</script>

<template>
  <div class="audit-view">
    <!-- 统计卡片 -->
    <div class="stats-row">
      <el-card shadow="never" class="stat-card">
        <div class="stat-value">{{ stats.total }}</div>
        <div class="stat-label">总调用</div>
      </el-card>
      <el-card shadow="never" class="stat-card stat-success">
        <div class="stat-value">{{ stats.completed }}</div>
        <div class="stat-label">成功</div>
      </el-card>
      <el-card shadow="never" class="stat-card stat-danger">
        <div class="stat-value">{{ stats.failed }}</div>
        <div class="stat-label">失败</div>
      </el-card>
      <el-card shadow="never" class="stat-card stat-warning">
        <div class="stat-value">{{ stats.running }}</div>
        <div class="stat-label">运行中</div>
      </el-card>
    </div>

    <!-- 筛选工具栏 -->
    <div class="filter-bar">
      <div class="filter-left">
        <el-icon><Filter /></el-icon>
        <el-select v-model="selectedPurpose" size="small" style="width: 140px;">
          <el-option v-for="opt in purposeOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
        </el-select>
        <el-select v-model="selectedStatus" size="small" style="width: 120px;">
          <el-option v-for="opt in statusOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
        </el-select>
      </div>
      <el-button :icon="Refresh" size="small" @click="loadRuns">刷新</el-button>
    </div>

    <!-- 数据表格 -->
    <el-table
      :data="filteredRuns"
      v-loading="loading"
      stripe
      size="small"
      style="width: 100%;"
      @row-click="viewDetail"
      row-class-name="clickable-row"
    >
      <el-table-column prop="provider" label="提供商" width="100" />
      <el-table-column prop="model" label="模型" width="150" />
      <el-table-column prop="purpose" label="用途" width="130">
        <template #default="{ row }">
          {{ getPurposeLabel(row.purpose) }}
        </template>
      </el-table-column>
      <el-table-column prop="status" label="状态" width="100">
        <template #default="{ row }">
          <el-tag :type="statusTagType[row.status]" size="small">
            {{ statusLabels[row.status] || row.status }}
          </el-tag>
        </template>
      </el-table-column>
      <el-table-column prop="inputHash" label="输入哈希" width="140">
        <template #default="{ row }">
          <span class="hash-text">{{ truncateHash(row.inputHash) }}</span>
        </template>
      </el-table-column>
      <el-table-column prop="outputHash" label="输出哈希" width="140">
        <template #default="{ row }">
          <span class="hash-text">{{ truncateHash(row.outputHash) }}</span>
        </template>
      </el-table-column>
      <el-table-column prop="createdAt" label="创建时间" width="150">
        <template #default="{ row }">
          {{ formatTime(row.createdAt) }}
        </template>
      </el-table-column>
      <el-table-column prop="completedAt" label="完成时间" width="150">
        <template #default="{ row }">
          {{ formatTime(row.completedAt) }}
        </template>
      </el-table-column>
      <el-table-column label="操作" width="60" fixed="right">
        <template #default="{ row }">
          <el-button :icon="View" size="small" text @click.stop="viewDetail(row)" />
        </template>
      </el-table-column>
    </el-table>

    <!-- 空状态 -->
    <div v-if="!loading && filteredRuns.length === 0" class="empty-state">
      <el-empty description="暂无 AI 调用记录" />
    </div>

    <!-- 详情对话框 -->
    <el-dialog v-model="showDetailDialog" title="AI 调用详情" width="560">
      <div v-if="selectedRun" class="detail-content">
        <el-descriptions :column="2" border size="small">
          <el-descriptions-item label="ID" :span="2">
            <span class="monospace">{{ selectedRun.id }}</span>
          </el-descriptions-item>
          <el-descriptions-item label="提供商">{{ selectedRun.provider }}</el-descriptions-item>
          <el-descriptions-item label="模型">{{ selectedRun.model }}</el-descriptions-item>
          <el-descriptions-item label="用途">{{ getPurposeLabel(selectedRun.purpose) }}</el-descriptions-item>
          <el-descriptions-item label="状态">
            <el-tag :type="statusTagType[selectedRun.status]" size="small">
              {{ statusLabels[selectedRun.status] || selectedRun.status }}
            </el-tag>
          </el-descriptions-item>
          <el-descriptions-item label="输入哈希" :span="2">
            <span class="monospace">{{ selectedRun.inputHash || '-' }}</span>
          </el-descriptions-item>
          <el-descriptions-item label="输出哈希" :span="2">
            <span class="monospace">{{ selectedRun.outputHash || '-' }}</span>
          </el-descriptions-item>
          <el-descriptions-item label="创建时间">{{ formatTime(selectedRun.createdAt) }}</el-descriptions-item>
          <el-descriptions-item label="完成时间">{{ formatTime(selectedRun.completedAt) }}</el-descriptions-item>
        </el-descriptions>
      </div>
      <template #footer>
        <el-button @click="showDetailDialog = false">关闭</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.audit-view {
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

.hash-text {
  font-family: monospace;
  font-size: 12px;
  color: #606266;
}

.monospace {
  font-family: monospace;
  font-size: 12px;
  word-break: break-all;
}

.clickable-row {
  cursor: pointer;
}

.empty-state {
  padding: 40px 0;
}

.detail-content {
  max-height: 400px;
  overflow-y: auto;
}
</style>
