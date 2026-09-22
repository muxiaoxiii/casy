import { defineConfig } from 'vitest/config';
import vue from '@vitejs/plugin-vue';
import { fileURLToPath } from 'node:url';

export default defineConfig({
  root: fileURLToPath(new URL('../../../', import.meta.url)),
  plugins: [vue()],
  test: {
    env: { CASY_EDITOR_STUDY_DIR: fileURLToPath(new URL('./', import.meta.url)) },
    environment: 'jsdom',
    include: ['docs/audits/markdown-editor-reference-study-2026-09-17/probe.test.ts'],
  },
});
