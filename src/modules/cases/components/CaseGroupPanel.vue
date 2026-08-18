<script setup>
import { computed } from 'vue'
import { Loading } from '@element-plus/icons-vue'

const props = defineProps({
  cases: { type: Array, default: () => [] },
  groupBy: { type: String, default: 'none' },
  loading: { type: Boolean, default: false },
})

const emit = defineEmits(['rowClick', 'delete'])

const trackLabels = {
  patent_invalidation: '专利无效',
  admin_litigation: '行政诉讼',
  civil_tort: '民事侵权',
  other: '其他',
}

const routeLabels = {
  '民事诉讼': '民事诉讼',
  '专利无效': '专利无效',
  '行政诉讼': '行政诉讼',
  '民事诉讼+专利无效': '诉讼+无效',
  '专利无效+行政诉讼': '无效+行政',
  '三轨并行': '三轨并行',
}

// 分组后的案件列表
const groupedCases = computed(() => {
  const cases = props.cases
  if (props.groupBy === 'none') {
    return [{ key: 'all', label: '', cases, collapsed: false }]
  }
  if (props.groupBy === 'client') {
    return groupByField(cases, 'clientName', '未知客户')
  }
  if (props.groupBy === 'track') {
    return groupByField(cases, 'track', 'other').map((g) => ({
      ...g,
      label: trackLabels[g.key] || g.key,
    }))
  }
  if (props.groupBy === 'court') {
    return groupByField(cases, 'court', '未知法院')
  }
  if (props.groupBy === 'route') {
    return groupByField(cases, 'caseRoute', '民事诉讼').map((g) => ({
      ...g,
      label: routeLabels[g.key] || g.key,
    }))
  }
  return [{ key: 'all', label: '', cases, collapsed: false }]
})

function groupByField(cases, field, fallback) {
  const groups = {}
  for (const c of cases) {
    const key = c[field] || fallback
    if (!groups[key]) groups[key] = []
    groups[key].push(c)
  }
  return Object.entries(groups).map(([key, items]) => ({
    key,
    label: key,
    cases: items,
    collapsed: false,
  }))
}

// 行颜色：🔴 3天内到期, 🟡 14天内到期, ⬜ 已完结
function rowClassName({ row }) {
  if (row.caseStatus === '已完结') return 'case-row-closed'
  const urgency = row.deadlineUrgency || ''
  if (urgency === 'red') return 'case-row-urgent'
  if (urgency === 'yellow') return 'case-row-warning'
  return ''
}

// 期限状态图标
function deadlineIcon(row) {
  if (row.caseStatus === '已完结') return '⬜'
  const urgency = row.deadlineUrgency || ''
  if (urgency === 'red') return '🔴'
  if (urgency === 'yellow') return '🟡'
  return ''
}
</script>

<template>
  <div>
    <!-- 加载状态 -->
    <div v-if="loading" class="loading-state">
      <el-icon class="is-loading"><Loading /></el-icon>
      加载中...
    </div>

    <!-- 空状态 -->
    <div v-else-if="!cases.length" class="empty-state">
      <slot name="empty">
        <el-empty description="还没有案件" />
      </slot>
    </div>

    <!-- 案件列表 -->
    <div v-else class="case-groups">
      <div v-for="group in groupedCases" :key="group.key" class="case-group">
        <div v-if="group.label" class="group-header" @click="group.collapsed = !group.collapsed">
          <span class="group-toggle">{{ group.collapsed ? '▶' : '▼' }}</span>
          <span class="group-label">{{ group.label }}</span>
          <el-tag size="small" type="info">{{ group.cases.length }}件</el-tag>
        </div>
        <el-table
          v-show="!group.collapsed"
          :data="group.cases"
          :row-class-name="rowClassName"
          stripe
          size="small"
          @row-click="(row) => emit('rowClick', row)"
          style="cursor: pointer"
        >
          <el-table-column prop="caseName" label="案件名称" min-width="200" show-overflow-tooltip />
          <el-table-column prop="caseNo" label="案号" width="180" show-overflow-tooltip>
            <template #default="{ row }">
              {{ row.caseNo || '—' }}
            </template>
          </el-table-column>
          <el-table-column prop="clientName" label="客户" width="150" show-overflow-tooltip />
          <el-table-column prop="opponentName" label="对方" width="150" show-overflow-tooltip />
          <el-table-column prop="track" label="轨道" width="100">
            <template #default="{ row }">
              <el-tag size="small" :type="row.track === 'patent_invalidation' ? 'primary' : row.track === 'admin_litigation' ? 'warning' : 'success'">
                {{ trackLabels[row.track] || row.track }}
              </el-tag>
            </template>
          </el-table-column>
          <el-table-column prop="caseRoute" label="路由" width="120">
            <template #default="{ row }">
              <el-tag v-if="row.caseRoute" size="small" effect="plain">
                {{ routeLabels[row.caseRoute] || row.caseRoute }}
              </el-tag>
              <span v-else>—</span>
            </template>
          </el-table-column>
          <el-table-column prop="court" label="法院" width="150" show-overflow-tooltip>
            <template #default="{ row }">
              {{ row.court || '—' }}
            </template>
          </el-table-column>
          <el-table-column prop="caseStatus" label="状态" width="80">
            <template #default="{ row }">
              <el-tag v-if="row.caseStatus" size="small" :type="row.caseStatus === '已完结' ? 'info' : 'success'">
                {{ row.caseStatus }}
              </el-tag>
            </template>
          </el-table-column>
          <el-table-column prop="filingDate" label="立案日期" width="110">
            <template #default="{ row }">
              {{ row.filingDate || '—' }}
            </template>
          </el-table-column>
          <el-table-column label="期限" width="50" align="center">
            <template #default="{ row }">
              <span :title="deadlineIcon(row) === '🔴' ? '3天内到期' : deadlineIcon(row) === '🟡' ? '14天内到期' : deadlineIcon(row) === '⬜' ? '已完结' : '无紧急期限'">
                {{ deadlineIcon(row) }}
              </span>
            </template>
          </el-table-column>
          <el-table-column label="操作" width="60" fixed="right">
            <template #default="{ row }">
              <el-button size="small" text type="danger" @click.stop="emit('delete', row)">删除</el-button>
            </template>
          </el-table-column>
        </el-table>
      </div>
    </div>
  </div>
</template>

<style scoped>
.loading-state {
  text-align: center;
  padding: 40px;
  color: #666;
}

.empty-state {
  text-align: center;
  padding: 60px;
}

.case-groups {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.group-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background: #f5f7fa;
  border-radius: 4px;
  cursor: pointer;
  user-select: none;
}

.group-header:hover {
  background: #ecf5ff;
}

.group-toggle {
  font-size: 12px;
  color: #999;
}

.group-label {
  font-weight: 500;
}

.case-row-closed {
  opacity: 0.6;
}

.case-row-urgent {
  background-color: #fef0f0 !important;
}

.case-row-urgent:hover td {
  background-color: #fde2e2 !important;
}

.case-row-warning {
  background-color: #fdf6ec !important;
}

.case-row-warning:hover td {
  background-color: #faecd8 !important;
}
</style>
