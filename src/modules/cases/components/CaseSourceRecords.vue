<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { tauriCallSafe } from '../../../core/tauriBridge'
import type { SourceRecord } from '../../../types/feishuSnapshot'
import { Download } from '../../../shared/icons'
import { isTauriRuntime } from '../../../core/mockData'
const props = defineProps<{caseId: string}>()
const records = ref<SourceRecord[]>([])
const query = ref('')
const error = ref('')
const loading = ref(false)
function attachments(record: SourceRecord): Array<{name:string; file_token:string}> {
  return record.schema.filter(f=>f.type===17).flatMap(f=>Array.isArray(record.raw[f.field_name]) ? record.raw[f.field_name] as Array<{name:string; file_token:string}> : [])
}
async function download(record: SourceRecord, token: string) {
  const result = await tauriCallSafe('get_imported_asset',{source:record.source,fileToken:token})
  if (!result.ok || !result.data) { error.value = result.error || '附件未下载'; return }
  const bytes = Uint8Array.from(atob(result.data.contentBase64),c=>c.charCodeAt(0))
  if (isTauriRuntime()) {
    const { save } = await import('@tauri-apps/plugin-dialog')
    const path = await save({defaultPath:result.data.name})
    if (path) {
      const saved = await tauriCallSafe('export_imported_asset',{source:record.source,fileToken:token,outputPath:path})
      if (!saved.ok) error.value = saved.error || '导出失败'
    }
  } else {
    const url = URL.createObjectURL(new Blob([bytes],{type:result.data.mimeType}))
    const a = document.createElement('a'); a.href=url; a.download=result.data.name; a.click()
    setTimeout(()=>URL.revokeObjectURL(url),1000)
  }
}
watch(() => props.caseId, async (caseId, _previous, onCleanup) => {
  let active = true
  onCleanup(() => { active = false })
  records.value = []
  error.value = ''
  loading.value = true
  const result = await tauriCallSafe('get_case_source_records',{caseId})
  if (!active) return
  loading.value = false
  if (result.ok) records.value = result.data || []
  else error.value = result.error || '读取失败'
}, {immediate:true})
const filtered = computed(() => records.value.filter(r => !query.value || JSON.stringify(r.fields).includes(query.value) || r.tableName.includes(query.value)))
</script>
<template>
  <section class="source-records">
    <h3>导入原始记录</h3>
    <el-input v-model="query" clearable placeholder="搜索原始字段或内容" />
    <el-alert v-if="error" :title="error" type="error" :closable="false" />
    <p v-else-if="loading" role="status">正在读取原始记录…</p>
    <el-empty v-else-if="!filtered.length" description="暂无原始记录" :image-size="48" />
    <el-collapse>
      <el-collapse-item v-for="record in filtered" :key="record.tableName + record.recordId" :title="record.tableName + ' · ' + String(Object.values(record.fields).find(Boolean) || record.recordId)">
        <dl><template v-for="(value,label) in record.fields" :key="label"><dt>{{ label }}</dt><dd>{{ value || '未填写' }}</dd></template></dl>
        <details><summary>原始数据与字段定义</summary><pre>{{ JSON.stringify({ fields: record.raw, schema: record.schema },null,2) }}</pre></details>
        <el-button v-for="file in attachments(record)" :key="file.file_token" :icon="Download" @click="download(record,file.file_token)">{{ file.name }}</el-button>
      </el-collapse-item>
    </el-collapse>
  </section>
</template>
<style scoped>
.source-records { margin-top: 24px; padding-top: 20px; border-top: 1px solid var(--c-border); }
h3 { font-size: 16px; } dl { display: grid; grid-template-columns: minmax(100px,180px) minmax(0,1fr); gap: 10px 20px; }
dt { color: var(--c-text-secondary); } dd { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; }
pre { white-space: pre-wrap; overflow-wrap: anywhere; max-height: 400px; overflow: auto; }
:deep(.el-collapse-item__header) { height: auto; min-height: 48px; text-align: left; }
@media(max-width: 520px) { dl { grid-template-columns: minmax(0,1fr); } dd { margin-bottom: 12px; } }
</style>
