<template>
  <div class="notion-legal-editor-shell" ref="editorContainer">
    <!-- 左侧悬浮块手柄 (Block Gutter Handle) -->
    <BlockActionHandle
      :editor="editor"
      :top="hoverHandleTop"
      :visible="hoverHandleVisible"
      @open-slash="openSlashAtCurrent"
    />

    <!-- Tiptap 编辑器核心内容区 -->
    <editor-content
      :editor="editor"
      class="editor-content-area"
      @mousemove="handleEditorMouseMove"
      @mouseleave="hoverHandleVisible = false"
      @click="handleEditorClick"
      @contextmenu="handleContextMenu"
      @drop="handleDrop"
    />

    <!-- 选中文字浮动格式栏 (Bubble Menu) -->
    <bubble-menu
      v-if="editor"
      :editor="editor"
      :tippy-options="{ duration: 150, zIndex: 99 }"
      class="notion-bubble-menu"
    >
      <button
        type="button"
        class="bubble-btn"
        :class="{ active: editor.isActive('bold') }"
        title="加粗 (⌘B)"
        @click="editor.chain().focus().toggleBold().run()"
      >
        <strong>B</strong>
      </button>

      <button
        type="button"
        class="bubble-btn"
        :class="{ active: editor.isActive('italic') }"
        title="斜体 (⌘I)"
        @click="editor.chain().focus().toggleItalic().run()"
      >
        <em>I</em>
      </button>

      <button
        type="button"
        class="bubble-btn"
        :class="{ active: editor.isActive('underline') }"
        title="下划线 (⌘U)"
        @click="editor.chain().focus().toggleUnderline().run()"
      >
        <u>U</u>
      </button>

      <button
        type="button"
        class="bubble-btn"
        :class="{ active: editor.isActive('strike') }"
        title="删除线"
        @click="editor.chain().focus().toggleStrike().run()"
      >
        <s>S</s>
      </button>

      <button
        type="button"
        class="bubble-btn"
        :class="{ active: editor.isActive('code') }"
        title="行内代码"
        @click="editor.chain().focus().toggleCode().run()"
      >
        <code>&lt;/&gt;</code>
      </button>

      <span class="bubble-divider"></span>

      <button
        type="button"
        class="bubble-btn"
        :class="{ active: editor.isActive({ textAlign: 'left' }) }"
        title="居左对齐"
        @click="editor.chain().focus().setTextAlign('left').run()"
      >
        <span>左</span>
      </button>

      <button
        type="button"
        class="bubble-btn"
        :class="{ active: editor.isActive({ textAlign: 'center' }) }"
        title="居中对齐"
        @click="editor.chain().focus().setTextAlign('center').run()"
      >
        <span>中</span>
      </button>

      <button
        type="button"
        class="bubble-btn"
        :class="{ active: editor.isActive({ textAlign: 'right' }) }"
        title="居右对齐"
        @click="editor.chain().focus().setTextAlign('right').run()"
      >
        <span>右</span>
      </button>

      <span class="bubble-divider"></span>

      <button
        type="button"
        class="bubble-btn ai-btn"
        title="AI 润色与术语转换"
        @click="openAiCopilot('polish')"
      >
        <el-icon><MagicStick /></el-icon>
        <span>AI 润色</span>
      </button>
    </bubble-menu>

    <!-- 斜杠指令浮窗 (Notion Slash Commands) -->
    <SlashCommandMenu
      v-if="editor"
      :editor="editor"
      :visible="slashMenuVisible"
      :position="slashMenuPos"
      :case-data="caseData"
      @close="slashMenuVisible = false"
      @open-ai="openAiCopilot"
      @open-law="insertLaw"
      @open-knowledge="insertKnowledge"
    />

    <!-- AI Copilot 弹窗 -->
    <el-dialog
      v-model="copilotDialog.visible"
      title="AI 智伴起草与润色 (Casy Copilot)"
      width="520px"
      append-to-body
      destroy-on-close
    >
      <el-form label-position="top">
        <el-form-item label="起草指令或修改要求">
          <el-input
            v-model="copilotDialog.prompt"
            type="textarea"
            rows="4"
            placeholder="例如：请根据本案争议焦点起草一段质证意见，重点论述对方提交的证据不具备真实性与关联性..."
          />
        </el-form-item>
        <el-form-item label="结合案件上下文">
          <div class="ai-context-tag">
            <el-icon><Briefcase /></el-icon>
            <span>{{ caseData?.caseName ? `当前关联：${caseData.caseName} (${caseData.caseCode || caseData.caseNo || 'CIV'})` : '未关联具体案件（按通用法律逻辑处理）' }}</span>
          </div>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="copilotDialog.visible = false">取消</el-button>
        <el-button type="primary" :loading="copilotDialog.generating" @click="generateWithAI">
          {{ copilotDialog.generating ? '生成中...' : '生成并插入文书' }}
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
          <el-icon :size="14"><Collection /></el-icon>
          <span>沉淀至专属智库</span>
        </div>
        <div class="ctx-menu-item" @click="captureAs('inspiration')">
          <el-icon class="ctx-icon" :size="14"><Opportunity /></el-icon> 灵感记录
        </div>
        <div class="ctx-menu-item" @click="captureAs('method')">
          <el-icon class="ctx-icon" :size="14"><Memo /></el-icon> 办案方法
        </div>
        <div class="ctx-menu-item" @click="captureAs('reference')">
          <el-icon class="ctx-icon" :size="14"><Reading /></el-icon> 参考资料 / 判例
        </div>
        <div class="ctx-menu-item" @click="captureAs('question')">
          <el-icon class="ctx-icon" :size="14"><QuestionFilled /></el-icon> 待研究焦点
        </div>
        <div class="ctx-menu-item" @click="captureAs('experience')">
          <el-icon class="ctx-icon" :size="14"><Medal /></el-icon> 胜诉经验总结
        </div>
        <div class="ctx-menu-item" @click="captureAs('log')">
          <el-icon class="ctx-icon" :size="14"><Document /></el-icon> 庭审备忘日志
        </div>
      </div>
    </Teleport>

    <!-- 知识入库弹窗 -->
    <el-dialog v-model="captureDialog.visible" title="沉淀到知识库" width="480px" append-to-body>
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
          <el-input v-model="captureDialog.tags" placeholder="如：专利无效, 创造性, 质证" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="captureDialog.visible = false">取消</el-button>
        <el-button type="primary" :loading="captureDialog.capturing" @click="doCapture">
          {{ captureDialog.capturing ? '入库中...' : '确认沉淀' }}
        </el-button>
      </template>
    </el-dialog>
    <!-- 证据链接选择器（W4 双链） -->
    <EvidenceLinkPicker
      v-model="evidencePickerVisible"
      :source-id="sourceId"
      :case-id="caseId"
      :all-cases="allCases"
      @insert="insertEvidenceLink"
    />
  </div>
