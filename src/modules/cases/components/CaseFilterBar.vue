<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { ArrowDown, Download, Search, Filter, FolderChecked, Plus } from '../../../shared/icons'
import { ElMessage } from 'element-plus'
import { casyContext } from '../../../core/plugin/context'
import { toLocalISODate } from '../../../shared/utils/date'
import { useFiltersStore, type SavedFilter } from '../../../stores/filters'
import type { CaseFilter, CaseRoute, CivilStatus, InvalidationStatus, AdminStatus, TrackType, CaseStatus } from '../../../types'
import {
  CIVIL_STATUS_LABELS,
  INVALIDATION_STATUS_LABELS,
  ADMIN_STATUS_LABELS,
  CASE_ROUTE_LABELS,
} from '../../../types'

interface Props {
  filter: CaseFilter
  groupBy?: string
  total?: number
  exporting?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  groupBy: 'none',
  total: 0,
  exporting: false,
})

const emit = defineEmits([
  'update:filter',
  'update:groupBy',
  'search',
  'export',
  'create',
])

const trackOptions = [
  { value: 'patent_invalidation', label: '专利无效' },
  { value: 'admin_litigation', label: '行政诉讼' },
  { value: 'civil_tort', label: '民事侵权' },
  { value: 'other', label: '其他' },
]

const statusOptions = [
  { value: '', label: '全部状态' },
  { value: '进行中', label: '进行中' },
  { value: '已完结', label: '已完结' },
]

// 新状态机：轨道路由选项
const routeOptions = Object.entries(CASE_ROUTE_LABELS).map(([value, label]) => ({ value, label }))

// 新状态机：各轨状态选项
const civilStatusOptions = Object.entries(CIVIL_STATUS_LABELS).map(([value, label]) => ({ value, label }))
const invalidationStatusOptions = Object.entries(INVALIDATION_STATUS_LABELS).map(([value, label]) => ({ value, label }))
const adminStatusOptions = Object.entries(ADMIN_STATUS_LABELS).map(([value, label]) => ({ value, label }))

const sortOptions = [
  { value: 'filing_date', label: '立案日期' },
  { value: 'updated_at', label: '最近更新' },
  { value: 'case_name', label: '案件名称' },
  { value: 'client_name', label: '客户名称' },
]

// 客户搜索
const clientSearchQuery = ref('')
const clientOptions = ref<Array<{ value: string; label: string }>>([])
const clientLoading = ref(false)

// 日期范围
const dateRange = ref<string[]>([])

// 跨类型筛选
const showAdvancedFilter = ref(false)
const deadlineRange = ref<string[]>([])
const hearingRange = ref<string[]>([])
const operatorFilter = ref('')

// 期限快捷选项
const deadlineQuickOptions = [
  { label: '不限', value: '' },
  { label: '今天到期', value: 'today' },
  { label: '3天内到期', value: '3d' },
  { label: '7天内到期', value: '7d' },
  { label: '30天内到期', value: '30d' },
  { label: '已逾期', value: 'overdue' },
]
const deadlineQuick = ref('')

// 保存的筛选方案（后端持久化，设计哲学 §9）
const filtersStore = useFiltersStore()
const showSaveFilterDialog = ref(false)
const savedFilterName = ref('')
const savedFilters = computed(() => filtersStore.filters)

// 搜索防抖
let searchTimer: ReturnType<typeof setTimeout> | null = null
function onSearchInput() {
  if (searchTimer) clearTimeout(searchTimer)
  searchTimer = setTimeout(() => emit('search'), 300)
}

function updateTrack(val: TrackType | null) {
  emit('update:filter', { ...props.filter, track: val || null })
}

function updateStatus(val: CaseStatus | null) {
  emit('update:filter', { ...props.filter, status: val || null })
}

function updateRoute(val: CaseRoute | null) {
  emit('update:filter', { ...props.filter, caseRoute: val || null })
}

function updateCivilStatus(val: CivilStatus | null) {
  emit('update:filter', { ...props.filter, civilStatus: val || null })
}

