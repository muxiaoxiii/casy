import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { fileURLToPath, URL } from 'node:url'

// https://vite.dev/config/
export default defineConfig({
  plugins: [vue()],
  build: {
    // 交由 Rolldown 按动态 import 边界拆包。强制把 Element Plus 拆成多个共享块
    // 会破坏部分循环依赖，曾导致打包应用启动时 Qe 未定义、整个界面空白。
    chunkSizeWarningLimit: 900,
  },
  css: {
    devSourcemap: true, // 样式问题可定位到源码（自 rescue/978f3b2 吸收）
  },
  server: {
    port: 1420,
    strictPort: true,
  },
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
})
