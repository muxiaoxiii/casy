<script setup>
import { getCurrentWindow } from '@tauri-apps/api/window'
import { computed, ref, onMounted } from 'vue'
import { casyContext } from '../../../core/plugin/context'
import { useSettingsStore } from '../../../stores/settings'
import { ElMessage, ElMessageBox } from 'element-plus'

const settingsStore = useSettingsStore()

const webdavSaving = ref(false)
const webdavTesting = ref(false)
const fullBusy = ref(false)
const fullDialog = ref(false)
const fullMode = ref('backup')
const backupPassword = ref('')
const confirmPassword = ref('')
const busy = computed(() => webdavSaving.value || webdavTesting.value || fullBusy.value)
const canTransfer = computed(() => !!settingsStore.webdavUrl.trim() && !!settingsStore.webdavUsername.trim())
const transferTarget = ref(null)

function openFull(mode) {
  if (busy.value || !canTransfer.value) return
  fullMode.value = mode
  backupPassword.value = ''
  confirmPassword.value = ''
  transferTarget.value = {
    url: settingsStore.webdavUrl.trim(),
    username: settingsStore.webdavUsername.trim(),
    password: settingsStore.webdavPassword,
  }
  fullDialog.value = true
}
function clearFullSecrets() {
  backupPassword.value = ''
  confirmPassword.value = ''
  transferTarget.value = null
}
async function submitFull() {
  if (fullBusy.value || !transferTarget.value) return
  if (!backupPassword.value || (fullMode.value === 'backup' &&
    (Array.from(backupPassword.value).length < 10 || backupPassword.value !== confirmPassword.value))) {
    ElMessage.error('备份密码至少 10 个字符，两次输入必须一致；恢复时请输入原备份密码')
    return
  }
  fullBusy.value = true
  try {
    const { url, username, password } = transferTarget.value
    const result = fullMode.value === 'backup'
      ? await casyContext.sync.backupFull(url, username, password, backupPassword.value)
      : await casyContext.sync.restoreFull(url, username, password, backupPassword.value)
    if (!result.ok) { ElMessage.error(result.error || '操作失败，请核对网络与备份密码'); return }
    fullDialog.value = false
    clearFullSecrets()
    if (fullMode.value === 'restore') {
      await ElMessageBox.alert('案件、任务、附件及设置已恢复。请退出并重新打开 Casy，使用恢复后的数据。新设备上的外部服务凭据需要重新配置。', '恢复完成', { confirmButtonText: '退出应用', showClose: false, closeOnClickModal: false, closeOnPressEscape: false })
      getCurrentWindow().close()
    } else ElMessage.success(result.data || '全部数据已备份到 WebDAV')
  } catch (error) {
    ElMessage.error(String(error))
  } finally { fullBusy.value = false }
}

const webdavStatus = ref(null)
const persistedStatus = ref(null)
async function refreshStatus() {
  const result = await casyContext.sync.status()
  if (result.ok) persistedStatus.value = result.data
}
onMounted(refreshStatus)

async function saveWebdavConfig() {
  if (busy.value) return
  webdavSaving.value = true
  try {
    const result = await settingsStore.save()
    if (result.ok) {
      ElMessage.success('WebDAV 配置已保存')
      await refreshStatus()
    } else ElMessage.error(result.error || '保存失败')
  } catch (error) { ElMessage.error(String(error)) }
  finally { webdavSaving.value = false }
}

async function testWebdavConnection() {
  if (busy.value) return
  webdavTesting.value = true
  webdavStatus.value = null
  try {
    const result = await casyContext.sync.testWebdav(
      settingsStore.webdavUrl.trim(), settingsStore.webdavUsername.trim(), settingsStore.webdavPassword
    )
    await refreshStatus()
    webdavStatus.value = result.ok ? 'ok' : 'fail'
    if (result.ok) ElMessage.success(result.data || '连接正常')
    else ElMessage.error(result.error || '连接失败')
  } catch (error) { ElMessage.error(String(error)) }
  finally { webdavTesting.value = false }
}
</script>

