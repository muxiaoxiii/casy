<script setup>
import { ref, computed, onMounted } from 'vue'
import { casyContext } from '../../../core/plugin/context'
import { useSettingsStore } from '../../../stores/settings'
import { ElMessage } from 'element-plus'

const settingsStore = useSettingsStore()

// ============================================================
// SMTP / ICS 邀请（settings 键：smtp_host/smtp_port/smtp_user/smtp_pass）
// ============================================================
const smtpSaving = ref(false)
const sendingTest = ref(false)

async function saveSmtpConfig() {
  smtpSaving.value = true
  const result = await settingsStore.save()
  smtpSaving.value = false
  if (result.ok) {
    ElMessage.success('SMTP 配置已保存')
  } else {
    ElMessage.error(result.error || '保存失败')
  }
}

/** 发送测试邀请：收件人填自己的邮箱（smtp_user） */
async function sendTestInvitation() {
  if (!settingsStore.smtp_host || !settingsStore.smtp_user) {
    ElMessage.warning('请先填写并保存 SMTP 服务器与用户名')
    return
  }
  sendingTest.value = true
  const startIso = new Date(Date.now() + 60 * 60 * 1000).toISOString()
  const result = await casyContext.calendar.sendIcsInvitation({
    to: settingsStore.smtp_user,
    subject: 'Casy 测试邀请',
    description: '这是一封来自 Casy 的 SMTP / ICS 配置测试邀请。',
    startIso,
    durationMinutes: 30,
    alarmMinutes: 15,
  })
  sendingTest.value = false
  if (result.ok) {
    ElMessage.success('测试邀请已发送，请查收邮箱')
  } else {
    ElMessage.error(result.error || '发送失败')
  }
}

// ============================================================
// 日历同步（CalDAV）
// settings 键：caldav_url / caldav_user / caldav_pass（密码后端优先走 keychain）
// calendar_sync_enabled / calendar_mask_case_name（'true'/'false'）
// ============================================================
const caldavSaving = ref(false)
const caldavTesting = ref(false)
const caldavTestResult = ref(null) // { ok, message }
const syncing = ref(false)
const syncStatus = ref(null)       // { enabled, configured, syncedCount, pendingCount, failedCount, lastSyncAt }
const syncStatusError = ref(false)

const calendarSyncEnabled = computed({
  get: () => settingsStore.calendar_sync_enabled === 'true',
  set: (v) => { settingsStore.calendar_sync_enabled = v ? 'true' : 'false' },
})

const maskCaseName = computed({
  get: () => settingsStore.calendar_mask_case_name !== 'false',
  set: (v) => { settingsStore.calendar_mask_case_name = v ? 'true' : 'false' },
})

async function saveCaldavConfig() {
  caldavSaving.value = true
  const result = await settingsStore.save()
  caldavSaving.value = false
  if (result.ok) {
    ElMessage.success('CalDAV 配置已保存')
    loadSyncStatus()
  } else {
    ElMessage.error(result.error || '保存失败')
  }
}

async function testCaldav() {
  caldavTesting.value = true
  caldavTestResult.value = null
  const result = await casyContext.calendar.testCaldavConnection()
  caldavTesting.value = false
  if (result.ok) {
    caldavTestResult.value = { ok: true, message: result.data || '连接成功' }
  } else {
    caldavTestResult.value = { ok: false, message: result.error || '连接失败' }
  }
}

async function loadSyncStatus() {
  const result = await casyContext.calendar.calendarSyncStatus()
  if (result.ok && result.data) {
    syncStatus.value = result.data
    syncStatusError.value = false
  } else {
    syncStatus.value = null
    syncStatusError.value = true
  }
}

async function syncNow() {
  syncing.value = true
  const result = await casyContext.calendar.syncRemindersToCalendar()
  syncing.value = false
  if (result.ok && result.data) {
    const r = result.data
    ElMessage.success(`补同步完成：成功 ${r.synced}，失败 ${r.failed}，跳过 ${r.skipped}`)
  } else {
    ElMessage.error(result.error || '补同步失败')
  }
  loadSyncStatus()
}

onMounted(loadSyncStatus)

// ============================================================
// MCP Server（settings 键：mcp_server_enabled，'true'/'false'，重启生效）
// ============================================================
const mcpEnabled = computed({
  get: () => settingsStore.mcp_server_enabled !== 'false',
  set: (v) => { settingsStore.mcp_server_enabled = v ? 'true' : 'false' },
})

