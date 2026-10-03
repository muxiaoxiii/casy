<script setup>
import { useViewMemory } from '../../../composables/useViewMemory'
import SaveConflictDialog from '../../../shared/components/SaveConflictDialog.vue'
import { formatTimestamp, relativeTimestamp } from '../../../shared/utils/date'
import ContextMenu from "../../../shared/components/ContextMenu.vue"
import { useContextActions } from "../../../shared/composables/useContextActions"
const { contextMenu: objectMenu, showContextMenu: showObjectMenu } = useContextActions()

import "../../../shared/editor/checklist.css"
import {exportDocument} from '../../../shared/editor/exportDocument'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { onBeforeRouteLeave } from 'vue-router'
import { ElMessage, ElMessageBox } from 'element-plus'
import { observeChanges } from '../../../core/observeChanges'
import { casyContext } from '../../../core/plugin/context'
import {
  Collection, Delete, Document, Folder, Link, Plus, Search,
  Tickets, View, EditPen, Clock, Connection, Download, MagicStick, List,
} from '../../../shared/icons'
import MarkdownCodeMirror from '../components/MarkdownCodeMirror.vue'
import MarkdownWysiwygEditor from '../components/MarkdownWysiwygEditor.vue'
import DocumentMarkdown from '../../../shared/components/DocumentMarkdown.vue'
import RelatedWork from '../../../shared/components/RelatedWork.vue'
import KnowledgeRelationsPanel from '../components/KnowledgeRelationsPanel.vue'
import KnowledgeHistoryPanel from '../components/KnowledgeHistoryPanel.vue'
import KnowledgeImportPanel from '../components/KnowledgeImportPanel.vue'
import KnowledgeSearchPanel from '../components/KnowledgeSearchPanel.vue'
import { useNotebookSave } from '../composables/useNotebookSave'
import { tauriCallSafe } from '../../../core/tauriBridge'
import { safeListen } from '../../../core/tauriEvents'
import WorkspaceDocumentPreview from '../components/WorkspaceDocumentPreview.vue'

const router = useRouter()
const route = useRoute()
const loading = ref(false)
const loadError = ref('')
const writingFocus = ref(false)
const notes = ref([])
const cases = ref([])
const selectedCaseIds = ref([])
const sources = ref([])
const selectedSource = ref(null)
const sourceError = ref('')
let sourceRevision = 0
let sourceTimer
let stopWorkspaceListener
async function loadSources() {
  const request = ++sourceRevision
  if (!selectedCaseIds.value.length) { sources.value = []; selectedSource.value = null; return }
  const result = await tauriCallSafe('list_workspace_sources', { caseIds: selectedCaseIds.value })
  if (unmounted || request !== sourceRevision) return
  sourceError.value = result.ok ? '' : result.error || '读取案卷失败'
  if (result.ok) {
    sources.value = result.data || []
    if (selectedSource.value) selectedSource.value = sources.value.find(s => s.fileId === selectedSource.value.fileId) || null
  }
}
async function changeCaseScope(ids) {
  if (!(await flushSave())) return
  selectedCaseIds.value = ids
  await loadSources()
  if (sources.value.length) await selectSource(sources.value[0])
}
async function selectSource(source) {
  if (documentBusy.value) return
  const request = ++selectionRevision
  if (!(await flushSave()) || request !== selectionRevision) return
  selectedSource.value = source
  mobilePane.value = 'editor'
}
async function openSourceNote(id) { selectedSource.value = null; await loadAll(id) }
const visibleSources = computed(() => {
  const q = search.value.trim().toLowerCase()
  return sources.value.filter(s => !q || s.fileName.toLowerCase().includes(q))
})
async function refreshNoteList() {
  const result = await casyContext.knowledge.list({})
  if (!unmounted && !dirty.value && !saving.value && result.ok) notes.value = normalizeList(result.data)
}
const selectedId = ref('')
const search = ref('')
const category = ref('all')
const mode = ref('rich')
const titleInput = ref(null)
const editorRef = ref(null)
const relationPanelRef = ref(null)
const historyPanelRef = ref(null)
const draft = ref(emptyDraft())
const infoTab = ref('relations')
useViewMemory('knowledge', { search, category, selectedCaseIds, infoTab, selectedId }, ["#main-content", ".note-scroll"])
const searchOpen = ref(false)
const exporting = ref(false)
const outlineItems = ref([])
let selectionRevision = 0
let loadRevision = 0
let stopCloseListener
let unmounted = false
const documentBusy = ref(false)
const mobilePane = ref('notes')
const editorSession = crypto.randomUUID()
const conflictDialog = ref(null)
const localConflict = () => ({ ...draft.value })
async function latestConflict() {
  const result = await casyContext.knowledge.getWithBlocks(draft.value.id)
  if (!result.ok || !result.data?.item) throw new Error(result.error || '读取最新笔记失败')
  return result.data.item
}
async function copyConflict(snapshot) {
  const { id, ...data } = snapshot
  const result = await casyContext.knowledge.create({ ...data, title: `${snapshot.title}（本地冲突副本）`, linkedCaseId: data.linkedCaseId || null, parentId: data.parentId || null })
  if (!result.ok) throw new Error(result.error || '副本保存失败')
  void refreshNoteList()
}
function applyConflict(value) {
  void persistence.clearRecovery().catch(() => {})
  persistence.hydrate({ id: value.id, title: value.title || '', content: value.content || '', category: value.category || 'reference', tags: value.tags || '', linkedCaseId: value.linkedCaseId || '', parentId: value.parentId || '' })
  void refreshNoteList()
}
const persistence = useNotebookSave({
  onConflict: () => conflictDialog.value?.open(),
  draft,
  syncEditor: syncEditorContent,
  update: (id, data) => casyContext.knowledge.update(id, data),
  onSaved: (id, data) => {
    const item = notes.value.find(note => note.id === id)
    if (item) Object.assign(item, { ...data, updatedAt: new Date().toISOString() })
    if (selectedId.value === id) {
      refreshRelations()
      historyPanelRef.value?.reload?.()
    }
  },
  onError: message => ElMessage.error(message),
  recovery: window.__TAURI_INTERNALS__ ? {
    write: async draft => {
      const result = await tauriCallSafe('save_editor_recovery', { sessionId: editorSession, draft })
      if (!result.ok) throw new Error(result.error)
    },
    clear: async () => {
      const result = await tauriCallSafe('save_editor_recovery', { sessionId: editorSession, draft: null })
      if (!result.ok) throw new Error(result.error)
    },
  } : undefined,
})
const { dirty, saving, error: saveError } = persistence

