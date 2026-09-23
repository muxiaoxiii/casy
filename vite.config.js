import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { cpSync, mkdirSync } from 'node:fs'
import { fileURLToPath, URL } from 'node:url'

// https://vite.dev/config/
export default defineConfig({
  plugins: [vue(), {name:'offline-canvas-fonts',buildStart() { mkdirSync('public/excalidraw-assets',{recursive:true}); cpSync('node_modules/@excalidraw/excalidraw/dist/prod/fonts','public/excalidraw-assets/fonts',{recursive:true}) }}],
  define: {
    // Interpret locale messages as ASTs so packaged Tauri works with its strict CSP.
    __INTLIFY_JIT_COMPILATION__: true,
  },
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
    // Native builds and release evidence do not change browser modules.
    watch: { ignored: ['**/src-tauri/**', '**/outputs/**', '**/release/**'] },
  },
  resolve: {
    dedupe: ['@tiptap/core', '@tiptap/pm', 'prosemirror-model', 'prosemirror-state', 'prosemirror-transform', 'prosemirror-view'],
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
})
