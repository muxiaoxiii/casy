<script setup>
import DraftHistoryDialog from '../components/DraftHistoryDialog.vue'
import { useSaveBeforeLeave } from '../../../composables/useSaveBeforeLeave'
import { useDraftRecovery } from '../../../composables/useDraftRecovery'
import { ref, shallowRef, reactive, onMounted, watch, onBeforeUnmount } from 'vue'
import { flushSourceEditors, hasSourceDraft } from '../../../shared/editor/SourceNodeView'
import { useRoute, useRouter } from 'vue-router'
import DocumentEditor from '../../../shared/editor/DocumentEditor.vue'
import {exportDocument} from '../../../shared/editor/exportDocument'
import StarterKit from '@tiptap/starter-kit'
import Underline from '@tiptap/extension-underline'
import Placeholder from '@tiptap/extension-placeholder'
import Highlight from '@tiptap/extension-highlight'
import BlockReference from '../extensions/BlockReference.ts'
import WikiLink from '../extensions/WikiLink.ts'
import WikiLinkSuggestion from '../extensions/WikiLinkSuggestion.ts'
import { useCasesStore } from '../../../stores/cases'
import { casyContext } from '../../../core/plugin/context'
import { ElMessage } from 'element-plus'
import {
  Collection,
  Opportunity,
  Memo,
  Reading,
  QuestionFilled,
  Medal,
  Document
} from '../../../shared/icons'
import CopilotSidebar from '../components/CopilotSidebar.vue'
import { useCopilot } from '../composables/useCopilot.js'

const route = useRoute()
const router = useRouter()
const casesStore = useCasesStore()

// 案件关联
const caseId = ref(route.params.caseId || null)
const caseData = ref(null)
const casesList = ref([])
const loading = ref(false)

// 草稿
const draftId = ref(null)
const recovery = useDraftRecovery()
const historyOpen = ref(false)
const historyDraft = ref(null)
async function openHistory() {
  if (!await saveDraft() || !draftId.value) return
  const result = await casyContext.docs.getDraft(draftId.value)
  if (!result.ok) return ElMessage.error(result.error || '文书读取失败')
  historyDraft.value = result.data
  historyOpen.value = true
}
async function beforeHistoryRestore() {
  if (!await saveDraft()) return false
  if (historyDraft.value) historyDraft.value.version = draftVersion
  return true
}
function onVersionRestored(draft) {
  if (draft.id !== draftId.value) return
  draftVersion = draft.version
  draftTitle.value = draft.title
  documentContent.value = draft.content || ''
  savedRevision = editRevision
}
let draftVersion = undefined
const draftTitle = ref('未命名文档')
const saving = ref(false)
let autoSaveTimer = null
let savePending = null
let editRevision = 0
let savedRevision = 0
useSaveBeforeLeave(() => editRevision !== savedRevision || saving.value || (docEditorRef.value && (hasSourceDraft(editor.value) || docEditorRef.value.hasPendingSerialize?.())), saveDraft)
watch(draftTitle, scheduleAutoSave)

// 右键知识入库
const contextMenu = reactive({ visible: false, x: 0, y: 0, selectedText: '' })
const captureDialog = reactive({
  visible: false,
  capturing: false,
  text: '',
  title: '',
  category: 'reference',
  tags: '',
  lawName: '',
  articleNo: '',
})

// 编辑器
const editor=shallowRef(null)
const docEditorRef=shallowRef(null)
const documentContent=ref('')
const writingExtensions=[BlockReference]
function editorReady(instance){editor.value=instance;instance.on('selectionUpdate',()=>scheduleCopilotSearch(instance))}
const exporting=ref(false)
async function exportWriting(format){if(!editor.value || exporting.value)return;exporting.value=true;try{if(!await saveDraft())return;const path=await exportDocument({content:editor.value.getHTML(),contentFormat:'html',title:draftTitle.value,format,document:editor.value.getJSON()});if(path)ElMessage.success(`文档已保存：${path}`)}catch(error){ElMessage.error(String(error))}finally{exporting.value=false}}

