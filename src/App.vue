<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { tauriCallSafe } from './core/tauriBridge'
import ReminderToast from './shared/components/ReminderToast.vue'
import ReminderBanner from './shared/components/ReminderBanner.vue'
import OverdueMorningBrief from './shared/components/OverdueMorningBrief.vue'
import AIStatusBadge from './shared/components/AIStatusBadge.vue'
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
  ArrowRight,
  Clock,
  Warning,
  Check,
  Folder,
  Timer,
  Bell,
  Cpu,
  ArrowDown,
} from '@element-plus/icons-vue'

const router = useRouter()
const route = useRoute()

// ============================================================
// 今日面板数据
// ============================================================
const todayStats = ref({
  hardSchedule: 0,    // 硬性日程（开庭/口审）
  dueToday: 0,        // 今日到期任务
  waitingOverdue: 0,  // 等待超3天
  needReview: 0,      // 需回顾案件
})

const loadingStats = ref(false)
const todayPanelCollapsed = ref(false)

async function loadTodayStats() {
  loadingStats.value = true
  try {
    const result = await tauriCallSafe('get_today_stats', {})
    if (result.ok && result.data) {
      todayStats.value = result.data
    }
  } catch (e) {
    console.error('Failed to load today stats:', e)
  }
  loadingStats.value = false
}

function toggleTodayPanel() {
  todayPanelCollapsed.value = !todayPanelCollapsed.value
}

// ============================================================
// 第二层：核心模块导航
// ============================================================
const coreModules = [
  { name: 'home', label: '首页', icon: DataBoard },
  { name: 'cases', label: '案件', icon: Briefcase },
  { name: 'tasks', label: '任务', icon: Finished },
  { name: 'calendar', label: '日历', icon: Calendar },
  { name: 'inbox', label: '收件箱', icon: Box },
  { name: 'knowledge', label: '知识库', icon: Collection },
  { name: 'docs', label: '文书', icon: Document },
  { name: 'settings', label: '设置', icon: Setting },
]

// ============================================================
// 第三层：模块内 Tab（根据当前模块动态显示）
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
  // 可以通过路由 query 参数传递 tab
  router.replace({ query: { tab } })
}

// ============================================================
// 捕获按钮
// ============================================================
const showCaptureMenu = ref(false)

function openCapture(type) {
  showCaptureMenu.value = false
  // 跳转到任务页面并打开捕获对话框
  router.push({ name: 'tasks', query: { capture: type } })
}

// ============================================================
// 当前模块信息
// ============================================================
const currentModule = computed(() => {
  const name = route.name || 'home'
  return coreModules.find(m => m.name === name) || coreModules[0]
})

const pageTitle = computed(() => {
  return route.meta?.title || currentModule.value.label
})

const hasUrgentItems = computed(() => {
  return todayStats.value.hardSchedule > 0 || 
         todayStats.value.dueToday > 0 || 
         todayStats.value.waitingOverdue > 0 || 
         todayStats.value.needReview > 0
})

// ============================================================
// 生命周期
// ============================================================
onMounted(() => {
  loadTodayStats()
  
  // 从路由 query 恢复 tab
  if (route.query.tab) {
    activeTab.value = route.query.tab
  }
})

watch(() => route.name, () => {
  // 切换模块时重置 tab
  const tabs = moduleTabs.value
  if (tabs.length > 0 && !tabs.find(t => t.key === activeTab.value)) {
    activeTab.value = tabs[0].key
    router.replace({ query: { tab: activeTab.value } })
  }
})

// ============================================================
// 导航
// ============================================================
function onMenuSelect(name) {
  router.push({ name })
}
</script>

