<script setup>
import { computed } from 'vue'
import { useRouter, useRoute } from 'vue-router'

const router = useRouter()
const route = useRoute()

const menuItems = [
  { name: 'home', label: '首页', icon: '📊' },
  { name: 'cases', label: '案件', icon: '📋' },
  { name: 'case-kanban', label: '看板', icon: '📌' },
  { name: 'case-network', label: '关系网络', icon: '🕸️' },
  { name: 'calendar', label: '日历', icon: '📅' },
  { name: 'tasks', label: '任务', icon: '✅' },
  { name: 'inbox', label: '收件箱', icon: '📥' },
  { name: 'knowledge', label: '知识库', icon: '📚' },
  { name: 'docs', label: '文书工坊', icon: '📝' },
  { name: 'write', label: '写作', icon: '✍️' },
  { name: 'sync', label: '同步', icon: '🔄' },
  { name: 'settings', label: '设置', icon: '⚙️' },
]

const activeMenu = computed(() => route.name || 'home')

function onMenuSelect(index) {
  router.push({ name: index })
}
</script>

<template>
  <el-container class="app-container">
    <aside class="app-sidebar" @mouseleave="() => {}">
      <div class="sidebar-brand" @click="router.push('/')" title="Casy">
        <span class="brand-letter">C</span>
      </div>
      <nav class="sidebar-nav">
        <div
          v-for="item in menuItems"
          :key="item.name"
          class="nav-item"
          :class="{ active: activeMenu === item.name }"
          @click="onMenuSelect(item.name)"
          :title="item.label"
        >
          <span class="nav-icon">{{ item.icon }}</span>
          <span class="nav-label">{{ item.label }}</span>
        </div>
      </nav>
    </aside>
    <el-container class="main-container">
      <el-header class="app-header">
        <h2>{{ route.meta?.title || 'Casy' }}</h2>
      </el-header>
      <el-main class="app-main">
        <router-view />
      </el-main>
    </el-container>
  </el-container>
</template>

<style scoped>
.app-container {
  height: 100vh;
}

/* ── 窄侧边栏 ─────────────────────────────────────────── */
.app-sidebar {
  width: var(--sidebar-width, 48px);
  min-width: var(--sidebar-width, 48px);
  background: var(--c-bg-sidebar, #f8f9fa);
  border-right: 1px solid var(--c-border-light, #ebeef5);
  display: flex;
  flex-direction: column;
  overflow: visible;
  position: relative;
  z-index: 100;
  transition: width 0.25s ease;
}

/* 悬浮展开 */
.app-sidebar:hover {
  width: var(--sidebar-width-expanded, 180px);
}

/* ── 品牌标识 ─────────────────────────────────────────── */
.sidebar-brand {
  height: 48px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  border-bottom: 1px solid var(--c-border-lighter, #f2f6fc);
  flex-shrink: 0;
}

.brand-letter {
  font-size: 20px;
  font-weight: 700;
  color: var(--c-primary, #409eff);
}

/* ── 导航列表 ─────────────────────────────────────────── */
.sidebar-nav {
  flex: 1;
  overflow-y: auto;
  overflow-x: hidden;
  padding: 4px 0;
}

.sidebar-nav::-webkit-scrollbar {
  width: 0;
}

.nav-item {
  display: flex;
  align-items: center;
  height: 40px;
  padding: 0 12px;
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
  white-space: nowrap;
  overflow: hidden;
  color: var(--c-text-regular, #606266);
  position: relative;
}

.nav-item:hover {
  background: var(--c-border-lighter, #f2f6fc);
  color: var(--c-text, #303133);
}

.nav-item.active {
  color: var(--c-primary, #409eff);
  background: var(--c-primary-light, #ecf5ff);
}

.nav-item.active::before {
  content: '';
  position: absolute;
  left: 0;
  top: 6px;
  bottom: 6px;
  width: 3px;
  border-radius: 0 3px 3px 0;
  background: var(--c-primary, #409eff);
}

.nav-icon {
  flex-shrink: 0;
  width: 24px;
  text-align: center;
  font-size: 16px;
  line-height: 1;
}

.nav-label {
  margin-left: 8px;
  font-size: var(--font-sm, 13px);
  opacity: 0;
  transition: opacity 0.2s ease;
  overflow: hidden;
}

.app-sidebar:hover .nav-label {
  opacity: 1;
}

/* ── 主内容区 ─────────────────────────────────────────── */
.main-container {
  min-width: 0;
}

.app-header {
  border-bottom: 1px solid var(--c-border-light, #ebeef5);
  display: flex;
  align-items: center;
  padding: 0 var(--space-md, 16px);
  background: var(--c-bg, #fff);
  height: 48px;
}

.app-header h2 {
  margin: 0;
  font-size: var(--font-lg, 18px);
  font-weight: 500;
  color: var(--c-text, #303133);
}

.app-main {
  padding: var(--space-md, 16px);
  overflow-y: auto;
  background: var(--c-bg-page, #f5f7fa);
}
</style>