// ---- Copilot Sidebar ----
const {
  searchQuery: copilotQuery,
  searchResults: copilotResults,
  searching: copilotSearching,
  generating: copilotGenerating,
  aiSuggestion: copilotSuggestion,
  aiDialogVisible: copilotDialogVisible,
  aiIntent: copilotIntent,
  expandedItemId: copilotExpandedId,
  searchKnowledge: copilotSearch,
  debouncedSearch: copilotDebouncedSearch,
  searchContext: copilotSearchContext,
  getCategoryLabel: copilotCategoryLabel,
  getCategoryIcon: copilotCategoryIcon,
  insertToEditor: copilotInsert,
  insertCitation: copilotCitation,
  copyContent: copilotCopy,
  toggleExpand: copilotToggleExpand,
  openAiDialog: copilotOpenAiDialog,
  closeAiDialog: copilotCloseAiDialog,
  executeAiWriting: copilotExecute,
} = useCopilot(editor, { style: 'general' })

// Copilot 光标检索防抖
let copilotSearchTimer = null
function scheduleCopilotSearch(ed) {
  if (copilotSearchTimer) clearTimeout(copilotSearchTimer)
  copilotSearchTimer = setTimeout(() => {
    const text = ed.getText().substring(0, 2000)
    copilotSearchContext(text)
  }, 500)
}

// AI 文书风格选择
const aiStyle = ref('general')

// Ctrl+K 快捷键处理
function handleKeydown(e) {
  if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
    e.preventDefault()
    copilotOpenAiDialog()
  }
}

// Copilot 事件处理
async function onCopyKnowledge(item) {
  const ok = await copilotCopy(item)
  if (ok) ElMessage.success('已复制到剪贴板')
}

async function onAiAction(intent) {
  const text = await copilotExecute(intent, aiStyle.value)
  if (text) {
    ElMessage.success('AI 建议已插入编辑器')
  }
}

// 加载案件数据
async function loadCaseData() {
  if (!caseId.value) return
  const result = await casesStore.loadCase(caseId.value)
  if (result.ok) {
    caseData.value = result.data
    if (editor.value) {
      editor.value.storage.caseData = result.data
    }
  }
}

// 加载案件列表供选择
async function loadCasesList() {
  const result = await casyContext.cases.list({ page: 1, perPage: 200 })
  if (result.ok) {
    casesList.value = result.data.items || []
  }
}

// 关联案件
function onCaseSelect(selectedId) {
  caseId.value = selectedId
  if (selectedId) {
    router.replace({ name: 'write', params: { caseId: selectedId } })
    loadCaseData()
  } else {
    caseData.value = null
    router.replace({ name: 'write' })
  }
}

// 自动保存
function scheduleAutoSave() {
  recovery.checkpoint({ id: draftId.value || 'new-document', title: draftTitle.value, content: editor.value?.getHTML() || '' })
  editRevision++
  if (autoSaveTimer) clearTimeout(autoSaveTimer)
  autoSaveTimer = setTimeout(() => saveDraft(false), 900)
}

function saveDraft(commitSources=true) {
  try{if(commitSources!==false && editor.value)flushSourceEditors(editor.value)}catch(error){ElMessage.warning(String(error));return Promise.resolve(false)}
  if (savePending) return savePending
  if (!editor.value) return Promise.resolve(true)
  // 400ms 防抖未落盘时必须先刷出最新正文，否则离开守卫会误判为干净并丢字。
  if (docEditorRef.value?.hasPendingSerialize?.()) docEditorRef.value.flushAndGetMarkdown?.(false)
  if (savedRevision === editRevision) return Promise.resolve(true)
  clearTimeout(autoSaveTimer)
  savePending = (async () => {
  try {
  while (savedRevision !== editRevision) {
  const revision = editRevision
  saving.value = true

  const content = editor.value.getHTML()
  const payload = {
    title: draftTitle.value,
    content,
    caseId: caseId.value || null,
    status: 'draft',
  }

  let result
  if (draftId.value) {
    result = await casyContext.docs.updateDraft(draftId.value, { ...payload, expectedVersion: draftVersion })
  } else {
    result = await casyContext.docs.createDraft(payload)
    if (result.ok && result.data?.id) {
      draftId.value = result.data.id
    }
  }

  if (!result.ok) {
    ElMessage.error(result.error || '文书保存失败，修改仍保留')
    return false
  }
  savedRevision = revision
  draftVersion = result.data?.version
  }
  await recovery.clear()
  return true
  } catch (error) { ElMessage.error(String(error)); return false }
  finally { saving.value = false; savePending = null }
  })()
  return savePending
}

