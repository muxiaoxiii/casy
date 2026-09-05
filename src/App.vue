<script setup>
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { safeListen } from './core/tauriEvents'
import { casyContext } from './core/plugin/context'
import ReminderToast from './shared/components/ReminderToast.vue'
import ReminderBanner from './shared/components/ReminderBanner.vue'
import DecisionReviewNotice from './shared/components/DecisionReviewNotice.vue'
import OverdueMorningBrief from './shared/components/OverdueMorningBrief.vue'
import AIStatusBadge from './shared/components/AIStatusBadge.vue'
import OnboardingWizard from './shared/components/OnboardingWizard.vue'
import GlobalSearch from './components/GlobalSearch.vue'
import NotificationBell from './modules/notifications/components/NotificationBell.vue'
import UnifiedCaptureDialog from './shared/components/UnifiedCaptureDialog.vue'
import { registerShortcut } from './shared/keyboard'
import { useProfileStore } from './stores/profile'
import { useSettingsStore } from './stores/settings'
import { useAiSettingsStore } from './stores/aiSettings'
import { applyThemePreference, disposeThemeListener } from './shared/theme'
import { isTauriRuntime } from './core/mockData'
import {
  DataBoard,
  Briefcase,
  Calendar,
  Finished,
  Box,
  Collection,
  Document,
  Setting,
  Plus,
  Search,
  Expand,
  Fold,
  Folder,
  Bell,
  Cpu,
  User,
  Sunny,
} from '@element-plus/icons-vue'

import { useI18n } from 'vue-i18n'

const router = useRouter()
const route = useRoute()
const settingsStore = useSettingsStore()
const aiSettings = useAiSettingsStore()
const { locale } = useI18n()
const isBrowserPreview = computed(() => !isTauriRuntime())

function applyTheme(theme) {
  applyThemePreference(theme || settingsStore.theme || 'system')
}

watch(() => settingsStore.theme, (newTheme) => {
  applyTheme(newTheme)
})

watch(() => settingsStore.language, (newLang) => {
  if (newLang) {
    locale.value = newLang
    localStorage.setItem('casy_language', newLang)
  }
})

// ============================================================
// 律师画像 / 首次使用引导
// ============================================================
const profileStore = useProfileStore()
const showOnboarding = ref(false)

async function checkOnboarding() {
  await profileStore.load()
  if (!profileStore.onboardingCompleted && !localStorage.getItem('casy_onboarding_dismissed')) {
    showOnboarding.value = true
  }
}

function onOnboardingDismiss() {
  localStorage.setItem('casy_onboarding_dismissed', '1')
}

// ============================================================
// 侧栏折叠
// ============================================================
const sidebarCollapsed = ref(localStorage.getItem('casy_sidebar_collapsed') === '1')
const mobileNavOpen = ref(false)
function toggleSidebar() {
  if (window.matchMedia('(max-width: 900px)').matches) {
    mobileNavOpen.value = !mobileNavOpen.value
    return
  }
  sidebarCollapsed.value = !sidebarCollapsed.value
  localStorage.setItem('casy_sidebar_collapsed', sidebarCollapsed.value ? '1' : '0')
}
watch(() => route.fullPath, () => { mobileNavOpen.value = false })
const showGlobalSearch = ref(false)

const aiStatusText = computed(() => {
  const profile = aiSettings.config.profiles.find(p => p.id === aiSettings.config.activeId)
  return profile ? `${profile.name} (${profile.model})` : 'AI 未配置'
})

// ============================================================
// 导航配置（对标 Stitch UI v4.0 核心三大分组）
// ============================================================
const navGroups = [
  {
    label: 'Focus',
    items: [
      { name: 'home', label: 'Today', sublabel: '今日', icon: Sunny, routePrefix: '/' },
      { name: 'inbox', label: 'Inbox', sublabel: '收件箱', icon: Box, routePrefix: '/inbox' },
    ],
  },
  {
    label: 'Matters',
    items: [
      { name: 'cases', label: 'Cases', sublabel: '案件', icon: Briefcase, routePrefix: '/cases' },
      { name: 'tasks', label: 'Tasks', sublabel: '任务', icon: Finished, routePrefix: '/tasks' },
      { name: 'calendar', label: 'Calendar', sublabel: '日历', icon: Calendar, routePrefix: '/calendar' },
    ],
  },
  {
    label: 'Knowledge',
    items: [
      { name: 'knowledge', label: 'Vault', sublabel: '知识库', icon: Collection, routePrefix: '/knowledge' },
      { name: 'docs', label: 'Drafting', sublabel: '文书工坊', icon: Document, routePrefix: '/docs' },
    ],
  },
]

