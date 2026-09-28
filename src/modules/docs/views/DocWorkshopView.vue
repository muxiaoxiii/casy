<template>
  <SaveConflictDialog ref="conflictDialog" :local="localConflict" :latest="latestConflict" :copy="copyConflict" :apply="applyConflict" :prepare="() => legalEditorRef?.getHtml()" />
  <el-alert v-if="conflicted" title="文书存在保存冲突，本地修改尚未覆盖原文书" type="warning" :closable="false"><el-button @click="conflictDialog?.open()">处理冲突</el-button></el-alert>
  <div class="doc-workshop" :class="'mobile-pane-' + mobilePane">
    <DraftHistoryDialog v-model="historyOpen" :draft="currentDraft" :before-restore="saveDraft" @restored="onVersionRestored" />
    <nav class="draft-mobile-nav" aria-label="文书工作区">
      <button :aria-pressed="mobilePane === 'list'" @click="mobilePane = 'list'">草稿与模板</button>
      <button :aria-pressed="mobilePane === 'editor'" @click="mobilePane = 'editor'">编辑文书</button>
    </nav>
    <!-- 左侧面板 -->
    <div class="draft-sidebar">
      <div class="sidebar-tabs">
        <button type="button"
          :class="['tab-item', { active: activeTab === 'drafts' }]"
          @click="activeTab = 'drafts'"
        >
          {{ $t('docs.drafts') }}
        </button>
        <button type="button"
          :class="['tab-item', { active: activeTab === 'templates' }]"
          @click="activeTab = 'templates'"
        >
          {{ $t('docs.templates') }}
        </button>
      </div>

      <!-- 草稿列表 -->
      <template v-if="activeTab === 'drafts'">
        <div class="draft-header">
          <h3>{{ $t('docs.drafts') }}</h3>
          <button class="btn-new-draft" :disabled="creatingDraft" @click="createNewDraft">
            + {{ $t('common.create') }}
          </button>
        </div>

        <el-input
          v-model="searchText"
          :placeholder="$t('common.search')"
          clearable
          size="small"
          class="draft-search"
        />

        <div class="draft-list" v-loading="loading">
          <el-alert v-if="loadError" :title="loadError" type="error" :closable="false"><el-button text @click="loadDrafts">重试</el-button></el-alert>
          <div
            v-for="draft in filteredDrafts"
            :key="draft.id"
            :class="['draft-item', { active: currentDraftId === draft.id }]"
            role="button" tabindex="0" :aria-label="`打开文书 ${draft.title || '未命名文书'}`"
            @keydown.enter.self="selectDraft(draft.id)" @keydown.space.prevent.self="selectDraft(draft.id)"
            @click="selectDraft(draft.id)"
            @contextmenu="showContextMenu($event, draft.title || '未命名文书', [{ label: '打开文书', run: () => selectDraft(draft.id) }, { label: '删除文书', danger: true, separator: true, run: () => deleteDraft(draft.id) }])"
          >
            <div class="draft-title">{{ draft.title || '未命名文书' }}</div>
            <div class="draft-meta">
              <span class="draft-status" :class="draft.status">
                {{ statusLabel(draft.status) }}
              </span>
              <span class="draft-time">{{ formatTime(draft.updatedAt) }}</span>
            </div>
            <el-button
              class="draft-delete"
              :disabled="!!deletingDraftId" :loading="deletingDraftId === draft.id"
              size="small"
              type="danger"
              text
              @click.stop="deleteDraft(draft.id)"
            >
              删除
            </el-button>
          </div>

          <el-empty
            v-if="!loading && !loadError && filteredDrafts.length === 0"
            :description="searchText ? '没有匹配的草稿' : '暂无草稿'"
            :image-size="60"
          />
        </div>
      </template>

      <!-- 模板浏览器 -->
      <template v-else-if="activeTab === 'templates'">
        <TemplateBrowser @select="onTemplateSelect" />
      </template>
    </div>

    <!-- 右侧编辑器面板 -->
    <div class="editor-panel">
      <template v-if="currentDraft">
        <div class="editor-header">
          <input
            v-model="currentDraft.title"
            :placeholder="$t('docs.draft_title')"
            class="notion-title-input"
            @input="scheduleSave"
          />
          <div class="editor-actions">
            <el-select
              v-model="currentDraft.status"
              size="small"
              @change="scheduleSave"
              style="width: 100px"
            >
              <el-option :label="$t('docs.status_draft')" value="draft" />
              <el-option :label="$t('docs.status_final')" value="final" />
              <el-option :label="$t('docs.status_archived')" value="archived" />
            </el-select>
            <el-select
              v-model="currentDraft.caseId"
              filterable
              clearable
              size="small"
              placeholder="关联案件..."
              @change="scheduleSave"
              style="width: 220px"
            >
              <el-option
                v-for="c in cases"
                :key="c.id"
                :label="c.caseName || c.caseNo || '未命名案件'"
                :value="c.id"
              />
            </el-select>
            <el-dropdown trigger="click" :disabled="exporting" @command="exportToDocx"><el-button type="primary" size="small" :loading="exporting">导出</el-button><template #dropdown><el-dropdown-menu><el-dropdown-item command="md">Markdown（.md）</el-dropdown-item><el-dropdown-item command="pdf">PDF 文档（.pdf）</el-dropdown-item><el-dropdown-item command="docx">Word 文档（.docx）</el-dropdown-item></el-dropdown-menu></template></el-dropdown>
            <el-button size="small" @click="openHistory">历史版本</el-button>
            <el-button :type="showTypesetPreview ? 'primary' : 'default'" plain size="small" @click="showTypesetPreview = !showTypesetPreview">A4 预览</el-button>
            <el-button plain size="small" @click="openEvidenceLinkPicker">
              <el-icon><Link /></el-icon> 证据链接
            </el-button>
            <el-button plain size="small" @click="showKnowledgeSidebar = !showKnowledgeSidebar">
              <el-icon><Collection /></el-icon> 知识抽屉
            </el-button>
          </div>
        </div>

        <!-- 编辑与真实分页共享同一份结构化文档 -->
        <div class="document-workspace" :class="{'with-preview':showTypesetPreview}">

        <LegalEditor
          :key="currentDraft.id"
          ref="legalEditorRef"
          v-model="currentDraft.content"
          :case-data="linkedCaseData"
          :all-cases="cases"
          :case-id="currentDraft.caseId"
          :source-id="currentDraft.id"
          @update:model-value="scheduleSave"
          @document-change="previewDocument = $event"
          @active-block="activeBlock = $event"
          @open-knowledge-drawer="showKnowledgeSidebar = true"
        />

        <TypesetPreview v-if="showTypesetPreview" :key="currentDraft.id" :document="previewDocument" :layout="layoutOptions" :active-block="activeBlock"
          @update:layout="layoutPreferences=$event" @select-block="legalEditorRef?.scrollToBlock($event)" />
        </div>
        <div class="editor-statusbar">
          <span>{{ $t('docs.word_count', { count: wordCount }) }}</span>
          <span :class="['save-status', saveStatus]">{{ saveStatusText }}</span>
          <span v-if="currentDraft.updatedAt">
            {{ $t('docs.last_saved') }} {{ formatTime(currentDraft.updatedAt) }}
          </span>
        </div>
      </template>

      <div v-else class="no-draft">
        <el-empty :description="$t('docs.new_draft')" :image-size="80">
          <el-button type="primary" @click="createNewDraft">{{ $t('docs.new_draft') }}</el-button>
        </el-empty>
      </div>
    </div>

    <!-- 右侧悬浮知识侧边栏 -->
    <div class="knowledge-drawer-container" v-show="showKnowledgeSidebar">
      <KnowledgeSidebar @close="showKnowledgeSidebar = false" />
    </div>
  </div>
  <ContextMenu v-bind="contextMenu" @close="contextMenu.open = false" />
