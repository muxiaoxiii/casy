<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import {
  UploadFilled,
  Document,
  Check,
  Close,
  Right,
  Back,
  Warning,
  CircleCheck,
  CircleClose,
  Refresh,
  Link,
  Setting,
  Key,
  InfoFilled,
  Promotion,
  Files,
  Plus,
} from '@element-plus/icons-vue'
import { ElMessage } from 'element-plus'
import { casyContext } from '../../../core/plugin/context'
import { isTauriRuntime } from '../../../core/mockData'

const props = defineProps<{
  modelValue: boolean
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', val: boolean): void
  (e: 'imported'): void
}>()

const visible = computed({
  get: () => props.modelValue,
  set: (val) => emit('update:modelValue', val),
})

// 导入数据源模式：'excel' | 'feishu'
const importSource = ref<'excel' | 'feishu'>('excel')

// 当前导入的业务实体类型：'cases' (案件主表) | 'tasks' (任务分表) | 'hearings' (庭审分表) | 'case_logs' (办案日志分表)
const targetEntity = ref<'cases' | 'tasks' | 'hearings' | 'case_logs'>('cases')

const ENTITY_OPTIONS = [
  { value: 'cases', label: '📁 案件主表', desc: '导入或更新案件主体要素 (案号/案名/当事人/代理人)' },
  { value: 'tasks', label: '✅ 任务/待办分表', desc: '按案号/案名自动关联至对应案件，或作为独立待办' },
  { value: 'hearings', label: '📅 庭审/口审分表', desc: '沉淀历次开庭口审排期、法官、法庭与出庭记录' },
  { value: 'case_logs', label: '📝 办案日志分表', desc: '归集办案动态、递交与沟通流水记录' },
]

// 向导步骤：1=选择源与配置, 2=字段映射, 3=数据预览与查重, 4=导入完成
const currentStep = ref(1)

// ══════════════════════════════════════════════
// 步骤 1 状态 — Excel 模式
// ══════════════════════════════════════════════
const filePath = ref('')
const fileName = ref('')
const excelSheets = ref<Array<{ name: string; rowCount: number; columnCount: number }>>([])
const selectedExcelSheet = ref('')
const headerRowIndex = ref(0)

// ══════════════════════════════════════════════
// 步骤 1 状态 — 飞书多维表格模式
// ══════════════════════════════════════════════
const feishuConfigured = ref(false)
const feishuAppId = ref('')
const checkingFeishuConfig = ref(false)
const showFeishuConfigForm = ref(false)

// 飞书凭证配置输入
const inputAppId = ref('')
const inputAppSecret = ref('')
const savingFeishuCred = ref(false)

// 飞书表格读取输入
const feishuUrl = ref('')
const feishuAppToken = ref('')
const feishuTableId = ref('')
const feishuTables = ref<Array<{ tableId: string; name: string; revision: number | null }>>([])
const selectedFeishuTableId = ref('')
const loadingFeishuInspect = ref(false)
const feishuErrorTip = ref('')

const loadingInspect = ref(false)

// ══════════════════════════════════════════════
// 步骤 2 状态 (通用列映射)
// ══════════════════════════════════════════════
interface ColMappingItem {
  columnIndex: number
  excelHeader: string
  sampleValues: string[]
  suggestedField?: string | null
  confidence: number
  selectedField: string // 绑定的系统字段或 'ignore'
  forwardFill: boolean
}
const columns = ref<ColMappingItem[]>([])

// 案件主表标准系统字段
const CASE_SYSTEM_FIELDS = [
  { value: 'ignore', label: '— 忽略此列 (不导入) —' },
  { value: 'caseNo', label: '法院案号 (caseNo) *' },
  { value: 'caseName', label: '案件名称/信息 (caseName) *' },
  { value: 'track', label: '案件类型/阶段 (track) [新! 支持独立类型]' },
  { value: 'clientName', label: '委托方/客户 (clientName)' },
  { value: 'opponentName', label: '相对方/对方当事人 (opponentName)' },
  { value: 'ourRole', label: '我方诉讼地位 (ourRole)' },
  { value: 'opponentRole', label: '对方诉讼地位 (opponentRole)' },
  { value: 'court', label: '审理法院/机构 (court)' },
  { value: 'judgePanel', label: '承办法官/合议庭 (judgePanel)' },
  { value: 'clerk', label: '书记员/法院联系电话 (clerk)' },
  { value: 'attorneys', label: '负责律师/团队 (attorneys)' },
  { value: 'causeAction', label: '案由/纠纷类型 (causeAction)' },
  { value: 'filingDate', label: '接案/立案时间 (filingDate)' },
  { value: 'trialDate', label: '首期开庭/口审时间 (trialDate)' },
  { value: 'trial2Date', label: '二次开庭/口审时间 (trial2Date)' },
  { value: 'trial3Date', label: '三次开庭/口审时间 (trial3Date)' },
  { value: 'verdictDate', label: '裁判/判决时间 (verdictDate)' },
  { value: 'completedText', label: '已完成事项/节点 (completedText)' },
  { value: 'caseLevel', label: '案件阶段/审级 (caseLevel)' },
  { value: 'caseProgress', label: '当前进展/办理情况 (caseProgress)' },
  { value: 'caseResult', label: '裁判结果 (caseResult)' },
  { value: 'stayDate', label: '保全期限/开始/结束时间 (stayDate)' },
  { value: 'reliefDeadline', label: '上诉/救济期限 (reliefDeadline)' },
  { value: 'internalNo', label: '案件编号/内部流水号 (internalNo)' },
  { value: 'patentName', label: '涉案专利名称 (patentName)' },
  { value: 'patentAppNo', label: '专利号/申请号 (patentAppNo)' },
  { value: 'priority', label: '重要紧急程度 (priority)' },
  { value: 'notes', label: '备注/日志/附注追加 (notes, 支持多列合并)' },
]

// 任务分表字段
const TASK_SYSTEM_FIELDS = [
  { value: 'ignore', label: '— 忽略此列 (不导入) —' },
  { value: 'taskName', label: '任务/待办名称 (taskName) *' },
  { value: 'caseNo', label: '关联法院案号 (caseNo)' },
  { value: 'caseName', label: '关联案件名称 (caseName)' },
  { value: 'deadline', label: '截止/完成日期 (deadline)' },
  { value: 'priority', label: '优先级/紧急程度 (priority: 高/中/低)' },
  { value: 'assignee', label: '负责人/承办人 (assignee)' },
  { value: 'description', label: '任务描述/详情 (description)' },
  { value: 'completed', label: '完成状态 (completed: 已完成/未完成)' },
]

