<script setup>
import { ref, onMounted, computed } from 'vue'
import { tauriCallSafe } from '../../../core/tauriBridge'
import { ElMessage, ElMessageBox } from 'element-plus'

// 同步状态
const syncStatus = ref({
  webdav: { connected: false, lastSync: null, error: null },
  feishu: { configured: false, lastPullAt: null, lastPushAt: null },
})
const loading = ref(false)

// WebDAV 配置
const webdavForm = ref({
  url: '',
  username: '',
  password: '',
})
const webdavTesting = ref(false)
const webdavTestResult = ref(null)
const webdavSyncing = ref(false)

// 冲突状态
const conflictState = ref({
  show: false,
  localEtag: null,
  remoteEtag: null,
  resolving: false,
})

// 飞书同步
const feishuPulling = ref(false)
const feishuPushing = ref(false)
const feishuSyncInfo = ref({
  configured: false,
  lastPullAt: null,
  lastPushAt: null,
  lastPullCount: null,
  lastPushCount: null,
  appToken: null,
  tableId: null,
})

async function loadSyncStatus() {
  loading.value = true
  const result = await tauriCallSafe('get_sync_status')
  if (result.ok && result.data) {
    syncStatus.value = result.data
  }
  loading.value = false
}

async function loadFeishuSyncInfo() {
  const result = await tauriCallSafe('get_feishu_sync_info')
  if (result.ok) {
    feishuSyncInfo.value = result.data
  }
}

// WebDAV 测试连接
async function testWebdav() {
  if (!webdavForm.value.url) {
    ElMessage.warning('请填写 WebDAV 地址')
    return
  }
  webdavTesting.value = true
  webdavTestResult.value = null
  const result = await tauriCallSafe('test_webdav_connection', {
    url: webdavForm.value.url,
    username: webdavForm.value.username,
    password: webdavForm.value.password,
  })
  webdavTesting.value = false
  if (result.ok) {
    webdavTestResult.value = result.data
    if (result.data.includes('成功')) {
      ElMessage.success(result.data)
    } else {
      ElMessage.warning(result.data)
    }
  } else {
    ElMessage.error(result.error || '测试失败')
  }
}

// WebDAV 同步检查
async function checkWebdavSync() {
  if (!webdavForm.value.url) {
    ElMessage.warning('请填写 WebDAV 地址')
    return
  }
  webdavSyncing.value = true
  const result = await tauriCallSafe('webdav_startup_sync', {
    url: webdavForm.value.url,
    username: webdavForm.value.username,
    password: webdavForm.value.password,
  })
  webdavSyncing.value = false

  if (result.ok) {
    const data = result.data
    if (data.conflict) {
      // 显示冲突解决器
      conflictState.value = {
        show: true,
        localEtag: data.localEtag,
        remoteEtag: data.remoteEtag,
        resolving: false,
      }
    } else if (data.direction === 'none') {
      ElMessage.success('数据库已是最新')
    } else if (data.direction === 'first_push') {
      await doWebdavPush()
    } else if (data.direction === 'pull') {
      await doWebdavPull()
    }
  } else {
    ElMessage.error(result.error || '同步检查失败')
  }
}

// WebDAV 推送
async function doWebdavPush() {
  if (!webdavForm.value.url) {
    ElMessage.warning('请填写 WebDAV 地址')
    return
  }
  webdavSyncing.value = true
  const result = await tauriCallSafe('webdav_push', {
    url: webdavForm.value.url,
    username: webdavForm.value.username,
    password: webdavForm.value.password,
  })
  webdavSyncing.value = false

  if (result.ok) {
    ElMessage.success('推送成功')
    await loadSyncStatus()
  } else {
    ElMessage.error(result.error || '推送失败')
  }
}

// WebDAV 拉取
async function doWebdavPull() {
  if (!webdavForm.value.url) {
    ElMessage.warning('请填写 WebDAV 地址')
    return
  }
  webdavSyncing.value = true
  const result = await tauriCallSafe('webdav_pull', {
    url: webdavForm.value.url,
    username: webdavForm.value.username,
    password: webdavForm.value.password,
  })
  webdavSyncing.value = false

  if (result.ok) {
    ElMessage.success('拉取成功')
    await loadSyncStatus()
  } else {
    ElMessage.error(result.error || '拉取失败')
  }
}

// 冲突解决：保留本地
async function resolveKeepLocal() {
  conflictState.value.resolving = true
  const result = await tauriCallSafe('webdav_resolve_keep_local', {
    url: webdavForm.value.url,
    username: webdavForm.value.username,
    password: webdavForm.value.password,
  })
  conflictState.value.resolving = false
  conflictState.value.show = false

  if (result.ok) {
    ElMessage.success('已保留本地版本并上传')
    await loadSyncStatus()
  } else {
    ElMessage.error(result.error || '操作失败')
  }
}