</template>

<script setup>
import { useViewMemory } from '../../../composables/useViewMemory'
import SaveConflictDialog from '../../../shared/components/SaveConflictDialog.vue'
import { formatTimestamp, relativeTimestamp } from '../../../shared/utils/date'
import ContextMenu from "../../../shared/components/ContextMenu.vue"
import { useContextActions } from "../../../shared/composables/useContextActions"
const { contextMenu, showContextMenu } = useContextActions()

import TypesetPreview from '../../../shared/editor/TypesetPreview.vue'
import {exportDocument} from '../../../shared/editor/exportDocument'
import DraftHistoryDialog from '../components/DraftHistoryDialog.vue'
import { useSaveBeforeLeave } from '../../../composables/useSaveBeforeLeave'
import { useDraftRecovery } from '../../../composables/useDraftRecovery'
import { useRoute } from 'vue-router'
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { observeChanges } from '../../../core/observeChanges'
import { casyContext } from '../../../core/plugin/context'
import { Collection, Link } from '../../../shared/icons'
import LegalEditor from '../components/LegalEditor.vue'
import KnowledgeSidebar from '../../knowledge/components/KnowledgeSidebar.vue'
import TemplateBrowser from './TemplateBrowser.vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import debounce from 'lodash-es/debounce'

