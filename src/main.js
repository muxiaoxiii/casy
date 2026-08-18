import { createApp } from 'vue'
import { createPinia } from 'pinia'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
import router from './router/index.js'
import App from './App.vue'
import './style.css'
import './assets/theme.css'

// ============================================================
// 插件系统初始化
// ============================================================
import { initializePluginSystem } from './core/plugin/initializer'

const app = createApp(App)
app.use(createPinia())
app.use(router)
app.use(ElementPlus)

// 初始化插件系统（异步）
initializePluginSystem()
  .then(() => {
    console.log('Plugin system initialized successfully')
  })
  .catch((error) => {
    console.error('Failed to initialize plugin system:', error)
  })

app.mount('#app')
