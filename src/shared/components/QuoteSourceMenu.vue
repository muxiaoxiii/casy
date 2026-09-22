<script setup lang="ts">
import type { Editor } from '@tiptap/core'
import { ChatLineSquare } from '../icons'
import { ElMessage } from 'element-plus'
import { useSettingsStore } from '../../stores/settings'
import { quoteColors, setQuoteDepth } from '../markdown/quoteSources'
defineProps<{ editor: Editor }>()
const settings = useSettingsStore()
function apply(editor: Editor, value: number) {
  if (!setQuoteDepth(editor, value)) ElMessage.info('请分别选择同一来源的引用段落')
}
</script>
<template>
  <el-dropdown trigger="click" @command="(value: number) => apply(editor, Number(value))">
    <button type="button" class="quote-source-button" title="引用来源" aria-label="引用来源"><ChatLineSquare /></button>
    <template #dropdown><el-dropdown-menu>
      <el-dropdown-item v-for="(color, index) in quoteColors" :key="color" :command="index + 1"><span class="quote-swatch" :style="{ background: color }" />{{ settings.quote_sources[index] || `来源 ${index + 1}` }}</el-dropdown-item>
      <el-dropdown-item divided :command="0">取消引用</el-dropdown-item>
    </el-dropdown-menu></template>
  </el-dropdown>
</template>
<style scoped>
.quote-source-button{display:grid;place-items:center;width:30px;height:30px;border:0;border-radius:4px;background:transparent;color:var(--c-text);cursor:pointer}
.quote-source-button:hover{background:var(--c-bg-hover)}
.quote-source-button svg{width:17px;height:17px}.quote-swatch{display:inline-block;width:10px;height:10px;margin-right:8px;border-radius:2px}
</style>