const showKnowledgeSidebar = ref(false)
const showTypesetPreview = ref(false)
const previewDocument = ref(null)
const activeBlock = ref(0)
const layoutPreferences = ref({marginMm:20,bindingMm:5,firstLineIndent:true,header:true,skipFirstHeader:true})
const layoutOptions = computed(()=>({...layoutPreferences.value,title:currentDraft.value?.title || '',caseNo:linkedCaseData.value?.caseNo || ''}))

// 证据链接（W4 双链）：转发到 LegalEditor 暴露的选择器
const legalEditorRef = ref(null)
function openEvidenceLinkPicker() {
  legalEditorRef.value?.openEvidenceLinkPicker()
}

const route = useRoute()
const props = defineProps([])
const drafts = ref([])
const cases = ref([])
const currentDraftId = ref(null)
const mobilePane = ref('list')
watch(currentDraftId, id => { if (id) mobilePane.value = 'editor' })
const currentDraft = ref(null)
const loading = ref(false)
const creatingDraft = ref(false)
const loadError = ref('')
const searchText = ref('')
const saveStatus = ref('idle') // idle | saving | saved | error
const saveTimer = ref(null)
const activeTab = ref('drafts') // drafts | templates
useViewMemory('docs', { searchText, activeTab, currentDraftId }, ["#main-content", ".draft-list"])
const exporting = ref(false)
const historyOpen = ref(false)
async function openHistory() { if (await saveDraft()) historyOpen.value = true }
function onVersionRestored(draft) {
  if (draft.id !== currentDraftId.value) return
  currentDraft.value = draft
  savedRevision = editRevision
  saveStatus.value = 'saved'
  void loadDrafts()
}

// 过滤草稿列表
const filteredDrafts = computed(() => {
  const keyword = searchText.value.toLowerCase()
  if (!keyword) return drafts.value
  return drafts.value.filter(d =>
    (d.title || '').toLowerCase().includes(keyword)
  )
})

// 关联案件数据
const linkedCaseData = computed(() => {
  if (!currentDraft.value?.caseId) return {}
  return cases.value.find(c => c.id === currentDraft.value.caseId) || {}
})

// 字数统计
const wordCount = computed(() => {
  if (!currentDraft.value?.content) return 0
  const text = currentDraft.value.content.replace(/<[^>]*>/g, '').trim()
  return text.length
})

const saveStatusText = computed(() => {
  switch (saveStatus.value) {
    case 'saving': return '● 正在保存...'
    case 'saved': return '✓ 已保存至本地数据库'
    case 'error': return '✕ 保存失败'
    default: return ''
  }
})