// 冲突解决：保留远程
async function resolveKeepRemote() {
  conflictState.value.resolving = true
  const result = await tauriCallSafe('webdav_resolve_keep_remote', {
    url: webdavForm.value.url,
    username: webdavForm.value.username,
    password: webdavForm.value.password,
  })
  conflictState.value.resolving = false
  conflictState.value.show = false

  if (result.ok) {
    ElMessage.success('已保留远程版本')
    await loadSyncStatus()
  } else {
    ElMessage.error(result.error || '操作失败')
  }
}

// 飞书拉取
async function doFeishuPull() {
  if (!feishuSyncInfo.value.appToken || !feishuSyncInfo.value.tableId) {
    ElMessage.warning('请先在设置中配置飞书表格')
    return
  }
  feishuPulling.value = true
  const result = await tauriCallSafe('sync_feishu_pull', {
    appToken: feishuSyncInfo.value.appToken,
    tableId: feishuSyncInfo.value.tableId,
  })
  feishuPulling.value = false
  if (result.ok) {
    ElMessage.success(`拉取完成：${result.data.pulled} 条`)
    await loadFeishuSyncInfo()
    await loadSyncStatus()
  } else {
    ElMessage.error(result.error || '拉取失败')
  }
}

// 飞书推送
async function doFeishuPush() {
  if (!feishuSyncInfo.value.appToken || !feishuSyncInfo.value.tableId) {
    ElMessage.warning('请先在设置中配置飞书表格')
    return
  }
  feishuPushing.value = true
  const result = await tauriCallSafe('sync_feishu_push', {
    appToken: feishuSyncInfo.value.appToken,
    tableId: feishuSyncInfo.value.tableId,
  })
  feishuPushing.value = false
  if (result.ok) {
    ElMessage.success(`推送完成：${result.data.pushed} 条`)
    await loadFeishuSyncInfo()
    await loadSyncStatus()
  } else {
    ElMessage.error(result.error || '推送失败')
  }
}

onMounted(() => {
  loadSyncStatus()
  loadFeishuSyncInfo()
})
</script>