<template>
  <el-container class="app-container">
    <!-- 第二层：左侧侧栏 -->
    <aside class="app-sidebar">
      <!-- 品牌标识 -->
      <div class="sidebar-brand" @click="router.push('/')" title="Casy">
        <span class="brand-letter">C</span>
        <span class="brand-name">Casy</span>
      </div>
      
      <!-- 核心模块导航 -->
      <nav class="sidebar-nav">
        <div
          v-for="item in coreModules"
          :key="item.name"
          class="nav-item"
          :class="{ active: route.name === item.name }"
          @click="onMenuSelect(item.name)"
          :title="item.label"
        >
          <el-icon class="nav-icon" :size="18">
            <component :is="item.icon" />
          </el-icon>
          <span class="nav-label">{{ item.label }}</span>
        </div>
      </nav>
      
      <!-- 浮动捕获按钮 -->
      <div class="capture-button-container">
        <el-popover
          v-model:visible="showCaptureMenu"
          placement="right"
          :width="160"
          trigger="click"
        >
          <template #reference>
            <div class="capture-button" title="快速捕获 (⌘T)">
              <el-icon :size="20"><Plus /></el-icon>
            </div>
          </template>
          
          <div class="capture-menu">
            <div class="capture-item" @click="openCapture('task')">
              <el-icon><Finished /></el-icon>
              <span>+ 任务</span>
            </div>
            <div class="capture-item" @click="openCapture('event')">
              <el-icon><Calendar /></el-icon>
              <span>+ 日程</span>
            </div>
            <div class="capture-item" @click="openCapture('note')">
              <el-icon><Collection /></el-icon>
              <span>+ 笔记</span>
            </div>
            <div class="capture-item" @click="openCapture('quick')">
              <el-icon><Timer /></el-icon>
              <span>+ 速记</span>
            </div>
          </div>
        </el-popover>
      </div>
    </aside>

    <!-- 主内容区 -->
    <el-container class="main-container">
      <!-- 第一层：今日面板（顶栏常驻，可折叠） -->
      <header class="today-panel">
        <div class="today-header" @click="toggleTodayPanel">
          <div class="today-title">
            <span class="greeting-text">今日概览</span>
            <el-icon :size="14" :class="['collapse-icon', { collapsed: todayPanelCollapsed }]">
              <ArrowDown />
            </el-icon>
          </div>
          <el-button text size="small" @click.stop="loadTodayStats" :loading="loadingStats">
            刷新
          </el-button>
        </div>
        
        <transition name="slide">
          <div v-show="!todayPanelCollapsed" class="today-content">
            <div class="today-stats">
              <div class="stat-item" @click="router.push({ name: 'calendar' })">
                <div class="stat-icon stat-icon-danger">
                  <el-icon :size="14"><Bell /></el-icon>
                </div>
                <div class="stat-info">
                  <span class="stat-value">{{ todayStats.hardSchedule }}</span>
                  <span class="stat-label">硬性日程</span>
                </div>
              </div>
              
              <div class="stat-item" @click="router.push({ name: 'tasks', query: { tab: 'today' } })">
                <div class="stat-icon stat-icon-warning">
                  <el-icon :size="14"><Warning /></el-icon>
                </div>
                <div class="stat-info">
                  <span class="stat-value">{{ todayStats.dueToday }}</span>
                  <span class="stat-label">今日到期</span>
                </div>
              </div>
              
              <div class="stat-item" @click="router.push({ name: 'tasks', query: { tab: 'waiting' } })">
                <div class="stat-icon stat-icon-muted">
                  <el-icon :size="14"><Clock /></el-icon>
                </div>
                <div class="stat-info">
                  <span class="stat-value">{{ todayStats.waitingOverdue }}</span>
                  <span class="stat-label">等待超时</span>
                </div>
              </div>
              
              <div class="stat-item" @click="router.push({ name: 'cases' })">
                <div class="stat-icon stat-icon-success">
                  <el-icon :size="14"><Folder /></el-icon>
                </div>
                <div class="stat-info">
                  <span class="stat-value">{{ todayStats.needReview }}</span>
                  <span class="stat-label">需回顾</span>
                </div>
              </div>
            </div>
            
            <div v-if="todayStats.hardSchedule === 0 && todayStats.dueToday === 0 && todayStats.waitingOverdue === 0 && todayStats.needReview === 0" class="today-empty">
              今天没有紧急事项
            </div>
          </div>
        </transition>
      </header>

      <!-- 页面标题栏 -->
      <div class="page-header">
        <div class="header-left">
          <el-icon :size="18">
            <component :is="currentModule.icon" />
          </el-icon>
          <h1>{{ pageTitle }}</h1>
        </div>

        <div class="header-right">
          <!-- 第三层：模块内 Tab -->
          <div v-if="moduleTabs.length > 0" class="module-tabs">
            <div
              v-for="tab in moduleTabs"
              :key="tab.key"
              :class="['tab-item', { active: activeTab === tab.key }]"
              @click="onTabChange(tab.key)"
            >
              {{ tab.label }}
            </div>
          </div>

          <!-- AI 状态徽标 -->
          <AIStatusBadge />
        </div>
      </div>

      <!-- 主内容 -->
      <el-main class="app-main">
        <router-view />
      </el-main>
    </el-container>
  </el-container>

  <!-- 全局提醒触发浮层 -->
  <ReminderToast />
  <!-- R2/R3/R4 横幅 -->
  <ReminderBanner />
  <!-- 每日逾期早报 -->
  <OverdueMorningBrief />
</template>

<style scoped>
.app-container {
  height: 100vh;
  background: #FAFAFA;
}

/* ── 第二层：侧边栏 ─────────────────────────────────────── */
.app-sidebar {
  width: 48px;
  min-width: 48px;
  background: #FFFFFF;
  border-right: 1px solid #E5E7EB;
  display: flex;
  flex-direction: column;
  position: relative;
  z-index: 100;
  transition: width 0.15s ease;
}

.app-sidebar:hover {
  width: 200px;
}

/* 品牌标识 */
.sidebar-brand {
  height: 48px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  cursor: pointer;
  border-bottom: 1px solid #F3F4F6;
  flex-shrink: 0;
  padding: 0 12px;
}

.brand-letter {
  font-size: 18px;
  font-weight: 700;
  color: #2563EB;
  flex-shrink: 0;
  width: 24px;
  text-align: center;
}

.brand-name {
  font-size: 14px;
  font-weight: 600;
  color: #111827;
  opacity: 0;
  white-space: nowrap;
  transition: opacity 0.15s ease;
}