// 加载草稿列表
async function loadDrafts() {
  loading.value = true
  loadError.value = ''
  try {
    const result = await casyContext.docs.listDrafts()
    if (!result.ok) throw new Error(result.error || '草稿列表加载失败')
    drafts.value = result.data || []
  } catch (error) { loadError.value = String(error) }
  finally { loading.value = false }
}

// 加载案件列表
async function loadCases() {
  const result = await casyContext.cases.list({ page: 1, perPage: 500 })
  if (result.ok) {
    cases.value = Array.isArray(result.data) ? result.data : (result.data?.items || [])
  }
}

// 选择草稿
let selectionRevision = 0
let selectingId = null
async function selectDraft(id) {
  if (deletingDraftId.value === id) return
  if (currentDraft.value?.id === id || selectingId === id) return
  const selection = ++selectionRevision
  if (currentDraft.value && !(await saveDraft())) return
  if (selection !== selectionRevision) return
  selectingId = id
  const result = await casyContext.docs.getDraft(id)
  if (selection !== selectionRevision) return
  selectingId = null
  if (result.ok) {
    currentDraftId.value = id
    currentDraft.value = result.data
    saveStatus.value = 'idle'
  } else ElMessage.error(result.error || '文书加载失败')
}

// 新建草稿（开箱即写）
async function createNewDraft() {
  if (creatingDraft.value) return
  creatingDraft.value = true
  try {
    if (!(await saveDraft())) return
    const result = await casyContext.docs.createDraft({ title: '未命名法律文书', content: '' })
    if (!result.ok || !result.data) { ElMessage.error(result.error || '创建文书失败'); return }
    drafts.value = [result.data, ...drafts.value]
    await selectDraft(result.data.id)
  } catch (error) { ElMessage.error(String(error)) }
  finally { creatingDraft.value = false }
}

// 保存草稿
let savePending = null
const recovery = useDraftRecovery()
let editRevision = 0
let savedRevision = 0
useSaveBeforeLeave(() => editRevision !== savedRevision || saveStatus.value === 'saving' || legalEditorRef.value?.hasSourceDraft() || legalEditorRef.value?.hasPendingSerialize?.(), saveDraft)
const conflictDialog = ref(null)
const conflicted = ref(false)
const localConflict = () => ({ ...currentDraft.value, title: currentDraft.value?.title || '', content: currentDraft.value?.content || '' })
async function latestConflict() {
  const result = await casyContext.docs.getDraft(localConflict().id)
  if (!result.ok || !result.data) throw new Error(result.error || '读取最新文书失败')
  return result.data
}
async function copyConflict(snapshot) {
  const result = await casyContext.docs.createDraft({ title: `${snapshot.title}（本地冲突副本）`, content: snapshot.content, caseId: snapshot.caseId || null })
  if (!result.ok) throw new Error(result.error || '副本保存失败')
  void loadDrafts()
}
function applyConflict(value) {
  currentDraft.value = value; currentDraftId.value = value.id; saveStatus.value = 'saved'; void loadDrafts()
  savedRevision = editRevision
  conflicted.value = false
  void recovery.clear()
}
function saveDraft(commitSources = true) {
  try { if(commitSources) legalEditorRef.value?.getHtml() } catch(error) { ElMessage.error(String(error)); return Promise.resolve(false) }
  if (conflicted.value) { if (commitSources !== false) conflictDialog.value?.open(); return Promise.resolve(false) }
  if (savePending) return savePending
  if (!currentDraft.value) return Promise.resolve(true)
  // 防抖未落盘时先刷出最新正文，避免离开守卫误判干净。
  if (legalEditorRef.value?.hasPendingSerialize?.()) legalEditorRef.value.getHtml()
  if (savedRevision === editRevision) return Promise.resolve(true)
  if (saveTimer.value) clearTimeout(saveTimer.value)
  savePending = (async () => {
  try {
  while (editRevision !== savedRevision) {
  const revision = editRevision
  const snapshot = { ...currentDraft.value }

  saveStatus.value = 'saving'
  const result = await casyContext.docs.updateDraft(snapshot.id, {
    title: snapshot.title,
    content: snapshot.content,
    status: snapshot.status,
    caseId: snapshot.caseId || null,
    expectedVersion: snapshot.version,
  })

  if (result.ok) {
    currentDraft.value.version = result.data.version
    currentDraft.value.updatedAt = result.data.updatedAt
    savedRevision = revision
    saveStatus.value = 'saved'
    const idx = drafts.value.findIndex(d => d.id === currentDraft.value.id)
    if (idx >= 0) {
      drafts.value[idx] = { ...drafts.value[idx], ...snapshot, version: result.data.version, updatedAt: result.data.updatedAt }
    }
    setTimeout(() => {
      if (saveStatus.value === 'saved') saveStatus.value = 'idle'
    }, 2500)
  } else {
    saveStatus.value = 'error'
    if (result.error?.includes('EDIT_CONFLICT')) { conflicted.value = true; conflictDialog.value?.open() }
    else ElMessage.error(result.error || '文书保存失败')
    return false
  }
  }
  await recovery.clear()
  return true
  } catch (error) { saveStatus.value = 'error'; ElMessage.error(String(error)); return false }
  finally { savePending = null }
  })()
  return savePending
}

