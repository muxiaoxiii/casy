<script setup lang="ts">
/**
 * KanbanView - 看板视图
 *
 * 按案件轨道路由动态渲染列，支持拖拽切换阶段。
 * 根据 case_route 自动切换：民事诉讼看板 / 专利无效看板 / 行政诉讼看板
 */
import { ref, computed, onMounted, watch, type Component } from 'vue'
import { useRouter } from 'vue-router'
import { VueDraggable } from 'vue-draggable-plus'
import {
  List,
  Document,
  ScaleToOriginal,
  SwitchButton,
  CircleCheck,
} from '@element-plus/icons-vue'
import { useCasesStore } from '../../../stores/cases'
import { casyContext } from '../../../core/plugin/context'
import { ElMessage } from 'element-plus'
import type { Case, CaseRoute, CivilStatus, InvalidationStatus, AdminStatus } from '../../../types'
import {
  CIVIL_STATUS_LABELS,
  INVALIDATION_STATUS_LABELS,
  ADMIN_STATUS_LABELS,
  CASE_ROUTE_LABELS,
} from '../../../types'

const router = useRouter()
const casesStore = useCasesStore()
const loading = ref(true)
const activeRoute = ref<CaseRoute>('民事诉讼')

// ==================== 看板列定义 ====================

interface KanbanColumn {
  key: string
  title: string
  icon: Component
  color: string
  statuses: string[]
}

/** 民事诉讼看板（5 列） */
const civilColumns: KanbanColumn[] = [
  { key: 'intake', title: '待办', icon: List, color: '#6b7280', statuses: ['intake', 'filed'] },
  { key: 'pre_trial', title: '庭前', icon: Document, color: '#3b82f6', statuses: ['pre_hearing', 'in_trial', 'awaiting_verdict'] },
  { key: 'special', title: '特殊', icon: ScaleToOriginal, color: '#f59e0b', statuses: ['settled', 'appeal_period', 'suspended'] },
  { key: 'appeal', title: '上诉/再审', icon: SwitchButton, color: '#ef4444', statuses: ['second_instance', 'second_verdict', 'retrial'] },
  { key: 'closed', title: '结案', icon: CircleCheck, color: '#10b981', statuses: ['enforcement', 'closed'] },
]

/** 专利无效看板（3 列） */
const invalidationColumns: KanbanColumn[] = [
  { key: 'preparing', title: '待办', icon: List, color: '#6b7280', statuses: ['preparing', 'filed'] },
  { key: 'review', title: '审理', icon: ScaleToOriginal, color: '#3b82f6', statuses: ['pre_oral', 'oral_done', 'awaiting_decision'] },
  { key: 'decided', title: '已决', icon: CircleCheck, color: '#10b981', statuses: ['decision_issued'] },
]

/** 行政诉讼看板（3 列） */
const adminColumns: KanbanColumn[] = [
  { key: 'first', title: '一审', icon: Document, color: '#3b82f6', statuses: ['filed', 'pre_hearing', 'in_trial', 'awaiting_verdict', 'verdict_issued'] },
  { key: 'second', title: '二审', icon: SwitchButton, color: '#ef4444', statuses: ['second_instance'] },
  { key: 'closed', title: '结案', icon: CircleCheck, color: '#10b981', statuses: ['closed'] },
]

/** 当前激活的列定义 */
const activeColumns = computed<KanbanColumn[]>(() => {
  switch (activeRoute.value) {
    case '专利无效':
    case '专利无效+行政诉讼':
      return invalidationColumns
    case '行政诉讼':
      return adminColumns
    default:
      return civilColumns
  }
})

// 轨道选项（只显示有案件的轨道）
const routeOptions = computed(() => {
  const routes = new Set<CaseRoute>()
  for (const c of casesStore.cases) {
    if (c.caseRoute) routes.add(c.caseRoute)
  }
  // 至少显示民事诉讼
  routes.add('民事诉讼')
  return Array.from(routes)
})

// 每列的案件数据
const columnCases = ref<Record<string, Case[]>>({})

// 将案件分配到列
function assignCasesToColumns(allCases: Case[]) {
  const result: Record<string, Case[]> = {}
  for (const col of activeColumns.value) {
    result[col.key] = []
  }

  // 根据当前激活轨道筛选案件
  const filtered = allCases.filter((c) => {
    const route = c.caseRoute || '民事诉讼'
    switch (activeRoute.value) {
      case '民事诉讼':
        return route.includes('民事诉讼')
      case '专利无效':
        return route.includes('专利无效')
      case '行政诉讼':
        return route.includes('行政诉讼')
      case '民事诉讼+专利无效':
        return route === '民事诉讼+专利无效' || route === '三轨并行'
      case '专利无效+行政诉讼':
        return route === '专利无效+行政诉讼' || route === '三轨并行'
      default:
        return true
    }
  })

  for (const c of filtered) {
    // 根据当前轨道取对应状态
    let status: string | null = null
    switch (activeRoute.value) {
      case '专利无效':
      case '专利无效+行政诉讼':
        status = c.invalidationStatus
        break
      case '行政诉讼':
        status = c.adminStatus
        break
      default:
        status = c.civilStatus
    }

    let placed = false
    for (const col of activeColumns.value) {
      if (status && col.statuses.includes(status)) {
        result[col.key].push(c)
        placed = true
        break
      }
    }
    if (!placed) {
      // 未匹配的放入第一列
      const firstKey = activeColumns.value[0]?.key
      if (firstKey) result[firstKey].push(c)
    }
  }

  columnCases.value = result
}