// 加载已有草稿
async function loadDraft(id) {
  const result = await casyContext.docs.getDraft(id)
  if (result.ok && result.data) {
    draftId.value = result.data.id
    draftVersion = result.data.version
    draftTitle.value = result.data.title || '未命名文档'
    if (result.data.caseId) {
      caseId.value = result.data.caseId
      loadCaseData()
    }
    documentContent.value = result.data.content || ''
  }
}

// ---- 右键知识入库 ----

function handleContextMenu(e) {
  if (!editor.value) return
  const { state } = editor.value
  const { from, to } = state.selection
  if (from === to) return

  const selectedText = state.doc.textBetween(from, to, ' ')
  if (!selectedText.trim()) return

  e.preventDefault()
  contextMenu.selectedText = selectedText.trim()
  contextMenu.x = e.clientX
  contextMenu.y = e.clientY
  contextMenu.visible = true
}

function hideContextMenu() {
  contextMenu.visible = false
}

function captureAs(category) {
  captureDialog.text = contextMenu.selectedText
  captureDialog.title = contextMenu.selectedText.substring(0, 50)
  captureDialog.category = category
  captureDialog.tags = ''
  captureDialog.lawName = ''
  captureDialog.articleNo = ''
  captureDialog.visible = true
  hideContextMenu()
}

async function doCapture() {
  if (!captureDialog.text) return
  captureDialog.capturing = true

  const result = await casyContext.knowledge.create({
    title: captureDialog.title,
    category: captureDialog.category,
    content: captureDialog.text,
    tags: captureDialog.tags || null,
    sourceType: 'editor',
    sourceId: draftId.value || null,
    linkedCaseId: caseId.value || null,
    lawName: captureDialog.lawName || null,
    articleNo: captureDialog.articleNo || null,
    status: 'current',
  })

  captureDialog.capturing = false

  if (result.ok) {
    ElMessage.success('知识已入库')
    captureDialog.visible = false
  } else {
    ElMessage.error(result.error || '入库失败')
  }
}

// 插入案件字段
function insertCaseField(field) {
  if (!editor.value || !caseData.value) return
  const value = caseData.value[field] || ''
  editor.value.chain().focus().insertContent(value).run()
}

const caseFields = [
  { key: 'caseNo', label: '案号' },
  { key: 'caseName', label: '案件名称' },
  { key: 'clientName', label: '客户名称' },
  { key: 'opponentName', label: '对方名称' },
  { key: 'court', label: '审理机关' },
  { key: 'causeAction', label: '案由' },
  { key: 'patentName', label: '专利名称' },
  { key: 'patentAppNo', label: '专利申请号' },
]

function onDocumentClick() {
  hideContextMenu()
}

onMounted(async () => {
  await recovery.recover()
  loadCasesList()
  if (caseId.value) {
    loadCaseData()
  }
  document.addEventListener('click', onDocumentClick)
  document.addEventListener('keydown', handleKeydown)
})

onBeforeUnmount(() => {
  if (autoSaveTimer) clearTimeout(autoSaveTimer)
  if (copilotSearchTimer) clearTimeout(copilotSearchTimer)
  document.removeEventListener('click', onDocumentClick)
  document.removeEventListener('keydown', handleKeydown)

})
</script>