// 庭审分表字段
const HEARING_SYSTEM_FIELDS = [
  { value: 'ignore', label: '— 忽略此列 (不导入) —' },
  { value: 'hearingDate', label: '开庭/口审时间 (hearingDate) *' },
  { value: 'hearingName', label: '庭审名称/轮次 (hearingName: 一审第一次等)' },
  { value: 'caseNo', label: '关联法院案号 (caseNo)' },
  { value: 'caseName', label: '关联案件名称 (caseName)' },
  { value: 'court', label: '审理法院/审理机构 (court)' },
  { value: 'judgePanel', label: '承办法官/合议庭 (judgePanel)' },
  { value: 'clerk', label: '书记员/联系方式 (clerk)' },
  { value: 'caseLevel', label: '案件阶段/审级 (caseLevel)' },
  { value: 'actualStatus', label: '开庭状态 (actualStatus: 已开/未开)' },
]

// 办案日志分表字段
const LOG_SYSTEM_FIELDS = [
  { value: 'ignore', label: '— 忽略此列 (不导入) —' },
  { value: 'content', label: '日志内容/事件说明 (content) *' },
  { value: 'eventDate', label: '事件/记录日期 (eventDate)' },
  { value: 'eventType', label: '事件类型 (eventType: 沟通/递交/文书等)' },
  { value: 'caseNo', label: '关联法院案号 (caseNo)' },
  { value: 'caseName', label: '关联案件名称 (caseName)' },
  { value: 'operator', label: '记录人/经办人 (operator)' },
]

const availableSystemFields = computed(() => {
  if (targetEntity.value === 'tasks') return TASK_SYSTEM_FIELDS
  if (targetEntity.value === 'hearings') return HEARING_SYSTEM_FIELDS
  if (targetEntity.value === 'case_logs') return LOG_SYSTEM_FIELDS
  return CASE_SYSTEM_FIELDS
})

// 检查是否为庭审字段
function isTrialField(field: string): boolean {
  return field === 'trialDate' || field === 'trial2Date' || field === 'trial3Date'
}

// 检查某个字段是否被多个列重复映射（notes、ignore 以及开庭字段允许多列映射，自动按轮次顺延分配）
function getConflictCols(col: ColMappingItem): number[] {
  if (
    col.selectedField === 'ignore' ||
    col.selectedField === 'notes' ||
    isTrialField(col.selectedField)
  ) {
    return []
  }
  return columns.value
    .filter((c) => c.columnIndex !== col.columnIndex && c.selectedField === col.selectedField)
    .map((c) => c.columnIndex + 1)
}

// ══════════════════════════════════════════════
// 步骤 3 状态 (预览与策略)
// ══════════════════════════════════════════════
const previewRows = ref<Array<Record<string, string>>>([])
const conflictStrategy = ref<'skip' | 'update' | 'duplicate'>('skip')
const defaultTrack = ref('civil_tort')
const importing = ref(false)

// ══════════════════════════════════════════════
// 步骤 4 状态 (导入报告)
// ══════════════════════════════════════════════
interface ImportReport {
  targetEntity?: string
  totalRowsProcessed: number
  createdCount: number
  updatedCount?: number
  skippedCount?: number
  linkedCasesCount?: number
  unlinkedCount?: number
  failedCount: number
  errors: string[]
  importedCaseIds?: string[]
}
const importReport = ref<ImportReport | null>(null)

// 选中的映射是否满足基本要求
const hasRequiredField = computed(() => {
  if (targetEntity.value === 'tasks') {
    return columns.value.some((c) => c.selectedField === 'taskName')
  }
  if (targetEntity.value === 'hearings') {
    return columns.value.some((c) => c.selectedField === 'hearingDate' || c.selectedField === 'trialDate')
  }
  if (targetEntity.value === 'case_logs') {
    return columns.value.some((c) => c.selectedField === 'content' || c.selectedField === 'notes')
  }
  return columns.value.some(
    (c) =>
      c.selectedField === 'caseName' ||
      c.selectedField === 'caseNo' ||
      c.selectedField === 'patentName' ||
      c.selectedField === 'clientName',
  )
})

// 映射列数统计
const mappedColCount = computed(() => {
  return columns.value.filter((c) => c.selectedField !== 'ignore').length
})

// ══════════════════════════════════════════════
// 飞书配置与连接逻辑
// ══════════════════════════════════════════════
async function checkFeishuStatus() {
  checkingFeishuConfig.value = true
  const res = await casyContext.cases.checkFeishuConfig()
  checkingFeishuConfig.value = false
  if (res.ok && res.data) {
    feishuConfigured.value = res.data.configured
    feishuAppId.value = res.data.appId || ''
    if (res.data.appId && !inputAppId.value) {
      inputAppId.value = res.data.appId
    }
    if (!res.data.configured) {
      showFeishuConfigForm.value = true
    } else {
      showFeishuConfigForm.value = false
    }
  }
}

async function saveAndTestFeishuCred() {
  const cleanId = inputAppId.value.trim()
  const cleanSecret = inputAppSecret.value.trim()
  if (!cleanId || !cleanSecret) {
    ElMessage.warning('请输入 App ID 和 App Secret')
    return
  }

  savingFeishuCred.value = true
  // 1. 保存凭据
  const confRes = await casyContext.sync.configureFeishu(cleanId, cleanSecret)
  if (!confRes.ok) {
    savingFeishuCred.value = false
    ElMessage.error(confRes.error || '保存飞书凭证失败')
    return
  }

  // 2. 测试连通性
  const testRes = await casyContext.sync.testFeishuConnection(cleanId, cleanSecret)
  savingFeishuCred.value = false

  if (testRes.ok) {
    ElMessage.success('飞书自建应用连接成功！')
    await checkFeishuStatus()
    showFeishuConfigForm.value = false
  } else {
    ElMessage.error(`连接测试未通过: ${testRes.error}，请检查 App ID 与 Secret`)
  }
}

// 飞书 URL 解析并探测
async function inspectFeishuUrl() {
  if (!feishuUrl.value.trim()) {
    ElMessage.warning('请输入飞书多维表格链接或 App Token')
    return
  }

  loadingFeishuInspect.value = true
  feishuErrorTip.value = ''

  const res = await casyContext.cases.inspectFeishuBitable(
    feishuUrl.value.trim(),
    selectedFeishuTableId.value || undefined,
  )
  loadingFeishuInspect.value = false

  if (res.ok && res.data) {
    feishuAppToken.value = res.data.appToken
    feishuTableId.value = res.data.tableId
    feishuTables.value = res.data.tables
    selectedFeishuTableId.value = res.data.tableId

    previewRows.value = res.data.previewRows
    columns.value = res.data.columns.map((c) => ({
      ...c,
      selectedField: c.suggestedField || 'ignore',
      forwardFill: c.suggestedField === 'clientName' || c.suggestedField === 'court' || c.suggestedField === 'caseNo',
    }))

    ElMessage.success(`成功读取飞书工作表「${res.data.tableName}」，共 ${res.data.totalRecords} 条记录`)
  } else {
    feishuErrorTip.value = res.error || '读取飞书多维表格失败'
    ElMessage.error('读取多维表格失败，请查看下方授权指引')
  }
}

