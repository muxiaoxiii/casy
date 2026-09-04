<script setup>
/**
 * 数据备份与恢复（R-5）
 *
 * - 备份 = VACUUM INTO 一致性快照（SQLCipher 加密保持），存应用数据目录 backups/
 * - 恢复 = 覆盖活动库文件，**必须重启应用生效**；恢复前自动生成 pre-restore 快照
 * - 仅接受 backups 目录内的 casy-backup-*.db 文件名
 */
import { ref, onMounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { casyContext } from '../../../core/plugin/context'

const backups = ref([])
const loading = ref(false)
const backingUp = ref(false)
const restoringId = ref(null)

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
    ).finally(() => window.close())
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
    <div class="backup-intro">
      所有数据以加密形式保存在本机。建议每周至少手动备份一次；备份为完整加密快照，
      可在换机或异常时整库恢复。
    </div>

    <div class="backup-actions">
      <el-button type="primary" :loading="backingUp" @click="createBackup">立即备份</el-button>
      <el-button :loading="loading" @click="load">刷新</el-button>
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
</style>
