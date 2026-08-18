<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { tauriCallSafe } from '../../../core/tauriBridge'
import { ElMessage } from 'element-plus'

// === 飞书导入 (legacy JSON dump) ===
const importing = ref(false)
const importResult = ref(null)

async function importFeishuData() {
  const { open } = await import('@tauri-apps/plugin-dialog')
  const selected = await open({
    multiple: false,
    filters: [{ name: 'JSON', extensions: ['json'] }],
  })
  if (!selected) return

  importing.value = true
  importResult.value = null
  const result = await tauriCallSafe('import_feishu_data', { jsonPath: selected })
  importing.value = false

  if (result.ok) {
    importResult.value = result.data
    ElMessage.success('导入完成')
  } else {
    ElMessage.error(result.error || '导入失败')
  }
}

// === 飞书同步配置 ===
const feishuAppId = ref('')
const feishuAppSecret = ref('')
const feishuAppToken = ref('')
const configuring = ref(false)
const testing = ref(false)
const connectionStatus = ref(null)

const syncInfo = ref({
  configured: false,
  lastPullAt: null,
  lastPushAt: null,
  lastPullCount: null,
  lastPushCount: null,
  appToken: null,
  tableId: null,
})

// === v3.0: 表结构发现 ===
const discovering = ref(false)
const discoveredTables = ref([])
const selectedTableId = ref('')
const selectedTableFields = ref([])

const loadingFields = ref(false)
const comparingTable = ref(false)
const comparingRecords = ref(false)

// 比较结果
const schemaDiff = ref(null)
const recordDiff = ref(null)

// 映射配置
const fieldMappings = ref([])
const localTable = ref('cases')
const matchField = ref('案件信息')
const savingMappings = ref(false)

// 导入
const importAllLoading = ref(false)
const importResult2 = ref(null)
const incrementalSince = ref('')

async function loadSyncInfo() {
  const result = await tauriCallSafe('get_feishu_sync_info')
  if (result.ok) {
    syncInfo.value = result.data
    if (result.data.appToken) feishuAppToken.value = result.data.appToken
  }
}

async function saveCredentials() {
  if (!feishuAppId.value.trim() || !feishuAppSecret.value.trim()) {
    ElMessage.warning('请填写 App ID 和 App Secret')
    return
  }
  configuring.value = true
  const result = await tauriCallSafe('configure_feishu', {
    appId: feishuAppId.value.trim(),
    appSecret: feishuAppSecret.value.trim(),
  })
  configuring.value = false

  if (result.ok) {
    ElMessage.success('凭证已保存')
    connectionStatus.value = null
    await loadSyncInfo()
  } else {
    ElMessage.error(result.error || '保存失败')
  }
}

async function testConnection() {
  testing.value = true
  connectionStatus.value = null
  const result = await tauriCallSafe('test_feishu_connection')
  testing.value = false

  if (result.ok) {
    connectionStatus.value = 'ok'
    ElMessage.success(result.data)
  } else {
    connectionStatus.value = 'fail'
    ElMessage.error(result.error || '连接失败')
  }
}

// === v3.0: 表发现 ===
async function discoverTables() {
  if (!feishuAppToken.value.trim()) {
    ElMessage.warning('请先填写 App Token')
    return
  }
  discovering.value = true
  discoveredTables.value = []
  const result = await tauriCallSafe('feishu_list_tables', {
    appToken: feishuAppToken.value.trim(),
  })
  discovering.value = false

  if (result.ok) {
    discoveredTables.value = result.data || []
    ElMessage.success(`发现 ${discoveredTables.value.length} 张表`)
  } else {
    ElMessage.error(result.error || '获取表列表失败')
  }
}

async function loadTableFields(tableId) {
  selectedTableId.value = tableId
  loadingFields.value = true
  selectedTableFields.value = []
  const result = await tauriCallSafe('feishu_list_fields', {
    appToken: feishuAppToken.value.trim(),
    tableId,
  })
  loadingFields.value = false

  if (result.ok) {
    selectedTableFields.value = result.data || []
    // 自动生成映射建议
    buildAutoMappings()
  } else {
    ElMessage.error(result.error || '获取字段列表失败')
  }
}

