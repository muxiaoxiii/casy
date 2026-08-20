<template>
  <div class="legal-editor">
    <div class="editor-toolbar">
      <button
        @click="editor?.chain().focus().toggleBold().run()"
        :class="{ active: editor?.isActive('bold') }"
        title="粗体"
      >B</button>
      <button
        @click="editor?.chain().focus().toggleItalic().run()"
        :class="{ active: editor?.isActive('italic') }"
        title="斜体"
      >I</button>
      <button
        @click="editor?.chain().focus().toggleUnderline().run()"
        :class="{ active: editor?.isActive('underline') }"
        title="下划线"
      >U</button>
      <span class="separator">|</span>
      <button @click="editor?.chain().focus().toggleHeading({ level: 1 }).run()"
        :class="{ active: editor?.isActive('heading', { level: 1 }) }">H1</button>
      <button @click="editor?.chain().focus().toggleHeading({ level: 2 }).run()"
        :class="{ active: editor?.isActive('heading', { level: 2 }) }">H2</button>
      <button @click="editor?.chain().focus().toggleHeading({ level: 3 }).run()"
        :class="{ active: editor?.isActive('heading', { level: 3 }) }">H3</button>
      <span class="separator">|</span>
      <button @click="editor?.chain().focus().toggleBulletList().run()"
        :class="{ active: editor?.isActive('bulletList') }">• 列表</button>
      <button @click="editor?.chain().focus().toggleOrderedList().run()"
        :class="{ active: editor?.isActive('orderedList') }">1. 列表</button>
      <span class="separator">|</span>
      <button @click="editor?.chain().focus().undo().run()">撤销</button>
      <button @click="editor?.chain().focus().redo().run()">重做</button>
      <span class="separator">|</span>
      <button @click="insertField" title="插入案件字段 {">{'{'}字段</button>
      <button @click="insertLaw" title="插入法条 【">【法条</button>
      <button @click="insertParty" title="插入当事人 @">@当事人</button>
    </div>
    <editor-content :editor="editor" class="editor-content" @contextmenu="handleContextMenu" />

    <!-- 右键知识入库菜单 -->
    <Teleport to="body">
      <div
        v-if="contextMenu.visible"
        class="knowledge-context-menu"
        :style="{ left: contextMenu.x + 'px', top: contextMenu.y + 'px' }"
        @click.stop
      >
        <div class="ctx-menu-header">📚 知识入库</div>
        <div class="ctx-menu-item" @click="captureAs('inspiration')">
          <span class="ctx-icon">💡</span> 灵感记录
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

<script setup>
import { reactive, watch, onBeforeUnmount, onMounted } from 'vue'
import { useEditor, EditorContent } from '@tiptap/vue-3'
import StarterKit from '@tiptap/starter-kit'
import Underline from '@tiptap/extension-underline'
import Placeholder from '@tiptap/extension-placeholder'
import { casyContext } from '../../../core/plugin/context'
import { ElMessage } from 'element-plus'
import { CaseFieldSuggestion } from '../composables/caseFieldSuggestion.js'
import { LegalProvisionSuggestion } from '../composables/legalProvisionSuggestion.js'
import { PartyNameSuggestion } from '../composables/partyNameSuggestion.js'

const props = defineProps({
  modelValue: { type: String, default: '' },
  caseData: { type: Object, default: () => ({}) },
  allCases: { type: Array, default: () => [] },
  caseId: { type: String, default: null },
  sourceId: { type: String, default: null },
})

const emit = defineEmits(['update:modelValue', 'save', 'knowledge-captured'])

// 右键菜单状态
const contextMenu = reactive({ visible: false, x: 0, y: 0, selectedText: '' })

// 捕获弹窗状态
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

const editor = useEditor({
  content: props.modelValue,
  extensions: [
    StarterKit.configure({
      heading: { levels: [1, 2, 3] },
    }),
    Underline,
    Placeholder.configure({ placeholder: '开始撰写... 输入 { 插入案件字段，【 插入法条，@ 插入当事人' }),
    CaseFieldSuggestion,
    LegalProvisionSuggestion,
    PartyNameSuggestion,
  ],
  onUpdate: ({ editor }) => {
    const html = editor.getHTML()
    emit('update:modelValue', html)
  },
})

