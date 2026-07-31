<script setup>
import { ref, reactive, onMounted, watch, onBeforeUnmount } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useEditor, EditorContent } from '@tiptap/vue-3'
import StarterKit from '@tiptap/starter-kit'
import Underline from '@tiptap/extension-underline'
import Placeholder from '@tiptap/extension-placeholder'
import Highlight from '@tiptap/extension-highlight'
import { useCasesStore } from '../../../stores/cases.js'
import { tauriCallSafe } from '../../../core/tauriBridge.js'
import { ElMessage } from 'element-plus'
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
const draftTitle = ref('未命名文档')
const saving = ref(false)
let autoSaveTimer = null

// 右键知识入库
const contextMenu = reactive({ visible: false, x: 0, y: 0, selectedText: '' })
const captureDialog = reactive({
  visible: false,
  capturing: false,
  text: '',
  title: '',
  category: 'other',
  tags: '',
  lawName: '',
  articleNo: '',
})

// 编辑器
const editor = useEditor({
  content: '<p>开始撰写...</p>',
  extensions: [
    StarterKit,
    Underline,
    Highlight,
    Placeholder.configure({ placeholder: '开始撰写法律文书...' }),
  ],
  onUpdate: ({ editor }) => {
    // 内容变化时触发自动保存
    scheduleAutoSave()
  },
  onSelectionUpdate: ({ editor }) => {
    // 光标移动时自动检索相关知识（防抖）
    scheduleCopilotSearch(editor)
  },
})

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
  const result = await tauriCallSafe('list_cases', { filter: { page: 1, perPage: 200 } })
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
  if (autoSaveTimer) clearTimeout(autoSaveTimer)
  autoSaveTimer = setTimeout(() => saveDraft(), 5000)
}

async function saveDraft() {
  if (!editor.value || saving.value) return
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
    result = await tauriCallSafe('update_draft', { id: draftId.value, data: payload })
  } else {
    result = await tauriCallSafe('create_draft', { data: payload })
    if (result.ok && result.data?.id) {
      draftId.value = result.data.id
    }
  }

  saving.value = false
  if (result.ok) {
    ElMessage.success('已自动保存')
  }
}