// === v3.0: 自动映射 ===
function buildAutoMappings() {
  const knownMappings = {
    '案件信息': { col: 'case_name', type: 'TEXT' },
    '案号': { col: 'case_no', type: 'TEXT' },
    '内部卷号': { col: 'internal_no', type: 'TEXT' },
    '案由': { col: 'cause_action', type: 'TEXT' },
    '客户名称': { col: 'client_name', type: 'TEXT' },
    '我方诉讼地位': { col: 'our_role', type: 'TEXT' },
    '对方名称': { col: 'opponent_name', type: 'TEXT' },
    '诉讼地位': { col: 'opponent_role', type: 'TEXT' },
    '对方代理律所': { col: 'opponent_firm', type: 'TEXT' },
    '对方代理人': { col: 'opponent_agent', type: 'TEXT' },
    '审理机关': { col: 'court', type: 'TEXT' },
    '合议庭': { col: 'judge_panel', type: 'TEXT' },
    '书记员': { col: 'clerk', type: 'TEXT' },
    '代理人': { col: 'attorneys', type: 'TEXT' },
    '审级': { col: 'case_level', type: 'TEXT' },
    '案件进展': { col: 'case_progress', type: 'TEXT' },
    '案件结果': { col: 'case_result', type: 'TEXT' },
    '专利名称': { col: 'patent_name', type: 'TEXT' },
    '专利申请号': { col: 'patent_app_no', type: 'TEXT' },
    '诉讼程序': { col: 'procedure_type', type: 'TEXT' },
    '立案': { col: 'filing_date', type: 'TEXT' },
    '收到起诉状时间': { col: 'complaint_received_date', type: 'TEXT' },
    '开庭|口审': { col: 'trial_date', type: 'TEXT' },
    '二次开庭|口审': { col: 'trial2_date', type: 'TEXT' },
    '三次开庭丨口审': { col: 'trial3_date', type: 'TEXT' },
    '收到判决/裁定/决定类型': { col: 'verdict_type', type: 'TEXT' },
    '收到判决/裁定/决定时间': { col: 'verdict_date', type: 'TEXT' },
    '备注': { col: 'notes', type: 'TEXT' },
  }

  const typeNameMap = {
    1: 'Text', 2: 'Number', 3: 'SingleSelect', 4: 'MultiSelect',
    5: 'DateTime', 7: 'Checkbox', 11: 'User', 13: 'Phone',
    15: 'Url', 17: 'Attachment', 18: 'SingleLink', 19: 'Lookup',
    20: 'Formula', 21: 'DuplexLink', 1001: 'CreatedTime', 1002: 'ModifiedTime',
  }

  fieldMappings.value = selectedTableFields.value.map((f) => {
    const known = knownMappings[f.fieldName]
    const isFormula = f.fieldType === 20
    const isLink = f.fieldType === 18 || f.fieldType === 21
    const isLookup = f.fieldType === 19

    let syncDir = 'bidirectional'
    if (isFormula || isLookup) syncDir = 'pull_only'
    else if (isLink) syncDir = 'none'

    return {
      feishuFieldName: f.fieldName,
      feishuFieldType: f.fieldType,
      feishuTypeName: typeNameMap[f.fieldType] || 'Unknown',
      feishuFieldId: f.fieldId,
      localColumn: known ? known.col : '',
      localType: known ? known.type : '',
      matched: !!known,
      syncDirection: syncDir,
      isFormula,
      isLink,
      isLookup,
      selected: !!known,
    }
  })
}