function updateInvalidationStatus(val: InvalidationStatus | null) {
  emit('update:filter', { ...props.filter, invalidationStatus: val || null })
}

function updateAdminStatus(val: AdminStatus | null) {
  emit('update:filter', { ...props.filter, adminStatus: val || null })
}

function updateSortBy(val: string) {
  emit('update:filter', { ...props.filter, sortBy: val })
}

function updateSearch(val: string) {
  emit('update:filter', { ...props.filter, search: val })
}

function updateClient(val: string | null) {
  emit('update:filter', { ...props.filter, client: val || null })
}

// 日期范围变化
function onDateRangeChange(val: string[] | null) {
  if (val && val.length === 2) {
    emit('update:filter', {
      ...props.filter,
      dateFrom: val[0],
      dateTo: val[1],
    })
  } else {
    emit('update:filter', {
      ...props.filter,
      dateFrom: null,
      dateTo: null,
    })
  }
}

// 期限快捷选择
function onDeadlineQuickChange(val: string) {
  const today = new Date()
  // 时区（审查 P1-2）：原本用 toISOString()（UTC）取日期，东八区凌晨窗口内
  // "逾期"快捷筛选/今天/3/7/30 天上界会偏一天。改用本地日期序列化。
  const fmt = (d: Date) => toLocalISODate(d)

  if (!val) {
    deadlineRange.value = []
    emit('update:filter', { ...props.filter, deadlineFrom: null, deadlineTo: null })
    return
  }

  if (val === 'overdue') {
    emit('update:filter', {
      ...props.filter,
      deadlineFrom: null,
      deadlineTo: fmt(today),
    })
    deadlineRange.value = []
    return
  }

  let days = 0
  if (val === 'today') days = 0
  else if (val === '3d') days = 3
  else if (val === '7d') days = 7
  else if (val === '30d') days = 30

  const to = new Date(today)
  to.setDate(to.getDate() + days)
  emit('update:filter', {
    ...props.filter,
    deadlineFrom: fmt(today),
    deadlineTo: fmt(to),
  })
  deadlineRange.value = [fmt(today), fmt(to)]
}

// 期限范围变化
function onDeadlineRangeChange(val: string[] | null) {
  deadlineQuick.value = ''
  if (val && val.length === 2) {
    emit('update:filter', { ...props.filter, deadlineFrom: val[0], deadlineTo: val[1] })
  } else {
    emit('update:filter', { ...props.filter, deadlineFrom: null, deadlineTo: null })
  }
}

// 开庭日期范围变化
function onHearingRangeChange(val: string[] | null) {
  if (val && val.length === 2) {
    emit('update:filter', { ...props.filter, hearingFrom: val[0], hearingTo: val[1] })
  } else {
    emit('update:filter', { ...props.filter, hearingFrom: null, hearingTo: null })
  }
}

// 办案人变化
function onOperatorChange(val: string) {
  emit('update:filter', { ...props.filter, operator: val || null })
}

// 远程搜索客户
async function remoteClientSearch(query: string) {
  if (!query) {
    clientOptions.value = []
    return
  }
  clientLoading.value = true
  const result = await casyContext.cases.search(query)
  if (result.ok && result.data) {
    // 从搜索结果中提取唯一的客户名
    const clients = [...new Set(result.data.map((c) => c.clientName).filter(Boolean))]
    clientOptions.value = clients.map((name) => ({ value: name, label: name }))
  }
  clientLoading.value = false
}

// 保存当前筛选方案（存后端 saved_filters，entity_type='cases'）
async function saveFilter() {
  if (!savedFilterName.value.trim()) return
  const result = await filtersStore.saveFilter({
    module: 'cases',
    name: savedFilterName.value.trim(),
    filter: { ...props.filter },
    groupBy: props.groupBy,
  })
  if (result.ok) {
    ElMessage.success('筛选方案已保存')
    showSaveFilterDialog.value = false
    savedFilterName.value = ''
  } else {
    ElMessage.error(result.error || '保存失败')
  }
}

