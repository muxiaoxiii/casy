import { defineConfig } from 'vitest/config'
import vue from '@vitejs/plugin-vue'

export default defineConfig({
  plugins: [vue()],
  test: {
    // 仅项目自有单测；排除仓库内第三方/设计稿目录
    include: ['tests/**/*.test.ts'],
    environment: 'node',
  },
})