// 切换飞书子工作表
async function onFeishuTableChange() {
  if (feishuAppToken.value && selectedFeishuTableId.value) {
    loadingFeishuInspect.value = true
    const res = await casyContext.cases.inspectFeishuBitable(
      feishuAppToken.value,
      selectedFeishuTableId.value,
    )
    loadingFeishuInspect.value = false
    if (res.ok && res.data) {
      previewRows.value = res.data.previewRows
      columns.value = res.data.columns.map((c) => ({
        ...c,
        selectedField: c.suggestedField || 'ignore',
        forwardFill: c.suggestedField === 'clientName' || c.suggestedField === 'court' || c.suggestedField === 'caseNo',
      }))
    }
  }
}

// ══════════════════════════════════════════════
// Excel 文件选择与探测逻辑
// ══════════════════════════════════════════════
async function pickFile() {
  if (isTauriRuntime()) {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog')
      const selected = await open({
        multiple: false,
        filters: [
          {
            name: 'Excel 工作簿',
            extensions: ['xlsx', 'xls', 'xlsb', 'ods', 'csv'],
          },
        ],
      })
      if (selected && typeof selected === 'string') {
        filePath.value = selected
        fileName.value = selected.split(/[\/\\]/).pop() || selected
        await loadExcelSheets()
      }
    } catch (e: any) {
      ElMessage.error(`选择文件失败: ${e.message || e}`)
    }
  } else {
    filePath.value = '/mock/cases_sample_2024.xlsx'
    fileName.value = 'cases_sample_2024.xlsx'
    excelSheets.value = [{ name: 'Sheet1', rowCount: 45, columnCount: 12 }]
    selectedExcelSheet.value = 'Sheet1'
    await inspectExcelCurrentSheet()
  }
}

async function loadExcelSheets() {
  if (!filePath.value) return
  loadingInspect.value = true
  const res = await casyContext.cases.getExcelSheets(filePath.value)
  loadingInspect.value = false
  if (res.ok && res.data && res.data.length > 0) {
    excelSheets.value = res.data
    selectedExcelSheet.value = res.data[0].name
    await inspectExcelCurrentSheet()
  } else {
    ElMessage.error(res.error || '解析 Excel 工作簿失败')
  }
}

async function inspectExcelCurrentSheet() {
  if (!filePath.value || !selectedExcelSheet.value) return
  loadingInspect.value = true
  const res = await casyContext.cases.inspectExcelSheet(
    filePath.value,
    selectedExcelSheet.value,
    headerRowIndex.value,
  )
  loadingInspect.value = false
  if (res.ok && res.data) {
    headerRowIndex.value = res.data.detectedHeaderRow
    previewRows.value = res.data.previewRows
    columns.value = res.data.columns.map((c) => ({
      ...c,
      selectedField: c.suggestedField || 'ignore',
      forwardFill: c.suggestedField === 'clientName' || c.suggestedField === 'court' || c.suggestedField === 'caseNo',
    }))
  } else {
    ElMessage.error(res.error || '工作表探测失败')
  }
}

// ══════════════════════════════════════════════
// 步骤切换与导入执行
// ══════════════════════════════════════════════
function goToStep2() {
  if (importSource.value === 'excel') {
    if (!filePath.value || !selectedExcelSheet.value) {
      ElMessage.warning('请先选择有效的 Excel 文件与工作表')
      return
    }
  } else {
    if (!feishuAppToken.value || !feishuTableId.value || columns.value.length === 0) {
      ElMessage.warning('请先成功读取飞书多维表格数据')
      return
    }
  }
  currentStep.value = 2
}

function goToStep3() {
  if (!hasRequiredField.value) {
    if (targetEntity.value === 'tasks') {
      ElMessage.warning('任务分表至少需要映射「任务/待办名称」')
    } else if (targetEntity.value === 'hearings') {
      ElMessage.warning('庭审分表至少需要映射「开庭/口审时间」')
    } else if (targetEntity.value === 'case_logs') {
      ElMessage.warning('办案日志分表至少需要映射「日志内容/事件说明」')
    } else {
      ElMessage.warning('至少需要将一列映射为「案件名称」或「法院案号」')
    }
    return
  }
  currentStep.value = 3
}

async function executeImport() {
  importing.value = true

  const columnMappings: Record<number, string> = {}
  const forwardFillColumns: number[] = []

  for (const c of columns.value) {
    if (c.selectedField && c.selectedField !== 'ignore') {
      columnMappings[c.columnIndex] = c.selectedField
      if (c.forwardFill) {
        forwardFillColumns.push(c.columnIndex)
      }
    }
  }

  let res: any

  if (targetEntity.value === 'cases') {
    const importConfig = {
      headerRow: headerRowIndex.value,
      columnMappings,
      forwardFillColumns,
      conflictStrategy: conflictStrategy.value,
      defaultTrack: defaultTrack.value,
    }

    if (importSource.value === 'excel') {
      res = await casyContext.cases.importExcelCases(
        filePath.value,
        selectedExcelSheet.value,
        importConfig,
      )
    } else {
      res = await casyContext.cases.importFeishuBitableCases(
        feishuAppToken.value,
        selectedFeishuTableId.value || feishuTableId.value,
        importConfig,
      )
    }
  } else {
    const subConfig = {
      targetEntity: targetEntity.value,
      headerRow: headerRowIndex.value,
      columnMappings,
      forwardFillColumns,
    }

    if (importSource.value === 'excel') {
      res = await casyContext.cases.importExcelSubtable(
        filePath.value,
        selectedExcelSheet.value,
        subConfig,
      )
    } else {
      res = await casyContext.cases.importFeishuSubtable(
        feishuAppToken.value,
        selectedFeishuTableId.value || feishuTableId.value,
        subConfig,
      )
    }
  }

  importing.value = false

  if (res.ok && res.data) {
    importReport.value = {
      ...res.data,
      targetEntity: targetEntity.value,
    }
    currentStep.value = 4
    emit('imported')
  } else {
    ElMessage.error(res.error || '导入失败，请检查数据')
  }
}

function continueWithOtherSheet() {
  currentStep.value = 1
  importReport.value = null
  targetEntity.value = 'tasks' // 默认切换到下一个常用分表
  if (importSource.value === 'excel' && filePath.value) {
    inspectExcelCurrentSheet()
  } else if (importSource.value === 'feishu' && feishuAppToken.value) {
    inspectFeishuUrl()
  }
}

function resetWizard() {
  currentStep.value = 1
  targetEntity.value = 'cases'
  filePath.value = ''
  fileName.value = ''
  excelSheets.value = []
  selectedExcelSheet.value = ''
  headerRowIndex.value = 0
  feishuUrl.value = ''
  feishuAppToken.value = ''
  feishuTableId.value = ''
  feishuTables.value = []
  selectedFeishuTableId.value = ''
  feishuErrorTip.value = ''
  columns.value = []
  previewRows.value = []
  importReport.value = null
}