// === v3.0: Schema 比较 ===
async function compareSchema() {
  if (!selectedTableId.value || !localTable.value) {
    ElMessage.warning('请先选择飞书表和本地表')
    return
  }
  comparingTable.value = true
  schemaDiff.value = null
  const result = await tauriCallSafe('feishu_compare_table', {
    appToken: feishuAppToken.value.trim(),
    tableId: selectedTableId.value,
    localTable: localTable.value,
  })
  comparingTable.value = false

  if (result.ok) {
    schemaDiff.value = result.data
    ElMessage.success(
      `映射: ${result.data.mapped?.length || 0} | ` +
      `飞书新增: ${result.data.feishuOnly?.length || 0} | ` +
      `仅本地: ${result.data.localOnly?.length || 0} | ` +
      `类型冲突: ${result.data.typeConflict?.length || 0}`
    )
  } else {
    ElMessage.error(result.error || '比较失败')
  }
}

// === v3.0: 记录比较 ===
async function compareRecords() {
  if (!selectedTableId.value || !localTable.value || !matchField.value) {
    ElMessage.warning('请先选择飞书表、本地表和匹配字段')
    return
  }
  comparingRecords.value = true
  recordDiff.value = null
  const result = await tauriCallSafe('feishu_compare_records', {
    appToken: feishuAppToken.value.trim(),
    tableId: selectedTableId.value,
    localTable: localTable.value,
    matchField: matchField.value,
  })
  comparingRecords.value = false

  if (result.ok) {
    recordDiff.value = result.data
    ElMessage.success(
      `相同: ${result.data.same?.length || 0} | ` +
      `仅飞书: ${result.data.feishuOnly?.length || 0} | ` +
      `仅本地: ${result.data.localOnly?.length || 0} | ` +
      `冲突: ${result.data.conflict?.length || 0}`
    )
  } else {
    ElMessage.error(result.error || '比较失败')
  }
}

// === v3.0: 保存映射 ===
async function saveMappings() {
  const activeMappings = fieldMappings.value.filter((m) => m.selected && m.localColumn)
  if (activeMappings.length === 0) {
    ElMessage.warning('请至少选择一个字段映射')
    return
  }
  savingMappings.value = true
  const payload = activeMappings.map((m) => ({
    connectionId: 'default',
    feishuTableId: selectedTableId.value,
    feishuFieldId: m.feishuFieldId,
    feishuFieldName: m.feishuFieldName,
    feishuFieldType: m.feishuFieldType,
    localTable: localTable.value,
    localColumn: m.localColumn,
    syncDirection: m.syncDirection,
    isFormula: m.isFormula ? 1 : 0,
    isLink: m.isLink ? 1 : 0,
    isLookup: m.isLookup ? 1 : 0,
  }))
  const result = await tauriCallSafe('feishu_save_mappings', { mappingsJson: payload })
  savingMappings.value = false

  if (result.ok) {
    ElMessage.success(result.data)
  } else {
    ElMessage.error(result.error || '保存映射失败')
  }
}

// === v3.0: 全量导入 ===
async function doImportAll() {
  const activeMappings = fieldMappings.value.filter((m) => m.selected && m.localColumn)
  if (activeMappings.length === 0) {
    ElMessage.warning('请先配置字段映射')
    return
  }
  importAllLoading.value = true
  importResult2.value = null
  const payload = activeMappings.map((m) => ({
    feishuFieldName: m.feishuFieldName,
    feishuFieldType: m.feishuFieldType,
    localColumn: m.localColumn,
    syncDirection: m.syncDirection,
    isFormula: m.isFormula,
    isLink: m.isLink,
  }))
  const result = await tauriCallSafe('feishu_import_all', {
    appToken: feishuAppToken.value.trim(),
    tableId: selectedTableId.value,
    localTable: localTable.value,
    mappingsJson: payload,
  })
  importAllLoading.value = false

  if (result.ok) {
    importResult2.value = result.data
    ElMessage.success(`导入完成: 新建 ${result.data.created}, 更新 ${result.data.updated}`)
  } else {
    ElMessage.error(result.error || '导入失败')
  }
}

