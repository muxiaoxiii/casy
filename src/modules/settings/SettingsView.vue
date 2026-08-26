<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useSettingsStore } from '../../stores/settings'
import FeishuSettings from './components/FeishuSettings.vue'
import WebDAVSettings from './components/WebDAVSettings.vue'
import AISettings from './components/AISettings.vue'
import ImapSettings from './components/ImapSettings.vue'
import GeneralSettings from './components/GeneralSettings.vue'
import FolderTemplateSettings from './components/FolderTemplateSettings.vue'
import ReminderSettings from './components/ReminderSettings.vue'
import SmtpMcpSettings from './components/SmtpMcpSettings.vue'
import ProfileSettings from './components/ProfileSettings.vue'
import BackupSettings from './components/BackupSettings.vue'
import { casyContext } from '../../core/plugin/context'
import { ElMessage, ElMessageBox } from 'element-plus'

const settingsStore = useSettingsStore()
const activeTab = ref('feishu')

const seeding = ref(false)
const seedDisabled = ref(false)

async function checkSeedDisabled() {
  const cases = await casyContext.cases.list()
  seedDisabled.value = !!(cases.ok && Array.isArray(cases.data) && (cases.data as unknown[]).length > 0)
}

async function onSeedDemo() {
  try {
    await ElMessageBox.confirm(
      '将注入一套演示数据（案件/任务/项目/知识）。仅可在空库使用；如已有真实数据请勿操作。',
      '载入演示数据',
      { confirmButtonText: '载入', cancelButtonText: '取消', type: 'info' },
    )
  } catch {
    return
  }
  seeding.value = true
  const result = await casyContext.settings.seedDemoData()
  seeding.value = false
  if (result.ok) {
    const d = result.data as Record<string, number>
    ElMessage.success(`已载入：案件 ${d.cases} · 任务 ${d.tasks} · 项目 ${d.projectsPersonal} · 知识 ${d.knowledge}`)
    await settingsStore.load()
  } else {
    ElMessage.error(result.error || '载入失败')
  }
}

onMounted(async () => {
  await settingsStore.load()
  void checkSeedDisabled()
})
</script>

<template>
  <div class="settings-page">
    <h2 class="page-title">设置</h2>
    <el-tabs v-model="activeTab" tab-position="left" class="settings-tabs">
      <el-tab-pane label="律师画像" name="profile">
        <ProfileSettings />
      </el-tab-pane>
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
      <el-tab-pane label="SMTP / MCP" name="smtp-mcp">
        <SmtpMcpSettings />
      </el-tab-pane>
      <el-tab-pane label="通用设置" name="general">
        <GeneralSettings />
        <el-divider style="margin: 18px 0" />
        <div class="demo-seed">
          <div class="ds-text">
            <b>演示数据</b>
            <span>空库时可用：一键注入 2 个法律案件（含庭审与分级期限）、跨桶任务群、个人项目与知识条目——用于体验看板/日历/整理仪式。已有真实数据时按钮不可用。</span>
          </div>
          <el-button
            type="warning"
            plain
            :disabled="seedDisabled"
            :loading="seeding"
            @click="onSeedDemo"
          >
            载入演示数据
          </el-button>
        </div>
      </el-tab-pane>
      <el-tab-pane label="文件夹模板" name="folder-template">
        <FolderTemplateSettings />
      </el-tab-pane>
      <el-tab-pane label="提醒" name="reminder">
        <ReminderSettings />
      </el-tab-pane>
      <el-tab-pane label="数据备份" name="backup">
        <BackupSettings />
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

.demo-seed {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 12px 14px;
  border: 1px dashed var(--c-border);
  border-radius: var(--c-radius-lg);
}
.demo-seed .ds-text {
  flex: 1;
  font-size: 12.5px;
  color: var(--c-text-secondary);
  line-height: 1.7;
  display: flex;
  flex-direction: column;
  gap: 3px;
}
.demo-seed .ds-text b { color: var(--c-text); font-size: 13px; }

.settings-tabs {
  height: calc(100vh - 120px);
}
</style>