<template>
  <div class="tab-content">
    <el-card>
      <template #header>
        <div class="card-header">
          <strong>WebDAV 备份与同步</strong>
          <el-tag v-if="settingsStore.webdavUrl" type="success" size="small">已配置</el-tag>
          <el-tag v-else type="info" size="small">未配置</el-tag>
        </div>
      </template>

      <p class="tip">将完整加密备份保存到你的 WebDAV 服务器。先填写连接信息、保存配置，再执行备份或恢复。</p>

      <p v-if="persistedStatus" class="tip">连接状态：{{ ({ unconfigured: '未配置', unknown: '尚未检测', connected: '最近检测成功', failed: '最近检测失败' })[persistedStatus.connectionState] || '尚未检测' }}<span v-if="persistedStatus.lastCheckedAt"> · {{ persistedStatus.lastCheckedAt }}</span></p>
      <el-form label-width="100px" size="default" :disabled="busy || fullDialog">
        <el-form-item label="WebDAV URL">
          <el-input
            v-model="settingsStore.webdavUrl"
            placeholder="https://dav.example.com/dav/casy"
          />
        </el-form-item>
        <el-form-item label="用户名">
          <el-input v-model="settingsStore.webdavUsername" placeholder="WebDAV 用户名" />
        </el-form-item>
        <el-form-item label="密码">
          <el-input
            v-model="settingsStore.webdavPassword"
            type="password"
            show-password
            :placeholder="settingsStore.webdavPassword_configured ? '已保存；留空保留' : '未设置密码'"
          /><el-button v-if="settingsStore.webdavPassword_configured" link type="danger" @click="settingsStore.clearSecret('webdavPassword')">清除密码（保存后生效）</el-button>
        </el-form-item>
        <el-form-item label="启动检查">
          <el-switch v-model="settingsStore.webdavAutoSync" />
          <span class="field-hint">仅检查旧版数据库快照的远程版本，不会自动备份附件</span>
        </el-form-item>
        <el-form-item>
          <el-button type="primary" :loading="webdavSaving" @click="saveWebdavConfig">保存配置</el-button>
          <el-button
            :loading="webdavTesting"
            @click="testWebdavConnection"
            :type="webdavStatus === 'ok' ? 'success' : webdavStatus === 'fail' ? 'danger' : 'default'"
          >
            {{ webdavStatus === 'ok' ? '✓ 连接正常' : webdavStatus === 'fail' ? '✗ 连接失败' : '测试连接' }}
          </el-button>
        </el-form-item>
      </el-form>
    </el-card>

    <el-card class="full-backup-card">
      <template #header><strong>全部数据备份与恢复</strong></template>
      <p class="backup-scope">包含案件、任务、笔记、设置、卷宗原件及本地附件。使用独立备份密码加密，支持在新设备恢复。</p>
      <p class="tip">系统钥匙串中的 AI、邮箱等服务凭据不随备份迁移，新设备需重新配置。备份前请保存正在编辑的内容，完成或取消文档处理任务。</p>
      <div class="backup-actions">
        <el-button type="primary" :disabled="busy || !canTransfer" @click="openFull('backup')">备份全部数据</el-button>
        <el-button :disabled="busy || !canTransfer" @click="openFull('restore')">恢复全部数据</el-button>
      </div>
      <p v-if="!canTransfer" class="tip">请先填写上方的 WebDAV 地址和用户名。</p>
      <p class="tip backup-location">远程文件：casy-full-backup.casy · 每次备份更新该文件，旧版 casy.db 数据库快照不受影响。</p>
    </el-card>

    <el-dialog v-model="fullDialog" :title="fullMode === 'backup' ? '备份全部数据到 WebDAV' : '从 WebDAV 恢复全部数据'" width="520px"
      :close-on-click-modal="false" :close-on-press-escape="!fullBusy" :show-close="!fullBusy"
      :before-close="(done) => { if (!fullBusy) done() }" @closed="clearFullSecrets">
      <p class="backup-location">{{ transferTarget?.url }}/casy-full-backup.casy</p>
      <el-alert v-if="fullMode === 'restore'" type="warning" :closable="false" title="当前案件、任务和设置将被备份中的数据替换。"
        description="恢复前会自动保存一份使用同一备份密码加密的本机保护备份。原始文件保留，恢复完成后请重启应用。" />
      <p v-else class="backup-scope">将更新此目录中的完整备份。请妥善保存独立备份密码，密码遗失后无法恢复。</p>
      <el-form label-position="top" @submit.prevent="submitFull">
        <el-form-item label="备份密码（不是 WebDAV 登录密码）"><el-input v-model="backupPassword" aria-label="备份密码" type="password" show-password :disabled="fullBusy" autocomplete="off" /></el-form-item>
        <el-form-item v-if="fullMode === 'backup'" label="再次输入备份密码"><el-input v-model="confirmPassword" aria-label="再次输入备份密码" type="password" show-password :disabled="fullBusy" autocomplete="off" /></el-form-item>
      </el-form>
      <p v-if="fullBusy" class="tip" role="status">{{ fullMode === 'backup' ? '正在打包、加密并上传全部数据' : '正在下载、校验并恢复全部数据' }}，耗时取决于附件大小和网络速度，请保持应用开启。</p>
      <template #footer>
        <el-button :disabled="fullBusy" @click="fullDialog = false">取消</el-button>
        <el-button :type="fullMode === 'restore' ? 'warning' : 'primary'" :loading="fullBusy" @click="submitFull">{{ fullMode === 'backup' ? '加密并备份' : '确认覆盖并恢复' }}</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.full-backup-card { margin-top: 20px; }
.backup-scope { line-height: 1.7; color: var(--c-text-regular); margin-block: 0 12px; }
.backup-actions { display: flex; flex-wrap: wrap; gap: 10px; }
.backup-actions .el-button { margin-left: 0; }
.backup-location { overflow-wrap: anywhere; font-size: 13px; color: var(--c-text-secondary); margin-top: 16px; }
.el-alert { margin-bottom: 20px; }

.tab-content {
  padding: 0 16px;
}

.card-header {
  display: flex;
  align-items: center;
  gap: 12px;
}

.tip {
  color: var(--c-text-secondary);
  font-size: 13px;
  margin-bottom: 16px;
}

.field-hint {
  color: var(--c-text-secondary);
  font-size: 12px;
  margin-left: 8px;
}
</style>