</template>

<script setup>
import { ref, reactive, watch, onBeforeUnmount, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useEditor, EditorContent } from '@tiptap/vue-3'
import { BubbleMenu } from '@tiptap/vue-3/menus'
import StarterKit from '@tiptap/starter-kit'
import Placeholder from '@tiptap/extension-placeholder'
import TaskList from '@tiptap/extension-task-list'
import TaskItem from '@tiptap/extension-task-item'
import { Table } from '@tiptap/extension-table'
import TableRow from '@tiptap/extension-table-row'
import TableCell from '@tiptap/extension-table-cell'
import TableHeader from '@tiptap/extension-table-header'
import Typography from '@tiptap/extension-typography'
import TextAlign from '@tiptap/extension-text-align'
import Image from '@tiptap/extension-image'
import { casyContext } from '../../../core/plugin/context'
import { ElMessage } from 'element-plus'
import {
  Collection, Opportunity, Memo, Reading, QuestionFilled,
  Medal, Document, MagicStick, Briefcase
} from '@element-plus/icons-vue'
import SlashCommandMenu from './SlashCommandMenu.vue'
import BlockActionHandle from './BlockActionHandle.vue'
import EvidenceLinkPicker from './EvidenceLinkPicker.vue'
import { EvidenceLink } from '../extensions/EvidenceLink'
import { CaseFieldSuggestion } from '../composables/caseFieldSuggestion.js'
import { LegalProvisionSuggestion } from '../composables/legalProvisionSuggestion.js'
import { PartyNameSuggestion } from '../composables/partyNameSuggestion.js'
import { KnowledgeReferenceSuggestion } from '../composables/knowledgeReferenceSuggestion.js'