const utilityModules = [
  { name: 'projects', label: 'Projects', sublabel: '项目', icon: Folder },
  { name: 'clients', label: 'Clients', sublabel: '客户', icon: Folder },
  { name: 'dashboard', label: 'Dashboard', sublabel: '数据看板', icon: Cpu },
  { name: 'ai', label: 'AI Companion', sublabel: 'AI 智伴', icon: Cpu },
  { name: 'reminder', label: 'Reminders', sublabel: '提醒预警', icon: Bell },
  { name: 'sync', label: 'Sync Status', sublabel: '同步状态', icon: Cpu },
]

function isNavActive(item) {
  if (item.name === 'home') {
    return route.path === '/' || route.name === 'home'
  }
  if (item.routePrefix && item.routePrefix !== '/') {
    return route.path.startsWith(item.routePrefix)
  }
  return route.name === item.name
}

// ============================================================
// 统一捕获
// ============================================================
const showUnifiedCapture = ref(false)
const captureInitialAction = ref('auto')

function openUnifiedCapture(action = 'auto') {
  const mapping = {
    note: 'save_knowledge',
    task: 'create_task',
    event: 'create_event',
    quick: 'auto',
  }
  captureInitialAction.value = mapping[action] || action || 'auto'
  showUnifiedCapture.value = true
}

function openCreateTask() {
  openUnifiedCapture('create_task')
}

// ============================================================
// 全局快速捕获
// ============================================================
let unlistenQuickCapture = null

async function setupQuickCaptureListener() {
  try {
    unlistenQuickCapture = await safeListen('global:quick_capture', (event) => {
      openUnifiedCapture(event.payload || 'auto')
    })
    
    // 全局拖拽文件支持 (Tauri 原生事件)
    await safeListen('tauri://drag-drop', (event) => {
      // payload 包含 paths (文件路径数组)
      window.dispatchEvent(new CustomEvent('casy:file-drop', { detail: event.payload }))
    })
  } catch (e) {
    console.warn('[Casy] 全局事件监听建立失败:', e)
  }
}

function handleOpenCapture(event) {
  openUnifiedCapture(event.detail?.action || 'auto')
}

// ============================================================
// 生命周期与快捷键
// ============================================================
let unregisterShortcuts = []

onMounted(async () => {
  void aiSettings.load()
  await settingsStore.load()
  applyTheme(settingsStore.theme)
  checkOnboarding()
  setupQuickCaptureListener()
  window.addEventListener('casy:open-capture', handleOpenCapture)

  // ⌘K 全局搜索
  unregisterShortcuts.push(
    registerShortcut(
      'meta+k',
      () => { showGlobalSearch.value = !showGlobalSearch.value },
      { description: '全局搜索面板开关' },
    ),
    registerShortcut('ctrl+k', () => { showGlobalSearch.value = !showGlobalSearch.value }, { description: '全局搜索面板开关' }),
    // ⌘I 快速捕获
    registerShortcut('meta+i', () => { openUnifiedCapture('auto') }, { description: '快速捕获' }),
    registerShortcut('ctrl+i', () => { openUnifiedCapture('auto') }, { description: '快速捕获' }),
  )
})

onUnmounted(() => {
  if (unlistenQuickCapture) unlistenQuickCapture()
  window.removeEventListener('casy:open-capture', handleOpenCapture)
  unregisterShortcuts.forEach(fn => fn())
  unregisterShortcuts = []
  disposeThemeListener()
})

function onMenuSelect(name) {
  router.push({ name })
}
</script>

