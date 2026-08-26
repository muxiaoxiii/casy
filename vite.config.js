import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { fileURLToPath, URL } from 'node:url'

// https://vite.dev/config/
export default defineConfig({
  plugins: [vue()],
  build: {
    // 审计 P2：主包 837KB → 拆分 element-plus / 图标 / tiptap / 供应商块
    rollupOptions: {
      output: {
        advancedChunks: {
          groups: [
            { name: 'element-plus', test: /node_modules[\\/]element-plus[\\/]/ },
            { name: 'ep-icons', test: /node_modules[\\/]@element-plus[\\/]icons-vue[\\/]/ },
            { name: 'tiptap', test: /node_modules[\\/](@tiptap|prosemirror)/ },
            { name: 'vendor', test: /node_modules[\\/]/ },
          ],
        },
      },
    },
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