const props = defineProps({
  modelValue: { type: String, default: '' },
  caseData: { type: Object, default: () => ({}) },
  allCases: { type: Array, default: () => [] },
  caseId: { type: String, default: null },
  sourceId: { type: String, default: null },
})

const emit = defineEmits(['update:modelValue', 'save', 'knowledge-captured'])

const editorContainer = ref<HTMLElement | null>(null)

// ── 斜杠菜单状态 ──
const slashMenuVisible = ref(false)
const slashMenuPos = reactive({ x: 0, y: 0 })

// ── 悬浮手柄状态 ──
const hoverHandleVisible = ref(false)
const hoverHandleTop = ref(0)

// ── AI 对话框 ──
const copilotDialog = reactive({
  visible: false,
  generating: false,
  prompt: '',
  type: 'draft',
})

// ── 右键菜单与知识入库 ──
const contextMenu = reactive({ visible: false, x: 0, y: 0, selectedText: '' })
const captureDialog = reactive({
  visible: false,
  capturing: false,
  text: '',
  title: '',
  category: 'reference',
  tags: '',
})

const editor = useEditor({
  content: props.modelValue,
  extensions: [
    StarterKit.configure({
      heading: { levels: [1, 2, 3] },
    }),
    Typography,
    TextAlign.configure({
      types: ['heading', 'paragraph'],
    }),
    Image.configure({ allowBase64: true, inline: false }),
    TaskList,
    TaskItem.configure({
      nested: true,
    }),
    Table.configure({
      resizable: true,
    }),
    TableRow,
    TableHeader,
    TableCell,
    Placeholder.configure({
      placeholder: '按「/」唤出 Notion 块菜单，或直接输入 Markdown 快速起草...',
    }),
    CaseFieldSuggestion,
    LegalProvisionSuggestion,
    PartyNameSuggestion,
    KnowledgeReferenceSuggestion,
    EvidenceLink,
  ],
  onUpdate: ({ editor }) => {
    const html = editor.getHTML()
    emit('update:modelValue', html)
    checkForSlashTrigger()
  },
})

// 监听按键以激活 / 浮窗
function checkForSlashTrigger() {
  if (!editor.value) return
  const { state } = editor.value
  const { from } = state.selection
  const textBefore = state.doc.textBetween(Math.max(0, from - 2), from, '\n')
  
  if (textBefore.endsWith('/')) {
    const view = editor.value.view
    const coords = view.coordsAtPos(from)
    slashMenuPos.x = coords.left
    slashMenuPos.y = coords.top
    slashMenuVisible.value = true
  }
}

// 块悬浮手柄位置探测
function handleEditorMouseMove(e) {
  if (!editor.value || !editorContainer.value) return
  
  const target = e.target?.closest?.('p, h1, h2, h3, ul, ol, blockquote, table, pre')
  if (target && editorContainer.value.contains(target)) {
    const containerRect = editorContainer.value.getBoundingClientRect()
    const targetRect = target.getBoundingClientRect()
    hoverHandleTop.value = targetRect.top - containerRect.top + editorContainer.value.scrollTop + 2
    hoverHandleVisible.value = true
  }
}

function openSlashAtCurrent() {
  if (!editor.value) return
  const { from } = editor.value.state.selection
  const view = editor.value.view
  const coords = view.coordsAtPos(from)
  slashMenuPos.x = coords.left || 200
  slashMenuPos.y = coords.top || 200
  slashMenuVisible.value = true
}

function insertLaw() {
  slashMenuVisible.value = false
  editor.value?.chain().focus().insertContent('【').run()
}

function insertKnowledge() {
  slashMenuVisible.value = false
  emit('open-knowledge-drawer')
}

// ── AI Copilot ──
function openAiCopilot(type = 'draft') {
  slashMenuVisible.value = false
  copilotDialog.type = type
  if (type === 'polish') {
    const selectedText = editor.value?.state.doc.textBetween(
      editor.value.state.selection.from,
      editor.value.state.selection.to,
      ' '
    )
    copilotDialog.prompt = selectedText ? `请优化并润色以下段落，提升法言法语专业度：\n${selectedText}` : '请将选中文本润色为符合法庭提交标准的规范文书。'
  } else {
    copilotDialog.prompt = ''
  }
  copilotDialog.visible = true
}