const categories = [
  { value: 'all', label: '全部笔记', color: '#64748b' },
  { value: 'inspiration', label: '灵感', color: '#7c6cae' },
  { value: 'method', label: '方法', color: '#3e5c9a' },
  { value: 'reference', label: '参考', color: '#4c8067' },
  { value: 'question', label: '问题', color: '#b4554f' },
  { value: 'experience', label: '经验', color: '#b0823a' },
  { value: 'log', label: '日志', color: '#7b8492' },
  { value: 'document_summary', label: '卷宗', color: '#9b5c52' },
]

function emptyDraft() {
  return { id: '', title: '', content: '', category: 'reference', tags: '', linkedCaseId: '', parentId: '' }
}

function normalizeList(data) {
  if (Array.isArray(data)) return data
  if (Array.isArray(data?.items)) return data.items
  return []
}

const filteredNotes = computed(() => {
  const q = search.value.trim().toLowerCase()
  return notes.value.filter((note) => {
    if (note.blockType === 'block') return false
    if (selectedCaseIds.value.length && !selectedCaseIds.value.includes(note.linkedCaseId)) return false
    if (category.value !== 'all' && note.category !== category.value) return false
    if (!q) return true
    return [note.title, note.content, note.tags].some((v) => String(v || '').toLowerCase().includes(q))
  })
})

const visibleNotes = computed(() => {
  const items = filteredNotes.value
  if (search.value.trim() || category.value !== 'all') return items.map(item => ({ ...item, depth: 0 }))
  const itemIds = new Set(items.map(item => item.id))
  const byParent = new Map()
  for (const item of items) {
    const parent = item.parentId && itemIds.has(item.parentId) ? item.parentId : ''
    if (!byParent.has(parent)) byParent.set(parent, [])
    byParent.get(parent).push(item)
  }
  const result = []
  const seen = new Set()
  const walk = (parentId, depth) => {
    for (const item of byParent.get(parentId) || []) {
      if (seen.has(item.id)) continue
      seen.add(item.id)
      result.push({ ...item, depth })
      walk(item.id, Math.min(depth + 1, 6))
    }
  }
  walk('', 0)
  for (const item of items) if (!seen.has(item.id)) result.push({ ...item, depth: 0 })
  return result
})

const counts = computed(() => {
  const result = { all: notes.value.filter(n => n.blockType !== 'block').length }
  for (const note of notes.value) result[note.category] = (result[note.category] || 0) + 1
  return result
})

const selectedNote = computed(() => notes.value.find(n => n.id === selectedId.value && (!selectedCaseIds.value.length || selectedCaseIds.value.includes(n.linkedCaseId))) || null)
const selectedCaseName = computed(() => {
  const found = cases.value.find(c => c.id === draft.value.linkedCaseId)
  return found?.caseName || found?.displayName || found?.caseNo || ''
})

