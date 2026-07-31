<script setup>
import { ref, onMounted } from 'vue'
import { useSettingsStore } from '../../stores/settings.js'
import FeishuSettings from './components/FeishuSettings.vue'
import WebDAVSettings from './components/WebDAVSettings.vue'
import AISettings from './components/AISettings.vue'
import ImapSettings from './components/ImapSettings.vue'
import GeneralSettings from './components/GeneralSettings.vue'

const settingsStore = useSettingsStore()
const activeTab = ref('feishu')

onMounted(async () => {
  await settingsStore.load()
})
</script>

<template>
  <div class="settings-page">
    <h2 class="page-title">设置</h2>
    <el-tabs v-model="activeTab" tab-position="left" class="settings-tabs">
      <el-tab-pane label="飞书同步" name="feishu">
        <FeishuSettings />
      </el-tab-pane>
      <el-tab-pane label="WebDAV 同步" name="webdav">
        <WebDAVSettings />
      </el-tab-pane>
      <el-tab-pane label="AI 后端" name="ai">
        <AISettings />
      </el-tab-pane>
      <el-tab-pane label="邮件监听" name="imap">
        <ImapSettings />
      </el-tab-pane>
      <el-tab-pane label="通用设置" name="general">
        <GeneralSettings />
      </el-tab-pane>
    </el-tabs>
  </div>
</template>

<style scoped>
.settings-page {
  padding: 20px;
  height: 100%;
  overflow: auto;
}

.page-title {
  margin: 0 0 20px;
  font-size: 20px;
  font-weight: 600;
}

.settings-tabs {
  height: calc(100vh - 120px);
}
</style>