async function generateWithAI() {
  if (!copilotDialog.prompt.trim()) return
  copilotDialog.generating = true

  const prompt = `你是一名精通中国法律的资深诉讼律师与文书专家。请根据以下要求撰写或润色文书。直接输出格式规范的 HTML 段落（包含 <h2>, <h3>, <p>, <ul>, <blockquote> 等）：\n\n${copilotDialog.prompt}`
  const result = await casyContext.ai.askAi(prompt, props.caseData)

  copilotDialog.generating = false
  if (result.ok && result.text) {
    editor.value?.chain().focus().insertContent(result.text).run()
    copilotDialog.visible = false
    copilotDialog.prompt = ''
    ElMessage.success('AI 内容已生成并插入')
  } else {
    ElMessage.error(result.error || 'AI 生成失败')
  }
}

// ── 证据链接（W4 双链） ──
const router = useRouter()
const evidencePickerVisible = ref(false)

function openEvidenceLinkPicker() {
  evidencePickerVisible.value = true
}

function insertEvidenceLink(attrs) {
  if (!editor.value) return
  editor.value.chain().focus().insertEvidenceLink(attrs).run()
}

// 点击证据链接：散发 CustomEvent（供其他模块监听）+ 默认路由跳转
function handleEditorClick(e) {
  const el = e.target?.closest?.('span[data-evidence-link]')
  if (!el) return
  e.preventDefault()
  e.stopPropagation()
  const detail = {
    linkId: el.getAttribute('data-link-id') || null,
    targetType: el.getAttribute('data-target-type'),
    targetId: el.getAttribute('data-target-id'),
    anchor: el.getAttribute('data-anchor') || null,
    label: el.getAttribute('data-label') || el.textContent || null,
    caseId: el.getAttribute('data-case-id') || null,
  }
  window.dispatchEvent(new CustomEvent('casy:evidence-link-activate', { detail }))
  navigateEvidenceLink(detail)
}

function navigateEvidenceLink({ targetType, targetId, anchor, caseId }) {
  if (!targetType || !targetId) return
  switch (targetType) {
    case 'file':
      // 跳转案卷库并选中文件（select/anchor 为约定 query，files 视图按需消费）
      if (caseId) {
        router.push({
          name: 'files',
          params: { caseId },
          query: { select: targetId, ...(anchor ? { anchor } : {}) },
        })
      } else {
        ElMessage.warning('该文件链接缺少所属案件信息，无法定位案卷库')
      }
      break
    case 'knowledge':
      router.push({ name: 'knowledge', query: { select: targetId } })
      break
    case 'task':
      router.push({ name: 'tasks', query: { edit: targetId } })
      break
    case 'case':
      router.push({ name: 'case-detail', params: { id: targetId } })
      break
    default:
      break
  }
}

function getDocumentJson() {
  return editor.value?.getJSON() || { type: 'doc', content: [] }
}

defineExpose({ openEvidenceLinkPicker, getDocumentJson })

// ── 右键知识入库 ──
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

function handleDrop(e) {
  const data = e.dataTransfer.getData('application/x-casy-reference')
  if (!data) return
  
  e.preventDefault()
  try {
    const item = JSON.parse(data)
    
    // 获取拖拽释放位置
    const coordinates = editor.value.view.posAtCoords({ left: e.clientX, top: e.clientY })
    if (coordinates) {
      editor.value.chain().focus().setTextSelection(coordinates.pos).run()
    }
    
    if (item.type === 'knowledge' && editor.value.commands.insertBlockReference) {
      editor.value.commands.insertBlockReference(item.id)
    } else {
      // 文件或其他类型，插入超链接或文本标识
      editor.value.chain().focus().insertContent(` <a href="#" data-ref-id="${item.id}" data-ref-type="${item.type}">📄 [${item.item_type === 'file' ? '案卷' : '知识'}] ${item.title}</a> `).run()
    }
  } catch (err) {
    console.error('Drop parsing error', err)
  }
}

function captureAs(category) {
  captureDialog.text = contextMenu.selectedText
  captureDialog.title = contextMenu.selectedText.substring(0, 40)
  captureDialog.category = category
  captureDialog.tags = ''
  captureDialog.visible = true
  contextMenu.visible = false
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
    status: 'current',
  })

  captureDialog.capturing = false
  if (result.ok) {
    ElMessage.success('已沉淀至知识库')
    captureDialog.visible = false
    emit('knowledge-captured', result.data)
  } else {
    ElMessage.error(result.error || '入库失败')
  }
}