<template>
  <div class="app-shell">
    <!-- ═══ 左侧侧栏 (Stitch UI 240px Fixed Sidebar) ═══ -->
    <button v-if="mobileNavOpen" class="nav-backdrop" aria-label="关闭导航" @click="mobileNavOpen = false" />
    <aside id="app-navigation" class="app-sidebar" :class="{ collapsed: sidebarCollapsed && !mobileNavOpen, 'mobile-open': mobileNavOpen }" @keydown.esc="mobileNavOpen = false">
      <!-- 品牌 Header -->
      <div class="sidebar-brand" @click="router.push('/')">
        <div class="brand-badge">
          <span class="brand-grid-icon">
            <span class="grid-cell" />
            <span class="grid-cell" />
            <span class="grid-cell" />
            <span class="grid-cell" />
          </span>
        </div>
        <div v-show="!sidebarCollapsed" class="brand-copy">
          <span class="brand-title">Casy v5.0</span>
          <span class="brand-subtitle">法律工作台</span>
        </div>
      </div>

      <!-- 导航组 -->
      <nav class="sidebar-nav">
        <section v-for="group in navGroups" :key="group.label" class="nav-group">
          <h3 v-show="!sidebarCollapsed" class="nav-group-label">{{ group.label }}</h3>
          <div class="nav-items-stack">
            <button
              v-for="item in group.items"
              :key="item.name"
              type="button"
              class="nav-item"
              :class="{ active: isNavActive(item) }"
              :aria-current="isNavActive(item) ? 'page' : undefined"
              @click="onMenuSelect(item.name)"
              :title="item.label"
            >
              <el-icon class="nav-icon" :size="18">
                <component :is="item.icon" />
              </el-icon>
              <span v-show="!sidebarCollapsed" class="nav-label-group">
                <span class="nav-label-main">{{ item.label }}</span>
                <span class="nav-label-sub">{{ item.sublabel }}</span>
              </span>
            </button>
          </div>
        </section>
      </nav>

      <!-- 侧栏底部：AI 状态 + 用户名片 + 设置 -->
      <div class="sidebar-footer">
        <!-- AI 模型状态药丸 -->
        <div v-show="!sidebarCollapsed" class="ai-status-pill" :class="{ disabled: !aiSettings.config.activeId }">
          <span class="ai-status-text">{{ aiStatusText }}</span>
        </div>

        <!-- 律师名片 -->
        <div
          v-show="!sidebarCollapsed"
          class="user-profile-card"
          @click="onMenuSelect('settings')"
        >
          <div class="user-avatar">
            {{ profileStore.name?.trim()?.slice(0, 1) || 'W' }}
          </div>
          <div class="user-info">
            <span class="user-name">{{ profileStore.name?.trim() || 'Lawyer Wang' }}</span>
            <span class="user-role">{{ profileStore.practice_areas?.[0] || 'Senior Partner' }}</span>
          </div>
        </div>

        <!-- 设置项 -->
        <button
          type="button"
          class="nav-item settings-item"
          :class="{ active: route.name === 'settings' }"
          @click="onMenuSelect('settings')"
          title="Settings / 设置"
        >
          <el-icon class="nav-icon" :size="18"><Setting /></el-icon>
          <span v-show="!sidebarCollapsed" class="nav-label-main">Settings</span>
        </button>
      </div>
    </aside>

    <!-- ═══ 右侧主工作区 ═══ -->
    <div class="app-main">
      <!-- 顶栏 (Stitch UI 64px Topbar with Backdrop Blur) -->
      <header class="topbar">
        <div class="topbar-left">
          <button class="sidebar-toggle-btn" @click="toggleSidebar" title="折叠/展开侧栏" aria-label="折叠/展开侧栏" aria-controls="app-navigation" :aria-expanded="mobileNavOpen || !sidebarCollapsed">
            <el-icon :size="17">
              <Expand v-if="sidebarCollapsed" />
              <Fold v-else />
            </el-icon>
          </button>

          <!-- 全局搜索框 -->
          <div class="search-trigger" @click="showGlobalSearch = true">
            <el-icon class="search-icon" :size="16"><Search /></el-icon>
            <span class="search-placeholder">Search matters, tasks, laws... (CMD+K)</span>
            <span class="kbd-badge">⌘K</span>
          </div>
        </div>

        <div class="topbar-right">
          <!-- W2 通知中心（铃铛 + 未读角标） -->
          <NotificationBell />

          <!-- 浏览器预览模式标识 -->
          <div v-if="isBrowserPreview" class="browser-preview-pill" title="当前在纯浏览器环境运行，数据由 Mock 驱动">
            <span class="preview-dot" />
            <span class="preview-text">Mock 预览</span>
          </div>

          <!-- 快捷新建任务 -->
          <button class="btn-secondary" @click="openCreateTask">
            <el-icon :size="15"><Finished /></el-icon>
            <span>+ Task</span>
          </button>

          <!-- 快速统一捕获 -->
          <button class="btn-primary" @click="openUnifiedCapture('auto')">
            <el-icon :size="15"><Plus /></el-icon>
            <span>+ Capture</span>
            <span class="shortcut-tag">⌘I</span>
          </button>
        </div>
      </header>

      <!-- 主体内容滚动区 -->
      <main class="content-scroll">
        <router-view />
      </main>
    </div>
  </div>

  <!-- 全局浮层与对话框 -->
  <ReminderToast />
  <ReminderBanner />
  <DecisionReviewNotice />
  <OverdueMorningBrief />
  <OnboardingWizard v-model="showOnboarding" @dismiss="onOnboardingDismiss" />
  <UnifiedCaptureDialog v-model="showUnifiedCapture" :initial-action="captureInitialAction" />
  <GlobalSearch v-model="showGlobalSearch" />