function scheduleSave() {
  if (currentDraft.value) recovery.checkpoint({ id: currentDraft.value.id, title: currentDraft.value.title, content: currentDraft.value.content || '' })
  editRevision++
  if (saveTimer.value) clearTimeout(saveTimer.value)
  saveTimer.value = setTimeout(() => {
    saveDraft(false)
  }, 1500)
}

// 删除其他草稿不切走当前编辑器；删除当前稿先等正在保存的请求结束。
const deletingDraftId = ref(null)
async function deleteDraft(id) {
  if (deletingDraftId.value) return
  deletingDraftId.value = id
  try {
    try { await ElMessageBox.confirm('删除文书将同时删除其历史版本，无法撤销。', '删除文书', { type: 'warning', confirmButtonText: '删除', cancelButtonText: '保留' }) } catch { return }
    if (currentDraftId.value === id && !(await saveDraft())) return
    const result = await casyContext.docs.deleteDraft(id)
    if (!result.ok) { ElMessage.error(result.error || '删除失败，文书已保留'); return }
    const deletedCurrent = currentDraftId.value === id
    if (deletedCurrent) {
      ++selectionRevision
      selectingId = null
      if (saveTimer.value) clearTimeout(saveTimer.value)
      currentDraftId.value = null
      currentDraft.value = null
      savedRevision = editRevision
      saveStatus.value = 'idle'
      await recovery.clear()
    }
    drafts.value = drafts.value.filter(draft => draft.id !== id)
    ElMessage.success('文书已删除')
    if (deletedCurrent && drafts.value.length) await selectDraft(drafts.value[0].id)
  } catch (error) { ElMessage.error(String(error)) }
  finally { deletingDraftId.value = null }
}

// 导出为 Docx
async function exportToDocx(format='docx') {
 if(!currentDraft.value || exporting.value)return
 exporting.value=true
 try{if(!await saveDraft())return;const path=await exportDocument({content:currentDraft.value.content || '',contentFormat:'html',title:currentDraft.value.title,format,document:legalEditorRef.value?.getDocumentJson?.(),layout:layoutOptions.value});if(path)ElMessage.success(`文档已保存：${path}`)}catch(error){ElMessage.error(String(error))}finally{exporting.value=false}
}

const formatTime = relativeTimestamp

function statusLabel(status) {
  switch (status) {
    case 'draft': return '草稿'
    case 'final': return '定稿'
    case 'archived': return '已归档'
    default: return status
  }
}

onMounted(async () => {
  await recovery.recover()
  await Promise.all([loadDrafts(), loadCases()])
  if (drafts.value.length > 0) {
    selectDraft(typeof route.query.select === 'string' ? route.query.select : drafts.value.find(item => item.id === currentDraftId.value)?.id || drafts.value[0].id)
  } else if (!loadError.value) {
    await createNewDraft()
  }
})

