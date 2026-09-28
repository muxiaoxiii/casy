<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useSettingsStore } from '../../stores/settings'
import FeishuSettings from './components/FeishuSettings.vue'
import WebDAVSettings from './components/WebDAVSettings.vue'
import ToolPolicySettings from './components/ToolPolicySettings.vue'
import AISettings from './components/AISettings.vue'
import ImapSettings from './components/ImapSettings.vue'
import GeneralSettings from './components/GeneralSettings.vue'
import FolderTemplateSettings from './components/FolderTemplateSettings.vue'
import ReminderSettings from './components/ReminderSettings.vue'
import DeadlineRulesSettings from './components/DeadlineRulesSettings.vue'
import SmartRulesSettings from './components/SmartRulesSettings.vue'
import SmtpMcpSettings from './components/SmtpMcpSettings.vue'
import ProfileSettings from './components/ProfileSettings.vue'
import BackupSettings from './components/BackupSettings.vue'
import BriefingStyleSettings from './components/BriefingStyleSettings.vue'
import AboutSettings from './components/AboutSettings.vue'
import { casyContext } from '../../core/plugin/context'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  User, Setting, Cpu, MagicStick, Folder, Bell,
  Cloudy, Link, Message, Connection, Briefcase, InfoFilled, AlarmClock, SetUp
} from '../../shared/icons'

const settingsStore = useSettingsStore()
const activeTab = ref('profile')
const visitedTabs = ref(new Set<string>())
const route = useRoute()
const router = useRouter()
const mobileTabs = [
  ['profile', '律师画像'], ['general', '常规设置'], ['ai', 'AI 引擎配置'],
  ['tools', 'AI 工具策略'], ['briefing-styles', '早报/周报样式'], ['folder-template', '文件夹模板'],
  ['reminder', '智能提醒'], ['deadline-rules', '期限规则'], ['smart-rules', '智能规则'],
  ['webdav', 'WebDAV 备份'], ['feishu', '飞书集成'], ['imap', '邮箱监听'],
  ['smtp-mcp', 'SMTP / MCP'], ['backup', '数据备份'], ['about', '关于 Casy'],
]