// 将案件数据存入 editor storage 供 suggestion 使用
watch(() => props.caseData, (data) => {
  if (editor.value) editor.value.storage.caseData = data
}, { immediate: true })

watch(() => props.allCases, (cases) => {
  if (editor.value) editor.value.storage.allCases = cases
}, { immediate: true })

// 外部内容变化时同步到编辑器（但避免循环更新）
watch(() => props.modelValue, (val) => {
  if (editor.value && editor.value.getHTML() !== val) {
    editor.value.commands.setContent(val, false)
  }
})

function insertField() {
  editor.value?.chain().focus().insertContent('{').run()
}

function insertLaw() {
  editor.value?.chain().focus().insertContent('【').run()
}

function insertParty() {
  editor.value?.chain().focus().insertContent('@').run()
}

// ---- 右键知识入库 ----

function handleContextMenu(e) {
  if (!editor.value) return
  const { state } = editor.value
  const { from, to } = state.selection
  if (from === to) return // 未选中文本，不拦截

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
    sourceId: props.sourceId || null,
    linkedCaseId: props.caseId || null,
    lawName: captureDialog.lawName || null,
    articleNo: captureDialog.articleNo || null,
    status: 'current',
  })

  captureDialog.capturing = false

  if (result.ok) {
    ElMessage.success('知识已入库')
    captureDialog.visible = false
    emit('knowledge-captured', result.data)
  } else {
    ElMessage.error(result.error || '入库失败')
  }
}

// 全局点击关闭菜单
function onDocumentClick() {
  hideContextMenu()
}

onMounted(() => {
  document.addEventListener('click', onDocumentClick)
})

onBeforeUnmount(() => {
  document.removeEventListener('click', onDocumentClick)
  editor.value?.destroy()
})
</script>

<style scoped>
.legal-editor {
  display: flex;
  flex-direction: column;
  height: 100%;
  border: 1px solid #e0e0e0;
  border-radius: 6px;
  overflow: hidden;
}

.editor-toolbar {
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 6px 8px;
  background: #fafafa;
  border-bottom: 1px solid #e0e0e0;
  flex-wrap: wrap;
}

.editor-toolbar button {
  padding: 4px 8px;
  border: 1px solid transparent;
  border-radius: 4px;
  background: transparent;
  cursor: pointer;
  font-size: 13px;
  color: #333;
  transition: all 0.15s;
}

.editor-toolbar button:hover {
  background: #ecf5ff;
  border-color: #d9ecff;
}

.editor-toolbar button.active {
  background: #409eff;
  color: white;
  border-color: #409eff;
}

.editor-toolbar .separator {
  color: #ddd;
  margin: 0 4px;
  user-select: none;
}

.editor-content {
  flex: 1;
  overflow-y: auto;
  padding: 16px 20px;
}

/* TipTap 编辑器样式 */
.editor-content :deep(.tiptap) {
  outline: none;
  min-height: 300px;
  font-size: 15px;
  line-height: 1.8;
  color: #333;
}

.editor-content :deep(.tiptap p) {
  margin: 0 0 8px;
}

.editor-content :deep(.tiptap h1) {
  font-size: 24px;
  margin: 16px 0 12px;
}

.editor-content :deep(.tiptap h2) {
  font-size: 20px;
  margin: 14px 0 10px;
}

.editor-content :deep(.tiptap h3) {
  font-size: 17px;
  margin: 12px 0 8px;
}

.editor-content :deep(.tiptap ul),
.editor-content :deep(.tiptap ol) {
  padding-left: 24px;
  margin: 8px 0;
}

.editor-content :deep(.tiptap blockquote) {
  border-left: 3px solid #409eff;
  padding-left: 12px;
  margin: 8px 0;
  color: #666;
}

.editor-content :deep(.tiptap p.is-editor-empty:first-child::before) {
  content: attr(data-placeholder);
  float: left;
  color: #adb5bd;
  pointer-events: none;
  height: 0;
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

/* 捕获预览 */
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
</style>
