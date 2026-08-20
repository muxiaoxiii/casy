<script setup>
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { safeListen } from './core/tauriEvents'
import { ElMessage } from 'element-plus'
import { casyContext } from './core/plugin/context'
import ReminderToast from './shared/components/ReminderToast.vue'
import ReminderBanner from './shared/components/ReminderBanner.vue'
import DecisionReviewNotice from './shared/components/DecisionReviewNotice.vue'
import OverdueMorningBrief from './shared/components/OverdueMorningBrief.vue'
import AIStatusBadge from './shared/components/AIStatusBadge.vue'
import OnboardingWizard from './shared/components/OnboardingWizard.vue'
import { useProfileStore } from './stores/profile'
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
  Clock,
  Warning,
  Folder,
  Bell,
  Cpu,
  Search,
  Expand,
  Fold,
} from '@element-plus/icons-vue'

const router = useRouter()
const route = useRoute()

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
const sidebarCollapsed = ref(false)
function toggleSidebar() {
  sidebarCollapsed.value = !sidebarCollapsed.value
}

// ============================================================
// 今日面板数据
// ============================================================
const todayStats = ref({
  hardSchedule: 0,
  dueToday: 0,
  waitingOverdue: 0,
  needReview: 0,
})

const loadingStats = ref(false)

async function loadTodayStats() {
  loadingStats.value = true
  try {
    const result = await casyContext.cases.todayStats()
    if (result.ok && result.data) {
      todayStats.value = result.data
    }
  } catch (e) {
    console.error('Failed to load today stats:', e)
  }
  loadingStats.value = false
}

// ============================================================
// 核心模块导航（分组）
// ============================================================
const navGroups = [
  {
    label: '工作台',
    items: [
      { name: 'home', label: '今日', icon: DataBoard },
    ],
  },
  {
    label: '核心',
    items: [
      { name: 'cases', label: '案件', icon: Briefcase },
      { name: 'tasks', label: '任务', icon: Finished },
      { name: 'calendar', label: '日历', icon: Calendar },
      { name: 'dashboard', label: '数据看板', icon: Cpu },
    ],
  },
  {
    label: '知识',
    items: [
      { name: 'clients', label: '客户', icon: Folder },
      { name: 'inbox', label: '收件箱', icon: Box },
      { name: 'knowledge', label: '知识库', icon: Collection },
      { name: 'docs', label: '文书', icon: Document },
    ],
  },
]

const allModules = navGroups.flatMap(g => g.items)

// ============================================================
// 模块内 Tab（第三层，内容区顶部）
// ============================================================
const moduleTabs = computed(() => {
  const moduleName = route.name || 'home'

  const tabsMap = {
    cases: [
      { key: 'all', label: '全部' },
      { key: 'my', label: '我负责' },
      { key: 'waiting', label: '等待中' },
      { key: 'closed', label: '已结案' },
      { key: 'client', label: '按客户' },
    ],
    tasks: [
      { key: 'inbox', label: '收件箱' },
      { key: 'next', label: '下一步' },
      { key: 'waiting', label: '等待' },
      { key: 'today', label: '今日' },
      { key: 'review', label: '回顾' },
      { key: 'someday', label: '某天' },
    ],
    knowledge: [
      { key: 'inspiration', label: '灵感' },
      { key: 'method', label: '方法' },
      { key: 'reference', label: '参考' },
      { key: 'question', label: '问题' },
      { key: 'experience', label: '经验' },
      { key: 'log', label: '日志' },
    ],
    calendar: [
      { key: 'month', label: '月视图' },
      { key: 'week', label: '周视图' },
      { key: 'forecast', label: '预测' },
    ],
  }

  return tabsMap[moduleName] || []
})

const activeTab = ref('')

function onTabChange(tab) {
  activeTab.value = tab
  router.replace({ query: { tab } })
}

// ============================================================
// 捕获
// ============================================================
const showCaptureMenu = ref(false)

function openCapture(type) {
  showCaptureMenu.value = false
  router.push({ name: 'tasks', query: { capture: type } })
}

// ============================================================
// 当前模块信息
// ============================================================
const currentModule = computed(() => {
  const name = route.name || 'home'
  return allModules.find(m => m.name === name) || allModules[0]
})

const pageTitle = computed(() => {
  return route.meta?.title || currentModule.value.label
})