// 外部内容同步
watch(() => props.modelValue, (val) => {
  if (editor.value && editor.value.getHTML() !== val) {
    editor.value.commands.setContent(val, { emitUpdate: false })
  }
})

// 将案件数据存入 storage
watch(() => props.caseData, (data) => {
  if (editor.value) editor.value.storage.caseData = data
}, { immediate: true })

watch(() => props.allCases, (cases) => {
  if (editor.value) editor.value.storage.allCases = cases
}, { immediate: true })

function onGlobalClick() {
  contextMenu.visible = false
}

onMounted(() => {
  document.addEventListener('click', onGlobalClick)
})

onBeforeUnmount(() => {
  document.removeEventListener('click', onGlobalClick)
  editor.value?.destroy()
})
</script>

<style scoped>
.notion-legal-editor-shell {
  position: relative;
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--c-bg-card);
  overflow-y: auto;
}

.editor-content-area {
  flex: 1;
  padding: 48px 72px 120px;
  max-width: 860px;
  width: 100%;
  margin: 0 auto;
}

/* ── 选区浮动菜单 ── */
.notion-bubble-menu {
  display: flex;
  align-items: center;
  gap: 2px;
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  padding: 4px 6px;
  box-shadow: var(--shadow-md);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
}

.bubble-btn {
  padding: 5px 8px;
  border-radius: var(--c-radius-md);
  border: none;
  background: transparent;
  color: var(--c-text-regular);
  font-size: 12.5px;
  font-weight: 500;
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  transition: all var(--motion-fast);
}

.bubble-btn:hover {
  background: var(--c-bg-hover);
  color: var(--c-text);
}

.bubble-btn.active {
  background: var(--c-bg-selected);
  color: var(--c-primary);
  font-weight: 700;
}

.bubble-btn.ai-btn {
  color: var(--c-primary);
  font-weight: 600;
}

.bubble-divider {
  width: 1px;
  height: 14px;
  background: var(--c-border);
  margin: 0 4px;
}

/* ── Tiptap 正文样式 ── */
.editor-content-area :deep(.tiptap) {
  outline: none;
  min-height: 500px;
  font-size: 15px;
  line-height: 1.8;
  color: var(--c-text);
  font-family: var(--font-family);
}

.editor-content-area :deep(.tiptap p) {
  margin: 0 0 10px;
}

.editor-content-area :deep(.tiptap h1) {
  font-size: 26px;
  font-weight: 700;
  margin: 32px 0 16px;
  color: var(--c-text-heading);
  letter-spacing: -0.5px;
  border-bottom: 1px solid var(--c-border-light);
  padding-bottom: 8px;
}

.editor-content-area :deep(.tiptap h2) {
  font-size: 20px;
  font-weight: 700;
  margin: 24px 0 12px;
  color: var(--c-text-heading);
}

.editor-content-area :deep(.tiptap h3) {
  font-size: 16.5px;
  font-weight: 600;
  margin: 18px 0 8px;
  color: var(--c-text-heading);
}

.editor-content-area :deep(.tiptap ul),
.editor-content-area :deep(.tiptap ol) {
  padding-left: 24px;
  margin: 10px 0;
}

.editor-content-area :deep(.tiptap li) {
  margin-bottom: 4px;
}

/* 待办清单 */
.editor-content-area :deep(.tiptap ul[data-type="taskList"]) {
  list-style: none;
  padding-left: 0;
}

.editor-content-area :deep(.tiptap li[data-type="taskItem"]) {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  margin-bottom: 6px;
}

.editor-content-area :deep(.tiptap li[data-type="taskItem"] input[type="checkbox"]) {
  margin-top: 5px;
  cursor: pointer;
}

/* Callout & 引用 */
.editor-content-area :deep(.tiptap blockquote) {
  border-left: 3px solid var(--c-primary);
  padding: 10px 16px;
  margin: 16px 0;
  color: var(--c-text-secondary);
  background: var(--c-bg-subtle);
  border-radius: 0 var(--c-radius-lg) var(--c-radius-lg) 0;
}