<template>
  <DraftHistoryDialog v-model="historyOpen" :draft="historyDraft" :before-restore="beforeHistoryRestore" @restored="onVersionRestored" />
  <div class="writing-view">
    <!-- 顶部工具栏 -->
    <div class="writing-toolbar">
      <div class="toolbar-left">
        <el-input
          v-model="draftTitle"
          placeholder="文档标题"
          class="title-input"
          size="large"
        />
      </div>
      <div class="toolbar-right">
        <el-select
          :model-value="caseId"
          placeholder="关联案件（可选）"
          clearable
          filterable
          @change="onCaseSelect"
          style="width: 240px"
        >
          <el-option
            v-for="c in casesList"
            :key="c.id"
            :label="`${c.caseNo || c.caseName} - ${c.clientName}`"
            :value="c.id"
          />
        </el-select>
        <el-dropdown trigger="click" :disabled="exporting" @command="exportWriting"><el-button :loading="exporting">导出</el-button><template #dropdown><el-dropdown-menu><el-dropdown-item command="md">Markdown</el-dropdown-item><el-dropdown-item command="pdf">PDF</el-dropdown-item><el-dropdown-item command="docx">Word</el-dropdown-item></el-dropdown-menu></template></el-dropdown>
        <el-button :disabled="!draftId" @click="openHistory">历史版本</el-button>
        <el-button type="primary" :loading="saving" @click="saveDraft">
          {{ saving ? '保存中...' : '保存' }}
        </el-button>
      </div>
    </div>

    <div class="writing-body">
      <!-- 左侧：编辑器区域 -->
      <div class="editor-panel">
        <!-- 案件字段快捷插入 -->
        <div v-if="caseData" class="field-panel">
          <div class="field-panel-title">案件字段</div>
          <div class="field-chips">
            <el-button
              v-for="f in caseFields"
              :key="f.key"
              size="small"
              @click="insertCaseField(f.key)"
              :title="caseData[f.key] || '（空）'"
            >
              {{ f.label }}
            </el-button>
          </div>
        </div>

        <!-- 编辑器区域 -->
        <div class="editor-container">
          <DocumentEditor ref="docEditorRef" v-model="documentContent" content-format="html" :extra-extensions="writingExtensions" source-type="doc" :source-id="draftId || undefined" :case-id="caseId" @ready="editorReady" @update:model-value="scheduleAutoSave" @save="saveDraft" @capture-selection="handleContextMenu" />
        </div>
      </div>

      <!-- 右侧：Copilot Sidebar -->
      <CopilotSidebar
        v-model:search-query="copilotQuery"
        :search-results="copilotResults"
        :searching="copilotSearching"
        :generating="copilotGenerating"
        :expanded-item-id="copilotExpandedId"
        :get-category-label="copilotCategoryLabel"
        :get-category-icon="copilotCategoryIcon"
        @search="copilotDebouncedSearch"
        @insert="copilotInsert"
        @citation="copilotCitation"
        @copy="onCopyKnowledge"
        @toggle-expand="copilotToggleExpand"
        @ai-action="onAiAction"
      />
    </div>

    <!-- AI 写作辅助对话框 (Ctrl+K) -->
    <el-dialog
      v-model="copilotDialogVisible"
      title="AI 写作辅助"
      width="520px"
      append-to-body
      @close="copilotCloseAiDialog"
    >
      <div class="ai-dialog-body">
        <div class="ai-dialog-hint">
          描述你想生成的内容，AI 将基于当前文书上下文和知识库生成建议。
        </div>
        <el-input
          v-model="copilotIntent"
          type="textarea"
          :rows="3"
          placeholder="例如：生成损害赔偿计算的事实与理由段落"
          resize="none"
          autofocus
        />
        <div class="ai-dialog-style">
          <span class="style-label">文书风格：</span>
          <el-select v-model="aiStyle" size="small" style="width: 160px">
            <el-option label="起诉状" value="complaint" />
            <el-option label="代理词" value="defense_brief" />
            <el-option label="法律意见" value="legal_opinion" />
            <el-option label="律师函" value="lawyer_letter" />
            <el-option label="答辩状" value="reply_brief" />
            <el-option label="通用" value="general" />
          </el-select>
        </div>
      </div>
      <template #footer>
        <el-button @click="copilotCloseAiDialog">取消</el-button>
        <el-button
          type="primary"
          :loading="copilotGenerating"
          :disabled="!copilotIntent?.trim()"
          @click="onAiAction(copilotIntent)"
        >
          {{ copilotGenerating ? '生成中...' : '生成建议' }}
        </el-button>
      </template>
    </el-dialog>

    <!-- 右键知识入库菜单 -->
    <Teleport to="body">
      <div
        v-if="contextMenu.visible"
        class="knowledge-context-menu"
        :style="{ left: contextMenu.x + 'px', top: contextMenu.y + 'px' }"
        @click.stop
      >
        <div class="ctx-menu-header">
          <el-icon :size="14"><Collection /></el-icon> 知识入库
        </div>
        <div class="ctx-menu-item" @click="captureAs('inspiration')">
          <el-icon class="ctx-icon" :size="14"><Opportunity /></el-icon> 灵感记录
        </div>
        <div class="ctx-menu-item" @click="captureAs('method')">
          <span class="ctx-icon">📐</span> 工作方法
        </div>
        <div class="ctx-menu-item" @click="captureAs('reference')">
          <span class="ctx-icon">📖</span> 参考资料
        </div>
        <div class="ctx-menu-item" @click="captureAs('question')">
          <span class="ctx-icon">❓</span> 待研究问题
        </div>
        <div class="ctx-menu-item" @click="captureAs('experience')">
          <span class="ctx-icon">⭐</span> 经验总结
        </div>
        <div class="ctx-menu-item" @click="captureAs('log')">
          <span class="ctx-icon">📝</span> 工作日志
        </div>
      </div>
    </Teleport>

    <!-- 标签输入弹窗 -->
    <el-dialog v-model="captureDialog.visible" title="知识入库" width="480px" append-to-body>
      <el-form label-width="80px">
        <el-form-item label="标题">
          <el-input v-model="captureDialog.title" placeholder="知识条目标题" />
        </el-form-item>
        <el-form-item label="内容预览">
          <div class="capture-preview">{{ captureDialog.text }}</div>
        </el-form-item>
        <el-form-item label="职能分类">
          <el-select v-model="captureDialog.category" style="width: 100%">
            <el-option label="灵感" value="inspiration" />
            <el-option label="方法" value="method" />
            <el-option label="参考" value="reference" />
            <el-option label="问题" value="question" />
            <el-option label="经验" value="experience" />
            <el-option label="日志" value="log" />
          </el-select>
        </el-form-item>
        <el-form-item label="标签">
          <el-input v-model="captureDialog.tags" placeholder="多个标签用逗号分隔" />
        </el-form-item>
        <el-form-item label="法律名称">
          <el-input v-model="captureDialog.lawName" placeholder="如：专利法" />
        </el-form-item>
        <el-form-item label="条款号">
          <el-input v-model="captureDialog.articleNo" placeholder="如：第65条" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="captureDialog.visible = false">取消</el-button>
        <el-button type="primary" :loading="captureDialog.capturing" @click="doCapture">
          {{ captureDialog.capturing ? '入库中...' : '确认入库' }}
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.writing-view {
  display: flex;
  flex-direction: column;
  height: calc(100vh - 100px);
}