</template>

<style scoped>
/* ═══════════════════════════════════════════════════════════
   Stitch UI Shell Layout
   ═══════════════════════════════════════════════════════════ */
.app-shell {
  display: flex;
  height: 100dvh;
  width: 100vw;
  background: var(--c-bg-page);
  color: var(--c-text);
  font-family: var(--font-family);
  overflow: hidden;
}

/* ── 侧栏 (240px 稳固侧栏) ────────────────────────────────── */
.app-sidebar {
  width: 240px;
  min-width: 240px;
  background: var(--c-bg-sidebar);
  border-right: 1px solid var(--c-border);
  display: flex;
  flex-direction: column;
  transition: width var(--motion-base) var(--ease-out), min-width var(--motion-base) var(--ease-out);
  overflow: hidden;
  z-index: 50;
  user-select: none;
}

.app-sidebar.collapsed {
  width: 68px;
  min-width: 68px;
}

/* 品牌区 */
.sidebar-brand {
  height: 64px;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 0 20px;
  border-bottom: 1px solid var(--c-border);
  cursor: pointer;
  flex-shrink: 0;
}

.brand-badge {
  width: 32px;
  height: 32px;
  border-radius: var(--c-radius-lg);
  background: var(--c-primary);
  display: grid;
  place-content: center;
  flex-shrink: 0;
  box-shadow: var(--shadow-sm);
}

.brand-grid-icon {
  display: grid;
  grid-template-columns: repeat(2, 6px);
  grid-template-rows: repeat(2, 6px);
  gap: 3px;
}

.grid-cell {
  width: 6px;
  height: 6px;
  border: 1.5px solid var(--c-primary-contrast);
  border-radius: 1px;
}

.brand-copy {
  display: flex;
  flex-direction: column;
  line-height: 1.2;
}

.brand-title {
  font-size: 15px;
  font-weight: 700;
  color: var(--c-primary);
  letter-spacing: -0.3px;
}

.brand-subtitle {
  font-size: 10px;
  color: var(--c-text-secondary);
  margin-top: 1px;
}

/* 导航组 */
.sidebar-nav {
  flex: 1;
  overflow-y: auto;
  overflow-x: hidden;
  padding: 18px 12px;
  display: flex;
  flex-direction: column;
  gap: 18px;
}

.nav-group {
  display: flex;
  flex-direction: column;
}

.nav-group-label {
  font-size: 10.5px;
  font-weight: 700;
  color: var(--slate-gray-light);
  text-transform: uppercase;
  letter-spacing: 0.8px;
  padding: 0 12px 6px;
  margin: 0;
}