const mcpSaving = ref(false)

async function saveMcpConfig() {
  mcpSaving.value = true
  const result = await settingsStore.save()
  mcpSaving.value = false
  if (result.ok) {
    ElMessage.success('已保存，重启应用后生效')
  } else {
    ElMessage.error(result.error || '保存失败')
  }
}

// 凭据状态（Keychain）
const keychainStatus = ref(null)
const keychainError = ref(false)

onMounted(async () => {
  const result = await casyContext.settings.keychainStatus()
  if (result.ok && result.data) {
    keychainStatus.value = result.data
    keychainError.value = false
  } else {
    keychainStatus.value = null
    keychainError.value = true
  }
})

const keychainSummary = computed(() => {
  if (keychainError.value) return '系统钥匙串不可用'
  if (!keychainStatus.value) return '检测中…'
  const accounts = keychainStatus.value.accounts || []
  const migrated = accounts.filter(a => a.hasKeychainPassword).length
  if (accounts.length === 0) return '钥匙串可用 · 暂无邮箱账号'
  return `钥匙串可用 · ${migrated}/${accounts.length} 个邮箱账号已迁移到钥匙串`
})

// ============================================================
// MCP 外部写操作待确认队列（list/approve/reject_mcp_write）
// ============================================================
const pendingWrites = ref([])
const pendingWritesError = ref(false)
const pendingWritesLoading = ref(false)
const writeActionBusy = ref({}) // id -> 'approve' | 'reject'

const MCP_TOOL_LABELS = {
  case_create_task: '新建任务',
  task_update_status: '更新任务状态',
}

function mcpToolLabel(tool) {
  return MCP_TOOL_LABELS[tool] || tool
}

/** arguments 为 JSON 字符串，提取关键字段做一行摘要 */
function summarizeArguments(raw) {
  if (!raw) return ''
  let args = raw
  if (typeof raw === 'string') {
    try {
      args = JSON.parse(raw)
    } catch {
      return raw.length > 60 ? raw.slice(0, 60) + '…' : raw
    }
  }
  const parts = []
  const title = args.task_name ?? args.taskName ?? args.title
  if (title) parts.push(`任务：${title}`)
  const caseRef = args.case_name ?? args.caseName ?? args.case_id ?? args.caseId
  if (caseRef) parts.push(`案件：${caseRef}`)
  if (args.status) parts.push(`状态：${args.status}`)
  if (parts.length === 0) {
    const s = JSON.stringify(args)
    return s.length > 60 ? s.slice(0, 60) + '…' : s
  }
  return parts.join(' · ')
}

function writeCreatedAt(item) {
  return item.created_at || item.createdAt || ''
}

async function loadPendingWrites() {
  pendingWritesLoading.value = true
  const result = await casyContext.settings.mcpPendingWrites()
  pendingWritesLoading.value = false
  if (result.ok) {
    pendingWrites.value = (result.data || []).filter(w => !w.status || w.status === 'pending')
    pendingWritesError.value = false
  } else {
    pendingWritesError.value = true
  }
}

async function approveWrite(item) {
  writeActionBusy.value = { ...writeActionBusy.value, [item.id]: 'approve' }
  const result = await casyContext.settings.approveMcpWrite(item.id)
  writeActionBusy.value = { ...writeActionBusy.value, [item.id]: null }
  if (result.ok) {
    const d = result.data
    const summary = typeof d === 'string' ? d : (d?.message || d?.summary || '执行成功')
    ElMessage.success(`已批准并执行：${summary}`)
    loadPendingWrites()
  } else {
    ElMessage.error(result.error || '执行失败')
  }
}

async function rejectWrite(item) {
  writeActionBusy.value = { ...writeActionBusy.value, [item.id]: 'reject' }
  const result = await casyContext.settings.rejectMcpWrite(item.id)
  writeActionBusy.value = { ...writeActionBusy.value, [item.id]: null }
  if (result.ok) {
    ElMessage.info('已拒绝该写操作')
    loadPendingWrites()
  } else {
    ElMessage.error(result.error || '操作失败')
  }
}

onMounted(loadPendingWrites)
</script>