// ============================================================
// 全局快速捕获（Cmd+I/E/N/T → emit 'global:quick_capture'）
// ============================================================
const showQuickCapture = ref(false)
const quickCaptureText = ref('')
const quickCaptureSaving = ref(false)
const quickCaptureType = ref('note') // note/task/event/quick
const quickCaptureTitle = computed(() => {
  const titles = { note: '快速笔记', task: '快速任务', event: '快速日程', quick: '速记' }
  return titles[quickCaptureType.value] || '快速捕获'
})
let unlistenQuickCapture = null

async function setupQuickCaptureListener() {
  try {
    unlistenQuickCapture = await safeListen('global:quick_capture', (event) => {
      quickCaptureType.value = event.payload || 'note'
      quickCaptureText.value = ''
      showQuickCapture.value = true
      // 自动聚焦输入框
      setTimeout(() => {
        const input = document.querySelector('.quick-capture-dialog textarea')
        if (input) input.focus()
      }, 100)
    })
  } catch (e) {
    console.warn('[Casy] 全局快速捕获监听未建立:', e)
  }
}

async function saveQuickCapture() {
  const text = quickCaptureText.value.trim()
  if (!text) return
  quickCaptureSaving.value = true
  
  let result
  const type = quickCaptureType.value
  
  if (type === 'task') {
    // 快速创建任务 - 解析文本中的日期
    const parsed = parseQuickTask(text)
    result = await casyContext.tasks.create({
      taskName: parsed.taskName || text,
      startDate: parsed.startDate,
      dueDate: parsed.dueDate,
      startBucket: parsed.startBucket || 'inbox',
      taskType: 'action',
    })
  } else if (type === 'event') {
    // 快速创建日程
    result = await casyContext.inbox.add('note', text)
    // 可以后续扩展为直接创建日历事件
  } else {
    // 笔记/速记 - 进收件箱
    result = await casyContext.inbox.add('note', text)
  }
  
  quickCaptureSaving.value = false
  if (result.ok) {
    ElMessage.success('已捕获')
    quickCaptureText.value = ''
    showQuickCapture.value = false
  } else {
    ElMessage.error(result.error || '保存失败')
  }
}

// 简单解析快速任务文本
function parseQuickTask(text) {
  const now = new Date()
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate())
  let taskName = text
  let startDate = null
  let dueDate = null
  let startBucket = 'inbox'
  
  // 匹配日期词
  const rel = text.match(/^(今天|明天|后天|下周[一二三四五六日天])s*/)
  if (rel) {
    const dateWord = rel[1]
    taskName = text.slice(dateWord.length).trim()
    
    if (dateWord === '今天') {
      startDate = formatDate(today)
      dueDate = formatDate(today)
      startBucket = 'today'
    } else if (dateWord === '明天') {
      const d = new Date(today)
      d.setDate(d.getDate() + 1)
      startDate = formatDate(d)
      dueDate = formatDate(d)
    } else if (dateWord === '后天') {
      const d = new Date(today)
      d.setDate(d.getDate() + 2)
      startDate = formatDate(d)
      dueDate = formatDate(d)
    } else if (dateWord.startsWith('下周')) {
      const wd = { 一: 1, 二: 2, 三: 3, 四: 4, 五: 5, 六: 6, 日: 0, 天: 0 }[dateWord[2]]
      const delta = (wd - today.getDay() + 7) % 7 + 7
      const d = new Date(today)
      d.setDate(d.getDate() + delta)
      startDate = formatDate(d)
      dueDate = formatDate(d)
    }
  }
  
  return { taskName, startDate, dueDate, startBucket }
}

function formatDate(d) {
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return y + '-' + m + '-' + day
}

// ============================================================
// 生命周期
// ============================================================
onMounted(() => {
  loadTodayStats()
  checkOnboarding()
  setupQuickCaptureListener()

  if (route.query.tab) {
    activeTab.value = route.query.tab
  }
})

onUnmounted(() => {
  if (unlistenQuickCapture) unlistenQuickCapture()
})

watch(() => route.name, () => {
  const tabs = moduleTabs.value
  if (tabs.length > 0 && !tabs.find(t => t.key === activeTab.value)) {
    activeTab.value = tabs[0].key
    router.replace({ query: { tab: activeTab.value } })
  }
})