watch(() => route.query.tab, tab => {
  activeTab.value = mobileTabs.some(([key]) => key === tab) ? String(tab) : 'profile'
  visitedTabs.value.add(activeTab.value)
}, { immediate: true })
watch(activeTab, tab => {
  visitedTabs.value.add(tab)
  if (tab !== route.query.tab) void router.replace({ query: { ...route.query, tab } })
})

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
    <el-select class="settings-mobile-nav" v-model="activeTab" aria-label="设置分类"><el-option v-for="[value,label] in mobileTabs" :key="value" :value="value" :label="label" /></el-select>
    <div class="settings-sidebar">
      <div class="sidebar-header">
        <h2 class="page-title">{{ $t('nav.settings') }}</h2>
        <span class="page-subtitle">Preferences</span>
      </div>

      <div class="nav-group">
        <div class="nav-title">通用与账号</div>
        <button type="button" class="nav-item" :class="{ active: activeTab === 'profile' }" @click="activeTab = 'profile'">
          <el-icon><User /></el-icon>
          <span>律师画像</span>
        </button>
        <button type="button" class="nav-item" :class="{ active: activeTab === 'general' }" @click="activeTab = 'general'">
          <el-icon><Setting /></el-icon>
          <span>{{ $t('settings.general') }}</span>
        </button>
      </div>

      <div class="nav-group">
        <div class="nav-title">系统引擎</div>
        <button type="button" class="nav-item" :class="{ active: activeTab === 'ai' }" @click="activeTab = 'ai'">
          <el-icon><Cpu /></el-icon>
          <span>{{ $t('settings.ai_model') }}</span>
        </button>
        <button type="button" class="nav-item" :class="{ active: activeTab === 'tools' }" @click="activeTab = 'tools'"><el-icon><SetUp /></el-icon><span>AI 工具策略</span></button>
        <button type="button" class="nav-item" :class="{ active: activeTab === 'briefing-styles' }" @click="activeTab = 'briefing-styles'">
          <el-icon><MagicStick /></el-icon>
          <span>{{ $t('settings.briefing_style') }}</span>
        </button>
        <button type="button" class="nav-item" :class="{ active: activeTab === 'folder-template' }" @click="activeTab = 'folder-template'">
          <el-icon><Folder /></el-icon>
          <span>文件夹模板</span>
        </button>
        <button type="button" class="nav-item" :class="{ active: activeTab === 'reminder' }" @click="activeTab = 'reminder'">
          <el-icon><Bell /></el-icon>
          <span>智能提醒</span>
        </button>
        <button type="button" class="nav-item" :class="{ active: activeTab === 'deadline-rules' }" @click="activeTab = 'deadline-rules'">
          <el-icon><AlarmClock /></el-icon>
          <span>期限规则</span>
        </button>
        <button type="button" class="nav-item" :class="{ active: activeTab === 'smart-rules' }" @click="activeTab = 'smart-rules'">
          <el-icon><SetUp /></el-icon>
          <span>智能规则</span>
        </button>
      </div>

      <div class="nav-group">
        <div class="nav-title">同步与连接</div>
        <button type="button" class="nav-item" :class="{ active: activeTab === 'webdav' }" @click="activeTab = 'webdav'">
          <el-icon><Cloudy /></el-icon>
          <span>WebDAV 备份</span>
        </button>
        <button type="button" class="nav-item" :class="{ active: activeTab === 'feishu' }" @click="activeTab = 'feishu'">
          <el-icon><Link /></el-icon>
          <span>飞书集成</span>
        </button>
        <button type="button" class="nav-item" :class="{ active: activeTab === 'imap' }" @click="activeTab = 'imap'">
          <el-icon><Message /></el-icon>
          <span>邮件监听</span>
        </button>
        <button type="button" class="nav-item" :class="{ active: activeTab === 'smtp-mcp' }" @click="activeTab = 'smtp-mcp'">
          <el-icon><Connection /></el-icon>
          <span>SMTP / MCP</span>
        </button>
      </div>

      <div class="nav-group">
        <div class="nav-title">维护与信息</div>
        <button type="button" class="nav-item" :class="{ active: activeTab === 'backup' }" @click="activeTab = 'backup'">
          <el-icon><Briefcase /></el-icon>
          <span>数据备份</span>
        </button>
        <button type="button" class="nav-item" :class="{ active: activeTab === 'about' }" @click="activeTab = 'about'">
          <el-icon><InfoFilled /></el-icon>
          <span>关于 Casy</span>
        </button>
      </div>
    </div>

    <div class="settings-content">
      <div v-if="visitedTabs.has('profile')" v-show="activeTab === 'profile'"><ProfileSettings /></div>
      <div v-if="visitedTabs.has('general')" v-show="activeTab === 'general'">
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
      <div v-if="visitedTabs.has('ai')" v-show="activeTab === 'ai'"><AISettings /></div>
      <ToolPolicySettings v-if="activeTab === 'tools'" />
      <div v-if="visitedTabs.has('briefing-styles')" v-show="activeTab === 'briefing-styles'"><BriefingStyleSettings /></div>
      <div v-if="visitedTabs.has('folder-template')" v-show="activeTab === 'folder-template'"><FolderTemplateSettings /></div>
      <div v-if="visitedTabs.has('reminder')" v-show="activeTab === 'reminder'"><ReminderSettings /></div>
      <div v-if="visitedTabs.has('deadline-rules')" v-show="activeTab === 'deadline-rules'"><DeadlineRulesSettings /></div>
      <div v-if="visitedTabs.has('smart-rules')" v-show="activeTab === 'smart-rules'"><SmartRulesSettings /></div>
      <div v-if="visitedTabs.has('webdav')" v-show="activeTab === 'webdav'"><WebDAVSettings /></div>
      <div v-if="visitedTabs.has('feishu')" v-show="activeTab === 'feishu'"><FeishuSettings /></div>
      <div v-if="visitedTabs.has('imap')" v-show="activeTab === 'imap'"><ImapSettings /></div>
      <div v-if="visitedTabs.has('smtp-mcp')" v-show="activeTab === 'smtp-mcp'"><SmtpMcpSettings /></div>
      <div v-if="visitedTabs.has('backup')" v-show="activeTab === 'backup'"><BackupSettings /></div>
      <div v-if="visitedTabs.has('about')" v-show="activeTab === 'about'"><AboutSettings /></div>
    </div>
  </div>
</template>

<style scoped>
.settings-page {
  display: flex;
  height: calc(100dvh - var(--app-topbar-height));
  background: var(--c-bg-page);
  overflow: hidden;
}

.settings-sidebar {
  width: 216px;
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
  letter-spacing: 0;
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
  border: 0;
  background: transparent;
  text-align: left;
  font-family: inherit;
  width: 100%;
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
  min-width: 0;
  padding: 32px clamp(20px, 4cqi, 56px);
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

.settings-mobile-nav { display: none; }
@media (max-width: 760px) {
  .settings-page { flex-direction: column; min-width: 0; }
  .settings-sidebar { display: none; }
  .settings-mobile-nav { display: block; width: auto; flex-shrink: 0; margin: 12px 16px 0; }
  .settings-content { padding: 20px 16px; min-height: 0; }
}
</style>
