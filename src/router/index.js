import { createRouter, createWebHashHistory } from 'vue-router'

const routes = [
  {
    path: '/',
    name: 'home',
    component: () => import('../modules/home/HomeView.vue'),
    meta: { title: '首页' },
  },
  {
    path: '/cases',
    name: 'cases',
    component: () => import('../modules/cases/views/CaseListView.vue'),
    meta: { title: '案件管理' },
  },
  {
    path: '/cases/:id',
    name: 'case-detail',
    component: () => import('../modules/cases/views/CaseDetailView.vue'),
    meta: { title: '案件详情' },
    props: true,
  },
  {
    path: '/cases/kanban',
    name: 'case-kanban',
    component: () => import('../modules/cases/views/KanbanView.vue'),
    meta: { title: '案件看板' },
  },
  {
    path: '/cases/network',
    name: 'case-network',
    component: () => import('../modules/cases/views/CaseNetworkView.vue'),
    meta: { title: '案件关系网络' },
  },
  {
    path: '/calendar',
    name: 'calendar',
    component: () => import('../modules/calendar/views/CalendarView.vue'),
    meta: { title: '日历' },
  },
  {
    path: '/tasks',
    name: 'tasks',
    component: () => import('../modules/tasks/views/TasksView.vue'),
    meta: { title: '任务' },
  },
  {
    path: '/inbox',
    name: 'inbox',
    component: () => import('../modules/inbox/views/InboxView.vue'),
    meta: { title: '收件箱' },
  },
  {
    path: '/docs',
    name: 'docs',
    component: () => import('../modules/docs/views/DocWorkshopView.vue'),
    meta: { title: '文书工坊' },
  },
  {
    path: '/docs/generate',
    name: 'doc-generate',
    component: () => import('../modules/docs/views/DocumentGenView.vue'),
    meta: { title: '文书生成' },
  },
  {
    path: '/write/:caseId?',
    name: 'write',
    component: () => import('../modules/docs/views/WritingView.vue'),
    meta: { title: '写作' },
    props: true,
  },
  {
    path: '/files/:caseId',
    name: 'files',
    component: () => import('../modules/files/views/CaseFilesView.vue'),
    meta: { title: '案件文件' },
    props: true,
  },
  {
    path: '/sync',
    name: 'sync',
    component: () => import('../modules/sync/views/SyncStatusView.vue'),
    meta: { title: '同步状态' },
  },
  {
    path: '/dashboard',
    name: 'dashboard',
    component: () => import('../modules/dashboard/DashboardView.vue'),
    meta: { title: '数据看板' },
  },
  {
    path: '/clients',
    name: 'clients',
    component: () => import('../modules/clients/views/ClientView.vue'),
    meta: { title: '客户管理' },
  },
  {
    path: '/settings',
    name: 'settings',
    component: () => import('../modules/settings/SettingsView.vue'),
    meta: { title: '设置' },
  },
  {
    path: '/knowledge',
    name: 'knowledge',
    component: () => import('../modules/knowledge/views/KnowledgeView.vue'),
    meta: { title: '知识库' },
  },
  {
    path: '/knowledge/style-guide',
    name: 'knowledge-style-guide',
    component: () => import('../modules/knowledge/views/KnowledgeStyleGuide.vue'),
    meta: { title: '文书风格指南' },
  },
  {
    path: '/knowledge/graph',
    name: 'knowledge-graph',
    component: () => import('../modules/knowledge/views/KnowledgeGraphView.vue'),
    meta: { title: '知识图谱' },
  },
  {
    path: '/ai',
    name: 'ai',
    component: () => import('../modules/ai/views/AICompanionView.vue'),
    meta: { title: 'AI 智伴' },
  },
  {
    path: '/reminder',
    name: 'reminder',
    component: () => import('../modules/reminder/views/ReminderView.vue'),
    meta: { title: '提醒预警' },
  },
]

const router = createRouter({
  history: createWebHashHistory(),
  routes,
})

export default router