<template>
  <div class="tab-content">
    <!-- SMTP / ICS 邀请 -->
    <el-card>
      <template #header>
        <div class="card-header">
          <strong>SMTP / ICS 邀请</strong>
          <el-tag v-if="settingsStore.smtp_host" type="success" size="small">已配置</el-tag>
          <el-tag v-else type="info" size="small">未配置</el-tag>
        </div>
      </template>

      <p class="tip">用于发送日历邀请（ICS）。配置后可将日程以邮件邀请形式发给同事或客户。</p>

      <el-form label-width="120px" size="default">
        <el-form-item label="SMTP 服务器">
          <el-input v-model="settingsStore.smtp_host" placeholder="如 smtp.exmail.qq.com" />
        </el-form-item>
        <el-form-item label="端口">
          <el-input v-model="settingsStore.smtp_port" placeholder="465（SSL）或 587（STARTTLS）" style="width: 220px" />
        </el-form-item>
        <el-form-item label="用户名">
          <el-input v-model="settingsStore.smtp_user" placeholder="完整邮箱地址" />
        </el-form-item>
        <el-form-item label="密码">
          <el-input
            v-model="settingsStore.smtp_pass"
            type="password"
            show-password
            placeholder="密码或客户端授权码"
          />
        </el-form-item>
        <el-form-item>
          <el-button type="primary" :loading="smtpSaving" @click="saveSmtpConfig">保存配置</el-button>
          <el-button :loading="sendingTest" @click="sendTestInvitation">发送测试邀请</el-button>
        </el-form-item>
      </el-form>
    </el-card>

    <!-- 日历同步（CalDAV） -->
    <el-card class="caldav-card">
      <template #header>
        <div class="card-header">
          <strong>日历同步（CalDAV）</strong>
          <el-tag v-if="syncStatus?.enabled" type="success" size="small">已启用</el-tag>
          <el-tag v-else-if="settingsStore.caldav_url" type="warning" size="small">已配置未启用</el-tag>
          <el-tag v-else type="info" size="small">未配置</el-tag>
        </div>
      </template>

      <p class="tip">
        同步后提醒由日历服务商准时推送，Casy 离线也不影响。日程内容会出现在日历应用中，高敏案件请注意（可开启脱敏）。
      </p>

      <el-form label-width="120px" size="default">
        <el-form-item label="CalDAV 地址">
          <el-input v-model="settingsStore.caldav_url" placeholder="如 https://caldav.example.com/calendars/user/default" />
        </el-form-item>
        <el-form-item label="用户名">
          <el-input v-model="settingsStore.caldav_user" placeholder="日历账号" />
        </el-form-item>
        <el-form-item label="密码">
          <el-input
            v-model="settingsStore.caldav_pass"
            type="password"
            show-password
            placeholder="密码或应用专用密码（优先存入系统钥匙串）"
          />
        </el-form-item>
        <el-form-item label="启用同步">
          <el-switch v-model="calendarSyncEnabled" />
          <span class="field-hint">保存后生效</span>
        </el-form-item>
        <el-form-item label="脱敏案件名">
          <el-switch v-model="maskCaseName" />
          <span class="field-hint">开启后日历中不显示真实案件名称</span>
        </el-form-item>
        <el-form-item>
          <el-button type="primary" :loading="caldavSaving" @click="saveCaldavConfig">保存配置</el-button>
          <el-button :loading="caldavTesting" @click="testCaldav">测试连接</el-button>
        </el-form-item>
        <el-form-item v-if="caldavTestResult">
          <el-alert
            :type="caldavTestResult.ok ? 'success' : 'error'"
            :title="caldavTestResult.ok ? '连接成功' : '连接失败'"
            :description="caldavTestResult.message"
            :closable="true"
            show-icon
            @close="caldavTestResult = null"
          />
        </el-form-item>
      </el-form>

      <!-- 同步状态 -->
      <div class="sync-status">
        <template v-if="syncStatus">
          <span class="sync-item">已同步 <strong>{{ syncStatus.syncedCount }}</strong></span>
          <span class="sync-item">待同步 <strong>{{ syncStatus.pendingCount }}</strong></span>
          <span class="sync-item" :class="{ 'text-danger': syncStatus.failedCount > 0 }">
            失败 <strong>{{ syncStatus.failedCount }}</strong>
          </span>
          <span class="sync-item sync-time">最近同步：{{ syncStatus.lastSyncAt || '从未' }}</span>
          <el-button size="small" :loading="syncing" :disabled="!syncStatus.configured" @click="syncNow">
            立即补同步
          </el-button>
        </template>
        <span v-else-if="syncStatusError" class="field-hint text-warning">同步状态获取失败</span>
        <span v-else class="field-hint">状态加载中…</span>
      </div>
    </el-card>

    <!-- MCP Server -->
    <el-card class="mcp-card">
      <template #header>
        <div class="card-header">
          <strong>MCP Server</strong>
          <el-tag v-if="mcpEnabled" type="success" size="small">已启用</el-tag>
          <el-tag v-else type="info" size="small">已禁用</el-tag>
        </div>
      </template>

      <p class="tip">本地只读接口 127.0.0.1:37877，供外部 AI 工具读取案件/任务数据，重启生效。</p>

      <el-form label-width="120px" size="default">
        <el-form-item label="启用 MCP">
          <el-switch v-model="mcpEnabled" />
          <span class="field-hint">仅监听本机回环地址，数据只读</span>
        </el-form-item>
        <el-form-item label="凭据状态">
          <span class="field-hint" :class="{ 'text-warning': keychainError }">{{ keychainSummary }}</span>
        </el-form-item>
        <el-form-item>
          <el-button type="primary" :loading="mcpSaving" @click="saveMcpConfig">保存</el-button>
        </el-form-item>
      </el-form>
    </el-card>

    <!-- MCP 外部写操作待确认 -->
    <el-card class="mcp-writes-card">
      <template #header>
        <div class="card-header">
          <strong>外部写操作待确认</strong>
          <el-tag v-if="pendingWrites.length > 0" type="warning" size="small">
            {{ pendingWrites.length }} 条待确认
          </el-tag>
          <el-button
            size="small"
            text
            class="refresh-btn"
            :loading="pendingWritesLoading"
            @click="loadPendingWrites"
          >
            刷新
          </el-button>
        </div>
      </template>

      <p class="tip">外部 AI 工具（经 127.0.0.1:37877）提交的写操作必须经你确认才会执行。</p>

      <div v-if="pendingWritesError" class="field-hint text-warning">待确认队列获取失败，请稍后重试</div>
      <template v-else>
        <div v-for="w in pendingWrites" :key="w.id" class="write-item">
          <div class="write-info">
            <div class="write-head">
              <span class="write-tool">{{ mcpToolLabel(w.tool) }}</span>
              <span class="write-time">{{ writeCreatedAt(w) }}</span>
            </div>
            <div class="write-args">{{ summarizeArguments(w.arguments) || '（无参数摘要）' }}</div>
          </div>
          <el-button
            size="small"
            type="primary"
            plain
            :loading="writeActionBusy[w.id] === 'approve'"
            :disabled="!!writeActionBusy[w.id]"
            @click="approveWrite(w)"
          >
            批准执行
          </el-button>
          <el-button
            size="small"
            text
            :loading="writeActionBusy[w.id] === 'reject'"
            :disabled="!!writeActionBusy[w.id]"
            @click="rejectWrite(w)"
          >
            拒绝
          </el-button>
        </div>
        <div v-if="pendingWrites.length === 0" class="writes-empty">暂无待确认的外部写操作</div>
      </template>
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