// 加载已保存的筛选方案
function loadFilter(filterConfig: SavedFilter) {
  emit('update:filter', { ...filterConfig.filter })
  emit('update:groupBy', filterConfig.groupBy || 'none')
  emit('search')
}

// 删除已保存的筛选方案
async function deleteFilter(id: string) {
  const result = await filtersStore.deleteFilter(id)
  if (result.ok) {
    ElMessage.success('已删除')
  } else {
    ElMessage.error(result.error || '删除失败')
  }
}

// 清除所有筛选
function clearFilters() {
  emit('update:filter', {
    track: null,
    client: null,
    court: null,
    status: null,
    search: '',
    sortBy: 'filing_date',
    dateFrom: null,
    dateTo: null,
    deadlineFrom: null,
    deadlineTo: null,
    hearingFrom: null,
    hearingTo: null,
    operator: null,
    // 新状态机筛选
    civilStatus: null,
    invalidationStatus: null,
    adminStatus: null,
    caseRoute: null,
  })
  dateRange.value = []
  deadlineRange.value = []
  hearingRange.value = []
  deadlineQuick.value = ''
  operatorFilter.value = ''
  emit('search')
}

// 初始化：从后端加载已保存的筛选方案
filtersStore.loadFilters('cases')
</script>

<template>
  <div class="filter-bar">
    <div class="filter-primary-row">
      <el-input
        :model-value="filter.search"
        placeholder="搜索案件名称、案号、当事人..."
        clearable
        class="case-search"
        :prefix-icon="Search"
        @input="(v: string) => { updateSearch(v); onSearchInput() }"
        @clear="() => { updateSearch(''); onSearchInput() }"
      />
      <el-select
        :model-value="filter.track"
        clearable
        placeholder="全部轨道"
        style="width: 140px"
        @change="updateTrack"
      >
        <el-option v-for="opt in trackOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
      </el-select>
      <el-select
        :model-value="filter.status || ''"
        clearable
        placeholder="全部状态"
        style="width: 120px"
        @change="updateStatus"
      >
        <el-option v-for="opt in statusOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
      </el-select>
      <span class="total-count">{{ total }} 件</span>
      <el-button :icon="Filter" @click="showAdvancedFilter = !showAdvancedFilter">
        {{ showAdvancedFilter ? '收起筛选' : '更多筛选' }}
      </el-button>
      <el-button type="primary" :icon="Plus" @click="emit('create')">新建案件</el-button>
    </div>

    <div v-if="showAdvancedFilter" class="advanced-filter-panel">
      <div class="advanced-filter-grid">
        <el-select :model-value="filter.client" clearable filterable remote reserve-keyword placeholder="客户" :remote-method="remoteClientSearch" :loading="clientLoading" @change="updateClient">
          <el-option v-for="opt in clientOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
        </el-select>
        <el-select :model-value="filter.caseRoute || ''" clearable placeholder="案件路由" @change="updateRoute">
          <el-option v-for="opt in routeOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
        </el-select>
        <el-select :model-value="filter.civilStatus || ''" clearable placeholder="民事诉讼状态" @change="updateCivilStatus">
          <el-option v-for="opt in civilStatusOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
        </el-select>
        <el-select :model-value="filter.invalidationStatus || ''" clearable placeholder="无效程序状态" @change="updateInvalidationStatus">
          <el-option v-for="opt in invalidationStatusOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
        </el-select>
        <el-select :model-value="filter.adminStatus || ''" clearable placeholder="行政诉讼状态" @change="updateAdminStatus">
          <el-option v-for="opt in adminStatusOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
        </el-select>
        <el-date-picker v-model="dateRange" type="daterange" range-separator="至" start-placeholder="立案起" end-placeholder="立案止" format="YYYY-MM-DD" value-format="YYYY-MM-DD" @change="onDateRangeChange" />
        <el-select
          v-model="deadlineQuick"
          placeholder="期限快捷"
          clearable
          @change="onDeadlineQuickChange"
        >
          <el-option v-for="opt in deadlineQuickOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
        </el-select>
        <el-date-picker v-model="deadlineRange" type="daterange" range-separator="至" start-placeholder="期限起" end-placeholder="期限止" format="YYYY-MM-DD" value-format="YYYY-MM-DD" @change="onDeadlineRangeChange" />
        <el-date-picker v-model="hearingRange" type="daterange" range-separator="至" start-placeholder="开庭起" end-placeholder="开庭止" format="YYYY-MM-DD" value-format="YYYY-MM-DD" @change="onHearingRangeChange" />
        <el-input v-model="operatorFilter" placeholder="办案人" clearable @input="onOperatorChange" />
        <el-select :model-value="groupBy" placeholder="分组方式" @change="(v: string) => emit('update:groupBy', v)">
          <el-option label="不分组" value="none" />
          <el-option label="按客户" value="client" />
          <el-option label="按轨道" value="track" />
          <el-option label="按路由" value="route" />
          <el-option label="按法院" value="court" />
        </el-select>
        <el-select :model-value="filter.sortBy" placeholder="排序方式" @change="updateSortBy">
          <el-option v-for="opt in sortOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
        </el-select>
      </div>
      <div class="advanced-actions">
        <el-button text @click="clearFilters">清除全部</el-button>
        <el-dropdown v-if="savedFilters.length > 0" trigger="click">
          <el-button text :icon="FolderChecked">已保存方案 <el-icon><ArrowDown /></el-icon></el-button>
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item v-for="sf in savedFilters" :key="sf.id">
                <div class="saved-filter-item" @click="loadFilter(sf)">
                  <span>{{ sf.name }}</span>
                  <el-button size="small" type="danger" text @click.stop="deleteFilter(sf.id)">删除</el-button>
                </div>
              </el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
        <el-button text type="primary" @click="showSaveFilterDialog = true">保存当前筛选</el-button>
        <el-button text :loading="exporting" @click="emit('export')"><el-icon><Download /></el-icon>导出 CSV</el-button>
      </div>
    </div>

    <!-- 保存筛选方案弹窗 -->
    <el-dialog v-model="showSaveFilterDialog" title="保存筛选方案" width="480">
      <el-form label-width="80px">
        <el-form-item label="方案名称">
          <el-input v-model="savedFilterName" placeholder="如：我的待办、本月到期案件" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showSaveFilterDialog = false">取消</el-button>
        <el-button type="primary" @click="saveFilter">保存</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.filter-bar {
  background: transparent;
  border-bottom: 1px solid var(--c-border);
  padding: 10px 0 14px;
  margin-bottom: 16px;
}