// === v3.0: 增量导入 ===
async function doImportIncremental() {
  if (!incrementalSince.value) {
    ElMessage.warning('请输入增量起始时间')
    return
  }
  const activeMappings = fieldMappings.value.filter((m) => m.selected && m.localColumn)
  if (activeMappings.length === 0) {
    ElMessage.warning('请先配置字段映射')
    return
  }
  importAllLoading.value = true
  importResult2.value = null
  const payload = activeMappings.map((m) => ({
    feishuFieldName: m.feishuFieldName,
    feishuFieldType: m.feishuFieldType,
    localColumn: m.localColumn,
    syncDirection: m.syncDirection,
    isFormula: m.isFormula,
    isLink: m.isLink,
  }))
  const result = await tauriCallSafe('feishu_import_incremental', {
    appToken: feishuAppToken.value.trim(),
    tableId: selectedTableId.value,
    localTable: localTable.value,
    sinceTimestamp: incrementalSince.value,
    mappingsJson: payload,
  })
  importAllLoading.value = false

  if (result.ok) {
    importResult2.value = result.data
    ElMessage.success(`增量导入完成: 新建 ${result.data.created}, 更新 ${result.data.updated}`)
  } else {
    ElMessage.error(result.error || '增量导入失败')
  }
}

const mappedCount = computed(() => fieldMappings.value.filter((m) => m.selected && m.localColumn).length)
const totalFields = computed(() => fieldMappings.value.length)

onMounted(() => {
  loadSyncInfo()
})
</script>