watch(visible, (newVal) => {
  if (newVal) {
    checkFeishuStatus()
    if (currentStep.value === 4) {
      resetWizard()
    }
  }
})

function handleFileDrop(e: DragEvent) {
  const file = e.dataTransfer?.files[0]
  if (file && (file.name.endsWith('.xlsx') || file.name.endsWith('.xls') || file.name.endsWith('.csv'))) {
    // 浏览器环境下 fallback
    if (!isTauriRuntime()) {
      filePath.value = file.name
      fileName.value = file.name
      excelSheets.value = [{ name: 'Sheet1', rowCount: 1, columnCount: 1 }]
      selectedExcelSheet.value = 'Sheet1'
    }
  }
}

function onWindowFileDrop(event: Event) {
  if (!visible.value || importSource.value !== 'excel') return
  const customEvent = event as CustomEvent<{ paths: string[] }>
  const paths = customEvent.detail?.paths
  if (paths && paths.length > 0) {
    const droppedFile = paths[0]
    if (droppedFile.match(/\.(xlsx|xls|xlsb|ods|csv)$/i)) {
      filePath.value = droppedFile
      fileName.value = droppedFile.split(/[\/\\]/).pop() || droppedFile
      loadExcelSheets()
    } else {
      ElMessage.warning('不支持的文件格式，请拖拽 Excel 或 CSV 文件')
    }
  }
}

onMounted(() => {
  window.addEventListener('casy:file-drop', onWindowFileDrop)
  if (visible.value) {
    checkFeishuStatus()
  }
})

onUnmounted(() => {
  window.removeEventListener('casy:file-drop', onWindowFileDrop)
})
</script>