.filter-primary-row {
  display: grid;
  grid-template-columns: minmax(260px, 1fr) 140px 120px auto auto auto;
  align-items: center;
  gap: 8px;
}

.case-search {
  min-width: 0;
}

.filter-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.filter-row + .filter-row {
  margin-top: 8px;
}

.filter-left {
  display: flex;
  gap: 8px;
  align-items: center;
  flex-wrap: wrap;
}

.filter-right {
  display: flex;
  gap: 12px;
  align-items: center;
}

.total-count {
  color: var(--c-text-secondary);
  font-size: 12px;
  white-space: nowrap;
}

.saved-filter-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  min-width: 160px;
}

.advanced-filter-panel {
  margin-top: 10px;
  border-radius: 8px;
  background: var(--c-bg-subtle);
  padding: 12px;
}

.advanced-filter-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(150px, 1fr));
  gap: 8px;
}

.advanced-actions {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  gap: 2px;
  margin-top: 8px;
}

@media (max-width: 1100px) {
  .filter-primary-row {
    grid-template-columns: minmax(220px, 1fr) 130px 120px auto auto;
  }

  .total-count {
    display: none;
  }

  .advanced-filter-grid {
    grid-template-columns: repeat(3, minmax(150px, 1fr));
  }
}

@media (max-width: 760px) {
  .filter-primary-row,
  .advanced-filter-grid {
    grid-template-columns: 1fr 1fr;
  }

  .case-search {
    grid-column: 1 / -1;
  }
}
</style>
