import { defineConfig } from 'vitest/config'
import vue from '@vitejs/plugin-vue'

export default defineConfig({
  plugins: [vue()],
  resolve: { dedupe: ['@tiptap/core', '@tiptap/pm', 'prosemirror-model', 'prosemirror-state', 'prosemirror-transform', 'prosemirror-view'] },
  test: {
    server: { deps: { inline: [/@excalidraw/, /open-color/, /@tiptap\//, /prosemirror-/] } },
    // 仅项目自有单测；排除仓库内第三方/设计稿目录
    include: ['tests/**/*.test.ts'],
    environment: 'node',
  },
})