.writing-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 0;
  border-bottom: 1px solid #e0e0e0;
  gap: 16px;
}

.toolbar-left {
  flex: 1;
}

.toolbar-right {
  display: flex;
  align-items: center;
  gap: 12px;
}

.title-input :deep(.el-input__inner) {
  font-size: 18px;
  font-weight: 600;
  border: none;
  padding: 0;
}

.writing-body {
  flex: 1;
  display: flex;
  flex-direction: row;
  overflow: hidden;
}

.editor-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  min-width: 0;
}

.field-panel {
  padding: 8px 0;
  border-bottom: 1px solid #f0f0f0;
}

.field-panel-title {
  font-size: 12px;
  color: var(--gray-400);
  margin-bottom: 6px;
}

.field-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.editor-container {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.editor-menubar {
  padding: 8px 0;
  border-bottom: 1px solid #f0f0f0;
}

.editor-content {
  flex: 1;
  overflow-y: auto;
  padding: 16px 0;
}

.editor-content :deep(.tiptap) {
  outline: none;
  min-height: 400px;
  font-size: 15px;
  line-height: 1.8;
}

.editor-content :deep(.tiptap p) {
  margin: 0.5em 0;
}

.editor-content :deep(.tiptap h1) {
  font-size: 24px;
  margin: 1em 0 0.5em;
}

.editor-content :deep(.tiptap h2) {
  font-size: 20px;
  margin: 0.8em 0 0.4em;
}

.editor-content :deep(.tiptap h3) {
  font-size: 17px;
  margin: 0.6em 0 0.3em;
}

.editor-content :deep(.tiptap blockquote) {
  border-left: 3px solid #409eff;
  padding-left: 16px;
  color: #606266;
  margin: 1em 0;
}

.editor-content :deep(.tiptap mark) {
  background-color: #fef08a;
  padding: 0 2px;
}

/* 右键知识入库菜单 */
.knowledge-context-menu {
  position: fixed;
  z-index: 9999;
  background: #fff;
  border: 1px solid var(--c-border);
  border-radius: 8px;
  box-shadow: 0 6px 16px rgba(0, 0, 0, 0.12);
  padding: 4px 0;
  min-width: 200px;
}

.ctx-menu-header {
  padding: 8px 16px;
  font-size: 12px;
  color: var(--gray-400);
  border-bottom: 1px solid #f0f0f0;
  font-weight: 600;
}

.ctx-menu-item {
  padding: 8px 16px;
  font-size: 13px;
  color: #303133;
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 8px;
  transition: background var(--motion-fast);
}

.ctx-menu-item:hover {
  background: #ecf5ff;
  color: #409eff;
}

.ctx-menu-divider {
  height: 1px;
  background: #f0f0f0;
  margin: 4px 0;
}

.ctx-icon {
  font-size: 14px;
}

.capture-preview {
  max-height: 120px;
  overflow-y: auto;
  padding: 8px 12px;
  background: var(--gray-50);
  border-radius: 4px;
  font-size: 13px;
  line-height: 1.6;
  color: #606266;
  white-space: pre-wrap;
  word-break: break-all;
}

/* Copilot Sidebar 宽度 */
.writing-body :deep(.copilot-sidebar) {
  width: 340px;
  flex-shrink: 0;
}

/* AI 写作辅助对话框 */
.ai-dialog-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.ai-dialog-hint {
  font-size: 13px;
  color: var(--gray-400);
  line-height: 1.5;
}

.ai-dialog-style {
  display: flex;
  align-items: center;
  gap: 8px;
}

.style-label {
  font-size: 13px;
  color: #606266;
  white-space: nowrap;
}

/* AI 生成内容标记 */
.editor-content :deep(.tiptap mark.ai-suggestion) {
  background-color: #e6f1fc;
  border-bottom: 2px dashed #409eff;
  padding: 0 2px;
}
/* ============================================================
 * 块引用样式（设计哲学 §9.3）
 * ============================================================ */
:deep(.block-reference) {
  display: inline-block;
  background: #f0f7ff;
  border: 1px solid #b3d8ff;
  border-radius: 4px;
  padding: 2px 8px;
  margin: 0 2px;
  cursor: pointer;
  transition: all var(--motion-base) var(--ease-out);
  vertical-align: baseline;
  font-size: 0.9em;
}

:deep(.block-reference:hover) {
  background: #d9ecff;
  border-color: #409eff;
}

:deep(.block-reference.is-loading) {
  opacity: 0.6;
  cursor: wait;
}

:deep(.block-reference.is-error) {
  background: #fef0f0;
  border-color: #fbc4c4;
  color: #f56c6c;
}

:deep(.block-ref-loading),
:deep(.block-ref-error) {
  display: flex;
  align-items: center;
  gap: 4px;
}

:deep(.block-ref-content) {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

:deep(.block-ref-header) {
  display: flex;
  align-items: center;
  gap: 4px;
}

:deep(.block-ref-icon) {
  font-size: 1em;
}

:deep(.block-ref-title) {
  font-weight: 500;
  color: #409eff;
}

:deep(.block-ref-type) {
  font-size: 0.8em;
  color: var(--gray-400);
  background: var(--gray-50);
  padding: 0 4px;
  border-radius: 2px;
}

:deep(.block-ref-body) {
  display: none; /* 内联引用不显示正文，悬浮时可扩展 */
}

/* ── WikiLink 自动补全样式 ── */
:deep(.wiki-link-suggestions) {
  position: absolute;
  background: white;
  border: 1px solid var(--c-border);
  border-radius: 4px;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15);
  max-height: 240px;
  overflow-y: auto;
  z-index: 1000;
  min-width: 240px;
  padding: 4px 0;
}

:deep(.wiki-link-item) {
  padding: 8px 12px;
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 8px;
  transition: background var(--motion-fast);
}

:deep(.wiki-link-item:hover),
:deep(.wiki-link-item.selected) {
  background: var(--gray-50);
}

:deep(.wiki-link-item .title) {
  font-weight: 500;
  color: #303133;
}

:deep(.wiki-link-item .category) {
  font-size: 12px;
  color: var(--gray-400);
  margin-left: auto;
  background: var(--gray-50);
  padding: 2px 6px;
  border-radius: 2px;
}

/* ── wikiLink mark 样式 ── */
:deep(.wiki-link) {
  color: #409eff;
  text-decoration: none;
  border-bottom: 1px dashed #409eff;
  cursor: pointer;
  transition: all var(--motion-base) var(--ease-out);
}

:deep(.wiki-link:hover) {
  background: #ecf5ff;
  border-bottom-style: solid;
}

</style>
