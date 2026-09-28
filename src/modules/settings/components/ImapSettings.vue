<script setup>
import { ref, computed, onMounted } from 'vue'
import { casyContext } from '../../../core/plugin/context'
import { ElMessage, ElMessageBox } from 'element-plus'
import { useUnsavedForm } from '../../../composables/useUnsavedForm'

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
const initialForm = JSON.stringify(imapForm.value)
const formDirty = computed(() => JSON.stringify(imapForm.value) !== initialForm)
const monitorBusy = ref(false)
const deletingAccount = ref('')
const accountsError = ref('')
const statusError = ref('')
useUnsavedForm('邮箱配置', () => formDirty.value, () => imapSaving.value || monitorBusy.value || !!deletingAccount.value)


async function loadEmailStatus() {
  try {
    const result = await casyContext.settings.emailMonitorStatus()
    if (!result.ok || !result.data) throw new Error(result.error || '监听状态读取失败')
    emailMonitoring.value = result.data.running
    emailAccountCount.value = result.data.accountCount
    statusError.value = ''
  } catch (error) { statusError.value = String(error) }
}

async function loadImapAccounts() {
  try {
    const result = await casyContext.settings.imapAccounts()
    if (!result.ok) throw new Error(result.error || '邮箱账号读取失败')
    imapAccounts.value = result.data || []
    accountsError.value = ''
  } catch (error) { accountsError.value = String(error) }
}

async function saveImapAccount() {
  if (imapSaving.value) return
  const snapshot = { ...imapForm.value }
  if (!snapshot.emailAddress.trim() || !snapshot.imapServer.trim() || !snapshot.username.trim() || !snapshot.password) {
    ElMessage.warning('请填写完整的邮箱配置')
    return
  }
  if (!Number.isInteger(snapshot.imapPort) || snapshot.imapPort < 1 || snapshot.imapPort > 65535) {
    ElMessage.warning('端口应为 1–65535 的整数')
    return
  }
  imapSaving.value = true
  try {
    const result = await casyContext.settings.configureImap(snapshot)
    if (!result.ok) throw new Error(result.error || '保存失败，输入已保留')
    ElMessage.success('IMAP 账号已保存')
    imapForm.value = JSON.parse(initialForm)
    await Promise.all([loadEmailStatus(), loadImapAccounts()])
  } catch (error) { ElMessage.error(String(error)) }
  finally { imapSaving.value = false }
}

async function deleteImapAccount(email) {
  if (deletingAccount.value || imapSaving.value) return
  deletingAccount.value = email
  try {
    try { await ElMessageBox.confirm(`移除 ${email} 的监听配置？已导入的邮件和材料会保留。`, '移除邮箱账号', { type: 'warning', confirmButtonText: '移除配置', cancelButtonText: '保留' }) } catch { return }
    const result = await casyContext.settings.deleteImapAccount(email)
    if (!result.ok) throw new Error(result.error || '删除失败')
    ElMessage.success('邮箱配置已移除')
    await Promise.all([loadEmailStatus(), loadImapAccounts()])
  } catch (error) { ElMessage.error(String(error)) }
  finally { deletingAccount.value = '' }
}

async function toggleEmailMonitor() {
  if (monitorBusy.value || statusError.value) return
  monitorBusy.value = true
  try {
    const result = emailMonitoring.value
      ? await casyContext.settings.stopEmailMonitor()
      : await casyContext.settings.startEmailMonitor()
    if (!result.ok) throw new Error(result.error || '操作失败')
    await loadEmailStatus()
    if (!statusError.value) ElMessage.success(result.data || '监听状态已更新')
  } catch (error) { ElMessage.error(String(error)) }
  finally { monitorBusy.value = false }
}

onMounted(() => {
  loadEmailStatus()
  loadImapAccounts()
})
</script>

<template>
  <div class="tab-content">
    <el-alert v-if="accountsError" :title="accountsError" type="error" :closable="false"><el-button text @click="loadImapAccounts">重新读取账号</el-button></el-alert>
    <el-alert v-if="statusError" :title="statusError" type="error" :closable="false"><el-button text @click="loadEmailStatus">重新读取监听状态</el-button></el-alert>
    <p v-if="formDirty" class="tip" role="status">邮箱配置有未保存修改，切换设置分类会保留输入；离开设置前请保存。</p>
    <el-card>
      <template #header>
        <div class="card-header">
          <strong>📧 邮件监听</strong>
          <el-tag v-if="statusError" type="warning" size="small">状态未知</el-tag>
          <el-tag v-else-if="emailMonitoring" type="success" size="small">监听中</el-tag>
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
              <el-button type="danger" link size="small" :disabled="!!deletingAccount || imapSaving" :loading="deletingAccount === row.emailAddress" @click="deleteImapAccount(row.emailAddress)">
                删除
              </el-button>
            </template>
          </el-table-column>
        </el-table>
      </div>

      <el-divider />

      <!-- 添加新账号 -->
      <h4>添加 IMAP 账号</h4>
      <el-form :disabled="imapSaving" label-width="100px" size="default">
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
            :loading="monitorBusy" @click="toggleEmailMonitor"
            :disabled="!!statusError || (emailAccountCount === 0 && !emailMonitoring)"
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
  color: var(--gray-400);
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
