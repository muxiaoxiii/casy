<script setup>
/**
 * KanbanView - 看板视图
 *
 * 按案件状态分列显示，支持拖拽切换阶段。
 * 列：待处理 → 证据交换 → 庭审准备 → 等待判决 → 已结案
 */
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { VueDraggable } from 'vue-draggable-plus'
import { useCasesStore } from '../../../stores/cases.js'
import { tauriCallSafe } from '../../../core/tauriBridge.js'
import { ElMessage } from 'element-plus'

const router = useRouter()
const casesStore = useCasesStore()
const loading = ref(true)

// 看板列定义
const columns = [
  {
    key: 'pending',
    title: '待处理',
    icon: '📋',
    color: '#909399',
    statuses: ['待补充意见', '中止'],
  },
  {
    key: 'evidence',
    title: '证据交换',
    icon: '📄',
    color: '#409eff',
    statuses: [],
  },
  {
    key: 'trial_prep',
    title: '庭审准备',
    icon: '⚖️',
    color: '#e6a23c',
    statuses: ['待开庭', '待口审'],
  },
  {
    key: 'verdict',
    title: '等待判决',
    icon: '🔨',
    color: '#f56c6c',
    statuses: ['待判决', '待无效决定'],
  },
  {
    key: 'closed',
    title: '已结案',
    icon: '✅',
    color: '#67c23a',
    statuses: ['胜诉', '败诉', '结案', '对方撤案', '撤诉'],
  },
]

// 每列的案件数据
const columnCases = ref({})

// 将案件分配到列
function assignCasesToColumns(allCases) {
  const result = {}
  for (const col of columns) {
    result[col.key] = []
  }

  for (const c of allCases) {
    const status = c.caseStatus || c.caseProgress || ''
    let placed = false
    for (const col of columns) {
      if (col.statuses.includes(status)) {
        result[col.key].push(c)
        placed = true
        break
      }
    }
    // 未匹配到任何列的，放入"待处理"
    if (!placed) {
      result.pending.push(c)
    }
  }

  columnCases.value = result
}

// 优先级标记
const priorityMap = {
  urgent_important: { label: '🔴', title: '重要紧急' },
  important: { label: '🟠', title: '重要' },
  urgent: { label: '🟡', title: '紧急' },
  normal: { label: '', title: '' },
}

function getPriority(caseItem) {
  return priorityMap[caseItem.priority] || priorityMap.normal
}

// 获取下个期限日期
function getNextDeadline(caseItem) {
  const dates = [
    caseItem.deadlineDate,
    caseItem.trialDate,
    caseItem.hearingDate,
  ].filter(Boolean).sort()
  return dates[0] || null
}

// 格式化日期
function formatDate(dateStr) {
  if (!dateStr) return ''
  const d = new Date(dateStr)
  const month = d.getMonth() + 1
  const day = d.getDate()
  return `${month}/${day}`
}

// 拖拽结束 → 更新案件状态
async function onDragChange(columnKey, evt) {
  const col = columns.find((c) => c.key === columnKey)
  if (!col) return

  // 找到被拖入的案件（added 事件）
  const added = evt.added
  if (!added?.element) return

  const caseItem = added.element
  const defaultStatus = col.statuses[0] || '待补充意见'

  // 更新案件状态
  const result = await tauriCallSafe('update_case', {
    id: caseItem.id,
    data: { caseStatus: defaultStatus, caseProgress: defaultStatus },
  })

  if (result.ok) {
    // 记录到 case_logs
    await tauriCallSafe('add_case_log', {
      caseId: caseItem.id,
      eventSummary: `看板拖拽: 状态变更为「${defaultStatus}」`,
      eventType: 'record',
      eventDate: new Date().toISOString().split('T')[0],
      content: `通过看板拖拽，从「${caseItem.caseStatus || '未分类'}」变更为「${defaultStatus}」`,
    })
    ElMessage.success(`案件状态已更新为「${defaultStatus}」`)
  }
}

function goToCase(caseItem) {
  router.push({ name: 'case-detail', params: { id: caseItem.id } })
}

onMounted(async () => {
  loading.value = true
  await casesStore.loadCases()
  assignCasesToColumns(casesStore.cases)
  loading.value = false
})
</script>