function onMenuSelect(name) {
  router.push({ name })
}
</script>

<template>
  <div class="app-shell">
    <!-- ═══ 左侧侧栏 ═══ -->
    <aside class="app-sidebar" :class="{ collapsed: sidebarCollapsed }">
      <!-- 品牌 -->
      <div class="sidebar-brand" @click="router.push('/')">
        <span class="brand-mark">C</span>
        <span v-show="!sidebarCollapsed" class="brand-name">Casy</span>
      </div>

      <!-- 导航 -->
      <nav class="sidebar-nav">
        <div v-for="group in navGroups" :key="group.label" class="nav-group">
          <div v-show="!sidebarCollapsed" class="nav-group-label">{{ group.label }}</div>
          <div
            v-for="item in group.items"
            :key="item.name"
            class="nav-item"
            :class="{ active: route.name === item.name }"
            @click="onMenuSelect(item.name)"
            :title="item.label"
          >
            <el-icon class="nav-icon" :size="17">
              <component :is="item.icon" />
            </el-icon>
            <span v-show="!sidebarCollapsed" class="nav-label">{{ item.label }}</span>
          </div>
        </div>
      </nav>

      <!-- 底部：设置 -->
      <div class="sidebar-footer">
        <div
          class="nav-item"
          :class="{ active: route.name === 'settings' }"
          @click="onMenuSelect('settings')"
          title="设置"
        >
          <el-icon class="nav-icon" :size="17"><Setting /></el-icon>
          <span v-show="!sidebarCollapsed" class="nav-label">设置</span>
        </div>
      </div>
    </aside>

    <!-- ═══ 右侧主区 ═══ -->
    <div class="app-main">
      <!-- 顶栏 -->
      <header class="topbar">
        <div class="topbar-left">
          <button class="icon-btn" @click="toggleSidebar" title="折叠/展开侧栏">
            <el-icon :size="16">
              <Expand v-if="sidebarCollapsed" />
              <Fold v-else />
            </el-icon>
          </button>

          <!-- 今日概览统计 -->
          <div class="today-stats">
            <div class="ts-item" @click="router.push({ name: 'calendar' })">
              <span class="ts-dot danger" />
              <span class="ts-value">{{ todayStats.hardSchedule }}</span>
              <span class="ts-label">硬性日程</span>
            </div>
            <div class="ts-item" @click="router.push({ name: 'tasks', query: { tab: 'today' } })">
              <span class="ts-dot warning" />
              <span class="ts-value">{{ todayStats.dueToday }}</span>
              <span class="ts-label">今日到期</span>
            </div>
            <div class="ts-item" @click="router.push({ name: 'tasks', query: { tab: 'waiting' } })">
              <span class="ts-dot gray" />
              <span class="ts-value">{{ todayStats.waitingOverdue }}</span>
              <span class="ts-label">等待超时</span>
            </div>
            <div class="ts-item" @click="router.push({ name: 'cases' })">
              <span class="ts-dot success" />
              <span class="ts-value">{{ todayStats.needReview }}</span>
              <span class="ts-label">需回顾</span>
            </div>
          </div>
        </div>

        <div class="topbar-right">
          <!-- 全局搜索 -->
          <div class="topbar-search">
            <el-icon :size="14"><Search /></el-icon>
            <input placeholder="搜索案件、任务、法条…" />
            <span class="kbd-hint">⌘K</span>
          </div>

          <!-- 捕获按钮 -->
          <div class="capture-wrap">
            <button class="btn-primary capture-btn" @click="showCaptureMenu = !showCaptureMenu">
              <el-icon :size="14"><Plus /></el-icon>
              <span>捕获</span>
            </button>
            <div v-if="showCaptureMenu" class="capture-menu" @mouseleave="showCaptureMenu = false">
              <div class="capture-item" @click="openCapture('task')">+ 任务</div>
              <div class="capture-item" @click="openCapture('event')">+ 日程</div>
              <div class="capture-item" @click="openCapture('note')">+ 笔记</div>
              <div class="capture-item" @click="openCapture('quick')">+ 速记</div>
            </div>
          </div>

          <AIStatusBadge />
        </div>
      </header>

      <!-- 内容区 -->
      <div class="content-area">
        <!-- 页面标题 + 模块 Tab -->
        <div v-if="moduleTabs.length > 0" class="content-tabs">
          <div
            v-for="tab in moduleTabs"
            :key="tab.key"
            :class="['tab-item', { active: activeTab === tab.key }]"
            @click="onTabChange(tab.key)"
          >
            {{ tab.label }}
          </div>
        </div>

        <main class="content-scroll">
          <router-view />
        </main>
      </div>
    </div>
  </div>

  <!-- 全局浮层 -->
  <ReminderToast />
  <ReminderBanner />
  <DecisionReviewNotice />
  <OverdueMorningBrief />
  <OnboardingWizard v-model="showOnboarding" @dismiss="onOnboardingDismiss" />

  <!-- 快速捕获对话框 -->
  <el-dialog v-model="showQuickCapture" :title="quickCaptureTitle" width="480" append-to-body class="quick-capture-dialog">
    <el-input
      v-model="quickCaptureText"
      type="textarea"
      :rows="4"
      :placeholder="quickCaptureType === 'task' ? '输入任务，开头可加 今天/明天/下周X 自动设日期…' : '有什么想法、材料、待办？先记下来，稍后厘清…'"
      @keydown.enter.ctrl.exact.prevent="saveQuickCapture"
    />
    <template #footer>
      <el-button @click="showQuickCapture = false">取消</el-button>
      <el-button
        type="primary"
        :loading="quickCaptureSaving"
        :disabled="!quickCaptureText.trim()"
        @click="saveQuickCapture"
      >
        {{ quickCaptureType === 'task' ? '创建任务' : quickCaptureType === 'event' ? '创建日程' : '捕获到收件箱' }}
      </el-button>
    </template>
  </el-dialog>
