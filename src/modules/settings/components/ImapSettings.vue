<script setup>
import { ref, onMounted } from 'vue'
import { tauriCallSafe } from '../../../core/tauriBridge'
import { ElMessage } from 'element-plus'

const emailMonitoring = ref(false)
const emailAccountCount = ref(0)

const imapForm = ref({
  emailAddress: '',
  imapServer: '',
  imapPort: 993,
  username: '',
  password: '',
  useTls: true,
  watchFolders: 'INBOX',
  filterFrom: '',
  filterSubject: '',
  enabled: true,
})
const imapSaving = ref(false)
const imapAccounts = ref([])

async function loadEmailStatus() {
  const result = await tauriCallSafe('get_email_monitor_status')
  if (result.ok) {
    emailMonitoring.value = result.data.running
    emailAccountCount.value = result.data.accountCount
  }
}

async function loadImapAccounts() {
  const result = await tauriCallSafe('list_imap_accounts')
  if (result.ok) {
    imapAccounts.value = result.data || []
  }
}

async function saveImapAccount() {
  if (!imapForm.value.emailAddress || !imapForm.value.imapServer || !imapForm.value.username || !imapForm.value.password) {
    ElMessage.warning('请填写完整的邮箱配置')
    return
  }
  imapSaving.value = true
  const result = await tauriCallSafe('configure_imap', { account: imapForm.value })
  imapSaving.value = false

  if (result.ok) {
    ElMessage.success('IMAP 账号已保存')
    imapForm.value = { emailAddress: '', imapServer: '', imapPort: 993, username: '', password: '', useTls: true, watchFolders: 'INBOX', filterFrom: '', filterSubject: '', enabled: true }
    await loadEmailStatus()
    await loadImapAccounts()
  } else {
    ElMessage.error(result.error || '保存失败')
  }
}

async function deleteImapAccount(email) {
  const result = await tauriCallSafe('delete_imap_account', { emailAddress: email })
  if (result.ok) {
    ElMessage.success('已删除')
    await loadEmailStatus()
    await loadImapAccounts()
  } else {
    ElMessage.error(result.error || '删除失败')
  }
}

async function toggleEmailMonitor() {
  const cmd = emailMonitoring.value ? 'stop_email_monitor' : 'start_email_monitor'
  const result = await tauriCallSafe(cmd)
  if (result.ok) {
    emailMonitoring.value = !emailMonitoring.value
    ElMessage.success(result.data)
  } else {
    ElMessage.error(result.error || '操作失败')
  }
}

onMounted(() => {
  loadEmailStatus()
  loadImapAccounts()
})
</script>

<template>
  <div class="tab-content">
    <el-card>
      <template #header>
        <div class="card-header">
          <strong>📧 邮件监听</strong>
          <el-tag v-if="emailMonitoring" type="success" size="small">监听中</el-tag>
          <el-tag v-else type="info" size="small">未启动</el-tag>
        </div>
      </template>

      <p class="tip">配置 IMAP 邮箱账号，系统将自动监听新邮件并导入收件箱。</p>

      <!-- 已配置账号列表 -->
      <div v-if="imapAccounts.length > 0" class="imap-account-list">
        <h4>已配置账号</h4>
        <el-table :data="imapAccounts" size="small" stripe>
          <el-table-column prop="emailAddress" label="邮箱地址" />
          <el-table-column prop="imapServer" label="IMAP 服务器" />
          <el-table-column prop="enabled" label="状态" width="80">
            <template #default="{ row }">
              <el-tag :type="row.enabled ? 'success' : 'info'" size="small">
                {{ row.enabled ? '启用' : '禁用' }}
              </el-tag>
            </template>
          </el-table-column>
          <el-table-column label="操作" width="80">
            <template #default="{ row }">
              <el-button type="danger" link size="small" @click="deleteImapAccount(row.emailAddress)">
                删除
              </el-button>
            </template>
          </el-table-column>
        </el-table>
      </div>

      <el-divider />

      <!-- 添加新账号 -->
      <h4>添加 IMAP 账号</h4>
      <el-form label-width="120px" size="default">
        <el-form-item label="邮箱地址">
          <el-input v-model="imapForm.emailAddress" placeholder="user@example.com" />
        </el-form-item>
        <el-form-item label="IMAP 服务器">
          <el-input v-model="imapForm.imapServer" placeholder="imap.example.com" />
        </el-form-item>
        <el-form-item label="端口">
          <el-input-number v-model="imapForm.imapPort" :min="1" :max="65535" />
        </el-form-item>
        <el-form-item label="用户名">
          <el-input v-model="imapForm.username" placeholder="通常是邮箱地址" />
        </el-form-item>
        <el-form-item label="密码">
          <el-input v-model="imapForm.password" type="password" show-password placeholder="邮箱密码或应用专用密码" />
        </el-form-item>
        <el-form-item label="使用 TLS">
          <el-switch v-model="imapForm.useTls" />
        </el-form-item>
        <el-form-item label="监听文件夹">
          <el-input v-model="imapForm.watchFolders" placeholder="INBOX" />
        </el-form-item>
        <el-form-item label="发件人过滤">
          <el-input v-model="imapForm.filterFrom" placeholder="多个用逗号分隔，留空不过滤" />
        </el-form-item>
        <el-form-item label="主题过滤">
          <el-input v-model="imapForm.filterSubject" placeholder="多个用逗号分隔，留空不过滤" />
        </el-form-item>
        <el-form-item>
          <el-button type="primary" :loading="imapSaving" @click="saveImapAccount">保存账号</el-button>
          <el-button
            :type="emailMonitoring ? 'danger' : 'success'"
            @click="toggleEmailMonitor"
            :disabled="emailAccountCount === 0 && !emailMonitoring"
          >
            {{ emailMonitoring ? '⏹ 停止监听' : '▶️ 启动监听' }}
          </el-button>
        </el-form-item>
      </el-form>
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

.tip {
  color: #909399;
  font-size: 13px;
  margin-bottom: 16px;
}

.imap-account-list {
  margin-bottom: 16px;
}

h4 {
  margin: 12px 0 8px;
  font-size: 14px;
  color: #606266;
}
</style>
