<script setup>
import { ref, onMounted } from 'vue'
import { tauriCallSafe } from '../../../core/tauriBridge.js'
import { ElMessage } from 'element-plus'

// === 飞书导入 ===
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
const feishuTableId = ref('')
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

const pulling = ref(false)
const pushing = ref(false)
const lastPullReport = ref(null)
const lastPushReport = ref(null)

async function loadSyncInfo() {
  const result = await tauriCallSafe('get_feishu_sync_info')
  if (result.ok) {
    syncInfo.value = result.data
    if (result.data.appToken) feishuAppToken.value = result.data.appToken
    if (result.data.tableId) feishuTableId.value = result.data.tableId
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

async function saveTableConfig() {
  if (!feishuAppToken.value.trim() || !feishuTableId.value.trim()) {
    ElMessage.warning('请填写 App Token 和 Table ID')
    return
  }
  const result = await tauriCallSafe('configure_feishu_table', {
    appToken: feishuAppToken.value.trim(),
    tableId: feishuTableId.value.trim(),
  })
  if (result.ok) {
    ElMessage.success('表格配置已保存')
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

async function doPull() {
  if (!syncInfo.value.appToken || !syncInfo.value.tableId) {
    ElMessage.warning('请先配置 App Token 和 Table ID')
    return
  }
  pulling.value = true
  lastPullReport.value = null
  const result = await tauriCallSafe('sync_feishu_pull', {
    appToken: syncInfo.value.appToken,
    tableId: syncInfo.value.tableId,
  })
  pulling.value = false

  if (result.ok) {
    lastPullReport.value = result.data
    ElMessage.success(`拉取完成：${result.data.pulled} 条`)
    await loadSyncInfo()
  } else {
    ElMessage.error(result.error || '拉取失败')
  }
}

async function doPush() {
  if (!syncInfo.value.appToken || !syncInfo.value.tableId) {
    ElMessage.warning('请先配置 App Token 和 Table ID')
    return
  }
  pushing.value = true
  lastPushReport.value = null
  const result = await tauriCallSafe('sync_feishu_push', {
    appToken: syncInfo.value.appToken,
    tableId: syncInfo.value.tableId,
  })
  pushing.value = false

  if (result.ok) {
    lastPushReport.value = result.data
    ElMessage.success(`推送完成：${result.data.pushed} 条`)
    await loadSyncInfo()
  } else {
    ElMessage.error(result.error || '推送失败')
  }
}

onMounted(() => {
  loadSyncInfo()
})
</script>

<template>
  <div class="tab-content">
    <!-- 数据导入 -->
    <el-card>
      <template #header><strong>📊 数据导入</strong></template>
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

    <!-- 飞书双向同步 -->
    <el-card style="margin-top: 16px">
      <template #header>
        <div class="card-header">
          <strong>🔄 飞书双向同步</strong>
          <el-tag v-if="syncInfo.configured" type="success" size="small">已配置</el-tag>
          <el-tag v-else type="info" size="small">未配置</el-tag>
        </div>
      </template>

      <h4>应用凭证</h4>
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

      <el-divider />

      <h4>多维表格配置</h4>
      <el-form label-width="100px" size="default">
        <el-form-item label="App Token">
          <el-input v-model="feishuAppToken" placeholder="多维表格的 App Token（URL 中获取）" />
        </el-form-item>
        <el-form-item label="Table ID">
          <el-input v-model="feishuTableId" placeholder="数据表的 Table ID" />
        </el-form-item>
        <el-form-item>
          <el-button type="primary" @click="saveTableConfig">保存表格配置</el-button>
        </el-form-item>
      </el-form>

      <el-divider />

      <h4>手动同步</h4>
      <div class="sync-actions">
        <el-button type="primary" :loading="pulling" @click="doPull" :disabled="!syncInfo.configured">
          ⬇️ 从飞书拉取
        </el-button>
        <el-button type="success" :loading="pushing" @click="doPush" :disabled="!syncInfo.configured">
          ⬆️ 推送到飞书
        </el-button>
      </div>

      <el-descriptions :column="2" border size="small" style="margin-top: 16px">
        <el-descriptions-item label="上次拉取时间">{{ syncInfo.lastPullAt || '无' }}</el-descriptions-item>
        <el-descriptions-item label="拉取记录数">{{ syncInfo.lastPullCount ?? '无' }}</el-descriptions-item>
        <el-descriptions-item label="上次推送时间">{{ syncInfo.lastPushAt || '无' }}</el-descriptions-item>
        <el-descriptions-item label="推送记录数">{{ syncInfo.lastPushCount ?? '无' }}</el-descriptions-item>
      </el-descriptions>

      <div v-if="lastPullReport" class="sync-report">
        <el-divider />
        <h4>拉取结果</h4>
        <ul>
          <li>拉取总数: {{ lastPullReport.pulled }}</li>
          <li>新建: {{ lastPullReport.created }}</li>
          <li>更新: {{ lastPullReport.updated }}</li>
          <li>跳过: {{ lastPullReport.skipped }}</li>
        </ul>
        <div v-if="lastPullReport.errors?.length" class="import-errors">
          <h4>错误</h4>
          <ul>
            <li v-for="(err, i) in lastPullReport.errors" :key="i" class="error-item">{{ err }}</li>
          </ul>
        </div>
      </div>

      <div v-if="lastPushReport" class="sync-report">
        <el-divider />
        <h4>推送结果</h4>
        <ul>
          <li>推送总数: {{ lastPushReport.pushed }}</li>
          <li>新建: {{ lastPushReport.created }}</li>
          <li>更新: {{ lastPushReport.updated }}</li>
        </ul>
        <div v-if="lastPushReport.errors?.length" class="import-errors">
          <h4>错误</h4>
          <ul>
            <li v-for="(err, i) in lastPushReport.errors" :key="i" class="error-item">{{ err }}</li>
          </ul>
        </div>
      </div>
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

h4 {
  margin: 12px 0 8px;
  font-size: 14px;
  color: #606266;
}
</style>