</template>

<style scoped>
/* ═══════════════════════════════════════════════════════════
   布局骨架
   ═══════════════════════════════════════════════════════════ */
.app-shell {
  display: flex;
  height: 100vh;
  background: #F6F7F9;
  color: #1F2430;
  font-size: 13px;
}

/* ── 侧栏 ─────────────────────────────────────────────── */
.app-sidebar {
  width: 200px;
  min-width: 200px;
  background: #FFFFFF;
  border-right: 1px solid #E0E3E9;
  display: flex;
  flex-direction: column;
  transition: width 0.2s ease, min-width 0.2s ease;
  overflow: hidden;
}

.app-sidebar.collapsed {
  width: 56px;
  min-width: 56px;
}

.sidebar-brand {
  height: 52px;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 16px;
  border-bottom: 1px solid #EEF0F3;
  cursor: pointer;
  flex-shrink: 0;
}

.brand-mark {
  width: 26px;
  height: 26px;
  border-radius: 7px;
  background: #3E5C9A;
  color: #fff;
  display: grid;
  place-items: center;
  font-weight: 700;
  font-size: 14px;
  flex-shrink: 0;
}

.brand-name {
  font-size: 15px;
  font-weight: 700;
  color: #1F2430;
  letter-spacing: -0.2px;
  white-space: nowrap;
}

.sidebar-nav {
  flex: 1;
  overflow-y: auto;
  overflow-x: hidden;
  padding: 8px;
}

.nav-group {
  margin-bottom: 4px;
}

.nav-group-label {
  font-size: 10px;
  font-weight: 600;
  color: #9BA2AF;
  text-transform: uppercase;
  letter-spacing: 0.8px;
  padding: 10px 10px 4px;
  white-space: nowrap;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  height: 32px;
  padding: 0 10px;
  border-radius: 6px;
  cursor: pointer;
  color: #4B5160;
  transition: all 0.12s ease;
  position: relative;
  white-space: nowrap;
  margin-bottom: 1px;
}

.nav-item:hover {
  background: #F0F2F5;
  color: #1F2430;
}

.nav-item.active {
  background: #EDF1F8;
  color: #3E5C9A;
  font-weight: 500;
}

.nav-item.active::before {
  content: '';
  position: absolute;
  left: -8px;
  top: 6px;
  bottom: 6px;
  width: 3px;
  background: #3E5C9A;
  border-radius: 0 2px 2px 0;
}

.nav-icon {
  flex-shrink: 0;
}

.nav-label {
  font-size: 13px;
}

.sidebar-footer {
  padding: 8px;
  border-top: 1px solid #EEF0F3;
}

/* ── 主区 ─────────────────────────────────────────────── */
.app-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
}