function noteSummary(note) {
  return String(note.content || '').slice(0, 512).replace(/[#>*_`\[\]-]/g, ' ').replace(/\s+/g, ' ').trim().slice(0, 110) || '空白笔记'
}

const displayTime = formatTimestamp

function safeFileName(value) {
  return String(value || '无标题笔记').replace(/[\\/:*?"<>|]/g, '_').replace(/[. ]+$/g, '').slice(0, 120) || '无标题笔记'
}

function onOutlineChange(items) {
  outlineItems.value = Array.isArray(items) ? items : []
}

function jumpToHeading(id) {
  editorRef.value?.scrollToHeading?.(id)
}

// 导出前展开 `assets/` 引用：导出链路按内嵌图落盘，正文引用本身不携带字节。
async function expandNoteAssets(markdown) {
  const pattern = /assets\/([a-f0-9]{64}\.png)/g
  const ids = [...new Set([...markdown.matchAll(pattern)].map((m) => m[1]))]
  if (!ids.length) return markdown
  let output = markdown
  for (const id of ids) {
    const encoded = await readNoteAsset(id)
    output = output.split(`assets/${id}`).join(`data:image/png;base64,${encoded}`)
  }
  return output
}

async function handleExportCommand(format) {
 if (!draft.value.id || exporting.value) return
 exporting.value=true
 try {
  if(!await flushSave())return
  const path=await exportDocument({content:await expandNoteAssets(draft.value.content),contentFormat:'markdown',title:draft.value.title || '无标题笔记',format:format==='markdown'?'md':format,document:editorRef.value?.getDocumentJson?.()})
  if(path)ElMessage.success(`文档已保存：${path}`)
 }catch(error){ElMessage.error(String(error))}finally{exporting.value=false}
}

// 预览与编辑器共用同一 MD 桥（GFM 表格/任务列表/wiki 链接全量渲染，v-html 前消毒）
// 知识快照正文含 `assets/<sha>.png` 引用：编辑与预览按需取图，序列化仍保留引用。
async function readNoteAsset(assetId) {
  const result = await tauriCallSafe("read_knowledge_asset", {
    noteId: draft.value.id,
    assetId,
  });
  if (!result.ok || !result.data) throw new Error(result.error || "图片加载失败");
  return result.data;
}

// wiki 补全/源码补全共用的标题源
const noteTitleOptions = computed(() =>
  notes.value.map(item => ({
    id: item.id,
    title: item.title,
    categoryLabel: categories.find(c => c.value === item.category)?.label,
  })),
)

// 编辑器内点击 wiki 双链 → 按标题跳转到对应笔记
async function onWikiLinkClick({ title }) {
  if (!title) return
  const target = notes.value.find(n => n.title === title)
  if (target) await selectNote(target)
  else ElMessage.info(`未找到标题为「${title}」的笔记`)
}

/** 从当前富文本编辑器取回权威 Markdown；必须在替换 draft/selectedId 前执行。 */
function syncEditorContent(commitSources = true) {
  const value = editorRef.value?.flushAndGetMarkdown?.(commitSources)
  if (typeof value === 'string' && value !== draft.value.content) {
    draft.value.content = value
  }
}

async function loadAll(preferredId = '') {
  if (!(await flushSave())) return
  const request = ++loadRevision
  loading.value = true
  const [noteRes, caseRes] = await Promise.all([
    casyContext.knowledge.list({}),
    loadCaseOptions(),
  ])
  if (request !== loadRevision) return
  loading.value = false
  loadError.value = noteRes.ok ? '' : noteRes.error || '无法读取笔记列表'
  if (!noteRes.ok) return
  notes.value = normalizeList(noteRes.data)
  if (caseRes.ok) cases.value = normalizeList(caseRes.data)
  const queryId = typeof route.query.select === 'string' ? route.query.select : ''
  const nextId = preferredId || queryId || notes.value.find(note => note.id === selectedId.value)?.id || filteredNotes.value[0]?.id
  if (nextId) await selectNote(notes.value.find(n => n.id === nextId) || { id: nextId })
}

async function loadCaseOptions() {
  const items = []
  for (let page = 1; !unmounted; page++) {
    const result = await casyContext.cases.list({ page, perPage: 200 })
    if (!result.ok) return result
    const batch = normalizeList(result.data)
    items.push(...batch)
    if (!batch.length || items.length >= (result.data?.total ?? items.length)) return { ok: true, data: items }
  }
  return { ok: true, data: items }
}

async function selectNote(note) {
  if (!note) return
  if (documentBusy.value) return
  if (selectedId.value === note.id && draft.value.id === note.id) { selectedSource.value = null; mobilePane.value = 'editor'; return }
  const request = ++selectionRevision
  if (!(await flushSave())) return
  if (request !== selectionRevision) return
  documentBusy.value = true
  try {
    const result = await casyContext.knowledge.getWithBlocks(note.id)
    if (request !== selectionRevision) return
    if (!result.ok || !result.data?.item) return ElMessage.error(result.error || '无法读取笔记')
    const current = result.data.item
    const index = notes.value.findIndex(item => item.id === current.id)
    if (index < 0) notes.value.unshift(current)
    else notes.value[index] = current
    selectedId.value = current.id
    selectedSource.value = null
    persistence.hydrate({
      id: current.id,
      title: current.title || '',
      content: current.content || '',
      category: current.category || 'reference',
      tags: current.tags || '',
      linkedCaseId: current.linkedCaseId || '',
      parentId: current.parentId || '',
    })
    outlineItems.value = []
    mobilePane.value = 'editor'
    await nextTick()
  } finally { documentBusy.value = false }
}

async function createNote(parentId = '') {
  if (documentBusy.value) return
  parentId = typeof parentId === 'string' ? parentId : ''
  if (!(await flushSave())) return
  const result = await casyContext.knowledge.create({
    title: '无标题笔记', content: '', category: 'reference',
    tags: '', linkedCaseId: null, parentId: parentId || null, sourceType: 'notebook', status: 'current', blockType: 'page',
  })
  if (!result.ok) return ElMessage.error(result.error || '新建失败')
  await loadAll(result.data)
  mode.value = 'rich'
  nextTick(() => titleInput.value?.focus())
}

function createChildNote() {
  if (!selectedId.value) return createNote()
  createNote(selectedId.value)
}

// Wiki 双链由后端 update_knowledge / restore_knowledge_version 权威同步；
// 前端只需在保存后刷新双链面板，不再自行 diff links 表（避免与后端语义分叉）。
function refreshRelations() {
  relationPanelRef.value?.reload?.()
}

async function saveNow(silent = false) {
  const ok = await persistence.flush()
  if (ok && !silent) ElMessage.success('笔记已保存')
  return ok
}

async function flushSave() {
  return persistence.flush()
}

function changeMode(nextMode) {
  if (documentBusy.value || mode.value === nextMode) return
  try { syncEditorContent() } catch(error) { ElMessage.error(String(error)); return }
  mode.value = nextMode
}

watch(() => route.query.select, (id) => {
  if (typeof id !== 'string') return
  selectNote(notes.value.find(item => item.id === id) || { id })
})

onBeforeRouteLeave(async () => {
  if (documentBusy.value) return false
  return await flushSave()
})

async function deleteNote() {
  if (!draft.value.id || documentBusy.value) return
  const id = draft.value.id
  try {
    await ElMessageBox.confirm(`删除笔记「${draft.value.title || '无标题笔记'}」？`, '删除确认', { type: 'warning' })
  } catch { /* 用户取消：属预期 */ return }
  if (draft.value.id !== id) return
  documentBusy.value = true
  try {
    if (!(await flushSave())) return
    const result = await casyContext.knowledge.remove(id)
    if (!result.ok) return ElMessage.error(result.error || '删除失败')
    selectedId.value = ''
    persistence.hydrate(emptyDraft())
  } finally { documentBusy.value = false }
  await loadAll()
  ElMessage.success('笔记已删除')
}

async function restoreVersion(noteId, versionId) {
  if (documentBusy.value || draft.value.id !== noteId) return { ok: false, error: '当前笔记已改变' }
  documentBusy.value = true
  try {
    if (!(await flushSave())) return { ok: false, error: '请先保存当前修改' }
    const result = await casyContext.knowledge.restoreVersion(noteId, versionId)
    if (!result.ok) return result
    const refreshed = await casyContext.knowledge.getWithBlocks(noteId)
    if (!refreshed.ok || !refreshed.data?.item) return { ok: false, error: '版本已恢复，但重新读取失败，请重新打开笔记' }
    const note = refreshed.data.item
    Object.assign(notes.value.find(item => item.id === noteId), note)
    persistence.hydrate({ ...emptyDraft(), ...note, tags: note.tags || '', linkedCaseId: note.linkedCaseId || '', parentId: note.parentId || '' })
    await nextTick()
    refreshRelations()
    return { ok: true }
  } finally { documentBusy.value = false }
}

function beforeUnload(event) {
  try { syncEditorContent() } catch { event.preventDefault(); event.returnValue = ''; return }
  if (!dirty.value && !saving.value && !documentBusy.value) return
  event.preventDefault()
  event.returnValue = ''
}

onMounted(async () => {
  sourceTimer = setInterval(loadSources, 10000)
  const stop = safeListen('workspace:updated', () => { void loadSources(); if (!dirty.value && !documentBusy.value) void refreshNoteList() })
  if (unmounted) stop?.()
  else stopWorkspaceListener = stop
  window.addEventListener('beforeunload', beforeUnload)
  try {
    if (window.__TAURI_INTERNALS__?.metadata?.currentWindow) {
      const { getCurrentWindow } = await import('@tauri-apps/api/window')
      if (unmounted) return
      const current = getCurrentWindow()
      const unlisten = await current.onCloseRequested(async event => {
        try { syncEditorContent() } catch(error) { event.preventDefault(); ElMessage.error(String(error)); return }
        if (!dirty.value && !saving.value && !documentBusy.value) return
        event.preventDefault()
        if (documentBusy.value) return
        documentBusy.value = true
        try {
          if (!(await flushSave())) return
          documentBusy.value = false
          await current.close()
        } catch {
          ElMessage.error('关闭窗口失败，笔记已保留')
        } finally { documentBusy.value = false }
      })
      if (unmounted) unlisten()
      else stopCloseListener = unlisten
    }
  } catch {
    ElMessage.error('无法启用窗口关闭保护，请保存笔记后再退出')
  }
  if (!unmounted) {
    if (window.__TAURI_INTERNALS__) {
      const recovered = await tauriCallSafe('recover_editor_drafts', {})
      if (recovered.ok && recovered.data > 0) ElMessage.info(`已找回 ${recovered.data} 份内容，保留为恢复草稿`)
    }
    await loadAll()
  }
})
onBeforeUnmount(() => {
  clearInterval(sourceTimer)
  stopWorkspaceListener?.()
  ++sourceRevision
  unmounted = true
  ++loadRevision
  ++selectionRevision
  window.removeEventListener('beforeunload', beforeUnload)
  stopCloseListener?.()
})

async function openSearch() {
  if (await flushSave()) searchOpen.value = true
}
// Refresh the catalog only: never replace a dirty or focused editor on background writes.
onBeforeUnmount(observeChanges(casyContext, ['knowledge', 'case', 'inbox'], async () => {
  const [noteRes, caseRes] = await Promise.all([casyContext.knowledge.list({}), loadCaseOptions()])
  if (unmounted) return
  if (noteRes.ok) notes.value = normalizeList(noteRes.data)
  if (caseRes.ok) cases.value = normalizeList(caseRes.data)
}))

async function openSearchHit(id) {
  await selectNote({ id })
  if (selectedId.value === id) searchOpen.value = false
}
</script>

<template>
  <SaveConflictDialog ref="conflictDialog" :local="localConflict" :latest="latestConflict" :copy="copyConflict" :apply="applyConflict" :prepare="syncEditorContent" />
  <el-alert v-if="persistence.conflicted.value" title="笔记存在保存冲突，本地内容仍保留" type="warning" :closable="false"><el-button @click="conflictDialog?.open()">处理冲突</el-button></el-alert>
  <div class="notebook-shell" :class="[`mobile-${mobilePane}`, { 'source-open': !!selectedSource, 'writing-focus': writingFocus }]">
    <nav class="mobile-notebook-nav" aria-label="笔记视图">
      <button :class="{ active: mobilePane === 'notes' }" @click="mobilePane = 'notes'">笔记</button>
      <button :disabled="!selectedNote && !selectedSource" :class="{ active: mobilePane === 'editor' }" @click="mobilePane = 'editor'">正文</button>
      <button v-if="!selectedSource" :disabled="!selectedNote" :class="{ active: mobilePane === 'info' }" @click="mobilePane = 'info'">关联与历史</button>
    </nav>
    <section class="note-list-panel">
      <div class="vault-head">
        <div class="vault-title"><Collection /><span>知识笔记</span></div>
        <button class="new-note new-note-icon" title="知识库检索" aria-label="知识库检索" @click="openSearch"><Search /></button>
        <button class="new-note new-note-icon" title="新建笔记" @click="createNote"><Plus /></button>
      </div>
      <div class="list-head">
        <h2>{{ categories.find(c => c.value === category)?.label }}</h2>
        <span>{{ visibleNotes.length + visibleSources.length }} 项</span>
      </div>
      <div class="search-box"><Search /><input v-model="search" placeholder="搜索标题、正文或标签" /></div>
      <el-select class="case-scope" :model-value="selectedCaseIds" multiple filterable clearable collapse-tags placeholder="筛选案件" aria-label="筛选案件" @change="changeCaseScope"><el-option v-for="c in cases" :key="c.id" :value="c.id" :label="c.caseName || c.caseNo" /></el-select>
      <div class="category-strip">
        <button v-for="item in categories" :key="item.value" :class="{ active: category === item.value }" @click="category = item.value">
          <span class="category-dot" :style="{ background: item.color }" />{{ item.label }}<b>{{ counts[item.value] || 0 }}</b>
        </button>
      </div>
      <div class="note-scroll" v-loading="loading">
        <el-alert v-if="loadError" :title="loadError" type="error" :closable="false"><el-button text @click="loadAll()">重试</el-button></el-alert>
        <el-alert v-if="sourceError" :title="sourceError" type="error" :closable="false" />
        <div v-if="visibleSources.length" class="source-group-title">案卷正文</div>
        <button v-for="source in visibleSources" :key="source.fileId" class="note-card" :class="{ active: selectedSource?.fileId === source.fileId }" @click="selectSource(source)"><div class="note-card-top"><strong>{{ source.fileName }}</strong><span class="note-category">案卷</span></div><p>{{ source.missing ? '原件缺失' : source.status === 'completed' ? 'Markdown' : source.status === 'running' ? '正文提取中' : source.status === 'queued' ? '等待提取正文' : source.status === 'failed' ? '提取失败' : '待提取正文' }}</p></button>
        <div v-if="visibleSources.length && visibleNotes.length" class="source-group-title">知识笔记与快照</div>
        <button v-for="note in visibleNotes" :key="note.id" class="note-card" :disabled="documentBusy" :class="{ active: !selectedSource && note.id === selectedId }" :style="{ '--tree-depth': note.depth }" @click="selectNote(note)" @contextmenu="showObjectMenu($event, note.title, [{ label: '打开笔记', disabled: documentBusy, run: () => selectNote(note) }, { label: '新建子笔记', disabled: documentBusy, run: () => createNote(note.id) }])">
          <div class="note-card-top"><strong>{{ note.title || '无标题笔记' }}</strong><span class="note-category">{{ categories.find(c => c.value === note.category)?.label || '其他' }}</span></div>
          <p>{{ noteSummary(note) }}</p>
          <div class="note-meta"><span><Clock /> {{ displayTime(note.updatedAt) }}</span><span v-if="note.linkedCaseId"><Folder /> {{ cases.find(c => c.id === note.linkedCaseId)?.caseName || '关联案件' }}</span></div>
        </button>
        <div v-if="!loading && !loadError && !filteredNotes.length && !visibleSources.length" class="empty-notes"><Document /><p>{{ search || category !== 'all' ? '没有匹配的笔记' : '这里还没有笔记' }}</p><button v-if="search || category !== 'all'" @click="search = ''; category = 'all'">清除筛选</button><button v-else @click="createNote">新建笔记</button></div>
      </div>
      <div class="list-utilities">
        <button @click="router.push({ name: 'knowledge-graph' })"><Link />知识图谱</button>
        <button @click="router.push({ name: 'cases' })"><Folder />案件沉淀</button>
      </div>
    </section>

    <el-drawer v-model="searchOpen" title="知识库检索" size="min(680px, 100vw)" class="knowledge-search-drawer" destroy-on-close>
      <KnowledgeSearchPanel v-if="searchOpen" @navigate="openSearchHit" @settings="router.push({ path: '/settings', query: { tab: 'ai' } })" />
    </el-drawer>

    <main class="editor-panel" :inert="documentBusy">
      <WorkspaceDocumentPreview v-if="selectedSource" :key="selectedSource.fileId" :source="selectedSource" @refreshed="loadSources" @note="openSourceNote" />
      <template v-else-if="selectedNote">
        <header class="editor-toolbar">
          <div class="save-state" role="status"><span :class="{ dirty }" />{{ saving ? '保存中…' : saveError ? '保存失败' : dirty ? '等待自动保存' : '已保存' }}</div>
          <button type="button" class="writing-focus-button" :aria-pressed="writingFocus" @click="writingFocus = !writingFocus">{{ writingFocus ? '退出专注' : '专注书写' }}</button><div class="mode-switch"><button title="富文本" :class="{ active: mode === 'rich' }" @click="changeMode('rich')"><MagicStick /> 富文本</button><button title="源码" :class="{ active: mode === 'edit' }" @click="changeMode('edit')"><EditPen /> 源码</button><button title="分栏" :class="{ active: mode === 'split' }" @click="changeMode('split')"><Tickets /> 分栏</button><button title="预览" :class="{ active: mode === 'preview' }" @click="changeMode('preview')"><View /> 预览</button></div>
          <el-dropdown trigger="click" :disabled="exporting" @command="handleExportCommand">
            <button class="export-button" :disabled="exporting"><Download />{{ exporting ? '导出中…' : '导出' }}</button>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item command="markdown">Markdown（.md）</el-dropdown-item>
                <el-dropdown-item command="pdf">PDF 文档（.pdf）</el-dropdown-item>
                <el-dropdown-item command="docx">Word 文档（.docx）</el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>
          <button class="save-button" @click="saveNow(false)">保存</button>
          <button class="child-button" title="在当前笔记下新建子笔记" aria-label="新建子笔记" @click="createChildNote"><Plus /></button>
          <button class="delete-button" title="删除笔记" @click="deleteNote"><Delete /></button>
        </header>
        <section class="editor-document">
        <div class="note-fields">
          <input ref="titleInput" v-model="draft.title" class="title-editor" placeholder="无标题笔记" />
          <details class="note-properties" :open="!writingFocus"><summary>笔记属性 <span>{{ selectedCaseName || '独立笔记' }}</span></summary><div class="property-row">
            <label>分类<select v-model="draft.category"><option v-for="item in categories.slice(1)" :key="item.value" :value="item.value">{{ item.label }}</option></select></label>
            <label>关联案件<select v-model="draft.linkedCaseId"><option value="">未关联案件</option><option v-for="c in cases" :key="c.id" :value="c.id">{{ c.caseName || c.displayName || c.caseNo }}</option></select></label>
            <label>上级笔记<select v-model="draft.parentId"><option value="">知识库根目录</option><option v-for="item in notes.filter(n => n.id !== draft.id && n.blockType !== 'block')" :key="item.id" :value="item.id">{{ item.title || '无标题笔记' }}</option></select></label>
            <label class="tags-field">标签<input v-model="draft.tags" placeholder="用逗号分隔" /></label>
          </div>
          </details>
          <div v-if="selectedCaseName" class="case-chip"><Folder /> 已关联：{{ selectedCaseName }}</div>
        </div>
        <div class="markdown-workspace" :class="`mode-${mode}`">
          <!-- :key 随笔记切换重建编辑器，撤销历史不跨笔记残留（对齐 CodeMirror 重建 state 的语义） -->
          <MarkdownWysiwygEditor v-if="mode === 'rich' || mode === 'split'" :key="`rich-${selectedId}`" ref="editorRef" v-model="draft.content" class="markdown-rich" :note-titles="noteTitleOptions" source-type="knowledge" :source-id="draft.id" :resolve-asset="draft.id ? readNoteAsset : undefined" :case-id="draft.linkedCaseId || null" @change="persistence.changed" @save="saveNow(false)" @wiki-link-click="onWikiLinkClick" @outline-change="onOutlineChange" />
          <MarkdownCodeMirror v-if="mode === 'edit'" :key="`src-${selectedId}`" ref="editorRef" v-model="draft.content" class="markdown-source" :note-titles="noteTitleOptions" @save="saveNow(false)" />
          <article v-if="mode === 'split' || mode === 'preview'" class="markdown-preview">
            <DocumentMarkdown :markdown="draft.content" :read="draft.id ? readNoteAsset : undefined" />
          </article>
        </div>
        </section>
      </template>
      <div v-else class="editor-empty"><Collection /><h3>选择或新建一篇笔记</h3><p>在这里用 Markdown 记录研究、方法、经验和案件思路。</p><button @click="createNote"><Plus /> 新建笔记</button></div>
    </main>
    <aside v-if="!selectedSource" class="knowledge-info-rail">
      <div class="info-tabs">
        <button :class="{active:infoTab==='work'}" @click="infoTab='work'"><Folder /> 工作</button>
        <button :class="{active:infoTab==='relations'}" @click="infoTab='relations'"><Connection /> 双链</button>
        <button :class="{active:infoTab==='outline'}" @click="infoTab='outline'"><List /> 目录</button>
        <button :class="{active:infoTab==='history'}" @click="infoTab='history'"><Clock /> 历史</button>
        <button :class="{active:infoTab==='import'}" @click="infoTab='import'"><Download /> 沉淀</button>
      </div>
      <div v-if="infoTab==='work'" class="note-related-work"><RelatedWork v-if="draft.linkedCaseId" :case-id="draft.linkedCaseId" /><p v-else>关联案件后，在这里查看同案任务、日程与文书。</p></div>
      <KnowledgeRelationsPanel v-else-if="infoTab==='relations'" ref="relationPanelRef" :note="selectedNote" :notes="notes" @navigate="id => selectNote(notes.find(item => item.id === id))" />
      <nav v-else-if="infoTab==='outline'" class="outline-panel">
        <div class="outline-heading">本文目录</div>
        <button v-for="item in outlineItems" :key="item.id" :style="{ paddingLeft: `${10 + (item.level - 1) * 13}px` }" @click="jumpToHeading(item.id)">{{ item.text }}</button>
        <div v-if="!outlineItems.length" class="outline-empty">添加标题后，这里会生成可跳转目录。</div>
      </nav>
      <KnowledgeHistoryPanel v-else-if="infoTab==='history'" ref="historyPanelRef" :note="selectedNote" :restore-version="restoreVersion" />
      <KnowledgeImportPanel v-else @navigate="id => selectNote(notes.find(item => item.id === id))" @imported="id => loadAll(id)" />
    </aside>
  </div>
  <ContextMenu v-bind="objectMenu" @close="objectMenu.open = false" />
</template>

<style scoped>
.notebook-shell{height:calc(100dvh - var(--app-topbar-height));min-height:0;display:grid;grid-template-columns:300px minmax(520px,1fr) 280px;background:var(--c-bg-page);overflow:hidden;color:var(--c-text)}
.notebook-sidebar{padding:22px 14px;border-right:1px solid var(--c-border);background:color-mix(in srgb,var(--c-bg-card) 86%,var(--c-bg-page));display:flex;flex-direction:column;gap:5px}.vault-title{display:flex;align-items:center;gap:9px;font-size:17px;font-weight:750;padding:0 8px 16px}.vault-title svg,.side-link svg,.new-note svg{width:16px}.new-note{display:flex;align-items:center;justify-content:center;gap:7px;border:0;border-radius:9px;padding:10px;background:var(--c-primary);color:var(--c-primary-contrast);font-weight:650;cursor:pointer;margin-bottom:16px}.side-label{padding:8px 10px 5px;text-transform:uppercase;letter-spacing:.08em;font-size:10px;color:var(--c-text-secondary)}.relation-label{margin-top:12px}.category-item,.side-link{width:100%;border:0;background:transparent;color:var(--c-text);display:flex;align-items:center;gap:8px;text-align:left;padding:8px 10px;border-radius:8px;cursor:pointer}.category-item:hover,.side-link:hover,.category-item.active{background:var(--c-bg-subtle)}.category-item.active{font-weight:650}.category-item b{margin-left:auto;font-size:11px;color:var(--c-text-secondary)}.category-dot{width:8px;height:8px;border-radius:50%}.license-note{margin-top:auto;padding:12px 9px;font-size:10px;line-height:1.5;color:var(--c-text-secondary)}
.note-list-panel{border-right:1px solid var(--c-border);background:var(--c-bg-card);display:flex;flex-direction:column;min-width:0}.list-head{padding:21px 18px 12px;display:flex;align-items:baseline;justify-content:space-between}.list-head h2{font-size:18px;margin:0}.list-head span{font-size:12px;color:var(--c-text-secondary)}.search-box{margin:0 14px 12px;display:flex;align-items:center;gap:7px;padding:8px 10px;border:1px solid var(--c-border);border-radius:8px;background:var(--c-bg-page)}.search-box svg{width:14px;color:var(--c-text-secondary)}.search-box input{min-width:0;flex:1;border:0;outline:0;background:transparent;color:inherit}.note-scroll{overflow:auto;padding:0 9px 20px}.note-card{width:100%;padding:14px 12px;border:1px solid transparent;border-bottom-color:var(--c-border);background:transparent;text-align:left;color:inherit;cursor:pointer}.note-card:hover{background:var(--c-bg-subtle)}.note-card.active{border-color:color-mix(in srgb,var(--c-primary) 35%,transparent);background:color-mix(in srgb,var(--c-primary) 8%,var(--c-bg-card));border-radius:10px}.note-card-top{display:flex;gap:10px;align-items:flex-start}.note-card-top strong{flex:1;font-size:14px;line-height:1.45}.note-category{font-size:10px;color:var(--c-primary);background:color-mix(in srgb,var(--c-primary) 10%,transparent);padding:2px 5px;border-radius:5px}.note-card p{font-size:12px;color:var(--c-text-secondary);line-height:1.55;margin:7px 0 10px}.note-meta{display:flex;gap:10px;flex-wrap:wrap;font-size:12px;color:var(--c-text-secondary)}.note-meta span{display:flex;align-items:center;gap:3px}.note-meta svg{width:11px}.empty-notes,.editor-empty{display:grid;place-items:center;text-align:center;color:var(--c-text-secondary);padding:50px 20px}.empty-notes svg,.editor-empty svg{width:36px}.empty-notes button,.editor-empty button{border:0;background:var(--c-primary);color:var(--c-primary-contrast);border-radius:8px;padding:9px 13px;cursor:pointer}
.editor-panel{min-width:0;display:flex;flex-direction:column;background:var(--c-bg-page)}.editor-toolbar{height:56px;padding:0 16px;border-bottom:1px solid var(--c-border);display:flex;align-items:center;gap:10px;background:color-mix(in srgb,var(--c-bg-card) 92%,var(--c-bg-page))}.save-state{display:flex;align-items:center;gap:7px;font-size:11px;color:var(--c-text-secondary);margin-right:auto;white-space:nowrap;flex-shrink:0}.save-state>span{width:8px;height:8px;border-radius:50%;background:#67c23a}.save-state>span.dirty{background:#e6a23c}.mode-switch{display:flex;padding:3px;gap:2px;background:var(--c-bg-subtle);border-radius:9px}.mode-switch button{display:flex;align-items:center;gap:5px;border:0;background:transparent;color:var(--c-text-secondary);padding:6px 10px;border-radius:7px;cursor:pointer;font-size:11.5px;font-weight:550;transition:all var(--motion-fast);white-space:nowrap;flex-shrink:0}.mode-switch button:hover{color:var(--c-text)}.mode-switch button.active{background:var(--c-bg-card);color:var(--c-text);box-shadow:var(--shadow-sm)}.mode-switch svg,.delete-button svg{width:13px}.save-button{border:0;background:var(--c-primary);color:var(--c-primary-contrast);border-radius:7px;padding:7px 13px;cursor:pointer;font-weight:600;white-space:nowrap;flex-shrink:0}.save-button:hover{filter:brightness(1.06)}.delete-button{border:0;background:transparent;color:var(--c-text-secondary);padding:6px;cursor:pointer;border-radius:6px}.delete-button:hover{color:var(--status-risk);background:color-mix(in srgb,var(--status-risk) 10%,transparent)}.note-fields{padding:24px 28px 14px;border-bottom:1px solid var(--c-border)}.title-editor{width:100%;border:0;outline:0;background:transparent;color:var(--c-text-heading);font-size:28px;font-weight:750;margin-bottom:16px}.property-row{display:flex;gap:16px;align-items:end;flex-wrap:wrap}.property-row label{display:flex;flex-direction:column;gap:5px;font-size:10px;text-transform:uppercase;letter-spacing:.05em;color:var(--c-text-secondary)}.property-row select,.property-row input{border:1px solid var(--c-border);border-radius:7px;background:var(--c-bg-page);color:var(--c-text);padding:7px 8px;min-width:145px}.tags-field{flex:1}.tags-field input{width:100%;box-sizing:border-box}.case-chip{display:inline-flex;align-items:center;gap:5px;margin-top:10px;padding:4px 8px;border-radius:6px;background:color-mix(in srgb,var(--c-primary) 9%,transparent);color:var(--c-primary);font-size:11px}.case-chip svg{width:12px}.markdown-workspace{flex:1;min-height:0;display:grid;overflow:hidden}.markdown-workspace.mode-split{grid-template-columns:1fr 1fr}.markdown-source,.markdown-preview{box-sizing:border-box;width:100%;height:100%;min-height:0;overflow:auto;padding:26px 32px;border:0;outline:0;background:var(--c-bg-card);color:var(--c-text);font-size:15px;line-height:1.75}.markdown-source{resize:none;font-family:'SFMono-Regular',Consolas,'Liberation Mono',monospace}.mode-split .markdown-source{border-right:1px solid var(--c-border)}.markdown-preview{font-family:var(--font-family);max-width:none}.markdown-preview :deep(h1){font-size:28px;border-bottom:1px solid var(--c-border);padding-bottom:8px}.markdown-preview :deep(h2){font-size:22px}.markdown-preview :deep(p){margin:0 0 13px}.markdown-preview :deep(blockquote){border-left:3px solid var(--c-primary);margin:12px 0;padding:5px 14px;color:var(--c-text-secondary);background:var(--c-bg-subtle)}.markdown-preview :deep(pre){background:#18202b;color:#e7edf5;padding:14px;border-radius:8px;overflow:auto}.markdown-preview :deep(code){font-family:monospace;background:var(--c-bg-subtle);padding:2px 4px;border-radius:4px}.markdown-preview :deep(pre code){background:transparent;padding:0}.markdown-preview :deep(a){color:var(--c-primary)}.editor-empty{height:100%;align-content:center}.editor-empty h3{color:var(--c-text-heading);margin:14px 0 4px}.editor-empty p{margin:0 0 18px}
.note-card{width:calc(100% - var(--tree-depth,0) * 12px);margin-left:calc(var(--tree-depth,0) * 12px);position:relative}
.child-button{display:flex;align-items:center;gap:3px;border:1px solid var(--c-border);background:transparent;color:var(--c-text);border-radius:7px;padding:6px 8px;cursor:pointer;font-size:11px}.child-button svg{width:12px}
.markdown-source{min-width:0;height:100%;overflow:hidden;padding:0;font-family:inherit}
.markdown-rich{min-width:0;height:100%;overflow:hidden}
.mode-split .markdown-rich{border-right:1px solid var(--c-border)}
.markdown-preview :deep(table){border-collapse:collapse;margin:14px 0;width:100%}
.markdown-preview :deep(th),.markdown-preview :deep(td){border:1px solid var(--c-border);padding:7px 10px;text-align:left}
.markdown-preview :deep(th){background:var(--c-bg-subtle);font-weight:600}
.markdown-preview :deep(span[data-wiki-link]){color:var(--c-primary);background:color-mix(in srgb,var(--c-primary) 9%,transparent);border-bottom:1px dashed var(--c-primary);border-radius:4px;padding:0 4px;cursor:pointer}
.markdown-preview :deep(hr){border:0;border-top:1px solid var(--c-border);margin:20px 0}
.markdown-preview :deep(mark){background:color-mix(in srgb,var(--c-primary) 22%,transparent);border-radius:3px;padding:0 2px}
.knowledge-info-rail{min-width:0;min-height:0;overflow:auto;display:flex;flex-direction:column;border-left:1px solid var(--c-border);background:var(--c-bg-card)}
.knowledge-info-rail>.history-panel{height:auto;min-height:0;flex:1}
.knowledge-info-rail>.info-tabs{flex-shrink:0}
.info-tabs{height:45px;display:grid;grid-template-columns:repeat(4,1fr);border-bottom:1px solid var(--c-border);padding:0 6px}
.info-tabs button{display:flex;align-items:center;justify-content:center;gap:4px;border:0;border-bottom:2px solid transparent;background:transparent;color:var(--c-text-secondary);font-size:10px;cursor:pointer}.info-tabs button.active{color:var(--c-primary);border-bottom-color:var(--c-primary)}.info-tabs svg{width:12px}
.vault-head{height:62px;display:flex;align-items:center;gap:10px;padding:0 14px;border-bottom:1px solid var(--c-border)}
.vault-head .vault-title{padding:0;flex:1;font-size:16px}
.new-note-icon{width:34px;height:34px;padding:0;margin:0;border-radius:9px}
.new-note-icon svg{width:16px}
.category-strip{display:flex;gap:5px;padding:0 12px 10px;overflow-x:auto;scrollbar-width:none}
.category-strip::-webkit-scrollbar{display:none}
.category-strip button{display:flex;align-items:center;gap:5px;border:1px solid var(--c-border);border-radius:999px;background:var(--c-bg-page);color:var(--c-text-secondary);padding:5px 8px;font-size:12px;white-space:nowrap;cursor:pointer}
.category-strip button.active{border-color:color-mix(in srgb,var(--c-primary) 45%,var(--c-border));background:color-mix(in srgb,var(--c-primary) 9%,var(--c-bg-card));color:var(--c-primary);font-weight:700}
.category-strip b{font-size:9px;font-weight:600;opacity:.7}
.list-utilities{display:grid;grid-template-columns:1fr 1fr;gap:6px;padding:10px;border-top:1px solid var(--c-border)}
.list-utilities button{display:flex;align-items:center;justify-content:center;gap:5px;border:0;border-radius:7px;background:transparent;color:var(--c-text-secondary);padding:7px;font-size:12px;cursor:pointer}
.list-utilities button:hover{background:var(--c-bg-subtle);color:var(--c-text)}
.list-utilities svg{width:12px}
.editor-document{width:calc(100% - 28px);max-width:1120px;min-height:0;flex:1;margin:14px auto;border:1px solid var(--c-border);border-radius:12px;background:var(--c-bg-card);box-shadow:var(--shadow-sm);overflow:hidden;display:flex;flex-direction:column}
.export-button{display:flex;align-items:center;gap:5px;border:1px solid var(--c-border);background:var(--c-bg-card);color:var(--c-text);border-radius:7px;padding:6px 9px;cursor:pointer;font-size:11px;font-weight:600;white-space:nowrap}
.export-button:hover{border-color:color-mix(in srgb,var(--c-primary) 45%,var(--c-border));color:var(--c-primary)}
.export-button:disabled{opacity:.55;cursor:wait}
.export-button svg{width:13px}
.outline-panel{padding:14px 9px}
.outline-heading{padding:0 8px 9px;font-size:11px;font-weight:750;color:var(--c-text-heading)}
.outline-panel button{width:100%;display:block;border:0;border-radius:6px;background:transparent;color:var(--c-text-secondary);padding-top:6px;padding-right:8px;padding-bottom:6px;text-align:left;font-size:11px;line-height:1.35;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;cursor:pointer}
.outline-panel button:hover{background:var(--c-bg-subtle);color:var(--c-primary)}
.outline-empty{padding:12px 8px;color:var(--c-text-secondary);font-size:10px;line-height:1.6}
@media(max-width:1380px){.notebook-shell{grid-template-columns:260px minmax(440px,1fr) 240px}.editor-toolbar{padding-left:10px;padding-right:10px;gap:6px}.child-button{font-size:0}.child-button svg{width:13px}.note-fields{padding-left:18px;padding-right:18px}.mode-switch button{font-size:12px}.mode-switch svg{width:14px}}
@media(max-width:980px){.notebook-shell{grid-template-columns:240px minmax(430px,1fr)}.knowledge-info-rail{display:none}}
.mobile-notebook-nav{display:none;gap:4px;padding:4px 12px;border-bottom:1px solid var(--c-border);background:var(--c-bg-card)}
.mobile-notebook-nav button{flex:1;border:0;padding:5px 12px;background:transparent;color:var(--c-text-secondary);border-radius:var(--c-radius);font:inherit;font-size:12px;cursor:pointer}
.mobile-notebook-nav button.active{background:var(--c-primary-light);color:var(--c-primary);font-weight:600}
@media(max-width:1180px){
  .notebook-shell{grid-template-columns:minmax(0,1fr);grid-template-rows:40px minmax(0,1fr);min-height:0}
  .mobile-notebook-nav{display:flex;gap:4px;padding:4px 8px;border-bottom:1px solid var(--c-border)}
  .mobile-notebook-nav button{flex:1;border:0;background:transparent;color:var(--c-text);border-radius:4px}
  .mobile-notebook-nav button.active{background:var(--c-bg-subtle);font-weight:600}
  .notebook-shell>.note-list-panel,.notebook-shell>.editor-panel,.notebook-shell>.knowledge-info-rail{display:none;min-height:0}
  .mobile-notes>.note-list-panel,.mobile-editor>.editor-panel,.mobile-info>.knowledge-info-rail{display:flex}
  .editor-toolbar{height:auto;min-height:56px;flex-wrap:wrap;padding:8px;gap:8px}
  .mode-switch{order:2;flex-basis:100%;justify-content:space-between}
  .mode-switch button{padding:6px;font-size:12px}
  .note-fields{padding:16px}
  .property-row{gap:10px}
  .property-row label{flex:1;min-width:0}
  .property-row select,.property-row input{min-width:0;width:100%;box-sizing:border-box}
  .markdown-workspace.mode-split{grid-template-columns:minmax(0,1fr)}
  .markdown-preview{padding:16px}
  .knowledge-info-rail{border-left:0}
}
@media(max-width:500px){
  .property-row{display:grid;grid-template-columns:repeat(2,minmax(0,1fr))}
}
@container (min-width: 1180px) {
  .notebook-shell { grid-template-columns: 260px minmax(0,1fr) 240px; }
}
@container (min-width: 880px) and (max-width: 1179px) {
  .notebook-shell { grid-template-columns: 240px minmax(0,1fr); grid-template-rows: 40px minmax(0,1fr); }
  .mobile-notebook-nav { display: flex; grid-column: 1 / -1; }
  .notebook-shell > .note-list-panel { display: flex; grid-column: 1; grid-row: 2; min-height: 0; }
  .notebook-shell > .editor-panel { display: flex; grid-column: 2; grid-row: 2; min-height: 0; }
  .notebook-shell > .knowledge-info-rail { display: none; }
  .notebook-shell.mobile-info > .editor-panel { display: none; }
  .notebook-shell.mobile-info > .knowledge-info-rail { display: flex; grid-column: 2; grid-row: 2; }
  .editor-toolbar { height: auto; min-height: 56px; flex-wrap: wrap; padding: 8px; }
  .mode-switch { order: 2; flex-basis: 100%; justify-content: space-between; }
}
</style>

<style scoped>
.writing-focus-button { color: var(--c-text-secondary); background: transparent; border: 1px solid var(--c-border); border-radius: 5px; padding: 6px 9px; font-size: 11px; cursor: pointer; white-space: nowrap; }
.writing-focus-button[aria-pressed=true] { color: var(--c-primary); background: var(--c-primary-light); }
.note-properties summary { cursor: pointer; font-size: 11px; color: var(--c-text-secondary); padding: 10px 0; }
.note-properties summary span { margin-left: 12px; opacity: .8; }
.note-properties .property-row { padding: 8px 0 12px; }
.notebook-shell.writing-focus { grid-template-columns: minmax(0, 1fr); }
.writing-focus .note-list-panel, .writing-focus .knowledge-info-rail, .writing-focus .mobile-notebook-nav, .writing-focus .case-chip { display: none !important; }
.writing-focus .editor-panel { display: flex !important; grid-column: 1 !important; }
.writing-focus .editor-document { width: min(900px, 100%); margin: 0 auto; padding-top: 20px; border: 0; box-shadow: none; }
.writing-focus .editor-toolbar { justify-content: flex-end; }
.writing-focus .save-state { margin-right: auto; }
.writing-focus :deep(.editor-toolbar-shell) { opacity: .6; }
.writing-focus :deep(.tiptap) { font-size: 17px; line-height: 1.95; }
.note-related-work { padding: 0 16px 20px; min-width: 0; overflow: auto; }
.note-related-work > p { font-size: 12px; color: var(--c-text-secondary); line-height: 1.8; }
.note-related-work :deep(nav) { flex-wrap: wrap; }
.info-tabs { flex-wrap: wrap; }
</style>
<style scoped>
.writing-focus :deep(.document-toolbar) { display: none; }
.writing-focus .mode-switch { max-width: 400px; }
.writing-focus .title-editor { padding-top: 24px; }
</style>