.mcp-card,
.caldav-card {
  margin-top: 16px;
}

.mcp-writes-card {
  margin-top: 16px;
}

.refresh-btn {
  margin-left: auto;
}

.write-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  border: 1px solid #E0E3E9;
  border-radius: 6px;
  margin-bottom: 8px;
}

.write-info {
  flex: 1;
  min-width: 0;
}

.write-head {
  display: flex;
  align-items: center;
  gap: 8px;
}

.write-tool {
  font-size: 13px;
  font-weight: 500;
  color: #1F2430;
}

.write-time {
  font-size: 12px;
  color: #909399;
}

.write-args {
  font-size: 12px;
  color: #606266;
  margin-top: 2px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.writes-empty {
  text-align: center;
  padding: 20px;
  color: #9BA2AF;
  font-size: 13px;
}

.tip {
  color: #909399;
  font-size: 13px;
  margin-bottom: 16px;
}

.field-hint {
  color: #909399;
  font-size: 12px;
  margin-left: 8px;
}

.text-warning {
  color: #B0823A;
}

.text-danger {
  color: #B4554F;
}

.sync-status {
  display: flex;
  align-items: center;
  gap: 16px;
  margin-top: 12px;
  padding-top: 12px;
  border-top: 1px solid #F3F4F6;
  flex-wrap: wrap;
}

.sync-item {
  font-size: 13px;
  color: #606266;
}

.sync-item strong {
  font-weight: 600;
}

.sync-time {
  color: #909399;
  font-size: 12px;
}
</style>
