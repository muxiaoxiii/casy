import { createApp } from 'vue'
import { createPinia } from 'pinia'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
import router from './router/index.js'
import App from './App.vue'
import './style.css'
import './assets/theme.css'
import i18n from './locales/index.ts'

// ============================================================
// R-6 前端崩溃基线：window.onerror / unhandledrejection → 本地 JSONL
// （经 append_fe_crash 命令写入与 Rust panic 同一目录；D-13 严格本地不上报）
// ============================================================
import { invoke } from '@tauri-apps/api/core'

function reportFeCrash(message, stack, url) {
  try {
    if (!window.__TAURI_INTERNALS__) return // 浏览器预览模式无后端可写
    invoke('append_fe_crash', {
      message: String(message ?? ''),
      stack: stack ?? null,
      url: url ?? null,
    }).catch(() => {})
  } catch {
    /* 崩溃上报自身绝不二次抛错 */
  }
}

window.addEventListener('error', (e) => {
  reportFeCrash(e.message, e.error?.stack ?? null, e.filename ?? null)
})
window.addEventListener('unhandledrejection', (e) => {
  const reason = e.reason
  reportFeCrash(reason?.message ?? String(reason), reason?.stack ?? null, null)
})

// ============================================================
// 插件系统初始化
// ============================================================
import { initializePluginSystem } from './core/plugin/initializer'

const app = createApp(App)
app.use(createPinia())
app.use(router)
app.use(ElementPlus)
app.use(i18n)

// 初始化插件系统（异步）
initializePluginSystem()
  .then(() => {
    console.log('Plugin system initialized successfully')
  })
  .catch((error) => {
    console.error('Failed to initialize plugin system:', error)
  })

app.mount('#app')
