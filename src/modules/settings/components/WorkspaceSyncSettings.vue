<script setup lang="ts">
import { formatTimestamp } from '../../../shared/utils/date'
import { onMounted, onUnmounted, ref } from 'vue'
import { useSettingsStore } from '../../../stores/settings'
import { tauriCallSafe } from '../../../core/tauriBridge'
import type { CommandMap } from '../../../types/commandMap'
const settings = useSettingsStore()
const status = ref<CommandMap['get_workspace_sync_status']['result'] | null>(null)
const flags = [
  ['register', '自动登记案卷目录中的新增文件'],
  ['ocr', '自动提取正文 / OCR（含原件更新）'],
  ['knowledge', '自动保存 Markdown 知识快照'],
  ['embeddings', '自动更新向量索引（使用已配置接口）'],
  ['name', '自动在文件名前添加案号'],
  ['content_name', '根据正文标题自动修正文件名'],
] as const
let timer: ReturnType<typeof setInterval> | undefined
async function refresh() { const result = await tauriCallSafe('get_workspace_sync_status', {}); if (result.ok) status.value = result.data! }
onMounted(() => { void refresh(); timer = setInterval(refresh, 10000) })
onUnmounted(() => clearInterval(timer))
</script>
<template>
  <fieldset class="workspace-settings"><legend>案卷与知识库同步</legend>
    <label v-for="[key, label] in flags" :key="key"><span>{{ label }}</span><el-switch v-model="settings.workspace_sync[key]" :aria-label="label" /></label>
    <div v-if="status?.checkedAt" class="sync-status ui-text--caption" role="status">最近检查 {{ formatTimestamp(status.checkedAt) }}<span v-if="status.missing"> · {{ status.missing }} 份原件缺失</span></div>
    <el-alert v-for="error in status?.errors || []" :key="error" :title="error" type="warning" :closable="false" />
  </fieldset>
</template>
<style scoped>
.workspace-settings{border:0;border-top:1px solid var(--c-border);margin:16px 0;padding:16px 0;max-width:680px}
.workspace-settings legend{font-weight:600;padding-right:12px}.workspace-settings label{display:flex;align-items:center;justify-content:space-between;gap:20px;padding:8px 0;font-size:13px}
.sync-status{margin-top:10px}
</style>