<template>
  <el-dialog
    v-model="visible"
    title="案件批量导入中心"
    width="940px"
    destroy-on-close
    class="case-import-dialog"
    :close-on-click-modal="false"
  >
    <!-- 步骤导航条 -->
    <div class="wizard-steps-bar">
      <div class="step-item" :class="{ active: currentStep === 1, done: currentStep > 1 }">
        <div class="step-badge">1</div>
        <span class="step-text">选择数据源与配置</span>
      </div>
      <div class="step-connector" :class="{ active: currentStep > 1 }" />
      <div class="step-item" :class="{ active: currentStep === 2, done: currentStep > 2 }">
        <div class="step-badge">2</div>
        <span class="step-text">智能字段映射</span>
      </div>
      <div class="step-connector" :class="{ active: currentStep > 2 }" />
      <div class="step-item" :class="{ active: currentStep === 3, done: currentStep > 3 }">
        <div class="step-badge">3</div>
        <span class="step-text">清洗预览与查重策略</span>
      </div>
      <div class="step-connector" :class="{ active: currentStep > 3 }" />
      <div class="step-item" :class="{ active: currentStep === 4 }">
        <div class="step-badge">4</div>
        <span class="step-text">导入报告</span>
      </div>
    </div>

    <!-- ═══ 步骤 1: 选择数据源与配置 ═══ -->
    <div v-if="currentStep === 1" class="step-content step-1">
      <!-- 实体/分表类型选择卡片 -->
      <div class="entity-select-section">
        <div class="section-badge-label">1. 选择导入目标分表实体类型</div>
        <div class="entity-options-grid">
          <div
            v-for="opt in ENTITY_OPTIONS"
            :key="opt.value"
            class="entity-card"
            :class="{ active: targetEntity === opt.value }"
            @click="targetEntity = opt.value as any"
          >
            <div class="entity-card-title">{{ opt.label }}</div>
            <div class="entity-card-desc">{{ opt.desc }}</div>
          </div>
        </div>
      </div>

      <div class="section-badge-label mt-4">2. 选择数据来源渠道</div>

      <!-- 数据源切换 Tab -->
      <div class="source-toggle-bar">
        <button
          class="source-tab-btn"
          :class="{ active: importSource === 'excel' }"
          @click="importSource = 'excel'"
        >
          <el-icon :size="16"><Files /></el-icon>
          <span>Excel / CSV 本地文件</span>
        </button>
        <button
          class="source-tab-btn"
          :class="{ active: importSource === 'feishu' }"
          @click="importSource = 'feishu'"
        >
          <el-icon :size="16"><Link /></el-icon>
          <span>飞书多维表格 (云端直连)</span>
          <span v-if="feishuConfigured" class="feishu-badge">已配置</span>
        </button>
      </div>

      <!-- ── 模式 A: Excel 文件导入 ── -->
      <div v-if="importSource === 'excel'" class="source-panel-excel">
        <div 
          class="upload-dropzone" 
          @click="pickFile"
          @drop.prevent="handleFileDrop"
          @dragover.prevent
          @dragenter.prevent
        >
          <el-icon class="dropzone-icon" :size="44"><UploadFilled /></el-icon>
          <div class="dropzone-title">点击选择或拖拽 Excel 文件至此处</div>
          <div class="dropzone-sub">支持 .xlsx、.xls、.xlsb、.ods、.csv 格式</div>
        </div>

        <div v-if="fileName" class="file-info-card">
          <div class="file-icon-box">
            <el-icon :size="22"><Document /></el-icon>
          </div>
          <div class="file-details">
            <div class="file-name">{{ fileName }}</div>
            <div class="file-path">{{ filePath }}</div>
          </div>
          <el-button type="primary" link @click="pickFile">重新选择</el-button>
        </div>

        <div v-if="excelSheets.length > 0" class="sheet-config-section">
          <div class="form-row">
            <label class="form-label">选择工作表 (Sheet):</label>
            <el-select
              v-model="selectedExcelSheet"
              placeholder="请选择 Sheet"
              style="width: 260px"
              @change="inspectExcelCurrentSheet"
            >
              <el-option
                v-for="s in excelSheets"
                :key="s.name"
                :label="`${s.name} (${s.rowCount} 行 × ${s.columnCount} 列)`"
                :value="s.name"
              />
            </el-select>
          </div>

          <div class="form-row">
            <label class="form-label">表头所在行:</label>
            <el-input-number
              v-model="headerRowIndex"
              :min="0"
              :max="10"
              size="small"
              @change="inspectExcelCurrentSheet"
            />
            <span class="hint-text">
              （系统智能探测推荐为第 {{ headerRowIndex + 1 }} 行，已自动跳过顶部大标题）
            </span>
          </div>
        </div>
      </div>

      <!-- ── 模式 B: 飞书多维表格导入 ── -->
      <div v-else class="source-panel-feishu">
        <!-- 飞书凭证配置卡片 (未配置或展开编辑) -->
        <div v-if="showFeishuConfigForm || !feishuConfigured" class="feishu-guide-card">
          <div class="guide-header">
            <el-icon :size="20" class="guide-icon"><Setting /></el-icon>
            <div class="guide-title">飞书开放平台应用接入指引</div>
            <el-button
              v-if="feishuConfigured"
              link
              size="small"
              @click="showFeishuConfigForm = false"
            >
              收起配置
            </el-button>
          </div>

          <div class="guide-steps-list">
            <div class="guide-step">
              <div class="g-num">1</div>
              <div class="g-text">
                登录 <strong><a href="https://open.feishu.cn/" target="_blank" class="link-styled">飞书开放平台</a></strong>，创建「企业自建应用」，获取 <code>App ID</code> 与 <code>App Secret</code>。
              </div>
            </div>
            <div class="guide-step">
              <div class="g-num">2</div>
              <div class="g-text">
                在应用的「权限管理」中搜索并开通 <strong>bitable:app</strong>（查看、编辑多维表格）。
              </div>
            </div>
            <div class="guide-step">
              <div class="g-num">3</div>
              <div class="g-text">
                在「版本管理与发布」中发布一个可用版本。
              </div>
            </div>
          </div>

          <div class="feishu-cred-form">
            <div class="form-grid">
              <div class="form-item">
                <label class="form-label">App ID (cli_...):</label>
                <el-input
                  v-model="inputAppId"
                  placeholder="例如: cli_a1b2c3d4e5f6..."
                  size="small"
                />
              </div>
              <div class="form-item">
                <label class="form-label">App Secret:</label>
                <el-input
                  v-model="inputAppSecret"
                  type="password"
                  show-password
                  placeholder="输入应用的 App Secret"
                  size="small"
                />
              </div>
            </div>

            <div class="form-actions">
              <el-button
                type="primary"
                size="small"
                :loading="savingFeishuCred"
                @click="saveAndTestFeishuCred"
              >
                <el-icon><Key /></el-icon>
                <span>保存并测试连接</span>
              </el-button>
            </div>
          </div>
        </div>

        <!-- 飞书凭证已就绪卡片 -->
        <div v-else class="feishu-connected-card">
          <div class="conn-status">
            <el-icon class="check-ico"><CircleCheck /></el-icon>
            <span>飞书自建应用已就绪 ({{ feishuAppId }})</span>
          </div>
          <el-button link size="small" type="primary" @click="showFeishuConfigForm = true">
            修改凭据配置
          </el-button>
        </div>

        <!-- 飞书多维表格 URL 输入区 -->
        <div class="feishu-url-section">
          <div class="section-title-box">
            <label class="form-label">飞书多维表格链接 (或 Base Token):</label>
            <span class="url-hint">支持完整分享网址，系统自动提取 app_token 与 table_id</span>
          </div>

          <div class="url-input-group">
            <el-input
              v-model="feishuUrl"
              placeholder="例如: https://myfirm.feishu.cn/base/bascnXYZ123?table=tblABC789"
              clearable
              @keyup.enter="inspectFeishuUrl"
            >
              <template #prefix>
                <el-icon><Link /></el-icon>
              </template>
            </el-input>
            <el-button
              type="primary"
              :loading="loadingFeishuInspect"
              :disabled="!feishuConfigured || !feishuUrl"
              @click="inspectFeishuUrl"
            >
              <el-icon><Promotion /></el-icon>
              <span>读取表格</span>
            </el-button>
          </div>

          <!-- 重要授权注意事项提示 -->
          <div class="permission-tip-box">
            <el-icon class="tip-ico"><InfoFilled /></el-icon>
            <div class="tip-text">
              <strong>文档授权卡点提示</strong>：在飞书多维表格页面右上角点击 <strong>「···」$\rightarrow$「添加文档应用」</strong> 并添加您的自建应用，飞书 API 方可获得读取权限。
            </div>
          </div>

          <!-- 错误明细提示 -->
          <div v-if="feishuErrorTip" class="feishu-error-alert">
            <el-icon><Warning /></el-icon>
            <div class="err-content">
              <pre>{{ feishuErrorTip }}</pre>
            </div>
          </div>

          <!-- 多工作表切换下拉框 -->
          <div v-if="feishuTables.length > 1" class="feishu-subtables-row">
            <label class="form-label">选择子工作表 (Table):</label>
            <el-select
              v-model="selectedFeishuTableId"
              placeholder="选择工作表"
              style="width: 280px"
              size="small"
              @change="onFeishuTableChange"
            >
              <el-option
                v-for="t in feishuTables"
                :key="t.tableId"
                :label="t.name"
                :value="t.tableId"
              />
            </el-select>
          </div>
        </div>
      </div>
    </div>

    <!-- ═══ 步骤 2: 智能字段映射与清洗规则 ═══ -->
    <div v-if="currentStep === 2" class="step-content step-2" v-loading="loadingInspect || loadingFeishuInspect">
      <div class="mapping-toolbar">
        <div class="mapping-summary">
          当前分表：<strong>{{ ENTITY_OPTIONS.find(o => o.value === targetEntity)?.label }}</strong>，来源：<strong>{{ importSource === 'excel' ? 'Excel 文件' : '飞书多维表格' }}</strong>，共识别到
          <strong>{{ columns.length }}</strong> 个列，已配置
          <strong>{{ mappedColCount }}</strong> 个映射。
        </div>
        <div class="mapping-alert" v-if="!hasRequiredField">
          <el-icon><Warning /></el-icon>
          <span v-if="targetEntity === 'tasks'">任务分表至少需要映射「任务/待办名称」</span>
          <span v-else-if="targetEntity === 'hearings'">庭审分表至少需要映射「开庭/口审时间」</span>
          <span v-else-if="targetEntity === 'case_logs'">办案日志分表至少需要映射「日志内容/事件说明」</span>
          <span v-else>必须至少将一列映射为「案件名称」、「法院案号」、「涉案专利」或「委托方」</span>
        </div>
      </div>

      <div class="mapping-table-wrapper">
        <table class="mapping-table">
          <thead>
            <tr>
              <th style="width: 24%">原始字段名</th>
              <th style="width: 32%">样本数据 (前 3 条)</th>
              <th style="width: 28%">映射到系统字段</th>
              <th style="width: 16%">向下填充 (合并单元格)</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="col in columns" :key="col.columnIndex">
              <td class="col-header-cell">
                <div class="col-name">{{ col.excelHeader }}</div>
                <div class="col-idx">第 {{ col.columnIndex + 1 }} 列</div>
              </td>
              <td class="col-sample-cell">
                <div v-if="col.sampleValues.length > 0" class="sample-tags">
                  <span
                    v-for="(val, idx) in col.sampleValues.slice(0, 3)"
                    :key="idx"
                    class="sample-tag"
                  >
                    {{ val }}
                  </span>
                </div>
                <span v-else class="empty-sample">无样本值</span>
              </td>
              <td class="col-mapping-cell">
                <el-select
                  v-model="col.selectedField"
                  size="small"
                  style="width: 100%"
                  :class="{
                    matched: col.selectedField !== 'ignore',
                    conflict: getConflictCols(col).length > 0,
                  }"
                >
                  <el-option
                    v-for="f in availableSystemFields"
                    :key="f.value"
                    :label="f.label"
                    :value="f.value"
                  />
                </el-select>
                <div v-if="getConflictCols(col).length > 0" class="conflict-badge">
                  ⚠️ 与第 {{ getConflictCols(col).join('、') }} 列重复映射，导入时将互相覆盖
                </div>
                <div v-else-if="isTrialField(col.selectedField)" class="trial-merge-badge">
                  📅 庭审时间（多列自动按首期/二次/三次顺延沉淀，不覆盖）
                </div>
                <div
                  v-else-if="col.confidence > 0.7 && col.selectedField !== 'ignore' && col.selectedField !== 'notes'"
                  class="match-badge"
                >
                  ✓ 智能匹配 ({{ Math.round(col.confidence * 100) }}%)
                </div>
                <div v-else-if="col.selectedField === 'notes'" class="notes-merge-badge">
                  📝 多列附注合并追加（不会丢失）
                </div>
              </td>
              <td class="col-fill-cell">
                <el-switch
                  v-model="col.forwardFill"
                  size="small"
                  title="当遇到空白行时，自动继承上方数据"
                />
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- ═══ 步骤 3: 清洗预览与查重策略 ═══ -->
    <div v-if="currentStep === 3" class="step-content step-3">
      <!-- 策略配置卡片 (仅案件主表显示冲突策略) -->
      <div v-if="targetEntity === 'cases'" class="strategy-card">
        <div class="strategy-section">
          <div class="section-title">查重与冲突处理策略</div>
          <el-radio-group v-model="conflictStrategy">
            <el-radio value="skip">
              <span class="radio-title">跳过已有案件 (推荐)</span>
              <span class="radio-desc">案号或案名存在冲突时自动跳过，绝不修改已有案件</span>
            </el-radio>
            <el-radio value="update">
              <span class="radio-title">覆盖更新已有案件</span>
              <span class="radio-desc">根据案号匹配，使用新数据更新已有案件的对应字段</span>
            </el-radio>
            <el-radio value="duplicate">
              <span class="radio-title">允许创建副本</span>
              <span class="radio-desc">即使案号或案名相同，也强制作为全新案件插入</span>
            </el-radio>
          </el-radio-group>
        </div>

        <div class="strategy-section">
          <div class="section-title">默认业务赛道</div>
          <el-select v-model="defaultTrack" size="small" style="width: 200px">
            <el-option label="民事诉讼 (民事侵权/合同纠纷)" value="civil_tort" />
            <el-option label="专利无效 (国知局口审/复审)" value="patent_invalidation" />
            <el-option label="行政诉讼 (不服行政决定起诉)" value="admin_litigation" />
            <el-option label="其他业务" value="other" />
          </el-select>
        </div>
      </div>
      <div v-else class="strategy-card">
        <div class="strategy-section">
          <div class="section-title">关联规则与对齐策略</div>
          <div class="text-sm text-text-secondary">
            系统将根据映射的「关联法院案号」或「关联案件名称」在本地数据库中自动寻址匹配主案件。若主案件未找到，将作为独立通用实体入库，绝不丢弃任何一行数据。
          </div>
        </div>
      </div>

      <!-- 实时预览表格 -->
      <div class="preview-section">
        <div class="preview-header">
          <div class="preview-title">清洗后数据预览 (前 10 行样本)</div>
          <div class="preview-hint">日期、金额与承办人已按照底层算法完成格式归一化</div>
        </div>

        <div class="preview-table-wrapper">
          <el-table :data="previewRows" size="small" border stripe max-height="240">
            <el-table-column
              v-for="col in columns.filter(c => c.selectedField !== 'ignore')"
              :key="col.columnIndex"
              :prop="col.excelHeader"
              :label="`${col.excelHeader} → ${col.selectedField}`"
              min-width="140"
              show-overflow-tooltip
            />
          </el-table>
        </div>
      </div>
    </div>

    <!-- ═══ 步骤 4: 导入报告 ═══ -->
    <div v-if="currentStep === 4 && importReport" class="step-content step-4">
      <div class="report-header">
        <el-icon class="success-icon" :size="56"><CircleCheck /></el-icon>
        <h3 class="report-title">
          {{ targetEntity === 'cases' ? '案件数据导入完成' : '关联分表导入完成' }}
        </h3>
        <p class="report-subtitle">
          共处理 {{ importReport.totalRowsProcessed }} 行数据，所有变更已通过单事务安全写入本地加密数据库。
        </p>
      </div>

      <!-- 案件主表统计 -->
      <div v-if="targetEntity === 'cases'" class="report-stats-grid">
        <div class="stat-box success">
          <div class="stat-num">{{ importReport.createdCount }}</div>
          <div class="stat-label">新增案件</div>
        </div>
        <div class="stat-box info">
          <div class="stat-num">{{ importReport.updatedCount || 0 }}</div>
          <div class="stat-label">更新已有案件</div>
        </div>
        <div class="stat-box warning">
          <div class="stat-num">{{ importReport.skippedCount || 0 }}</div>
          <div class="stat-label">跳过 (重复/空行/汇总)</div>
        </div>
        <div class="stat-box danger" v-if="importReport.failedCount > 0">
          <div class="stat-num">{{ importReport.failedCount }}</div>
          <div class="stat-label">失败行数</div>
        </div>
      </div>

      <!-- 关联分表统计 -->
      <div v-else class="report-stats-grid">
        <div class="stat-box success">
          <div class="stat-num">{{ importReport.createdCount }}</div>
          <div class="stat-label">成功入库记录</div>
        </div>
        <div class="stat-box info">
          <div class="stat-num">{{ importReport.linkedCasesCount || 0 }}</div>
          <div class="stat-label">成功关联至对应案件</div>
        </div>
        <div class="stat-box warning" v-if="(importReport.unlinkedCount || 0) > 0">
          <div class="stat-num">{{ importReport.unlinkedCount }}</div>
          <div class="stat-label">未匹配到主案件 (已转为全局待办)</div>
        </div>
        <div class="stat-box danger" v-if="importReport.failedCount > 0">
          <div class="stat-num">{{ importReport.failedCount }}</div>
          <div class="stat-label">失败行数</div>
        </div>
      </div>

      <div v-if="importReport.errors.length > 0" class="error-log-section">
        <div class="error-log-title">异常明细 ({{ importReport.errors.length }} 条)</div>
        <div class="error-log-box">
          <div v-for="(err, idx) in importReport.errors" :key="idx" class="error-log-item">
            {{ err }}
          </div>
        </div>
      </div>
    </div>

    <!-- 底部操作按钮 -->
    <template #footer>
      <div class="dialog-footer">
        <div class="footer-left">
          <el-button v-if="currentStep > 1 && currentStep < 4" @click="currentStep--">
            <el-icon><Back /></el-icon>
            <span>上一步</span>
          </el-button>
        </div>

        <div class="footer-right">
          <el-button v-if="currentStep < 4" @click="visible = false">取消</el-button>

          <el-button
            v-if="currentStep === 1"
            type="primary"
            :disabled="
              (importSource === 'excel' && (!fileName || !selectedExcelSheet)) ||
              (importSource === 'feishu' && columns.length === 0)
            "
            @click="goToStep2"
          >
            <span>下一步: 配置字段映射</span>
            <el-icon><Right /></el-icon>
          </el-button>

          <el-button
            v-if="currentStep === 2"
            type="primary"
            :disabled="!hasRequiredField"
            @click="goToStep3"
          >
            <span>下一步: 预览与策略</span>
            <el-icon><Right /></el-icon>
          </el-button>

          <el-button
            v-if="currentStep === 3"
            type="primary"
            :loading="importing"
            @click="executeImport"
          >
            <el-icon><Check /></el-icon>
            <span>确认执行导入</span>
          </el-button>

          <template v-if="currentStep === 4">
            <el-button type="success" :icon="Plus" @click="continueWithOtherSheet">
              继续导入其他分表 (任务/庭审/日志)
            </el-button>
            <el-button type="primary" @click="visible = false">
              完成并关闭
            </el-button>
          </template>
        </div>
      </div>
    </template>
  </el-dialog>