// 格式化日期
function formatDate(dateStr: string | null): string {
  if (!dateStr) return ''
  const d = new Date(dateStr)
  const month = d.getMonth() + 1
  const day = d.getDate()
  return `${month}/${day}`
}

// 获取当前轨道的状态标签
function getStatusLabel(c: Case): string {
  switch (activeRoute.value) {
    case '专利无效':
    case '专利无效+行政诉讼':
      return c.invalidationStatus ? INVALIDATION_STATUS_LABELS[c.invalidationStatus] : ''
    case '行政诉讼':
      return c.adminStatus ? ADMIN_STATUS_LABELS[c.adminStatus] : ''
    default:
      return c.civilStatus ? CIVIL_STATUS_LABELS[c.civilStatus] : ''
  }
}

// 获取当前轨道的状态字段名（用于拖拽更新）
function getStatusFieldName(): string {
  switch (activeRoute.value) {
    case '专利无效':
    case '专利无效+行政诉讼':
      return 'invalidationStatus'
    case '行政诉讼':
      return 'adminStatus'
    default:
      return 'civilStatus'
  }
}

// 拖拽结束 → 更新案件状态
async function onDragChange(columnKey: string, evt: Record<string, unknown>) {
  const col = activeColumns.value.find((c) => c.key === columnKey)
  if (!col) return

  const added = evt.added as { element?: Case } | undefined
  if (!added?.element) return

  const caseItem = added.element
  const newStatus = col.statuses[0]
  if (!newStatus) return

  const statusField = getStatusFieldName()

  // 更新案件状态
  const result = await casyContext.cases.update(caseItem.id, { [statusField]: newStatus })

  if (result.ok) {
    // 记录到 case_track_history
    await casyContext.cases.addLog({
      caseId: caseItem.id,
      eventSummary: `看板拖拽: ${CASE_ROUTE_LABELS[activeRoute.value]}状态变更为「${getStatusLabel(caseItem)}」`,
      eventType: 'record',
      eventDate: new Date().toISOString().split('T')[0],
      content: `通过看板拖拽，${statusField} 从「${(caseItem as unknown as Record<string, unknown>)[statusField] || '未分类'}」变更为「${newStatus}」`,
    })
    ElMessage.success(`案件状态已更新`)
    // 重新加载
    await casesStore.loadCases()
    assignCasesToColumns(casesStore.cases)
  }
}

function goToCase(caseItem: Case) {
  router.push({ name: 'case-detail', params: { id: caseItem.id } })
}

// 切换轨道时重新分配
watch(activeRoute, () => {
  assignCasesToColumns(casesStore.cases)
})

onMounted(async () => {
  loading.value = true
  await casesStore.loadCases()
  // 默认显示有案件的轨道
  if (routeOptions.value.length > 0) {
    activeRoute.value = routeOptions.value[0]
  }
  assignCasesToColumns(casesStore.cases)
  loading.value = false
})
</script>

<template>
  <div class="kanban-page">
    <div class="kanban-header">
      <h2>案件看板</h2>
      <div class="kanban-controls">
        <el-segmented
          v-model="activeRoute"
          :options="routeOptions"
          size="small"
        />
        <el-button text @click="router.push({ name: 'cases' })">← 返回列表</el-button>
      </div>
    </div>

    <div v-if="loading" class="kanban-loading">加载中...</div>

    <div v-else class="kanban-board">
      <div
        v-for="col in activeColumns"
        :key="col.key"
        class="kanban-column"
      >
        <div class="column-header" :style="{ borderTopColor: col.color }">
          <el-icon class="column-icon" :color="col.color" :size="15">
            <component :is="col.icon" />
          </el-icon>
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
          @change="(evt: any) => onDragChange(col.key, evt)"
        >
          <div
            v-for="caseItem in columnCases[col.key]"
            :key="caseItem.id"
            class="kanban-card"
            @click="goToCase(caseItem)"
          >
            <div class="card-top">
              <span class="card-case-name">{{ caseItem.caseName || '未命名' }}</span>
              <el-tag v-if="getStatusLabel(caseItem)" size="small" effect="plain" type="info">
                {{ getStatusLabel(caseItem) }}
              </el-tag>
            </div>
            <div class="card-case-no">{{ caseItem.caseNo || '—' }}</div>
            <div class="card-parties">
              <span>{{ caseItem.clientName || '—' }}</span>
              <span class="vs">vs</span>
              <span>{{ caseItem.opponentName || '—' }}</span>
            </div>
            <div class="card-route">
              <el-tag size="small" effect="plain">
                {{ CASE_ROUTE_LABELS[caseItem.caseRoute] || caseItem.caseRoute || '民事诉讼' }}
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

.kanban-controls {
  display: flex;
  align-items: center;
  gap: 12px;
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

.card-route {
  margin-top: 6px;
}
</style>
