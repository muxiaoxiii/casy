<script setup>
import { getCurrentWindow } from '@tauri-apps/api/window'
import { ref, onMounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { open, save } from '@tauri-apps/plugin-dialog'
import { Download, Upload, Refresh } from '../../../shared/icons'
import { tauriCallSafe } from '../../../core/tauriBridge'
import { casyContext } from '../../../core/plugin/context'

const keychainBusy = ref(false)
async function backupKeyToKeychain() {
  keychainBusy.value = true
  try {
    const result = await tauriCallSafe('backup_database_key_to_keychain')
    if (!result.ok) { ElMessage.error(result.error || '钥匙串备份失败'); return }
    ElMessage.success('数据库密钥已备份到系统钥匙串，本地密钥保留')
  } finally { keychainBusy.value = false }
}

const backups = ref([])
const loading = ref(false)
const backingUp = ref(false)
const restoringId = ref(null)
const fullBusy = ref(false)
const password = ref('')
const confirmation = ref('')
const backupDialog = ref(false)
const backupMode = ref('export')
const backupPath = ref('')

async function chooseFull(mode) {
  try {
    const filters = [{ name: 'Casy 加密备份', extensions: ['casy'] }]
    const selected = mode === 'export'
      ? await save({ filters, defaultPath: `Casy-${new Date().toISOString().slice(0, 10)}.casy` })
      : await open({ filters, multiple: false, directory: false })
    if (typeof selected !== 'string') return
    backupMode.value = mode
    backupPath.value = selected
    password.value = ''
    confirmation.value = ''
    backupDialog.value = true
  } catch (error) { ElMessage.error(String(error)) }
}

async function submitFull() {
  if (backupMode.value === 'export' && (password.value.length < 10 || password.value !== confirmation.value)) {
    ElMessage.error('密码至少 10 个字符，两次输入必须一致')
    return
  }
  if (!password.value) return
  fullBusy.value = true
  try {
    const result = backupMode.value === 'export'
      ? await casyContext.backup.exportFull(backupPath.value, password.value)
      : await casyContext.backup.importFull(backupPath.value, password.value)
    if (!result.ok) { ElMessage.error(result.error || '操作失败'); return }
    backupDialog.value = false
    password.value = ''
    confirmation.value = ''
    if (backupMode.value === 'import') {
      await ElMessageBox.alert('卷宗和数据库已恢复，界面将重新加载。外部服务的账号密钥需要在新机器上重新配置。', '恢复完成', { confirmButtonText: '重新加载', showClose: false })
      window.location.reload()
    } else ElMessage.success('完整加密备份已保存')
  } finally { fullBusy.value = false }
}

function fmtSize(bytes) {
  if (bytes > 1024 * 1024) return (bytes / 1024 / 1024).toFixed(1) + ' MB'
  return Math.max(1, Math.round(bytes / 1024)) + ' KB'
}

async function load() {
  loading.value = true
  const result = await casyContext.backup.list()
  loading.value = false
  if (result.ok) backups.value = result.data || []
  else ElMessage.error(result.error || '加载备份列表失败')
}

async function createBackup() {
  backingUp.value = true
  const result = await casyContext.backup.create()
  backingUp.value = false
  if (result.ok) {
    ElMessage.success(`备份完成：${result.data.filename}（${fmtSize(result.data.sizeBytes)}）`)
    await load()
  } else {
    ElMessage.error(result.error || '备份失败')
  }
}

async function restoreBackup(b) {
  try {
    await ElMessageBox.confirm(
      `将用 ${b.modifiedAt} 的备份覆盖当前全部数据，并自动保存一份"恢复前快照"。覆盖后需要重启应用。确定继续？`,
      '恢复备份',
      { confirmButtonText: '恢复并重启', cancelButtonText: '取消', type: 'warning' },
    )
  } catch { /* 用户取消：属预期 */ return }
  restoringId.value = b.filename
  const result = await casyContext.backup.restore(b.filename)
  restoringId.value = null
  if (result.ok) {
    await ElMessageBox.alert(
      '数据已恢复。点击确定关闭应用，重新打开即可使用恢复的数据。',
      '恢复完成',
      { confirmButtonText: '立即退出' },
    ).finally(() => getCurrentWindow().close())
  } else {
    ElMessage.error(result.error || '恢复失败')
  }
}

onMounted(() => {
  load()
})
</script>

<template>
  <div class="backup-settings">
    <section class="backup-intro">
      <h3>数据库密钥与系统钥匙串</h3>
      <p>新资料库默认使用本机密钥文件加密，不在首次打开时请求钥匙串。密钥文件与资料库一起由当前系统用户保管，请定期导出完整加密备份。</p>
      <p>你可以把 Casy 自己的数据库密钥备份到系统钥匙串。点击后系统可能要求授权；此操作保留本地密钥。已有资料库若仅在钥匙串保存密钥，打开时仍需授权。</p>
      <p>邮箱、日历等账号凭据在配置或使用对应功能时访问。</p>
      <el-button :loading="keychainBusy" @click="backupKeyToKeychain">将数据库密钥备份到钥匙串</el-button>
    </section>
    <div class="backup-intro">
      完整备份包含数据库、原始卷宗和本地文档产物，使用独立密码加密。请保管好密码，遗失后无法恢复。
    </div>

    <div class="backup-actions">
      <el-button type="primary" :icon="Download" :disabled="fullBusy" @click="chooseFull('export')">导出完整备份</el-button>
      <el-button :icon="Upload" :disabled="fullBusy" @click="chooseFull('import')">从完整备份恢复</el-button>
    </div>

    <el-dialog v-model="backupDialog" :title="backupMode === 'export' ? '导出完整备份' : '恢复完整备份'" width="480px" :close-on-click-modal="false" :close-on-press-escape="!fullBusy" :show-close="!fullBusy" :before-close="(done) => { if (!fullBusy) done() }">
      <div class="backup-path">{{ backupPath }}</div>
      <p v-if="backupMode === 'import'">将恢复案件及卷宗，并自动创建使用同一密码加密的恢复前备份。当前数据库将被替换，原始文件保留。</p>
      <el-form label-position="top" @submit.prevent="submitFull">
        <el-form-item label="备份密码"><el-input v-model="password" type="password" show-password :disabled="fullBusy" autocomplete="off" /></el-form-item>
        <el-form-item v-if="backupMode === 'export'" label="再次输入密码"><el-input v-model="confirmation" type="password" show-password :disabled="fullBusy" autocomplete="off" /></el-form-item>
      </el-form>
      <template #footer>
        <el-button :disabled="fullBusy" @click="backupDialog = false">取消</el-button>
        <el-button type="primary" :loading="fullBusy" @click="submitFull">{{ backupMode === 'export' ? '加密并导出' : '恢复数据' }}</el-button>
      </template>
    </el-dialog>

    <h3 class="snapshot-title">本机数据库快照</h3>
    <p class="backup-note">仅包含数据库，不包含卷宗原件，也不能代替完整备份。</p>

    <div class="backup-actions">
      <el-button type="primary" :loading="backingUp" @click="createBackup">立即备份</el-button>
      <el-button :icon="Refresh" :loading="loading" @click="load">刷新</el-button>
    </div>

    <div v-loading="loading" class="backup-list">
      <div v-for="b in backups" :key="b.filename" class="backup-row">
        <span class="bf-name">{{ b.filename }}</span>
        <span class="bf-meta">{{ b.modifiedAt }} · {{ fmtSize(b.sizeBytes) }}</span>
        <el-button
          size="small"
          type="warning"
          plain
          :loading="restoringId === b.filename"
          @click="restoreBackup(b)"
        >
          恢复此备份
        </el-button>
      </div>
      <div v-if="!loading && backups.length === 0" class="backup-empty">
        还没有备份。点击「立即备份」生成第一份加密快照。
      </div>
    </div>

    <div class="backup-note">
      恢复操作会先自动保存当前数据的「恢复前快照」（pre-restore-*.db），误操作时可再回退。
    </div>
  </div>
</template>

<style scoped>
.backup-settings {
  max-width: 640px;
}
.backup-intro {
  font-size: 13px;
  color: var(--c-text-regular);
  line-height: 1.7;
  margin-bottom: 14px;
}
.backup-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  margin-bottom: 16px;
}
.backup-list {
  border: 1px solid var(--c-border);
  border-radius: 8px;
  min-height: 80px;
}
.backup-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 9px 14px;
  border-bottom: 1px solid var(--c-border-lighter);
}
.backup-row:last-child { border-bottom: none; }
.bf-name {
  flex: 1;
  min-width: 0;
  overflow-wrap: anywhere;
  font-family: var(--font-mono);
  font-size: 12px;
  color: var(--c-text);
}
.bf-meta { font-size: 12px; color: var(--c-text-secondary); }
.backup-empty {
  padding: 24px;
  text-align: center;
  font-size: 13px;
  color: var(--c-text-secondary);
}
.backup-note {
  margin-top: 12px;
  font-size: 12px;
  color: var(--c-text-secondary);
}
.backup-path { overflow-wrap: anywhere; font-size: 12px; margin-bottom: 16px; }
.snapshot-title { font-size: 15px; margin: 28px 0 8px; }
@media (max-width: 600px) { .backup-row { flex-wrap: wrap; } }
</style>
