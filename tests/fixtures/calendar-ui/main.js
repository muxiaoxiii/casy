import { createApp, h } from 'vue'
import { createRouter, createWebHashHistory, RouterView } from 'vue-router'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
import '../../../src/style.css'
import '../../../src/assets/theme.css'
import '../../../src/assets/workspace.css'
import '../../../src/assets/docket.css'
import CalendarView from '../../../src/modules/calendar/views/CalendarView.vue'
import { casyContext } from '../../../src/core/plugin/context'
import i18n from '../../../src/locales/index.ts'
import { fixtureServices } from './data.js'
const params = new URLSearchParams(location.search)
document.documentElement.dataset.theme = params.get('theme') || 'docket-light'
for (const [name, service] of Object.entries(fixtureServices(params.get('mode') || 'mixed'))) casyContext.provide(name, service)
const router = createRouter({ history: createWebHashHistory(), routes: [
  { path: '/calendar', component: CalendarView },
  { path: '/cases/:id', name: 'case-detail', component: { render: () => h('p', '已打开案件详情') } },
  { path: '/tasks', name: 'tasks', component: { render: () => h('p', '已打开任务详情') } },
] })
if (!location.hash || location.hash === '#/') await router.push('/calendar?date=2026-09-25&view=timeline')
const width = Math.max(300, Number(params.get('width')) || 1100)
createApp({ render: () => h('main', { style: { width: `${width}px`, maxWidth: '100%', margin: '0 auto' }, class: 'content-scroll' }, [h(RouterView)]) }).use(router).use(ElementPlus).use(i18n).mount('#app')
