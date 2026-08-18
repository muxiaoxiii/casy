<script setup>
import { ref, onMounted, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useCasesStore } from '../../../stores/cases'
import { tauriCallSafe } from '../../../core/tauriBridge'
import { ElMessage, ElMessageBox } from 'element-plus'
import CaseFilterBar from '../components/CaseFilterBar.vue'
import CaseGroupPanel from '../components/CaseGroupPanel.vue'

const router = useRouter()
const casesStore = useCasesStore()

const groupBy = ref('none')
const showCreateDialog = ref(false)
const newCase = ref({
  caseName: '',
  clientName: '',
  opponentName: '',
  track: 'patent_invalidation',
  causeAction: '',
  court: '',
  caseNo: '',
})

const trackOptions = [
  { value: 'patent_invalidation', label: '专利无效' },
  { value: 'admin_litigation', label: '行政诉讼' },
  { value: 'civil_tort', label: '民事侵权' },
  { value: 'other', label: '其他' },
]

onMounted(async () => {
  await casesStore.loadCases()
})

// 监听筛选变化
watch(
  () => [
    casesStore.filter.track,
    casesStore.filter.client,
    casesStore.filter.court,
    casesStore.filter.status,
    casesStore.filter.sortBy,
    casesStore.filter.dateFrom,
    casesStore.filter.dateTo,
    casesStore.filter.deadlineFrom,
    casesStore.filter.deadlineTo,
    casesStore.filter.hearingFrom,
    casesStore.filter.hearingTo,
    casesStore.filter.operator,
  ],
  () => {
    casesStore.page = 1
    casesStore.loadCases()
  }
)

function onSearch() {
  casesStore.page = 1
  casesStore.loadCases()
}

// 新建案件
async function createCase() {
  if (!newCase.value.caseName.trim()) {
    ElMessage.warning('请输入案件名称')
    return
  }
  if (!newCase.value.clientName.trim()) {
    ElMessage.warning('请输入客户名称')
    return
  }
  const result = await casesStore.createCase(newCase.value)
  if (result.ok) {
    ElMessage.success('案件已创建')
    showCreateDialog.value = false
    newCase.value = {
      caseName: '',
      clientName: '',
      opponentName: '',
      track: 'patent_invalidation',
      causeAction: '',
      court: '',
      caseNo: '',
    }
    router.push({ name: 'case-detail', params: { id: result.data.id } })
  } else {
    ElMessage.error(result.error || '创建失败')
  }
}

// 删除案件
async function deleteCase(caseItem) {
  try {
    await ElMessageBox.confirm(
      `删除案件将同时删除关联的日志、庭审、任务和文件。确定删除"${caseItem.caseName}"？`,
      '确认删除',
      { confirmButtonText: '删除', cancelButtonText: '取消', type: 'warning' }
    )
    const result = await casesStore.deleteCase(caseItem.id)
    if (result.ok) {
      ElMessage.success('案件已删除')
    }
  } catch {
    // 用户取消
  }
}

function goToCase(row) {
  router.push({ name: 'case-detail', params: { id: row.id } })
}

// 导出 CSV
const exporting = ref(false)
async function exportCases() {
  exporting.value = true
  try {
    const result = await tauriCallSafe('export_cases', {
      format: 'csv',
      filter: {
        track: casesStore.filter.track || null,
        client: casesStore.filter.client || null,
        court: casesStore.filter.court || null,
        status: casesStore.filter.status || null,
        search: casesStore.filter.search || null,
      },
    })
    if (result.ok) {
      ElMessage.success(`已导出到: ${result.data}`)
    } else {
      ElMessage.error(result.error || '导出失败')
    }
  } finally {
    exporting.value = false
  }
}
</script>

<template>
  <div class="case-list-page">
    <CaseFilterBar
      :filter="casesStore.filter"
      :group-by="groupBy"
      :total="casesStore.total"
      :exporting="exporting"
      @update:filter="(v) => Object.assign(casesStore.filter, v)"
      @update:groupBy="(v) => groupBy = v"
      @search="onSearch"
      @export="exportCases"
      @create="showCreateDialog = true"
    />

    <!-- Loading Skeleton -->
    <div v-if="casesStore.loading" class="skeleton-wrapper">
      <el-skeleton :rows="6" animated>
        <template #template>
          <div v-for="i in 5" :key="i" class="skeleton-row">
            <el-skeleton-item variant="text" style="width: 25%; height: 20px;" />
            <el-skeleton-item variant="text" style="width: 35%; height: 20px;" />
            <el-skeleton-item variant="text" style="width: 20%; height: 20px;" />
            <el-skeleton-item variant="text" style="width: 15%; height: 20px;" />
          </div>
        </template>
      </el-skeleton>
    </div>

    <CaseGroupPanel
      v-else
      :cases="casesStore.cases"
      :group-by="groupBy"
      :loading="casesStore.loading"
      @row-click="goToCase"
      @delete="deleteCase"
    >
      <template #empty>
        <el-empty description="还没有案件">
          <el-button type="primary" @click="showCreateDialog = true">创建第一个案件</el-button>
        </el-empty>
      </template>
    </CaseGroupPanel>

    <!-- 分页 -->
    <div v-if="casesStore.total > casesStore.perPage" class="pagination">
      <el-pagination
        v-model:current-page="casesStore.page"
        :page-size="casesStore.perPage"
        :total="casesStore.total"
        layout="prev, pager, next"
        @current-change="casesStore.loadCases()"
      />
    </div>

    <!-- 新建案件弹窗 -->
    <el-dialog v-model="showCreateDialog" title="新建案件" width="500">
      <el-form label-width="80px">
        <el-form-item label="案件名称" required>
          <el-input v-model="newCase.caseName" placeholder="如：隆基244号无效" />
        </el-form-item>
        <el-form-item label="客户名称" required>
          <el-input v-model="newCase.clientName" placeholder="如：隆基绿能" />
        </el-form-item>
        <el-form-item label="对方名称">
          <el-input v-model="newCase.opponentName" placeholder="如：晶科能源" />
        </el-form-item>
        <el-form-item label="案件轨道">
          <el-select v-model="newCase.track" style="width: 100%">
            <el-option v-for="opt in trackOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
          </el-select>
        </el-form-item>
        <el-form-item label="案由">
          <el-input v-model="newCase.causeAction" placeholder="如：专利无效" />
        </el-form-item>
        <el-form-item label="法院">
          <el-input v-model="newCase.court" placeholder="如：国知局" />
        </el-form-item>
        <el-form-item label="案号">
          <el-input v-model="newCase.caseNo" placeholder="如：(2024)京73行初1号" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showCreateDialog = false">取消</el-button>
        <el-button type="primary" @click="createCase">创建</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.case-list-page {
  max-width: 1400px;
  margin: 0 auto;
}

.pagination {
  margin-top: 16px;
  display: flex;
  justify-content: center;
}

.skeleton-wrapper {
  padding: 16px;
}

.skeleton-row {
  display: flex;
  gap: 16px;
  padding: 12px 0;
  border-bottom: 1px solid #f0f0f0;
}
</style>