/* ── 顶栏 ─────────────────────────────────────────────── */
.topbar {
  height: 52px;
  background: #FFFFFF;
  border-bottom: 1px solid #E0E3E9;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 16px;
  gap: 16px;
  flex-shrink: 0;
}

.topbar-left {
  display: flex;
  align-items: center;
  gap: 16px;
  min-width: 0;
}

.icon-btn {
  width: 30px;
  height: 30px;
  border-radius: 6px;
  display: grid;
  place-items: center;
  color: #4B5160;
  cursor: pointer;
  border: none;
  background: transparent;
  transition: background 0.12s;
  flex-shrink: 0;
}

.icon-btn:hover {
  background: #F0F2F5;
}

/* 今日概览 */
.today-stats {
  display: flex;
  align-items: center;
  gap: 2px;
}

.ts-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  border-radius: 6px;
  cursor: pointer;
  transition: background 0.12s;
}

.ts-item:hover {
  background: #F0F2F5;
}

.ts-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex-shrink: 0;
}

.ts-dot.danger { background: #B4554F; }
.ts-dot.warning { background: #B0823A; }
.ts-dot.gray { background: #9BA2AF; }
.ts-dot.success { background: #4C8067; }

.ts-value {
  font-size: 14px;
  font-weight: 600;
  color: #1F2430;
}

.ts-label {
  font-size: 11px;
  color: #9BA2AF;
  white-space: nowrap;
}

.topbar-right {
  display: flex;
  align-items: center;
  gap: 12px;
  flex-shrink: 0;
}

.topbar-search {
  display: flex;
  align-items: center;
  gap: 8px;
  background: #F6F7F9;
  border: 1px solid #E0E3E9;
  border-radius: 6px;
  padding: 5px 10px;
  color: #9BA2AF;
  width: 220px;
  transition: all 0.15s;
}

.topbar-search:focus-within {
  border-color: #3E5C9A;
  background: #fff;
  box-shadow: 0 0 0 3px rgba(62, 92, 154, 0.1);
}

.topbar-search input {
  border: none;
  outline: none;
  flex: 1;
  background: transparent;
  font-size: 12.5px;
  color: #1F2430;
  font-family: inherit;
}

.kbd-hint {
  font-size: 10px;
  color: #9BA2AF;
  border: 1px solid #E0E3E9;
  border-radius: 4px;
  padding: 1px 5px;
  background: #fff;
  font-family: 'SF Mono', Menlo, monospace;
  white-space: nowrap;
}

/* 捕获按钮 */
.capture-wrap {
  position: relative;
}

.btn-primary {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 14px;
  border-radius: 6px;
  background: #3E5C9A;
  color: #fff;
  border: none;
  cursor: pointer;
  font-size: 12.5px;
  font-weight: 500;
  font-family: inherit;
  transition: background 0.12s;
}

.btn-primary:hover {
  background: #334D82;
}

.capture-menu {
  position: absolute;
  top: calc(100% + 6px);
  right: 0;
  background: #fff;
  border: 1px solid #E0E3E9;
  border-radius: 8px;
  box-shadow: 0 12px 32px rgba(31, 36, 48, 0.12);
  padding: 4px;
  z-index: 200;
  min-width: 120px;
}

.capture-item {
  padding: 7px 12px;
  border-radius: 6px;
  font-size: 13px;
  color: #4B5160;
  cursor: pointer;
  transition: background 0.12s;
}

.capture-item:hover {
  background: #EDF1F8;
  color: #3E5C9A;
}

/* ── 内容区 ───────────────────────────────────────────── */
.content-area {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.content-tabs {
  display: flex;
  gap: 2px;
  padding: 8px 16px 0;
  background: #F6F7F9;
  flex-shrink: 0;
}

.tab-item {
  padding: 6px 14px;
  border-radius: 6px 6px 0 0;
  font-size: 12.5px;
  color: #4B5160;
  cursor: pointer;
  font-weight: 500;
  transition: all 0.12s;
  border-bottom: 2px solid transparent;
}

.tab-item:hover {
  color: #1F2430;
  background: #fff;
}

.tab-item.active {
  color: #3E5C9A;
  background: #fff;
  border-bottom-color: #3E5C9A;
}

.content-scroll {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
  min-height: 0;
}
</style>