.nav-items-stack {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
  height: 38px;
  padding: 0 12px;
  border-radius: var(--c-radius-lg);
  cursor: pointer;
  color: var(--c-text-regular);
  transition: all var(--motion-fast) var(--ease-out);
  border: none;
  background: transparent;
  text-align: left;
  font-family: inherit;
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

.nav-icon {
  flex-shrink: 0;
}

.nav-label-group {
  display: flex;
  align-items: baseline;
  gap: 6px;
  min-width: 0;
}

.nav-label-main {
  font-size: 13px;
  font-weight: 500;
}

.nav-label-sub {
  font-size: 11px;
  color: var(--c-text-secondary);
  opacity: 0.8;
}

.nav-item.active .nav-label-sub {
  color: var(--c-primary);
  opacity: 0.7;
}

/* 侧栏底部 */
.sidebar-footer {
  padding: 12px;
  border-top: 1px solid var(--c-border);
  display: flex;
  flex-direction: column;
  gap: 8px;
  background: var(--c-bg-sidebar);
}

.ai-status-pill {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-radius: var(--c-radius-full);
  background: var(--bg-success-weak);
  border: 1px solid color-mix(in srgb, var(--status-success) 20%, transparent);
}

.ai-dot-pulse {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--status-success);
  flex-shrink: 0;
}

.ai-status-text {
  font-size: 10.5px;
  font-weight: 600;
  color: var(--status-success);
  text-transform: uppercase;
  letter-spacing: 0.4px;
}

.user-profile-card {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border-radius: var(--c-radius-lg);
  cursor: pointer;
  transition: background var(--motion-fast);
}

.user-profile-card:hover {
  background: var(--c-bg-hover);
}

.user-avatar {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  background: var(--c-primary);
  color: var(--c-primary-contrast);
  display: grid;
  place-items: center;
  font-size: 12px;
  font-weight: 700;
  flex-shrink: 0;
}

