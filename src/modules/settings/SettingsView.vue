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
import BriefingStyleSettings from './components/BriefingStyleSettings.vue'
import AboutSettings from './components/AboutSettings.vue'
import { casyContext } from '../../core/plugin/context'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  User, Setting, Cpu, MagicStick, Folder, Bell,
  Cloudy, Link, Message, Connection, Briefcase, InfoFilled
} from '@element-plus/icons-vue'

const settingsStore = useSettingsStore()
const activeTab = ref('profile')

const seeding = ref(false)
const seedDisabled = ref(false)

async function checkSeedDisabled() {
  try {
    const cases = await casyContext.cases.list()
    if (cases.ok && cases.data) {
      const items = Array.isArray(cases.data) ? cases.data : (cases.data as any).items || []
      seedDisabled.value = items.length > 0
    }
  } catch (e) {
    console.warn('Failed to check seed status', e)
  }
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
    await checkSeedDisabled()
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
    <div class="settings-sidebar">
      <div class="sidebar-header">
        <h2 class="page-title">{{ $t('nav.settings') }}</h2>
        <span class="page-subtitle">Preferences</span>
      </div>

      <div class="nav-group">
        <div class="nav-title">通用与账号</div>
        <div class="nav-item" :class="{ active: activeTab === 'profile' }" @click="activeTab = 'profile'">
          <el-icon><User /></el-icon>
          <span>律师画像</span>
        </div>
        <div class="nav-item" :class="{ active: activeTab === 'general' }" @click="activeTab = 'general'">
          <el-icon><Setting /></el-icon>
          <span>{{ $t('settings.general') }}</span>
        </div>
      </div>

      <div class="nav-group">
        <div class="nav-title">系统引擎</div>
        <div class="nav-item" :class="{ active: activeTab === 'ai' }" @click="activeTab = 'ai'">
          <el-icon><Cpu /></el-icon>
          <span>{{ $t('settings.ai_model') }}</span>
        </div>
        <div class="nav-item" :class="{ active: activeTab === 'briefing-styles' }" @click="activeTab = 'briefing-styles'">
          <el-icon><MagicStick /></el-icon>
          <span>{{ $t('settings.briefing_style') }}</span>
        </div>
        <div class="nav-item" :class="{ active: activeTab === 'folder-template' }" @click="activeTab = 'folder-template'">
          <el-icon><Folder /></el-icon>
          <span>文件夹模板</span>
        </div>
        <div class="nav-item" :class="{ active: activeTab === 'reminder' }" @click="activeTab = 'reminder'">
          <el-icon><Bell /></el-icon>
          <span>智能提醒</span>
        </div>
      </div>

      <div class="nav-group">
        <div class="nav-title">同步与连接</div>
        <div class="nav-item" :class="{ active: activeTab === 'webdav' }" @click="activeTab = 'webdav'">
          <el-icon><Cloudy /></el-icon>
          <span>WebDAV 同步</span>
        </div>
        <div class="nav-item" :class="{ active: activeTab === 'feishu' }" @click="activeTab = 'feishu'">
          <el-icon><Link /></el-icon>
          <span>飞书集成</span>
        </div>
        <div class="nav-item" :class="{ active: activeTab === 'imap' }" @click="activeTab = 'imap'">
          <el-icon><Message /></el-icon>
          <span>邮件监听</span>
        </div>
        <div class="nav-item" :class="{ active: activeTab === 'smtp-mcp' }" @click="activeTab = 'smtp-mcp'">
          <el-icon><Connection /></el-icon>
          <span>SMTP / MCP</span>
        </div>
      </div>

      <div class="nav-group">
        <div class="nav-title">维护与信息</div>
        <div class="nav-item" :class="{ active: activeTab === 'backup' }" @click="activeTab = 'backup'">
          <el-icon><Briefcase /></el-icon>
          <span>数据备份</span>
        </div>
        <div class="nav-item" :class="{ active: activeTab === 'about' }" @click="activeTab = 'about'">
          <el-icon><InfoFilled /></el-icon>
          <span>关于 Casy</span>
        </div>
      </div>
    </div>

    <div class="settings-content">
      <div v-show="activeTab === 'profile'"><ProfileSettings /></div>
      <div v-show="activeTab === 'general'">
        <GeneralSettings />
        <el-divider style="margin: 20px 0" />
        <div class="demo-seed">
          <div class="ds-text">
            <b>演示数据</b>
            <span>空库时可用：一键注入测试数据。已有真实数据时按钮不可用。</span>
          </div>
          <el-button type="warning" plain :disabled="seedDisabled" :loading="seeding" @click="onSeedDemo">
            载入演示数据
          </el-button>
        </div>
      </div>
      <div v-show="activeTab === 'ai'"><AISettings /></div>
      <div v-show="activeTab === 'briefing-styles'"><BriefingStyleSettings /></div>
      <div v-show="activeTab === 'folder-template'"><FolderTemplateSettings /></div>
      <div v-show="activeTab === 'reminder'"><ReminderSettings /></div>
      <div v-show="activeTab === 'webdav'"><WebDAVSettings /></div>
      <div v-show="activeTab === 'feishu'"><FeishuSettings /></div>
      <div v-show="activeTab === 'imap'"><ImapSettings /></div>
      <div v-show="activeTab === 'smtp-mcp'"><SmtpMcpSettings /></div>
      <div v-show="activeTab === 'backup'"><BackupSettings /></div>
      <div v-show="activeTab === 'about'"><AboutSettings /></div>
    </div>
  </div>
</template>

<style scoped>
.settings-page {
  display: flex;
  height: calc(100vh - 64px);
  background: var(--c-bg-page);
  overflow: hidden;
}

.settings-sidebar {
  width: 240px;
  background: var(--c-bg-sidebar);
  border-right: 1px solid var(--c-border);
  padding: 20px 12px;
  display: flex;
  flex-direction: column;
  gap: 16px;
  overflow-y: auto;
  flex-shrink: 0;
  user-select: none;
}

.sidebar-header {
  padding: 0 8px 4px;
}

.page-title {
  margin: 0;
  font-size: 18px;
  font-weight: 700;
  color: var(--c-text-heading);
  letter-spacing: -0.3px;
}

.page-subtitle {
  font-size: 11px;
  color: var(--slate-gray-light);
  text-transform: uppercase;
  font-family: var(--font-mono);
}

.nav-group {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.nav-title {
  font-size: 10.5px;
  font-weight: 700;
  color: var(--slate-gray-light);
  text-transform: uppercase;
  letter-spacing: 0.8px;
  padding: 4px 8px;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  border-radius: var(--c-radius-lg);
  font-size: 13px;
  color: var(--c-text-regular);
  cursor: pointer;
  transition: all var(--motion-fast);
  font-weight: 500;
}

.nav-item:hover {
  background: var(--c-bg-hover);
  color: var(--c-text);
}

.nav-item.active {
  background: var(--c-bg-selected);
  color: var(--c-primary);
  font-weight: 600;
}

.nav-item .el-icon {
  font-size: 16px;
  flex-shrink: 0;
}

.settings-content {
  flex: 1;
  padding: 28px 36px;
  overflow-y: auto;
  background: var(--c-bg-page);
}

.demo-seed {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 14px 18px;
  border: 1px dashed var(--c-border);
  border-radius: var(--c-radius-lg);
  background: var(--c-bg-card);
}

.demo-seed .ds-text {
  flex: 1;
  font-size: 12.5px;
  color: var(--c-text-secondary);
  line-height: 1.5;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.demo-seed .ds-text b {
  color: var(--c-text-heading);
  font-size: 13px;
}
</style>
