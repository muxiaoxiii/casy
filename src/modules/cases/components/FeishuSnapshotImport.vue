<script setup lang="ts">
import { computed, ref } from 'vue'
import { Download, Upload, Check } from '../../../shared/icons'
import { tauriCallSafe } from '../../../core/tauriBridge'
import { isTauriRuntime } from '../../../core/mockData'
import type { FeishuSnapshot, SnapshotReport } from '../../../types/feishuSnapshot'
const emit = defineEmits(['imported', 'busy'])
const url = ref('')
const snapshot = ref<FeishuSnapshot | null>(null)
const primaryId = ref('')
const selected = ref<string[]>([])
const scope = ref('all')
const busy = ref(false)
const error = ref('')
const report = ref<SnapshotReport | null>(null)
const input = ref<HTMLInputElement | null>(null)
const primary = computed(() => snapshot.value?.tables.find(t => t.table_id === primaryId.value))
const rows = computed(() => primary.value?.records || [])
function accept(value: unknown) {
  const data = value as FeishuSnapshot
  if (!data?.appToken || !Array.isArray(data.tables) || data.tables.some(t => !Array.isArray(t.fields) || !Array.isArray(t.records))) throw new Error('文件不是完整的飞书多维表格快照')
  snapshot.value = data
  primaryId.value = data.tables.find(t => t.name === '案件主表')?.table_id || data.tables[0]?.table_id || ''
  selected.value = []
  report.value = null
}
async function loadFile(event: Event) {
  const file = (event.target as HTMLInputElement).files?.[0]
  if (!file) return
  error.value = ''
  try { accept(JSON.parse(await file.text())) } catch (e) { error.value = String(e) }
  ;(event.target as HTMLInputElement).value = ''
}
async function download() {
  busy.value = true; emit('busy',true); error.value = ''
  try {
    const result = await tauriCallSafe('feishu_download_snapshot',{urlOrToken:url.value})
    if (!result.ok) throw new Error(result.error)
    accept(result.data)
  } catch(e) { error.value = String(e) }
  finally { busy.value = false; emit('busy',false) }
}
async function exportSnapshot() {
  if (isTauriRuntime() && snapshot.value) {
    const { save } = await import('@tauri-apps/plugin-dialog')
    const outputPath = await save({defaultPath:'casy-feishu-snapshot.json',filters:[{name:'JSON',extensions:['json']}]})
    if (outputPath) {
      const result = await tauriCallSafe('save_feishu_snapshot',{snapshot:snapshot.value,outputPath})
      if (!result.ok) error.value = result.error || '保存失败'
    }
    return
  }
  const blob = new Blob([JSON.stringify(snapshot.value,null,2)],{type:'application/json'})
  const objectUrl = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = objectUrl; a.download = 'casy-feishu-snapshot.json'; a.click()
  setTimeout(() => URL.revokeObjectURL(objectUrl), 1000)
}
async function runImport() {
  if (!snapshot.value || (scope.value === 'selected' && !selected.value.length)) return
  busy.value = true; emit('busy',true); error.value = ''
  try {
    const result = await tauriCallSafe('feishu_import_snapshot',{snapshot:snapshot.value,caseTableId:primaryId.value,selectedRecordIds:scope.value === 'all' ? [] : selected.value})
    if (!result.ok) throw new Error(result.error)
    report.value = result.data!
    emit('imported')
  } catch(e) { error.value = String(e) }
  finally { busy.value = false; emit('busy',false) }
}
function name(row: {fields: Record<string,unknown>}) { return String(row.fields['案件信息'] || row.fields['案件名称'] || Object.values(row.fields)[0] || '') }
</script>
<template>
  <section class="snapshot-import">
    <el-form label-position="top" :disabled="busy">
      <el-form-item label="飞书多维表格链接">
        <div class="source-row"><el-input v-model="url" clearable /><el-button :icon="Download" :loading="busy" :disabled="!url.trim()" @click="download">只读获取</el-button></div>
      </el-form-item>
      <div class="source-row"><el-button :icon="Upload" @click="input?.click()">打开本地快照</el-button><el-button v-if="snapshot" :icon="Download" @click="exportSnapshot">保存快照</el-button><input ref="input" type="file" accept=".json,application/json" hidden @change="loadFile" /></div>
      <template v-if="snapshot">
        <el-table :data="snapshot.tables" class="tables" size="small"><el-table-column prop="name" label="工作表" /><el-table-column label="字段数" width="100"><template #default="{row}">{{ row.fields.length }}</template></el-table-column><el-table-column label="记录数" width="100"><template #default="{row}">{{ row.records.length }}</template></el-table-column></el-table>
        <el-form-item label="案件主表"><el-select v-model="primaryId" @change="selected = []"><el-option v-for="table in snapshot.tables" :key="table.table_id" :value="table.table_id" :label="table.name" /></el-select></el-form-item>
        <el-radio-group v-model="scope"><el-radio-button value="all">全部案件</el-radio-button><el-radio-button value="selected">指定案件及关联案件</el-radio-button></el-radio-group>
        <el-select v-if="scope === 'selected'" v-model="selected" multiple filterable class="case-picker" placeholder="选择案件"><el-option v-for="row in rows" :key="row.record_id" :value="row.record_id" :label="name(row)" /></el-select>
        <div class="import-action"><el-button type="primary" :icon="Check" :loading="busy" :disabled="!primaryId || (scope === 'selected' && !selected.length)" @click="runImport">写入本地 Casy</el-button></div>
      </template>
    </el-form>
    <el-alert v-if="error" :title="error" type="error" :closable="false" show-icon />
    <div v-if="report" class="report" role="status">
      <h3>本地导入完成</h3>
      <p>本地附件：{{ report.assets }} · 卷宗文件：{{ report.files }}</p>
      <dl><template v-for="[key,label] in [['cases','案件'],['logs','日志'],['hearings','庭审'],['tasks','任务'],['officials','联系人'],['relations','案件关系'],['sourceRecords','原始记录'],['sourceLinks','原始引用'],['skipped','已有案件']]" :key="key"><dt>{{ label }}</dt><dd>{{ report[key as keyof SnapshotReport] }}</dd></template></dl>
      <el-alert v-for="warning in report.warnings" :key="warning" :title="warning" type="warning" :closable="false" />
    </div>
  </section>
</template>
<style scoped>
.snapshot-import { padding: 16px 0; }
.source-row { display: flex; width: 100%; gap: 12px; }
.tables { margin: 24px 0; }
.case-picker { display: block; margin-top: 16px; }
.import-action { display: flex; justify-content: flex-end; padding-top: 24px; }
h3 { font-size: 16px; }
dl { display: grid; grid-template-columns: repeat(3,minmax(0,1fr) 40px); gap: 12px; }
dt { color: var(--c-text-secondary); } dd { margin: 0; font-weight: 600; }
.report .el-alert { margin-top: 8px; }
@media(max-width: 520px) { .source-row { flex-wrap: wrap; } dl { grid-template-columns: 1fr 40px; } }
</style>