</template>

<style scoped>
.case-import-dialog :deep(.el-dialog__body) {
  padding: 16px 24px;
}

/* 步骤导航条 */
.wizard-steps-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 20px;
  padding: 10px 16px;
  background: var(--c-bg-subtle, #f8fafc);
  border-radius: 8px;
  border: 1px solid var(--c-border, #e2e8f0);
}

.step-item {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--c-text-muted, #94a3b8);
}

.step-item.active {
  color: var(--c-primary, #2563eb);
  font-weight: 600;
}

.step-item.done {
  color: var(--c-success, #16a34a);
}

.step-badge {
  width: 22px;
  height: 22px;
  border-radius: 50%;
  background: var(--c-border, #cbd5e1);
  color: #fff;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 11px;
  font-weight: 600;
}

.step-item.active .step-badge {
  background: var(--c-primary, #2563eb);
}

.step-item.done .step-badge {
  background: var(--c-success, #16a34a);
}

.step-connector {
  flex: 1;
  height: 2px;
  background: var(--c-border, #e2e8f0);
  margin: 0 10px;
}

.step-connector.active {
  background: var(--c-success, #16a34a);
}

/* 实体/分表选择 */
.entity-select-section {
  margin-bottom: 16px;
}

.section-badge-label {
  font-size: 12px;
  font-weight: 600;
  color: var(--c-text-muted, #64748b);
  margin-bottom: 8px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.entity-options-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 10px;
}

.entity-card {
  padding: 10px 12px;
  border-radius: 8px;
  border: 1px solid var(--c-border, #e2e8f0);
  background: var(--c-bg-card, #ffffff);
  cursor: pointer;
  transition: all 0.2s ease;
}

.entity-card:hover {
  border-color: var(--c-primary, #2563eb);
  background: var(--c-bg-subtle, #f8fafc);
}

.entity-card.active {
  border-color: var(--c-primary, #2563eb);
  background: var(--c-primary-subtle, #eff6ff);
  box-shadow: 0 0 0 1px var(--c-primary, #2563eb);
}

.entity-card-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--c-text-main, #1e293b);
  margin-bottom: 2px;
}

.entity-card-desc {
  font-size: 11px;
  color: var(--c-text-muted, #64748b);
  line-height: 1.3;
}

/* 数据源切换 Tab */
.source-toggle-bar {
  display: flex;
  gap: 12px;
  margin-bottom: 18px;
}

.source-tab-btn {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 10px 16px;
  border-radius: 8px;
  border: 1px solid var(--c-border, #e2e8f0);
  background: var(--c-bg-subtle, #f8fafc);
  color: var(--c-text-main, #334155);
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;
}

.source-tab-btn:hover {
  background: var(--c-bg-card, #ffffff);
  border-color: var(--c-primary, #2563eb);
}

.source-tab-btn.active {
  background: var(--c-primary-subtle, #eff6ff);
  border-color: var(--c-primary, #2563eb);
  color: var(--c-primary, #2563eb);
  font-weight: 600;
}

.feishu-badge {
  font-size: 10px;
  background: #dcfce7;
  color: #15803d;
  padding: 1px 6px;
  border-radius: 10px;
}

/* 步骤 1: Excel 模式 */
.upload-dropzone {
  border: 2px dashed var(--c-border-strong, #cbd5e1);
  border-radius: 12px;
  padding: 32px 24px;
  text-align: center;
  cursor: pointer;
  background: var(--c-bg-card, #ffffff);
  transition: all 0.2s ease;
}

.upload-dropzone:hover {
  border-color: var(--c-primary, #2563eb);
  background: var(--c-primary-subtle, #eff6ff);
}

.dropzone-icon {
  color: var(--c-primary, #2563eb);
  margin-bottom: 8px;
}

.dropzone-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--c-text-main, #1e293b);
  margin-bottom: 4px;
}

.dropzone-sub {
  font-size: 12px;
  color: var(--c-text-muted, #64748b);
}

.file-info-card {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-top: 14px;
  padding: 10px 14px;
  background: var(--c-bg-subtle, #f8fafc);
  border: 1px solid var(--c-border, #e2e8f0);
  border-radius: 8px;
}

.file-icon-box {
  width: 36px;
  height: 36px;
  border-radius: 6px;
  background: var(--c-primary-subtle, #eff6ff);
  color: var(--c-primary, #2563eb);
  display: flex;
  align-items: center;
  justify-content: center;
}

.file-details {
  flex: 1;
  overflow: hidden;
}

.file-name {
  font-weight: 600;
  font-size: 13px;
  color: var(--c-text-main, #1e293b);
}

.file-path {
  font-size: 11px;
  color: var(--c-text-muted, #64748b);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.sheet-config-section {
  margin-top: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 14px;
  background: var(--c-bg-card, #ffffff);
  border: 1px solid var(--c-border, #e2e8f0);
  border-radius: 8px;
}

.form-row {
  display: flex;
  align-items: center;
  gap: 12px;
}

.form-label {
  font-size: 13px;
  font-weight: 500;
  color: var(--c-text-main, #334155);
}

.hint-text {
  font-size: 12px;
  color: var(--c-text-muted, #64748b);
}

/* 步骤 1: 飞书模式 */
.feishu-guide-card {
  background: #f0f9ff;
  border: 1px solid #bae6fd;
  border-radius: 8px;
  padding: 14px 16px;
  margin-bottom: 16px;
}

.guide-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 10px;
}

.guide-icon {
  color: #0284c7;
}

.guide-title {
  font-weight: 600;
  font-size: 13px;
  color: #0369a1;
  flex: 1;
}

.guide-steps-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: 12px;
}

.guide-step {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  font-size: 12px;
  color: #334155;
}

.g-num {
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: #0284c7;
  color: #fff;
  font-size: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  margin-top: 1px;
}

.link-styled {
  color: #0284c7;
  text-decoration: underline;
}

.feishu-cred-form {
  background: #ffffff;
  border: 1px solid #e0f2fe;
  border-radius: 6px;
  padding: 12px;
}

.form-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
  margin-bottom: 10px;
}

.feishu-connected-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  background: #f0fdf4;
  border: 1px solid #bbf7d0;
  border-radius: 8px;
  padding: 10px 14px;
  margin-bottom: 16px;
}

.conn-status {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  font-weight: 600;
  color: #15803d;
}

.check-ico {
  color: #16a34a;
}

.feishu-url-section {
  display: flex;
  flex-direction: column;
  gap: 12px;
  background: var(--c-bg-card, #ffffff);
  border: 1px solid var(--c-border, #e2e8f0);
  border-radius: 8px;
  padding: 16px;
}

.section-title-box {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.url-hint {
  font-size: 11px;
  color: var(--c-text-muted, #64748b);
}

.url-input-group {
  display: flex;
  gap: 10px;
}

.permission-tip-box {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  background: #fefce8;
  border: 1px solid #fef08a;
  border-radius: 6px;
  padding: 8px 12px;
  font-size: 12px;
  color: #854d0e;
}

.tip-ico {
  color: #ca8a04;
  margin-top: 2px;
}

.feishu-error-alert {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  background: #fef2f2;
  border: 1px solid #fecaca;
  border-radius: 6px;
  padding: 10px 12px;
  color: #b91c1c;
  font-size: 12px;
}

.feishu-error-alert pre {
  margin: 0;
  white-space: pre-wrap;
  font-family: inherit;
}

.feishu-subtables-row {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-top: 4px;
}

/* 步骤 2: 字段映射 */
.mapping-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}

.mapping-summary {
  font-size: 13px;
  color: var(--c-text-main, #334155);
}

.mapping-alert {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--c-warning, #d97706);
  font-size: 12px;
  font-weight: 500;
}

.mapping-table-wrapper {
  max-height: 400px;
  overflow-y: auto;
  border: 1px solid var(--c-border, #e2e8f0);
  border-radius: 8px;
}

.mapping-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}

.mapping-table th {
  background: var(--c-bg-subtle, #f8fafc);
  padding: 10px 12px;
  text-align: left;
  font-weight: 600;
  color: var(--c-text-main, #475569);
  border-bottom: 1px solid var(--c-border, #e2e8f0);
  position: sticky;
  top: 0;
  z-index: 1;
}

.mapping-table td {
  padding: 8px 12px;
  border-bottom: 1px solid var(--c-border, #f1f5f9);
  vertical-align: middle;
}

.col-header-cell .col-name {
  font-weight: 600;
  color: var(--c-text-main, #1e293b);
}

.col-header-cell .col-idx {
  font-size: 11px;
  color: var(--c-text-muted, #94a3b8);
}

.sample-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.sample-tag {
  background: var(--c-bg-subtle, #f1f5f9);
  padding: 2px 6px;
  border-radius: 4px;
  font-size: 11px;
  color: var(--c-text-muted, #475569);
  max-width: 160px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.match-badge {
  font-size: 11px;
  color: var(--c-success, #16a34a);
  margin-top: 2px;
}

.conflict-badge {
  font-size: 11px;
  color: var(--c-danger, #ef4444);
  background: #fef2f2;
  border-radius: 4px;
  padding: 2px 4px;
  margin-top: 2px;
}

.notes-merge-badge {
  font-size: 11px;
  color: #2563eb;
  background: #eff6ff;
  border-radius: 4px;
  padding: 2px 4px;
  margin-top: 2px;
}

.trial-merge-badge {
  font-size: 11px;
  color: #059669;
  background: #ecfdf5;
  border-radius: 4px;
  padding: 2px 4px;
  margin-top: 2px;
}

/* 步骤 3: 查重与预览 */
.strategy-card {
  background: var(--c-bg-subtle, #f8fafc);
  border: 1px solid var(--c-border, #e2e8f0);
  border-radius: 8px;
  padding: 14px 16px;
  margin-bottom: 16px;
}

.strategy-section {
  margin-bottom: 14px;
}

.strategy-section:last-child {
  margin-bottom: 0;
}

.section-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--c-text-main, #1e293b);
  margin-bottom: 8px;
}

.strategy-section :deep(.el-radio-group) {
  display: flex;
  flex-direction: column;
  gap: 6px;
  align-items: flex-start;
}

.radio-title {
  font-weight: 600;
  margin-right: 8px;
}

.radio-desc {
  font-size: 12px;
  color: var(--c-text-muted, #64748b);
}

.preview-section {
  border: 1px solid var(--c-border, #e2e8f0);
  border-radius: 8px;
  padding: 12px;
}

.preview-header {
  margin-bottom: 8px;
}

.preview-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--c-text-main, #1e293b);
}

.preview-hint {
  font-size: 11px;
  color: var(--c-text-muted, #64748b);
}

/* 步骤 4: 报告 */
.report-header {
  text-align: center;
  padding: 16px 0;
}

.success-icon {
  color: var(--c-success, #16a34a);
  margin-bottom: 10px;
}

.report-title {
  font-size: 18px;
  font-weight: 700;
  color: var(--c-text-main, #1e293b);
  margin-bottom: 4px;
}

.report-subtitle {
  font-size: 13px;
  color: var(--c-text-muted, #64748b);
}

.report-stats-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 14px;
  margin: 16px 0;
}

.stat-box {
  background: var(--c-bg-subtle, #f8fafc);
  border: 1px solid var(--c-border, #e2e8f0);
  border-radius: 8px;
  padding: 14px;
  text-align: center;
}

.stat-box.success {
  border-color: #bbf7d0;
  background: #f0fdf4;
}

.stat-box.success .stat-num {
  color: var(--c-success, #16a34a);
}

.stat-box.info .stat-num {
  color: var(--c-primary, #2563eb);
}

.stat-box.warning .stat-num {
  color: var(--c-warning, #d97706);
}

.stat-box.danger .stat-num {
  color: var(--c-danger, #ef4444);
}

.stat-num {
  font-size: 22px;
  font-weight: 700;
  margin-bottom: 4px;
}

.stat-label {
  font-size: 11px;
  color: var(--c-text-muted, #64748b);
}

.error-log-section {
  margin-top: 14px;
  background: #fef2f2;
  border: 1px solid #fecaca;
  border-radius: 8px;
  padding: 12px 14px;
}

.error-log-title {
  font-size: 13px;
  font-weight: 600;
  color: #b91c1c;
  margin-bottom: 6px;
}

.error-log-box {
  max-height: 120px;
  overflow-y: auto;
  font-size: 12px;
  color: #991b1b;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

/* 底部操作 */
.dialog-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.footer-right {
  display: flex;
  align-items: center;
  gap: 8px;
}
</style>