<template>
  <div class="tab-content">
    <!-- 数据导入 (legacy) -->
    <el-card>
      <template #header><strong>📊 数据导入 (JSON)</strong></template>
      <p>从飞书多维表格导出的 JSON 文件导入案件数据。</p>
      <el-button type="primary" :loading="importing" @click="importFeishuData">
        导入飞书数据
      </el-button>

      <div v-if="importResult" class="import-result">
        <el-divider />
        <h4>导入结果</h4>
        <ul>
          <li>案件: {{ importResult.cases }} 条</li>
          <li>日志: {{ importResult.logs }} 条</li>
          <li>庭审: {{ importResult.hearings }} 条</li>
          <li>任务: {{ importResult.tasks }} 条</li>
          <li>人员: {{ importResult.officials }} 条</li>
        </ul>
        <div v-if="importResult.errors?.length" class="import-errors">
          <h4>错误</h4>
          <ul>
            <li v-for="(err, i) in importResult.errors" :key="i" class="error-item">{{ err }}</li>
          </ul>
        </div>
      </div>
    </el-card>

    <!-- 飞书凭证配置 -->
    <el-card style="margin-top: 16px">
      <template #header>
        <div class="card-header">
          <strong>🔑 飞书凭证</strong>
          <el-tag v-if="syncInfo.configured" type="success" size="small">已配置</el-tag>
          <el-tag v-else type="info" size="small">未配置</el-tag>
        </div>
      </template>

      <el-form label-width="100px" size="default">
        <el-form-item label="App ID">
          <el-input v-model="feishuAppId" placeholder="飞书自建应用的 App ID" type="password" show-password />
        </el-form-item>
        <el-form-item label="App Secret">
          <el-input v-model="feishuAppSecret" placeholder="飞书自建应用的 App Secret" type="password" show-password />
        </el-form-item>
        <el-form-item>
          <el-button type="primary" :loading="configuring" @click="saveCredentials">保存凭证</el-button>
          <el-button
            :loading="testing"
            @click="testConnection"
            :type="connectionStatus === 'ok' ? 'success' : connectionStatus === 'fail' ? 'danger' : 'default'"
          >
            {{ connectionStatus === 'ok' ? '✓ 连接正常' : connectionStatus === 'fail' ? '✗ 连接失败' : '测试连接' }}
          </el-button>
        </el-form-item>
      </el-form>
    </el-card>

    <!-- v3.0: 多维表格发现 -->
    <el-card style="margin-top: 16px">
      <template #header>
        <div class="card-header">
          <strong>🔍 表结构发现 (v3.0)</strong>
        </div>
      </template>

      <el-form label-width="100px" size="default">
        <el-form-item label="App Token">
          <el-input v-model="feishuAppToken" placeholder="多维表格的 App Token（URL 中获取）" />
        </el-form-item>
        <el-form-item>
          <el-button type="primary" :loading="discovering" @click="discoverTables">
            发现所有表
          </el-button>
        </el-form-item>
      </el-form>

      <!-- 表列表 -->
      <div v-if="discoveredTables.length > 0" class="table-list">
        <h4>多维表格中的表 ({{ discoveredTables.length }})</h4>
        <el-table :data="discoveredTables" size="small" stripe highlight-current-row
          @current-change="(row) => row && loadTableFields(row.tableId)">
          <el-table-column prop="tableId" label="Table ID" width="200" />
          <el-table-column prop="name" label="表名" />
          <el-table-column prop="revision" label="版本" width="80" />
          <el-table-column label="操作" width="100">
            <template #default="{ row }">
              <el-button
                size="small"
                :type="selectedTableId === row.tableId ? 'success' : 'primary'"
                :loading="loadingFields && selectedTableId === row.tableId"
                @click.stop="loadTableFields(row.tableId)"
              >
                {{ selectedTableId === row.tableId ? '已选' : '选择' }}
              </el-button>
            </template>
          </el-table-column>
        </el-table>
      </div>

      <!-- 字段列表 -->
      <div v-if="selectedTableFields.length > 0" class="fields-list">
        <h4>字段列表 ({{ selectedTableFields.length }} 个字段)</h4>
        <el-table :data="selectedTableFields" size="small" stripe max-height="300">
          <el-table-column prop="fieldId" label="Field ID" width="120" />
          <el-table-column prop="fieldName" label="字段名" />
          <el-table-column prop="fieldType" label="类型码" width="80" />
          <el-table-column label="类型名" width="120">
            <template #default="{ row }">
              <el-tag size="small">{{ { 1:'Text', 2:'Number', 3:'SingleSelect', 4:'MultiSelect', 5:'DateTime', 7:'Checkbox', 11:'User', 13:'Phone', 15:'Url', 17:'Attachment', 18:'SingleLink', 19:'Lookup', 20:'Formula', 21:'DuplexLink' }[row.fieldType] || 'Other' }}</el-tag>
            </template>
          </el-table-column>
          <el-table-column label="主键" width="60">
            <template #default="{ row }">
              <span v-if="row.isPrimary">✅</span>
            </template>
          </el-table-column>
        </el-table>
      </div>
    </el-card>

    <!-- v3.0: 字段映射 -->
    <el-card v-if="fieldMappings.length > 0" style="margin-top: 16px">
      <template #header>
        <div class="card-header">
          <strong>🔗 字段映射</strong>
          <el-tag size="small">{{ mappedCount }}/{{ totalFields }} 已映射</el-tag>
        </div>
      </template>

      <el-form inline size="small" style="margin-bottom: 12px">
        <el-form-item label="本地表">
          <el-input v-model="localTable" style="width: 150px" />
        </el-form-item>
        <el-form-item label="匹配字段">
          <el-input v-model="matchField" style="width: 150px" placeholder="用于匹配记录的飞书字段名" />
        </el-form-item>
        <el-form-item>
          <el-button type="primary" :loading="savingMappings" @click="saveMappings">保存映射</el-button>
        </el-form-item>
      </el-form>

      <el-table :data="fieldMappings" size="small" stripe max-height="400">
        <el-table-column width="50">
          <template #default="{ row }">
            <el-checkbox v-model="row.selected" :disabled="row.isLink || row.isLookup" />
          </template>
        </el-table-column>
        <el-table-column prop="feishuFieldName" label="飞书字段" width="150" />
        <el-table-column prop="feishuTypeName" label="飞书类型" width="110">
          <template #default="{ row }">
            <el-tag :type="row.isFormula ? 'warning' : row.isLink ? 'info' : 'default'" size="small">
              {{ row.feishuTypeName }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="→" width="40" align="center">
          <template #default>→</template>
        </el-table-column>
        <el-table-column label="本地列" width="180">
          <template #default="{ row }">
            <el-input v-model="row.localColumn" size="small" placeholder="本地列名"
              :disabled="row.isLink || row.isLookup" />
          </template>
        </el-table-column>
        <el-table-column label="同步方向" width="140">
          <template #default="{ row }">
            <el-select v-model="row.syncDirection" size="small" :disabled="row.isLink || row.isLookup">
              <el-option label="↔ 双向" value="bidirectional" />
              <el-option label="← 仅拉取" value="pull_only" />
              <el-option label="→ 仅推送" value="push_only" />
              <el-option label="— 不同步" value="none" />
            </el-select>
          </template>
        </el-table-column>
        <el-table-column label="状态" width="80">
          <template #default="{ row }">
            <el-tag v-if="row.matched" type="success" size="small">自动</el-tag>
            <el-tag v-else-if="row.localColumn" type="warning" size="small">手动</el-tag>
            <el-tag v-else type="info" size="small">未映射</el-tag>
          </template>
        </el-table-column>
      </el-table>
    </el-card>

    <!-- v3.0: 比较引擎 -->
    <el-card v-if="fieldMappings.length > 0" style="margin-top: 16px">
      <template #header><strong>📊 比较引擎</strong></template>

      <div class="sync-actions">
        <el-button type="primary" :loading="comparingTable" @click="compareSchema">
          🔍 Schema 比较
        </el-button>
        <el-button type="warning" :loading="comparingRecords" @click="compareRecords">
          📋 记录比较
        </el-button>
      </div>

      <!-- Schema 比较结果 -->
      <div v-if="schemaDiff" class="diff-result">
        <el-descriptions :column="4" border size="small">
          <el-descriptions-item label="可映射">
            <el-tag type="success">{{ schemaDiff.mapped?.length || 0 }}</el-tag>
          </el-descriptions-item>
          <el-descriptions-item label="飞书新增">
            <el-tag type="primary">{{ schemaDiff.feishuOnly?.length || 0 }}</el-tag>
          </el-descriptions-item>
          <el-descriptions-item label="仅本地">
            <el-tag type="info">{{ schemaDiff.localOnly?.length || 0 }}</el-tag>
          </el-descriptions-item>
          <el-descriptions-item label="类型冲突">
            <el-tag type="danger">{{ schemaDiff.typeConflict?.length || 0 }}</el-tag>
          </el-descriptions-item>
        </el-descriptions>

        <div v-if="schemaDiff.feishuOnly?.length" style="margin-top: 12px">
          <h4>飞书新增字段（本地无对应列）</h4>
          <el-table :data="schemaDiff.feishuOnly" size="small" max-height="200">
            <el-table-column prop="feishuField" label="飞书字段" />
            <el-table-column prop="feishuType" label="类型" />
          </el-table>
        </div>

        <div v-if="schemaDiff.typeConflict?.length" style="margin-top: 12px">
          <h4>类型冲突</h4>
          <el-table :data="schemaDiff.typeConflict" size="small" max-height="200">
            <el-table-column prop="feishuField" label="飞书字段" />
            <el-table-column prop="feishuType" label="飞书类型" />
            <el-table-column prop="localColumn" label="本地列" />
            <el-table-column prop="localType" label="本地类型" />
          </el-table>
        </div>
      </div>

      <!-- 记录比较结果 -->
      <div v-if="recordDiff" class="diff-result">
        <el-descriptions :column="4" border size="small">
          <el-descriptions-item label="相同">
            <el-tag type="success">{{ recordDiff.same?.length || 0 }}</el-tag>
          </el-descriptions-item>
          <el-descriptions-item label="仅飞书">
            <el-tag type="primary">{{ recordDiff.feishuOnly?.length || 0 }}</el-tag>
          </el-descriptions-item>
          <el-descriptions-item label="仅本地">
            <el-tag type="info">{{ recordDiff.localOnly?.length || 0 }}</el-tag>
          </el-descriptions-item>
          <el-descriptions-item label="冲突">
            <el-tag type="danger">{{ recordDiff.conflict?.length || 0 }}</el-tag>
          </el-descriptions-item>
        </el-descriptions>
      </div>
    </el-card>

    <!-- v3.0: 导入引擎 -->
    <el-card v-if="fieldMappings.length > 0" style="margin-top: 16px">
      <template #header><strong>📥 导入引擎</strong></template>

      <div class="sync-actions">
        <el-button type="primary" :loading="importAllLoading" @click="doImportAll" :disabled="mappedCount === 0">
          全量导入 ({{ mappedCount }} 字段)
        </el-button>
      </div>

      <el-divider />
      <h4>增量导入</h4>
      <el-form inline size="small">
        <el-form-item label="起始时间">
          <el-date-picker
            v-model="incrementalSince"
            type="datetime"
            placeholder="选择时间"
            format="YYYY-MM-DD HH:mm:ss"
            value-format="YYYY-MM-DD HH:mm:ss"
          />
        </el-form-item>
        <el-form-item>
          <el-button type="warning" :loading="importAllLoading" @click="doImportIncremental">
            增量导入
          </el-button>
        </el-form-item>
      </el-form>

      <div v-if="importResult2" class="import-result">
        <el-divider />
        <h4>导入结果</h4>
        <el-descriptions :column="4" border size="small">
          <el-descriptions-item label="总数">{{ importResult2.total }}</el-descriptions-item>
          <el-descriptions-item label="新建">{{ importResult2.created }}</el-descriptions-item>
          <el-descriptions-item label="更新">{{ importResult2.updated }}</el-descriptions-item>
          <el-descriptions-item label="跳过">{{ importResult2.skipped }}</el-descriptions-item>
        </el-descriptions>
        <div v-if="importResult2.errors?.length" class="import-errors">
          <h4>错误 ({{ importResult2.errors.length }})</h4>
          <ul>
            <li v-for="(err, i) in importResult2.errors.slice(0, 10)" :key="i" class="error-item">{{ err }}</li>
          </ul>
        </div>
      </div>
    </el-card>

    <!-- 传统 Pull/Push -->
    <el-card style="margin-top: 16px">
      <template #header><strong>🔄 传统同步 (固定映射)</strong></template>
      <div class="sync-actions">
        <el-button type="primary" @click="doPull" :disabled="!syncInfo.configured">
          ⬇️ 从飞书拉取
        </el-button>
        <el-button type="success" @click="doPush" :disabled="!syncInfo.configured">
          ⬆️ 推送到飞书
        </el-button>
      </div>
      <el-descriptions :column="2" border size="small">
        <el-descriptions-item label="上次拉取时间">{{ syncInfo.lastPullAt || '无' }}</el-descriptions-item>
        <el-descriptions-item label="拉取记录数">{{ syncInfo.lastPullCount ?? '无' }}</el-descriptions-item>
        <el-descriptions-item label="上次推送时间">{{ syncInfo.lastPushAt || '无' }}</el-descriptions-item>
        <el-descriptions-item label="推送记录数">{{ syncInfo.lastPushCount ?? '无' }}</el-descriptions-item>
      </el-descriptions>
    </el-card>
  </div>
</template>

<style scoped>
.tab-content {
  padding: 0 16px;
}

.card-header {
  display: flex;
  align-items: center;
  gap: 12px;
}

.import-result,
.sync-report {
  margin-top: 12px;
}

.import-errors {
  margin-top: 8px;
  color: #f56c6c;
}

.error-item {
  font-size: 13px;
}

.sync-actions {
  display: flex;
  gap: 12px;
  margin-bottom: 16px;
}

.table-list {
  margin-top: 16px;
}

.fields-list {
  margin-top: 16px;
}

.diff-result {
  margin-top: 16px;
}

h4 {
  margin: 12px 0 8px;
  font-size: 14px;
  color: #606266;
}
</style>