<template>
  <div class="kanban-page">
    <div class="kanban-header">
      <h2>案件看板</h2>
      <el-button text @click="router.push({ name: 'cases' })">← 返回列表</el-button>
    </div>

    <div v-if="loading" class="kanban-loading">加载中...</div>

    <div v-else class="kanban-board">
      <div
        v-for="col in columns"
        :key="col.key"
        class="kanban-column"
      >
        <div class="column-header" :style="{ borderTopColor: col.color }">
          <span class="column-icon">{{ col.icon }}</span>
          <span class="column-title">{{ col.title }}</span>
          <el-badge :value="columnCases[col.key]?.length || 0" :max="99" class="column-count" />
        </div>

        <VueDraggable
          v-model="columnCases[col.key]"
          group="kanban"
          item-key="id"
          class="column-body"
          ghost-class="kanban-ghost"
          drag-class="kanban-drag"
          :animation="200"
          @change="(evt) => onDragChange(col.key, evt)"
        >
          <div
            v-for="caseItem in columnCases[col.key]"
            :key="caseItem.id"
            class="kanban-card"
            @click="goToCase(caseItem)"
          >
            <div class="card-top">
              <span class="card-case-name">{{ caseItem.caseName || '未命名' }}</span>
              <span v-if="getPriority(caseItem).label" class="card-priority" :title="getPriority(caseItem).title">
                {{ getPriority(caseItem).label }}
              </span>
            </div>
            <div class="card-case-no">{{ caseItem.caseNo || '—' }}</div>
            <div class="card-parties">
              <span>{{ caseItem.clientName || '—' }}</span>
              <span class="vs">vs</span>
              <span>{{ caseItem.opponentName || '—' }}</span>
            </div>
            <div v-if="getNextDeadline(caseItem)" class="card-deadline">
              📅 {{ formatDate(getNextDeadline(caseItem)) }}
            </div>
            <div class="card-track" v-if="caseItem.track">
              <el-tag size="small" effect="plain" type="info">
                {{ { patent_invalidation: '专利无效', admin_litigation: '行政诉讼', civil_tort: '民事侵权', other: '其他' }[caseItem.track] || caseItem.track }}
              </el-tag>
            </div>
          </div>

          <div v-if="!columnCases[col.key]?.length" class="column-empty">
            拖拽案件到此处
          </div>
        </VueDraggable>
      </div>
    </div>
  </div>
</template>

<style scoped>
.kanban-page {
  height: calc(100vh - 80px);
  display: flex;
  flex-direction: column;
}

.kanban-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 16px;
  flex-shrink: 0;
}

.kanban-header h2 {
  margin: 0;
  font-size: 18px;
  font-weight: 500;
}

.kanban-loading {
  text-align: center;
  padding: 60px;
  color: #999;
}

.kanban-board {
  display: flex;
  gap: 12px;
  flex: 1;
  overflow-x: auto;
  overflow-y: hidden;
  padding-bottom: 8px;
}

.kanban-column {
  min-width: 240px;
  max-width: 280px;
  flex: 1;
  display: flex;
  flex-direction: column;
  background: #f5f7fa;
  border-radius: 8px;
  border-top: 3px solid transparent;
}

.column-header {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 12px;
  font-size: 13px;
  font-weight: 500;
  color: #303133;
  border-bottom: 1px solid #ebeef5;
}

.column-icon {
  font-size: 14px;
}

.column-title {
  flex: 1;
}

.column-count {
  flex-shrink: 0;
}

.column-body {
  flex: 1;
  overflow-y: auto;
  padding: 8px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-height: 100px;
}

.column-empty {
  text-align: center;
  color: #c0c4cc;
  font-size: 12px;
  padding: 32px 8px;
}

.kanban-card {
  background: #fff;
  border-radius: 6px;
  padding: 10px;
  cursor: pointer;
  transition: box-shadow 0.2s, transform 0.15s;
  border: 1px solid #ebeef5;
}

.kanban-card:hover {
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

.kanban-ghost {
  opacity: 0.4;
}

.kanban-drag {
  transform: rotate(2deg);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15);
}

.card-top {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 4px;
}

.card-case-name {
  font-size: 13px;
  font-weight: 500;
  color: #303133;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
}

.card-priority {
  flex-shrink: 0;
  font-size: 12px;
}

.card-case-no {
  font-size: 11px;
  color: #909399;
  margin-top: 4px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.card-parties {
  font-size: 11px;
  color: #606266;
  margin-top: 6px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.card-parties .vs {
  color: #c0c4cc;
  margin: 0 4px;
}

.card-deadline {
  font-size: 11px;
  color: #e6a23c;
  margin-top: 6px;
}

.card-track {
  margin-top: 6px;
}
</style>