// 加载已有草稿
async function loadDraft(id) {
  const result = await tauriCallSafe('get_draft', { id })
  if (result.ok && result.data) {
    draftId.value = result.data.id
    draftTitle.value = result.data.title || '未命名文档'
    if (result.data.caseId) {
      caseId.value = result.data.caseId
      loadCaseData()
    }
    if (editor.value && result.data.content) {
      editor.value.commands.setContent(result.data.content)
    }
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

  const result = await tauriCallSafe('create_knowledge', {
    data: {
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
    },
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
  { key: 'caseNo', label: '案号', icon: '📋' },
  { key: 'caseName', label: '案件名称', icon: '📋' },
  { key: 'clientName', label: '客户名称', icon: '👤' },
  { key: 'opponentName', label: '对方名称', icon: '👥' },
  { key: 'court', label: '审理机关', icon: '🏛️' },
  { key: 'causeAction', label: '案由', icon: '📝' },
  { key: 'patentName', label: '专利名称', icon: '📄' },
  { key: 'patentAppNo', label: '专利申请号', icon: '📄' },
]

function onDocumentClick() {
  hideContextMenu()
}

onMounted(() => {
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
  if (editor.value) editor.value.destroy()
})
</script>

<template>
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
              {{ f.icon }} {{ f.label }}
            </el-button>
          </div>
        </div>

        <!-- 编辑器区域 -->
        <div class="editor-container">
          <div v-if="editor" class="editor-menubar">
            <el-button-group size="small">
              <el-button
                :type="editor.isActive('bold') ? 'primary' : 'default'"
                @click="editor.chain().focus().toggleBold().run()"
              >B</el-button>
              <el-button
                :type="editor.isActive('italic') ? 'primary' : 'default'"
                @click="editor.chain().focus().toggleItalic().run()"
              >I</el-button>
              <el-button
                :type="editor.isActive('underline') ? 'primary' : 'default'"
                @click="editor.chain().focus().toggleUnderline().run()"
              >U</el-button>
              <el-button
                :type="editor.isActive('highlight') ? 'primary' : 'default'"
                @click="editor.chain().focus().toggleHighlight().run()"
              >H</el-button>
            </el-button-group>
            <el-button-group size="small" style="margin-left: 8px">
              <el-button @click="editor.chain().focus().setHeading({ level: 1 }).run()">H1</el-button>
              <el-button @click="editor.chain().focus().setHeading({ level: 2 }).run()">H2</el-button>
              <el-button @click="editor.chain().focus().setHeading({ level: 3 }).run()">H3</el-button>
            </el-button-group>
            <el-button-group size="small" style="margin-left: 8px">
              <el-button @click="editor.chain().focus().toggleBulletList().run()">列表</el-button>
              <el-button @click="editor.chain().focus().toggleOrderedList().run()">编号</el-button>
              <el-button @click="editor.chain().focus().toggleBlockquote().run()">引用</el-button>
            </el-button-group>
            <el-button-group size="small" style="margin-left: 8px">
              <el-button @click="editor.chain().focus().undo().run()">撤销</el-button>
              <el-button @click="editor.chain().focus().redo().run()">重做</el-button>
            </el-button-group>
            <el-button-group size="small" style="margin-left: 8px">
              <el-button @click="copilotOpenAiDialog()" title="AI 写作辅助 (Ctrl+K)">
                ✨ AI 辅助
              </el-button>
            </el-button-group>
          </div>
          <EditorContent :editor="editor" class="editor-content" @contextmenu="handleContextMenu" />
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
      title="✨ AI 写作辅助"
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
          {{ copilotGenerating ? '生成中...' : '✨ 生成建议' }}
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
        <div class="ctx-menu-header">📚 知识入库</div>
        <div class="ctx-menu-item" @click="captureAs('common_paragraph')">
          <span class="ctx-icon">📌</span> 标记为常用段落
        </div>
        <div class="ctx-menu-item" @click="captureAs('law_reference')">
          <span class="ctx-icon">📖</span> 提取法条引用
        </div>
        <div class="ctx-menu-item" @click="captureAs('case_note')">
          <span class="ctx-icon">⚖️</span> 记录判例要点
        </div>
        <div class="ctx-menu-divider" />
        <div class="ctx-menu-item" @click="captureAs('complaint')">
          <span class="ctx-icon">📜</span> 标注：起诉状风格
        </div>
        <div class="ctx-menu-item" @click="captureAs('defense_brief')">
          <span class="ctx-icon">⚖️</span> 标注：代理词风格
        </div>
        <div class="ctx-menu-item" @click="captureAs('legal_opinion')">
          <span class="ctx-icon">📋</span> 标注：法律意见风格
        </div>
        <div class="ctx-menu-item" @click="captureAs('lawyer_letter')">
          <span class="ctx-icon">✉️</span> 标注：律师函风格
        </div>
        <div class="ctx-menu-item" @click="captureAs('reply_brief')">
          <span class="ctx-icon">🛡️</span> 标注：答辩状风格
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
        <el-form-item label="分类">
          <el-select v-model="captureDialog.category" style="width: 100%">
            <el-option label="常用段落" value="common_paragraph" />
            <el-option label="法条引用" value="law_reference" />
            <el-option label="判例要点" value="case_note" />
            <el-option label="起诉状" value="complaint" />
            <el-option label="代理词" value="defense_brief" />
            <el-option label="法律意见" value="legal_opinion" />
            <el-option label="律师函" value="lawyer_letter" />
            <el-option label="答辩状" value="reply_brief" />
            <el-option label="其他" value="other" />
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
  color: #909399;
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
  border: 1px solid #e4e7ed;
  border-radius: 8px;
  box-shadow: 0 6px 16px rgba(0, 0, 0, 0.12);
  padding: 4px 0;
  min-width: 200px;
}

.ctx-menu-header {
  padding: 8px 16px;
  font-size: 12px;
  color: #909399;
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
  transition: background 0.15s;
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
  background: #f5f7fa;
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
  color: #909399;
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
</style>
