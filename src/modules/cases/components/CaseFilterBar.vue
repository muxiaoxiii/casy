<script setup>
import { ref, computed, watch } from 'vue'
import { Download, Search, Filter, FolderChecked } from '@element-plus/icons-vue'
import { tauriCallSafe } from '../../../core/tauriBridge.js'

const props = defineProps({
  filter: { type: Object, required: true },
  groupBy: { type: String, default: 'none' },
  total: { type: Number, default: 0 },
  exporting: { type: Boolean, default: false },
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

const sortOptions = [
  { value: 'filing_date', label: '立案日期' },
  { value: 'updated_at', label: '最近更新' },
  { value: 'case_name', label: '案件名称' },
  { value: 'client_name', label: '客户名称' },
]

// 客户搜索
const clientSearchQuery = ref('')
const clientOptions = ref([])
const clientLoading = ref(false)

// 日期范围
const dateRange = ref([])

// 跨类型筛选
const showAdvancedFilter = ref(false)
const deadlineRange = ref([])
const hearingRange = ref([])
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

// 保存的筛选方案
const showSaveFilterDialog = ref(false)
const savedFilterName = ref('')
const savedFilters = ref([])

// 搜索防抖
let searchTimer = null
function onSearchInput() {
  if (searchTimer) clearTimeout(searchTimer)
  searchTimer = setTimeout(() => emit('search'), 300)
}

function updateTrack(val) {
  emit('update:filter', { ...props.filter, track: val || null })
}

function updateStatus(val) {
  emit('update:filter', { ...props.filter, status: val || null })
}

function updateSortBy(val) {
  emit('update:filter', { ...props.filter, sortBy: val })
}

function updateSearch(val) {
  emit('update:filter', { ...props.filter, search: val })
}

function updateClient(val) {
  emit('update:filter', { ...props.filter, client: val || null })
}

// 日期范围变化
function onDateRangeChange(val) {
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
function onDeadlineQuickChange(val) {
  const today = new Date()
  const fmt = (d) => d.toISOString().split('T')[0]

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
function onDeadlineRangeChange(val) {
  deadlineQuick.value = ''
  if (val && val.length === 2) {
    emit('update:filter', { ...props.filter, deadlineFrom: val[0], deadlineTo: val[1] })
  } else {
    emit('update:filter', { ...props.filter, deadlineFrom: null, deadlineTo: null })
  }
}

// 开庭日期范围变化
function onHearingRangeChange(val) {
  if (val && val.length === 2) {
    emit('update:filter', { ...props.filter, hearingFrom: val[0], hearingTo: val[1] })
  } else {
    emit('update:filter', { ...props.filter, hearingFrom: null, hearingTo: null })
  }
}

// 办案人变化
function onOperatorChange(val) {
  emit('update:filter', { ...props.filter, operator: val || null })
}

// 远程搜索客户
async function remoteClientSearch(query) {
  if (!query) {
    clientOptions.value = []
    return
  }
  clientLoading.value = true
  const result = await tauriCallSafe('search_cases', { query })
  if (result.ok) {
    // 从搜索结果中提取唯一的客户名
    const clients = [...new Set((result.data || []).map((c) => c.clientName).filter(Boolean))]
    clientOptions.value = clients.map((name) => ({ value: name, label: name }))
  }
  clientLoading.value = false
}

// 保存当前筛选方案
function saveFilter() {
  if (!savedFilterName.value.trim()) return
  const filterConfig = {
    name: savedFilterName.value,
    filter: { ...props.filter },
    groupBy: props.groupBy,
    createdAt: new Date().toISOString(),
  }
  savedFilters.value.push(filterConfig)
  // 保存到 localStorage
  localStorage.setItem('casy_saved_filters', JSON.stringify(savedFilters.value))
  showSaveFilterDialog.value = false
  savedFilterName.value = ''
}

// 加载已保存的筛选方案
function loadFilter(filterConfig) {
  emit('update:filter', { ...filterConfig.filter })
  emit('update:groupBy', filterConfig.groupBy)
  emit('search')
}

// 删除已保存的筛选方案
function deleteFilter(index) {
  savedFilters.value.splice(index, 1)
  localStorage.setItem('casy_saved_filters', JSON.stringify(savedFilters.value))
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
  })
  dateRange.value = []
  deadlineRange.value = []
  hearingRange.value = []
  deadlineQuick.value = ''
  operatorFilter.value = ''
  emit('search')
}

// 初始化：加载已保存的筛选方案
const initSavedFilters = () => {
  try {
    const stored = localStorage.getItem('casy_saved_filters')
    if (stored) {
      savedFilters.value = JSON.parse(stored)
    }
  } catch (e) {
    console.warn('加载筛选方案失败:', e)
  }
}

initSavedFilters()
</script>

<template>
  <div class="filter-bar">
    <!-- 第一行：搜索 + 主要筛选 -->
    <div class="filter-row">
      <el-input
        :model-value="filter.search"
        placeholder="搜索案件名称、案号、当事人..."
        clearable
        style="width: 280px"
        :prefix-icon="Search"
        @input="(v) => { updateSearch(v); onSearchInput() }"
        @clear="() => { updateSearch(''); onSearchInput() }"
      />
      <el-select
        :model-value="filter.track"
        clearable
        placeholder="全部轨道"
        style="width: 130px"
        @change="updateTrack"
      >
        <el-option v-for="opt in trackOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
      </el-select>
      <el-select
        :model-value="filter.status || ''"
        clearable
        placeholder="全部状态"
        style="width: 130px"
        @change="updateStatus"
      >
        <el-option v-for="opt in statusOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
      </el-select>
      <el-select
        :model-value="filter.client"
        clearable
        filterable
        remote
        reserve-keyword
        placeholder="搜索客户..."
        style="width: 180px"
        :remote-method="remoteClientSearch"
        :loading="clientLoading"
        @change="updateClient"
      >
        <el-option v-for="opt in clientOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
      </el-select>
      <el-date-picker
        v-model="dateRange"
        type="daterange"
        range-separator="至"
        start-placeholder="开始日期"
        end-placeholder="结束日期"
        style="width: 260px"
        format="YYYY-MM-DD"
        value-format="YYYY-MM-DD"
        @change="onDateRangeChange"
      />
    </div>

    <!-- 第二行：分组、排序、操作 -->
    <div class="filter-row">
      <div class="filter-left">
        <el-select :model-value="groupBy" style="width: 120px" @change="(v) => emit('update:groupBy', v)">
          <el-option label="不分组" value="none" />
          <el-option label="按客户" value="client" />
          <el-option label="按轨道" value="track" />
          <el-option label="按法院" value="court" />
        </el-select>
        <el-select :model-value="filter.sortBy" style="width: 120px" @change="updateSortBy">
          <el-option v-for="opt in sortOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
        </el-select>
        <el-button @click="showAdvancedFilter = !showAdvancedFilter" :type="showAdvancedFilter ? 'primary' : ''" text>
          {{ showAdvancedFilter ? '收起筛选 ▴' : '跨类型筛选 ▾' }}
        </el-button>
        <el-button @click="clearFilters" :icon="Filter">清除筛选</el-button>
        <el-dropdown v-if="savedFilters.length > 0" trigger="click">
          <el-button :icon="FolderChecked">
            已保存方案 <el-icon class="el-icon--right"><ArrowDown /></el-icon>
          </el-button>
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item v-for="(sf, idx) in savedFilters" :key="idx">
                <div class="saved-filter-item" @click="loadFilter(sf)">
                  <span>{{ sf.name }}</span>
                  <el-button size="small" type="danger" text @click.stop="deleteFilter(idx)">删除</el-button>
                </div>
              </el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
        <el-button @click="showSaveFilterDialog = true" text type="primary">
          保存当前筛选
        </el-button>
      </div>
      <div class="filter-right">
        <span class="total-count">共 {{ total }} 件</span>
        <el-button @click="emit('export')" :loading="exporting">
          <el-icon><Download /></el-icon> 导出 CSV
        </el-button>
        <el-button type="primary" @click="emit('create')">➕ 新建案件</el-button>
      </div>
    </div>

    <!-- 第三行：跨类型筛选（展开/收起） -->
    <div v-if="showAdvancedFilter" class="filter-row advanced-filter-row">
      <div class="filter-left">
        <el-select
          v-model="deadlineQuick"
          placeholder="期限快捷"
          style="width: 130px"
          clearable
          @change="onDeadlineQuickChange"
        >
          <el-option v-for="opt in deadlineQuickOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
        </el-select>
        <el-date-picker
          v-model="deadlineRange"
          type="daterange"
          range-separator="至"
          start-placeholder="期限起"
          end-placeholder="期限止"
          style="width: 240px"
          format="YYYY-MM-DD"
          value-format="YYYY-MM-DD"
          @change="onDeadlineRangeChange"
        />
        <el-date-picker
          v-model="hearingRange"
          type="daterange"
          range-separator="至"
          start-placeholder="开庭起"
          end-placeholder="开庭止"
          style="width: 240px"
          format="YYYY-MM-DD"
          value-format="YYYY-MM-DD"
          @change="onHearingRangeChange"
        />
        <el-input
          v-model="operatorFilter"
          placeholder="办案人"
          clearable
          style="width: 120px"
          @input="onOperatorChange"
        />
      </div>
    </div>

    <!-- 保存筛选方案弹窗 -->
    <el-dialog v-model="showSaveFilterDialog" title="保存筛选方案" width="400">
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
  background: #fff;
  border-radius: 8px;
  padding: 12px;
  margin-bottom: 16px;
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.08);
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
  color: #666;
  font-size: 14px;
}

.saved-filter-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  min-width: 160px;
}

.advanced-filter-row {
  padding-top: 8px;
  border-top: 1px dashed #e0e0e0;
  margin-top: 4px;
}
</style>