watch(() => route.query.select, id => { if (typeof id === 'string') void selectDraft(id) })

// 模板选择回调
async function onTemplateSelect(template) {
  if (creatingDraft.value) return
  creatingDraft.value = true
  try {
    if (!(await saveDraft())) return
    const result = await casyContext.docs.createDraft({
      title: template.name,
      content: `<p>基于模板 <strong>${template.name}</strong> 创建</p>`,
      templatePath: template.path,
    })
    if (!result.ok || !result.data) { ElMessage.error(result.error || '根据模板创建文书失败'); return }
    drafts.value = [result.data, ...drafts.value]
    await selectDraft(result.data.id)
    activeTab.value = 'drafts'
  } catch (error) { ElMessage.error(String(error)) }
  finally { creatingDraft.value = false }
}

onUnmounted(() => {
  if (saveTimer.value) clearTimeout(saveTimer.value)
})
onUnmounted(observeChanges(casyContext, ['doc', 'case'], async () => { await Promise.all([loadDrafts(), loadCases()]) }))
</script>

<style scoped>
.document-workspace{display:grid;grid-template-columns:minmax(0,1fr);flex:1;min-height:0;overflow:hidden;}
.document-workspace.with-preview{grid-template-columns:minmax(0,1fr) minmax(300px,1fr);}
.document-workspace>.notion-legal-editor-shell{min-width:0;min-height:0;}
@media(max-width:1100px){.document-workspace.with-preview{grid-template-columns:minmax(0,1fr);grid-template-rows:minmax(220px,1fr) minmax(220px,1fr);}}

.doc-workshop {
  display: flex;
  height: calc(100dvh - var(--app-topbar-height));
  background: var(--c-bg-page);
  overflow: hidden;
}

.draft-sidebar {
  width: 260px;
  background: var(--c-bg-sidebar);
  border-right: 1px solid var(--c-border);
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
}

.sidebar-tabs {
  display: flex;
  border-bottom: 1px solid var(--c-border);
  background: var(--c-bg-subtle);
}

.tab-item {
  border: 0;
  background: transparent;
  font-family: inherit;
  flex: 1;
  text-align: center;
  padding: 10px 0;
  font-size: 13px;
  font-weight: 500;
  color: var(--c-text-secondary);
  cursor: pointer;
  border-bottom: 2px solid transparent;
  transition: all var(--motion-fast);
}

.tab-item.active {
  color: var(--c-primary);
  font-weight: 600;
  border-bottom-color: var(--c-primary);
  background: var(--c-bg-sidebar);
}

.draft-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 16px 8px;
}

.draft-header h3 {
  margin: 0;
  font-size: 14px;
  font-weight: 700;
  color: var(--c-text-heading);
}

.btn-new-draft {
  padding: 4px 10px;
  border-radius: var(--c-radius-md);
  border: 1px solid var(--c-border);
  background: var(--c-bg-card);
  color: var(--c-primary);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: all var(--motion-fast);
}

.btn-new-draft:hover {
  background: var(--c-primary-light);
  border-color: var(--c-primary);
}

.draft-search {
  padding: 0 14px 8px;
}

.draft-list {
  flex: 1;
  overflow-y: auto;
  padding: 4px 8px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.draft-item {
  position: relative;
  padding: 10px 12px;
  border-radius: var(--c-radius-lg);
  cursor: pointer;
  transition: all var(--motion-fast);
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.draft-item:hover {
  background: var(--c-bg-hover);
}

.draft-item.active {
  background: var(--c-bg-selected);
}

.draft-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--c-text-heading);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  padding-right: 28px;
}

.draft-item.active .draft-title {
  color: var(--c-primary);
}

.draft-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 11px;
}

.draft-status {
  padding: 1px 5px;
  border-radius: 3px;
  font-weight: 500;
  background: var(--c-bg-subtle);
  color: var(--slate-gray-light);
}