.user-info {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.user-name {
  font-size: 12.5px;
  font-weight: 600;
  color: var(--c-text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.user-role {
  font-size: 10.5px;
  color: var(--slate-gray-light);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.settings-item {
  height: 34px;
  padding: 0 10px;
}

/* ── 主区 ─────────────────────────────────────────────── */
.app-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  height: 100dvh;
  background: var(--c-bg-page);
}

/* ── 顶栏 ─────────────────────────────────────────────── */
.topbar {
  height: 64px;
  background: var(--c-bg-topbar);
  backdrop-filter: blur(16px);
  -webkit-backdrop-filter: blur(16px);
  border-bottom: 1px solid var(--c-border);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 28px;
  gap: 16px;
  flex-shrink: 0;
  z-index: 40;
}

.topbar-left {
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 14px;
  flex: 1;
  max-width: 480px;
}

.sidebar-toggle-btn {
  flex-shrink: 0;
  width: 32px;
  height: 32px;
  border-radius: 6px;
  display: grid;
  place-items: center;
  color: var(--c-text-regular);
  cursor: pointer;
  border: none;
  background: transparent;
  transition: background var(--motion-fast);
}

.sidebar-toggle-btn:hover {
  background: var(--c-bg-hover);
  color: var(--c-text);
}

.search-trigger {
  min-width: 0;
  flex: 1;
  height: 36px;
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  padding: 0 12px;
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  transition: all var(--motion-fast) var(--ease-out);
}

.search-trigger:hover {
  border-color: var(--c-primary);
  background: var(--c-bg-card);
}

.search-icon {
  color: var(--slate-gray-light);
}

.search-placeholder {
  flex: 1;
  font-size: 12.5px;
  color: var(--slate-gray-light);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.kbd-badge {
  font-size: 10px;
  font-family: var(--font-mono);
  color: var(--slate-gray-light);
  border: 1px solid var(--c-border);
  border-radius: 4px;
  padding: 1px 5px;
  background: var(--c-bg-subtle);
}

.topbar-right {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 12px;
}

.browser-preview-pill {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 3px 9px;
  border-radius: 999px;
  background: rgba(234, 179, 8, 0.12);
  border: 1px solid rgba(234, 179, 8, 0.3);
  color: #ca8a04;
  font-size: 11px;
  font-weight: 500;
  user-select: none;
  flex-shrink: 0;
}

.preview-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #eab308;
  box-shadow: 0 0 6px rgba(234, 179, 8, 0.6);
}

.preview-text {
  letter-spacing: 0.2px;
}

.btn-secondary {
  white-space: nowrap;
  height: 34px;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 0 12px;
  border-radius: 6px;
  border: 1px solid var(--c-border);
  background: var(--c-bg-card);
  color: var(--c-text-regular);
  font-size: 12.5px;
  font-weight: 500;
  font-family: inherit;
  cursor: pointer;
  transition: all var(--motion-fast);
}

.btn-secondary:hover {
  background: var(--c-bg-hover);
  color: var(--c-text);
  border-color: var(--c-border-strong);
}

.btn-primary {
  white-space: nowrap;
  height: 34px;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 0 14px;
  border-radius: 6px;
  border: none;
  background: var(--c-primary);
  color: var(--c-primary-contrast);
  font-size: 12.5px;
  font-weight: 600;
  font-family: inherit;
  cursor: pointer;
  box-shadow: var(--shadow-sm);
  transition: all var(--motion-fast);
}

.btn-primary:hover {
  background: var(--c-primary-hover);
  transform: translateY(-1px);
}

.btn-primary:active {
  transform: translateY(0);
}

.shortcut-tag {
  font-size: 10px;
  opacity: 0.8;
  margin-left: 2px;
}

/* ── 浏览器预览 Mock 水印 ─────────────────────────────────────────── */
.mock-watermark {
  position: fixed;
  top: 0;
  left: 50%;
  transform: translateX(-50%);
  background-color: var(--c-warning);
  color: var(--c-text-inverse);
  padding: 4px 12px;
  font-size: 11px;
  font-weight: 600;
  border-bottom-left-radius: 6px;
  border-bottom-right-radius: 6px;
  z-index: 9999;
  box-shadow: 0 2px 8px rgba(0,0,0,0.15);
  pointer-events: none;
  opacity: 0.85;
}

/* ── 页面主体滚动 ─────────────────────────────────────────── */
.content-scroll {
  min-height: 0;
  container-type: inline-size;
  flex: 1;
  overflow-y: auto;
  overflow-x: hidden;
  background: var(--c-bg-page);
}

@media (max-width: 900px) {
  .topbar { padding: 0 16px; gap: 10px; }
  .topbar-right { gap: 8px; }
  .shortcut-tag, .kbd-badge { display: none; }
  .app-sidebar:not(.mobile-open) {
    width: 68px;
    min-width: 68px;
  }
  .app-sidebar:not(.mobile-open) .brand-copy,
  .app-sidebar:not(.mobile-open) .nav-group-label,
  .app-sidebar:not(.mobile-open) .nav-label-group,
  .app-sidebar:not(.mobile-open) .ai-status-pill,
  .app-sidebar:not(.mobile-open) .user-profile-card,
  .app-sidebar:not(.mobile-open) .settings-item .nav-label-main {
    display: none;
  }
  .app-sidebar.mobile-open { position: fixed; inset: 0 auto 0 0; width: 240px; min-width: 240px; box-shadow: var(--shadow-lg); }
  .mobile-open .brand-copy, .mobile-open .nav-label-group, .mobile-open .nav-group-label { display: flex !important; }
  .nav-backdrop { position: fixed; inset: 0; background: var(--c-overlay); border: 0; z-index: 45; }
}
@media (max-width: 600px) {
  .app-sidebar:not(.mobile-open) { display: none; }
  .topbar { height: 56px; padding: 0 12px; }
  .topbar-left { gap: 8px; }
  .search-trigger { flex: 0 0 34px; width: 34px; padding: 0; justify-content: center; }
  .search-placeholder, .topbar .btn-secondary { display: none; }
  .btn-primary { padding: 0 10px; }
  .browser-preview-pill { padding: 3px 6px; font-size: 10px; }
}
</style>
