<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { tauriCallSafe } from '../../../core/tauriBridge'
import { isTauriRuntime } from '../../../core/mockData'
import { Search, Close, Document, FolderOpened } from '@element-plus/icons-vue'
import type { DocumentPassage } from '../../../types/documentRetrieval'

const props = defineProps<{ modelValue: boolean; caseId: string; files: Array<{id:string;fileName:string}> }>()
const emit = defineEmits<{ (e:'update:modelValue', value:boolean):void }>()
const query = ref(''), mode = ref('source'), busy = ref(false), error = ref(''), searched = ref(false)
const passages = ref<DocumentPassage[]>([])
const selectedFileIds = ref<string[]>([])
const logs = ref<string[]>([])
const answer = ref<{answer:string; excerpts:Array<{source:string;quote:string}>} | null>(null)
let unlisten: UnlistenFn | null = null
let disposed = false, revision = 0

onMounted(async () => {
  if (!isTauriRuntime()) return
  try {
    const stop = await listen<{message:string}>('reasoning_progress', event => {
      if (busy.value && mode.value === 'answer') logs.value = [...logs.value.slice(-19),event.payload.message]
    })
    if (disposed) stop(); else unlisten = stop
  } catch { /* Progress events are optional; command errors are shown below. */ }
})
onUnmounted(() => { disposed = true; revision++; unlisten?.() })
watch(() => [props.modelValue,props.caseId], () => {
  revision++; busy.value = false; passages.value = []; answer.value = null; error.value = ''; searched.value = false; logs.value = []
  selectedFileIds.value = props.files.map(file => file.id)
})
watch([mode,selectedFileIds], () => {
  revision++; busy.value = false; passages.value = []; answer.value = null; error.value = ''; searched.value = false; logs.value = []
})

async function search() {
  if (busy.value || !query.value.trim()) return
  if (!selectedFileIds.value.length) { error.value = '请选择需要检索的卷宗文件'; return }
  if (selectedFileIds.value.length > 200) { error.value = '单次最多选择 200 份文档，请缩小范围'; return }
  const current = ++revision
  busy.value = true; error.value = ''; answer.value = null; passages.value = []; logs.value = []; searched.value = false
  try {
    if (mode.value === 'source') {
      const result = await tauriCallSafe('search_document_passages',{query:query.value,scope:selectedFileIds.value})
      if (current !== revision) return
      if (!result.ok) throw new Error(result.error || '原文检索失败')
      passages.value = result.data || []
    } else {
      const result = await tauriCallSafe('reasoning_search',{query:query.value,scope:selectedFileIds.value})
      if (current !== revision) return
      if (!result.ok || !result.data) throw new Error(result.error || '问答失败')
      const parsed = JSON.parse(result.data)
      if (parsed.status !== 'success' || !parsed.data?.[0]?.answer) throw new Error('没有收到有效回答')
      answer.value = parsed.data[0]
    }
    searched.value = true
  } catch (e) { if (current === revision) error.value = e instanceof Error ? e.message : String(e) }
  finally { if (current === revision) busy.value = false }
}
async function openSource(passage:DocumentPassage) {
  const result = await tauriCallSafe('open_file_with_default',{path:passage.sourcePath})
  if (!result.ok) error.value = result.error || '无法打开原文件'
}
</script>

<template>
  <el-drawer :model-value="modelValue" @update:model-value="emit('update:modelValue',false)" size="min(680px, 100vw)" class="reasoning-drawer" :with-header="false">
    <section class="document-search">
      <header><h2>卷宗检索</h2><el-button :icon="Close" title="关闭" aria-label="关闭" @click="emit('update:modelValue',false)" /></header>
      <el-radio-group v-model="mode" :disabled="busy"><el-radio-button value="source">原文检索</el-radio-button><el-radio-button value="answer">AI 问答</el-radio-button></el-radio-group>
      <el-select v-model="selectedFileIds" multiple filterable collapse-tags collapse-tags-tooltip :disabled="busy" placeholder="选择卷宗文件" aria-label="检索范围">
        <el-option v-for="file in files" :key="file.id" :label="file.fileName" :value="file.id" />
      </el-select>
      <form class="query-row" @submit.prevent="search"><el-input v-model="query" :disabled="busy" placeholder="关键词或案情问题" maxlength="500" clearable /><el-button :icon="Search" :loading="busy" :disabled="!query.trim()" type="primary" native-type="submit" aria-label="检索" title="检索" /></form>
      <el-alert v-if="error" :title="error" type="error" :closable="false" show-icon />
      <p v-if="busy" role="status" class="progress">{{ logs[logs.length-1] || (mode === 'source' ? '正在查找原文…' : '正在读取文档证据…') }}</p>
      <div class="results">
        <p v-if="searched && !answer" class="result-count">{{ passages.length }} 处命中</p>
        <article v-for="passage in passages" :key="passage.citation" class="passage">
          <div class="passage-heading"><Document /><strong>{{ passage.fileName }}</strong><span>第 {{ passage.number }} {{ passage.locationKind === 'segment' ? '段' : '页' }}</span><el-button :icon="FolderOpened" title="打开原文件" aria-label="打开原文件" @click="openSource(passage)" /></div>
          <pre>{{ passage.content.slice(0,800) }}</pre>
          <details v-if="passage.content.length > 800"><summary>展开原文片段</summary><pre>{{ passage.content }}</pre></details>
        </article>
        <template v-if="answer"><h3>回答</h3><p class="answer">{{ answer.answer }}</p><h3>原文依据</h3><blockquote v-for="(citation,index) in answer.excerpts" :key="index"><p>{{ citation.quote }}</p><cite>{{ citation.source }}</cite></blockquote></template>
        <el-empty v-if="searched && !passages.length && !answer" description="没有找到相关原文" :image-size="60" />
      </div>
    </section>
  </el-drawer>
</template>

<style scoped>
.document-search {height:100%;display:flex;flex-direction:column;gap:16px;min-width:0;color:var(--c-text)}
header {display:flex;align-items:center;justify-content:space-between;gap:12px}
h2 {font-size:20px;margin:0} h3 {font-size:15px;margin:16px 0 10px}
.query-row {display:flex;gap:8px}.query-row .el-input {min-width:0}
.results {flex:1;min-height:0;overflow:auto}.result-count,.progress {font-size:13px;color:var(--c-text-secondary);margin:0}
.passage {padding:18px 0;border-bottom:1px solid var(--c-border)}
.passage-heading {display:flex;align-items:center;gap:8px;font-size:13px}.passage-heading>svg {width:16px;flex-shrink:0}
.passage-heading strong {flex:1;min-width:0;overflow-wrap:anywhere}.passage-heading>span {white-space:nowrap;color:var(--c-text-secondary)}
pre,.answer,blockquote p {white-space:pre-wrap;overflow-wrap:anywhere;font:inherit;font-size:14px;line-height:1.7}
pre {margin:12px 0}.answer {margin:0} summary {font-size:12px;color:var(--c-primary);cursor:pointer}
blockquote {margin:12px 0;padding:4px 14px;border-left:3px solid var(--c-border)}
cite {font-size:11px;color:var(--c-text-secondary);overflow-wrap:anywhere}
@media(max-width:520px){.passage-heading {flex-wrap:wrap}.passage-heading strong {flex-basis:60%}}
</style>