.editor-content-area :deep(.tiptap blockquote.callout-box) {
  border-left: 3px solid var(--status-warning);
  background: var(--bg-warning-weak);
  color: var(--c-text);
  border-radius: var(--c-radius-lg);
  padding: 12px 18px;
}

/* 表格 */
.editor-content-area :deep(.tiptap table) {
  border-collapse: collapse;
  margin: 20px 0;
  width: 100%;
  table-layout: fixed;
  border-radius: var(--c-radius-lg);
  overflow: hidden;
  border: 1px solid var(--c-border);
}

.editor-content-area :deep(.tiptap th) {
  background: var(--c-bg-subtle);
  font-weight: 600;
  text-align: left;
  padding: 10px 12px;
  border: 1px solid var(--c-border);
  font-size: 13px;
  color: var(--c-text-heading);
}

.editor-content-area :deep(.tiptap td) {
  padding: 8px 12px;
  border: 1px solid var(--c-border);
  font-size: 13.5px;
}

.editor-content-area :deep(.tiptap hr) {
  border: none;
  border-top: 1px solid var(--c-border);
  margin: 24px 0;
}

/* ── 证据链接（W4 双链）：徽标 + 高亮锚文本 ── */
.editor-content-area :deep(.tiptap .evidence-link) {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 0 6px 0 2px;
  margin: 0 1px;
  border-radius: var(--c-radius-md);
  background: var(--c-primary-light);
  color: var(--c-primary);
  font-weight: 500;
  cursor: pointer;
  border: 1px solid transparent;
  border-bottom: 1px dashed var(--c-primary);
  transition: background var(--motion-fast) var(--ease-out);
  user-select: none;
}

.editor-content-area :deep(.tiptap .evidence-link:hover) {
  background: var(--c-bg-selected);
  border-color: var(--c-primary);
}

.editor-content-area :deep(.tiptap .evidence-link)::before {
  content: attr(data-badge);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 15px;
  height: 15px;
  padding: 0 2px;
  border-radius: 4px;
  background: var(--c-primary);
  color: var(--c-bg-card);
  font-size: 10px;
  font-weight: 700;
  line-height: 1;
}

/* 类型着色：知识=绿、任务=橙、案件=中性 */
.editor-content-area :deep(.tiptap .evidence-link--knowledge) {
  background: var(--bg-success-weak);
  color: var(--status-success);
  border-bottom-color: var(--status-success);
}
.editor-content-area :deep(.tiptap .evidence-link--knowledge)::before {
  background: var(--status-success);
}
.editor-content-area :deep(.tiptap .evidence-link--task) {
  background: var(--bg-warning-weak);
  color: var(--status-warning);
  border-bottom-color: var(--status-warning);
}
.editor-content-area :deep(.tiptap .evidence-link--task)::before {
  background: var(--status-warning);
}
.editor-content-area :deep(.tiptap .evidence-link--case) {
  background: var(--c-bg-subtle);
  color: var(--c-text-heading);
  border-bottom-color: var(--c-text-secondary);
}
.editor-content-area :deep(.tiptap .evidence-link--case)::before {
  background: var(--c-text-secondary);
}

.editor-content-area :deep(.tiptap .evidence-link.ProseMirror-selectednode) {
  outline: 2px solid var(--c-primary);
  outline-offset: 1px;
}

.ai-context-tag {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--c-primary);
  padding: 6px 10px;
  border-radius: 6px;
  background: var(--c-primary-light);
}

.capture-preview {
  max-height: 120px;
  overflow-y: auto;
  padding: 10px;
  background: var(--c-bg-subtle);
  border-radius: 6px;
  font-size: 12.5px;
  color: var(--c-text-secondary);
}

.knowledge-context-menu {
  position: fixed;
  width: 180px;
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  box-shadow: var(--shadow-md);
  padding: 4px;
  z-index: 99999;
}

.ctx-menu-header {
  font-size: 10.5px;
  font-weight: 700;
  color: var(--slate-gray-light);
  text-transform: uppercase;
  padding: 6px 8px 4px;
  display: flex;
  align-items: center;
  gap: 6px;
}

.ctx-menu-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border-radius: 6px;
  font-size: 12.5px;
  color: var(--c-text-regular);
  cursor: pointer;
  transition: background var(--motion-fast);
}

.ctx-menu-item:hover {
  background: var(--c-bg-hover);
  color: var(--c-text);
}
</style>