.draft-status.final {
  background: var(--bg-success-weak);
  color: var(--status-success);
}

.draft-time {
  color: var(--slate-gray-light);
}

.draft-delete {
  position: absolute;
  right: 8px;
  top: 8px;
  opacity: 0;
  transition: opacity var(--motion-fast);
}

.draft-item:hover .draft-delete {
  opacity: 1;
}

.editor-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  background: var(--c-bg-card);
  min-width: 0;
}

.editor-header {
  padding: 16px 36px 12px;
  border-bottom: 1px solid var(--c-border);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  background: var(--c-bg-card);
}

.notion-title-input {
  flex: 1;
  border: none;
  background: transparent;
  outline: none;
  font-size: 20px;
  font-weight: 700;
  color: var(--c-text-heading);
  letter-spacing: -0.3px;
}

.notion-title-input::placeholder {
  color: var(--slate-gray-light);
  opacity: 0.6;
}

.editor-actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

.editor-statusbar {
  padding: 8px 36px;
  border-top: 1px solid var(--c-border);
  background: var(--c-bg-subtle);
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 11.5px;
  color: var(--slate-gray-light);
}

.editor-statusbar span {
  margin-left: 16px;
}

.save-status.saved {
  color: var(--el-color-success);
}

.save-status.saving {
  color: var(--el-color-primary);
}

.save-status.error {
  color: var(--status-risk);
}

.knowledge-drawer-container {
  height: 100vh;
  position: relative;
  flex-shrink: 0;
  transition: all 0.3s ease;
  z-index: 10;
}

.no-draft {
  flex: 1;
  display: grid;
  place-items: center;
}
</style>
<style scoped>
.document-workspace{display:grid;grid-template-columns:minmax(0,1fr);flex:1;min-height:0;overflow:hidden;}
.document-workspace.with-preview{grid-template-columns:minmax(0,1fr) minmax(300px,1fr);}
.document-workspace>.notion-legal-editor-shell{min-width:0;min-height:0;}
@media(max-width:1100px){.document-workspace.with-preview{grid-template-columns:minmax(0,1fr);grid-template-rows:minmax(220px,1fr) minmax(220px,1fr);}}

.editor-header{flex-direction:column;align-items:stretch;padding:18px 24px 14px;gap:12px}.notion-title-input{width:100%;min-width:0;box-sizing:border-box}.editor-actions{flex-wrap:wrap;gap:8px}.editor-actions .el-button{margin-left:0}.editor-panel{min-height:0}.doc-workshop{height:100%;min-height:0}.draft-sidebar{width:240px;flex-shrink:0}@media(max-width:1000px){.draft-sidebar{width:205px}.editor-header{padding:14px 16px}.editor-statusbar{padding:8px 16px;flex-wrap:wrap;gap:6px}}
.draft-mobile-nav { display: none; }
@container (max-width: 700px) {
  .doc-workshop { flex-direction: column; }
  .draft-mobile-nav { display: flex; gap: 4px; padding: 6px 12px; border-bottom: 1px solid var(--c-border); flex-shrink: 0; background: var(--c-bg-card); }
  .draft-mobile-nav button { flex: 1; border: 0; padding: 8px; background: transparent; color: var(--c-text-secondary); border-radius: var(--c-radius); font: inherit; font-size: 13px; cursor: pointer; }
  .draft-mobile-nav button[aria-pressed="true"] { background: var(--c-primary-light); color: var(--c-primary); font-weight: 600; }
  .doc-workshop > .draft-sidebar, .doc-workshop > .editor-panel { display: none; width: 100%; min-width: 0; min-height: 0; flex: 1; }
  .mobile-pane-list > .draft-sidebar, .mobile-pane-editor > .editor-panel { display: flex; }
  .draft-sidebar { border-right: 0; }
  .draft-search { width: auto; }
  .editor-actions :deep(.el-select) { max-width: 100%; }
}
</style>
