<script setup>
import { ref } from 'vue'
import { tauriCallSafe } from '../../../core/tauriBridge'
import { useSettingsStore } from '../../../stores/settings'
import { ElMessage } from 'element-plus'

const settingsStore = useSettingsStore()

const webdavSaving = ref(false)
const webdavTesting = ref(false)
const webdavStatus = ref(null)

async function saveWebdavConfig() {
  webdavSaving.value = true
  const result = await settingsStore.save()
  webdavSaving.value = false
  if (result.ok) {
    ElMessage.success('WebDAV 配置已保存')
  } else {
    ElMessage.error(result.error || '保存失败')
  }
}

async function testWebdavConnection() {
  webdavTesting.value = true
  webdavStatus.value = null
  const result = await tauriCallSafe('test_webdav_connection')
  webdavTesting.value = false
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
          <strong>☁️ WebDAV 同步</strong>
          <el-tag v-if="settingsStore.webdavUrl" type="success" size="small">已配置</el-tag>
          <el-tag v-else type="info" size="small">未配置</el-tag>
        </div>
      </template>

      <p class="tip">通过 WebDAV 同步数据库到云端，支持坚果云、NextCloud 等服务。</p>

      <el-form label-width="120px" size="default">
        <el-form-item label="WebDAV URL">
          <el-input
            v-model="settingsStore.webdavUrl"
            placeholder="https://dav.example.com/dav/casy.db"
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
            placeholder="WebDAV 密码或应用专用密码"
          />
        </el-form-item>
        <el-form-item label="自动同步">
          <el-switch v-model="settingsStore.webdavAutoSync" />
          <span class="field-hint">开启后每次启动自动同步</span>
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
  color: #909399;
  font-size: 13px;
  margin-bottom: 16px;
}

.field-hint {
  color: #909399;
  font-size: 12px;
  margin-left: 8px;
}
</style>