.app-sidebar:hover .brand-name {
  opacity: 1;
}

/* 导航列表 */
.sidebar-nav {
  flex: 1;
  overflow-y: auto;
  overflow-x: hidden;
  padding: 8px 0;
}

.sidebar-nav::-webkit-scrollbar {
  width: 0;
}

.nav-item {
  display: flex;
  align-items: center;
  height: 36px;
  margin: 2px 8px;
  padding: 0 8px;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.12s ease;
  color: #6B7280;
  position: relative;
}

.nav-item:hover {
  background: #F9FAFB;
  color: #374151;
}

.nav-item.active {
  background: #EFF6FF;
  color: #2563EB;
}

.nav-item.active::before {
  content: '';
  position: absolute;
  left: -8px;
  top: 6px;
  bottom: 6px;
  width: 3px;
  background: #2563EB;
  border-radius: 0 2px 2px 0;
}

.nav-icon {
  flex-shrink: 0;
  width: 24px;
  text-align: center;
}

.nav-label {
  margin-left: 8px;
  font-size: 13px;
  font-weight: 500;
  opacity: 0;
  white-space: nowrap;
  transition: opacity 0.15s ease;
}

.app-sidebar:hover .nav-label {
  opacity: 1;
}

/* 捕获按钮 */
.capture-button-container {
  padding: 12px 8px;
  border-top: 1px solid #F3F4F6;
}

.capture-button {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  background: #2563EB;
  color: #FFFFFF;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: all 0.15s ease;
  margin: 0 auto;
}

.capture-button:hover {
  background: #1D4ED8;
  transform: scale(1.05);
}

.capture-menu {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.capture-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-radius: 6px;
  cursor: pointer;
  transition: background 0.12s ease;
  font-size: 13px;
  color: #374151;
}

.capture-item:hover {
  background: #F3F4F6;
}

/* ── 第一层：今日面板 ──────────────────────────────────── */
.today-panel {
  background: #F9FAFB;
  border-bottom: 1px solid #E5E7EB;
  padding: 0 20px;
}

.today-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 0;
  cursor: pointer;
  user-select: none;
}

.today-title {
  display: flex;
  align-items: center;
  gap: 6px;
}

.greeting-text {
  font-size: 13px;
  font-weight: 600;
  color: #374151;
}

.collapse-icon {
  transition: transform 0.15s ease;
  color: #9CA3AF;
}

.collapse-icon.collapsed {
  transform: rotate(-90deg);
}

.today-content {
  padding-bottom: 12px;
}

.today-stats {
  display: flex;
  gap: 16px;
}

.stat-item {
  display: flex;
  align-items: center;
  gap: 10px;
  cursor: pointer;
  padding: 8px 12px;
  border-radius: 8px;
  background: #FFFFFF;
  border: 1px solid #E5E7EB;
  transition: all 0.12s ease;
  flex: 1;
}

.stat-item:hover {
  border-color: #D1D5DB;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
}

.stat-icon {
  width: 28px;
  height: 28px;
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.stat-icon-danger {
  background: #FEF2F2;
  color: #EF4444;
}

.stat-icon-warning {
  background: #FFFBEB;
  color: #F59E0B;
}

.stat-icon-muted {
  background: #F3F4F6;
  color: #6B7280;
}

.stat-icon-success {
  background: #F0FDF4;
  color: #22C55E;
}

.stat-info {
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.stat-value {
  font-size: 16px;
  font-weight: 600;
  color: #111827;
  line-height: 1.2;
}

.stat-label {
  font-size: 11px;
  color: #9CA3AF;
  line-height: 1.2;
}

.today-empty {
  text-align: center;
  padding: 12px;
  font-size: 13px;
  color: #9CA3AF;
}

/* 滑动过渡 */
.slide-enter-active,
.slide-leave-active {
  transition: all 0.15s ease;
  overflow: hidden;
}

.slide-enter-from,
.slide-leave-to {
  opacity: 0;
  max-height: 0;
  padding-bottom: 0;
}

.slide-enter-to,
.slide-leave-from {
  opacity: 1;
  max-height: 100px;
}

/* ── 页面标题栏 ────────────────────────────────────────── */
.page-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 20px;
  background: #FFFFFF;
  border-bottom: 1px solid #E5E7EB;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 8px;
  color: #6B7280;
}

.header-left h1 {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
  color: #111827;
}

.header-right {
  display: flex;
  align-items: center;
  gap: 12px;
}

/* 第三层：模块内 Tab */
.module-tabs {
  display: flex;
  gap: 2px;
  background: #F3F4F6;
  padding: 2px;
  border-radius: 8px;
}

.tab-item {
  padding: 5px 12px;
  border-radius: 6px;
  font-size: 12px;
  color: #6B7280;
  cursor: pointer;
  transition: all 0.12s ease;
  font-weight: 500;
}

.tab-item:hover {
  color: #374151;
}

.tab-item.active {
  background: #FFFFFF;
  color: #111827;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
}

/* ── 主内容 ────────────────────────────────────────────── */
.app-main {
  padding: 0;
  overflow: hidden;
}
</style>
