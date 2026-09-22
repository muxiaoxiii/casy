<script setup>
import { ref, onMounted } from 'vue'
import { casyContext } from '../../../core/plugin/context'
import { useSettingsStore } from '../../../stores/settings'
import { ElMessage } from 'element-plus'

const settingsStore = useSettingsStore()

const webdavSaving = ref(false)
const webdavTesting = ref(false)
const webdavStatus = ref(null)
const persistedStatus = ref(null)
async function refreshStatus() {
  const result = await casyContext.sync.status()
  if (result.ok) persistedStatus.value = result.data
}
onMounted(refreshStatus)

async function saveWebdavConfig() {
  webdavSaving.value = true
  const result = await settingsStore.save()
  webdavSaving.value = false
  if (result.ok) {
    ElMessage.success('WebDAV 配置已保存')
    await refreshStatus()
  } else {
    ElMessage.error(result.error || '保存失败')
  }
}

async function testWebdavConnection() {
  webdavTesting.value = true
  webdavStatus.value = null
  const result = await casyContext.sync.testWebdav(
    settingsStore.webdavUrl,
    settingsStore.webdavUsername,
    settingsStore.webdavPassword
  )
  webdavTesting.value = false
  await refreshStatus()
  if (result.ok) {
    webdavStatus.value = 'ok'
    ElMessage.success(result.data || '连接正常')
  } else {
    webdavStatus.value = 'fail'
    ElMessage.error(result.error || '连接失败')
  }
}
</script>

<template>
  <div class="tab-content">
    <el-card>
      <template #header>
        <div class="card-header">
          <strong>WebDAV 同步</strong>
          <el-tag v-if="settingsStore.webdavUrl" type="success" size="small">已配置</el-tag>
          <el-tag v-else type="info" size="small">未配置</el-tag>
        </div>
      </template>

      <p class="tip">通过 WebDAV 传输加密数据库快照。仅适用于使用同一数据库密钥的设备；附件请通过完整加密备份迁移。</p>

      <p v-if="persistedStatus" class="tip">连接状态：{{ ({ unconfigured: '未配置', unknown: '尚未检测', connected: '最近检测成功', failed: '最近检测失败' })[persistedStatus.connectionState] || '尚未检测' }}<span v-if="persistedStatus.lastCheckedAt"> · {{ persistedStatus.lastCheckedAt }}</span></p>
      <el-form label-width="100px" size="default">
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
          <span class="field-hint">启动时检查远程版本，有冲突时手动处理</span>
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

.field-hint {
  color: var(--gray-400);
  font-size: 12px;
  margin-left: 8px;
}
</style>