<template>
  <div class="sync-status-view">
    <el-row :gutter="16">
      <!-- WebDAV 同步 -->
      <el-col :span="12">
        <el-card>
          <template #header>
            <div class="card-header">
              <strong>☁️ WebDAV 同步</strong>
              <el-tag
                :type="syncStatus.webdav?.connected ? 'success' : 'info'"
                size="small"
              >
                {{ syncStatus.webdav?.connected ? '已连接' : '未连接' }}
              </el-tag>
            </div>
          </template>

          <el-form label-width="80px" size="default">
            <el-form-item label="地址">
              <el-input
                v-model="webdavForm.url"
                placeholder="https://dav.example.com/remote.php/dav/files/user/"
              />
            </el-form-item>
            <el-form-item label="用户名">
              <el-input v-model="webdavForm.username" placeholder="WebDAV 用户名" />
            </el-form-item>
            <el-form-item label="密码">
              <el-input v-model="webdavForm.password" type="password" show-password placeholder="WebDAV 密码" />
            </el-form-item>
            <el-form-item>
              <el-button
                :loading="webdavTesting"
                @click="testWebdav"
                :type="webdavTestResult?.includes('成功') ? 'success' : webdavTestResult ? 'warning' : 'default'"
              >
                {{ webdavTestResult || '测试连接' }}
              </el-button>
            </el-form-item>
          </el-form>

          <div class="sync-actions">
            <el-button
              type="primary"
              :loading="webdavSyncing"
              @click="checkWebdavSync"
            >
              🔄 检查同步
            </el-button>
            <el-button
              type="success"
              :loading="webdavSyncing"
              @click="doWebdavPush"
            >
              ⬆️ 推送
            </el-button>
            <el-button
              type="warning"
              :loading="webdavSyncing"
              @click="doWebdavPull"
            >
              ⬇️ 拉取
            </el-button>
          </div>

          <el-descriptions :column="1" border size="small" style="margin-top: 12px" v-if="syncStatus.webdav?.lastSync">
            <el-descriptions-item label="上次同步">
              {{ syncStatus.webdav.lastSync }}
            </el-descriptions-item>
          </el-descriptions>
        </el-card>
      </el-col>

      <!-- 飞书同步 -->
      <el-col :span="12">
        <el-card>
          <template #header>
            <div class="card-header">
              <strong>🔄 飞书同步</strong>
              <el-tag
                :type="feishuSyncInfo.configured ? 'success' : 'info'"
                size="small"
              >
                {{ feishuSyncInfo.configured ? '已配置' : '未配置' }}
              </el-tag>
            </div>
          </template>

          <div class="sync-info">
            <el-descriptions :column="1" border size="small">
              <el-descriptions-item label="上次拉取">
                {{ feishuSyncInfo.lastPullAt || '无' }}
              </el-descriptions-item>
              <el-descriptions-item label="拉取记录数">
                {{ feishuSyncInfo.lastPullCount ?? '-' }}
              </el-descriptions-item>
              <el-descriptions-item label="上次推送">
                {{ feishuSyncInfo.lastPushAt || '无' }}
              </el-descriptions-item>
              <el-descriptions-item label="推送记录数">
                {{ feishuSyncInfo.lastPushCount ?? '-' }}
              </el-descriptions-item>
            </el-descriptions>
          </div>

          <div class="sync-actions">
            <el-button
              type="primary"
              :loading="feishuPulling"
              @click="doFeishuPull"
              :disabled="!feishuSyncInfo.configured"
            >
              ⬇️ 从飞书拉取
            </el-button>
            <el-button
              type="success"
              :loading="feishuPushing"
              @click="doFeishuPush"
              :disabled="!feishuSyncInfo.configured"
            >
              ⬆️ 推送到飞书
            </el-button>
          </div>

          <el-alert
            v-if="!feishuSyncInfo.configured"
            title="请先在「设置」页面配置飞书凭证和表格信息"
            type="info"
            show-icon
            :closable="false"
            style="margin-top: 12px"
          />
        </el-card>
      </el-col>
    </el-row>

    <!-- 冲突解决器 -->
    <el-dialog
      v-model="conflictState.show"
      title="⚠️ 同步冲突"
      width="600px"
      :close-on-click-modal="false"
      :close-on-press-escape="false"
      :show-close="!conflictState.resolving"
    >
      <div class="conflict-resolver">
        <el-alert
          title="检测到数据库版本冲突"
          description="本地数据库和远程数据库的版本不一致，请选择要保留的版本。"
          type="warning"
          show-icon
          :closable="false"
        />

        <div class="conflict-compare">
          <el-row :gutter="20">
            <el-col :span="12">
              <el-card class="conflict-card local" shadow="hover">
                <template #header>
                  <div class="conflict-card-header">
                    <strong>💻 本地版本</strong>
                    <el-tag type="info" size="small">当前设备</el-tag>
                  </div>
                </template>
                <div class="conflict-detail">
                  <p><strong>ETag:</strong></p>
                  <code>{{ conflictState.localEtag || '未知' }}</code>
                  <p style="margin-top: 12px">保留此版本将上传本地数据库覆盖远程。</p>
                </div>
                <el-button
                  type="primary"
                  :loading="conflictState.resolving"
                  @click="resolveKeepLocal"
                  style="width: 100%; margin-top: 12px"
                >
                  保留本地版本
                </el-button>
              </el-card>
            </el-col>
            <el-col :span="12">
              <el-card class="conflict-card remote" shadow="hover">
                <template #header>
                  <div class="conflict-card-header">
                    <strong>☁️ 远程版本</strong>
                    <el-tag type="warning" size="small">WebDAV</el-tag>
                  </div>
                </template>
                <div class="conflict-detail">
                  <p><strong>ETag:</strong></p>
                  <code>{{ conflictState.remoteEtag || '未知' }}</code>
                  <p style="margin-top: 12px">保留此版本将下载远程数据库覆盖本地。</p>
                </div>
                <el-button
                  type="warning"
                  :loading="conflictState.resolving"
                  @click="resolveKeepRemote"
                  style="width: 100%; margin-top: 12px"
                >
                  保留远程版本
                </el-button>
              </el-card>
            </el-col>
          </el-row>
        </div>

        <el-alert
          title="注意"
          description="选择保留某个版本后，另一个版本的数据将被覆盖。请确保已备份重要数据。"
          type="info"
          show-icon
          :closable="false"
          style="margin-top: 16px"
        />
      </div>
    </el-dialog>

    <!-- 全局同步状态 -->
    <el-card style="margin-top: 16px">
      <template #header>
        <div class="card-header">
          <strong>📊 同步概览</strong>
          <el-button size="small" @click="loadSyncStatus" :loading="loading">刷新</el-button>
        </div>
      </template>

      <el-empty v-if="!syncStatus.webdav?.connected && !feishuSyncInfo.configured" description="尚未配置任何同步服务" />

      <el-row :gutter="16" v-else>
        <el-col :span="12">
          <el-statistic title="WebDAV 状态" :value="syncStatus.webdav?.connected ? '已连接' : '未连接'" />
        </el-col>
        <el-col :span="12">
          <el-statistic title="飞书状态" :value="feishuSyncInfo.configured ? '已配置' : '未配置'" />
        </el-col>
      </el-row>
    </el-card>
  </div>
</template>

<style scoped>
.sync-status-view {
  max-width: 1000px;
}

.card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.sync-info {
  margin-bottom: 16px;
}

.sync-actions {
  display: flex;
  gap: 12px;
  margin-top: 16px;
}

.conflict-resolver {
  padding: 8px 0;
}

.conflict-compare {
  margin-top: 16px;
}

.conflict-card {
  height: 100%;
}

.conflict-card.local {
  border-left: 4px solid #409eff;
}

.conflict-card.remote {
  border-left: 4px solid #e6a23c;
}

.conflict-card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.conflict-detail {
  font-size: 14px;
  line-height: 1.6;
}

.conflict-detail code {
  background: #f5f7fa;
  padding: 4px 8px;
  border-radius: 4px;
  font-size: 12px;
  word-break: break-all;
}
</style>
