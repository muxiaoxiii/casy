<template>
  <div class="document-gen-view">
    <el-row :gutter="16" class="gen-layout">
      <!-- 左侧：模板浏览器 -->
      <el-col :span="8" class="gen-left">
        <TemplateBrowser v-model="selectedTemplate" @select="onTemplateSelect" />
      </el-col>

      <!-- 右侧：案件选择 + 预览 + 操作 -->
      <el-col :span="16" class="gen-right">
        <div class="gen-content">
          <!-- 案件选择 -->
          <div class="case-selector">
            <el-select
              v-model="selectedCaseId"
              filterable
              clearable
              placeholder="选择案件"
              size="default"
              style="width: 100%"
              @change="onCaseChange"
            >
              <el-option
                v-for="c in cases"
                :key="c.id"
                :label="`${c.caseName || c.caseNo || '未命名案件'}`"
                :value="c.id"
              >
                <span>{{ c.caseName || c.caseNo || '未命名案件' }}</span>
                <span style="float: right; color: #8492a6; font-size: 12px">
                  {{ c.clientName }}
                </span>
              </el-option>
            </el-select>
          </div>

          <!-- 模板信息 -->
          <div v-if="selectedTemplate" class="template-info-card">
            <div class="info-header ui-row">
              <h3>{{ selectedTemplate.name }}</h3>
              <el-tag size="small">{{ selectedTemplate.category }}</el-tag>
            </div>
            <p v-if="selectedTemplate.description" class="info-desc">
              {{ selectedTemplate.description }}
            </p>
            <div class="info-stats">
              <span>{{ selectedTemplate.fieldCount }} 个字段</span>
              <span v-if="renderResult">
                已填充 {{ Object.keys(renderResult.usedFields || {}).length }} 个
              </span>
            </div>
          </div>

          <!-- 字段预览表格 -->
          <div v-if="fieldRows.length > 0" class="field-preview">
            <div class="preview-header ui-row ui-row--between">
              <h4>字段映射预览</h4>
              <el-input
                v-model="fieldFilter"
                placeholder="筛选字段..."
                clearable
                size="small"
                style="width: 200px"
              />
            </div>
            <el-table
              :data="filteredFieldRows"
              border
              size="small"
              max-height="400"
              class="field-table"
            >
              <el-table-column prop="field" label="模板字段" width="160" />
              <el-table-column prop="value" label="映射值">
                <template #default="{ row }">
                  <span :class="['field-value', { empty: row.value === '(空)' }]">
                    {{ row.value }}
                  </span>
                </template>
              </el-table-column>
              <el-table-column prop="type" label="类型" width="100">
                <template #default="{ row }">
                  <el-tag size="small" :type="fieldTypeTag(row.type)">
                    {{ fieldTypeLabel(row.type) }}
                  </el-tag>
                </template>
              </el-table-column>
            </el-table>
          </div>

          <!-- 渲染预览 -->
          <div v-if="renderResult" class="render-preview">
            <el-tabs v-model="previewTab">
              <el-tab-pane label="HTML 预览" name="html">
                <div class="html-preview" v-html="safePreviewHtml"></div>
              </el-tab-pane>
              <el-tab-pane label="纯文本" name="text">
                <pre class="text-preview">{{ displayText }}</pre>
              </el-tab-pane>
              <el-tab-pane name="edit">
                <template #label>
                  <span class="edit-tab-label">
                    所见即所得编辑
                    <span
                      v-if="hasUnsavedEdits"
                      class="dirty-dot"
                      title="有未保存的修改"
                    ></span>
                  </span>
                </template>
                <div class="edit-toolbar">
                  <span
                    :class="['edit-status', { dirty: hasUnsavedEdits }]"
                  >
                    {{ hasUnsavedEdits ? '已修改，尚未保存为草稿' : draftId ? '已保存为草稿' : '与渲染结果一致' }}
                  </span>
                  <div class="edit-toolbar-actions ui-row">
                    <el-button
                      size="small"
                      :disabled="!isEditedFromRender"
                      @click="handleRevertEdits"
                    >
                      放弃修改
                    </el-button>
                    <el-button
                      size="small"
                      type="primary"
                      plain
                      :loading="savingDraft"
                      @click="handleSaveDraft"
                    >
                      保存为草稿
                    </el-button>
                  </div>
                </div>
                <div class="generated-editor-wrapper">
                  <LegalEditor
                    ref="legalEditorRef"
                    v-model="editedHtml"
                    :case-data="selectedCase || {}"
                    :all-cases="cases"
                    :case-id="selectedCaseId"
                  />
                </div>
              </el-tab-pane>
            </el-tabs>

            <!-- 缺失字段提示 -->
            <el-alert
              v-if="renderResult.missingFields?.length > 0"
              :title="`有 ${renderResult.missingFields.length} 个字段未填充`"
              type="warning"
              show-icon
              :closable="false"
              style="margin-top: 12px"
            >
              <template #default>
                {{ renderResult.missingFields.join('、') }}
              </template>
            </el-alert>
          </div>

          <!-- 操作按钮 -->
          <div class="gen-actions">
            <el-button
              type="primary"
              size="large"
              :disabled="!canGenerate"
              :loading="generating"
              @click="handleGenerate"
            >
              <el-icon><Document /></el-icon>
              生成文书
            </el-button>

            <el-button
              size="large"
              :disabled="!canGenerate"
              :loading="renderLoading"
              @click="handlePreview"
            >
              <el-icon><View /></el-icon>
              预览
            </el-button>

            <el-button
              size="large"
              :disabled="!canExport"
              :loading="exporting"
              @click="handleExport"
            >
              <el-icon><Download /></el-icon>
              导出 DOCX
            </el-button>

            <el-button
              size="large"
              :disabled="!renderResult"
              :loading="savingDraft"
              @click="handleSaveDraft"
            >
              <el-icon><EditPen /></el-icon>
              保存为草稿
            </el-button>
          </div>

          <!-- 空状态 -->
          <div
            v-if="!selectedTemplate || !selectedCaseId"
            class="empty-state"
          >
            <EmptyState
              type="custom"
              title="请选择模板和案件"
              :description="!selectedTemplate ? '👈 请先选择一个模板' : '请选择一个案件'"
            />
          </div>
        </div>
      </el-col>
    </el-row>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Document, View, Download, EditPen } from '../../../shared/icons'
import EmptyState from '../../../shared/components/EmptyState.vue'
import { casyContext } from '../../../core/plugin/context'
import { useDocsyBridge } from '../composables/useDocsyBridge.js'
import {
  fieldTypeLabel,
  fieldTypeTag,
  filterFieldRows,
  mapCaseToTemplate,
  mapToFieldRows,
} from '../utils/fieldMapping.js'
import TemplateBrowser from './TemplateBrowser.vue'
import LegalEditor from '../components/LegalEditor.vue'
// 审查 P0-3：displayHtml 含 Rust 模板渲染结果与用户可编辑 HTML，渲染前必须消毒
import { sanitizePreviewHtml } from '../../../shared/markdown/mdBridge'
// 审查 P2-2：displayText 用惰性 DOMParser 提取纯文本，避免 innerHTML 将编辑内容当可执行 HTML
import { htmlToText } from '../../../shared/utils/htmlToText'

const {
  renderResult,
  renderLoading,
  renderTemplate,
  clearRenderResult,
  exportDocx,
} = useDocsyBridge()

// 状态
const selectedTemplate = ref(null)
const selectedCaseId = ref(null)
const cases = ref([])
const fieldFilter = ref('')
const previewTab = ref('html')
const generating = ref(false)
const exporting = ref(false)
const savingDraft = ref(false)
const draftId = ref(null)
const lastSavedHtml = ref('')
const renderIdentity = ref(null)
const legalEditorRef = ref(null)

// 所见即所得编辑内容（初始与渲染结果一致，编辑后分叉）
const editedHtml = ref('')

// 计算属性
const canGenerate = computed(
  () => selectedTemplate.value && selectedCaseId.value
)

const identityMatchesSelection = computed(() =>
  !!renderIdentity.value &&
  renderIdentity.value.templateId === selectedTemplate.value?.id &&
  renderIdentity.value.caseId === selectedCaseId.value
)

const canExport = computed(
  () => canGenerate.value && renderResult.value && identityMatchesSelection.value
)

const selectedCase = computed(() =>
  cases.value.find((c) => c.id === selectedCaseId.value)
)

// 是否偏离模板渲染结果（决定是否走“编辑稿”导出）
const isEditedFromRender = computed(
  () => !!renderResult.value && editedHtml.value !== renderResult.value.html
)

// 是否偏离最近一次成功保存的草稿（决定脏点和丢弃确认）
const hasUnsavedEdits = computed(
  () => !!renderResult.value && editedHtml.value !== lastSavedHtml.value
)

// HTML 预览 tab：编辑后同步展示编辑结果
const displayHtml = computed(() =>
  isEditedFromRender.value ? editedHtml.value : renderResult.value?.html || ''
)

// HTML 预览 tab 的消毒结果：模板中直接调用 sanitizePreviewHtml 会在每次重渲染都重跑 DOMPurify，
// 这里在 computed 里预计算一次（审查 P2-1）。displayHtml 含 Rust 模板渲染结果与用户可编辑 HTML。
const safePreviewHtml = computed(() => sanitizePreviewHtml(displayHtml.value))

// 纯文本 tab：编辑后从编辑内容提取文本同步展示
// 审查 P2-2：改用惰性 DOMParser（htmlToText）而非 innerHTML 临时元素，避免把编辑内容当可执行 HTML
const displayText = computed(() => {
  if (!renderResult.value) return ''
  if (!isEditedFromRender.value) return renderResult.value.text || ''
  return htmlToText(editedHtml.value)
})

// 新渲染结果到达时重置编辑内容（覆盖防护在渲染触发前完成）
watch(renderResult, (val) => {
  editedHtml.value = val?.html || ''
  lastSavedHtml.value = val?.html || ''
  draftId.value = null
})

// 字段行数据
const fieldRows = computed(() => {
  if (!selectedCase.value) return []
  const values = mapCaseToTemplate(selectedCase.value)
  return mapToFieldRows(values)
})

// 过滤后的字段行
const filteredFieldRows = computed(() => filterFieldRows(fieldRows.value, fieldFilter.value))

// 加载案件列表
async function loadCases() {
  const result = await casyContext.cases.list({ page: 1, perPage: 500 })
  if (result.ok) {
    cases.value = result.data?.items || []
  }
}

function rollbackSelectionToRenderedIdentity() {
  const identity = renderIdentity.value
  if (!identity) return
  selectedTemplate.value = identity.template
  selectedCaseId.value = identity.caseId
}

function clearRenderedDocument() {
  clearRenderResult()
  renderIdentity.value = null
  editedHtml.value = ''
  lastSavedHtml.value = ''
  draftId.value = null
}

// 模板选择回调：v-model 已先更新，取消丢弃时必须显式回滚。
async function onTemplateSelect() {
  if (!(await confirmDiscardEdits())) {
    rollbackSelectionToRenderedIdentity()
    return
  }
  if (selectedTemplate.value && selectedCaseId.value) {
    await runPreview(false)
  } else {
    clearRenderedDocument()
  }
}

// 案件选择回调：与模板选择使用同一提交式状态语义。
async function onCaseChange() {
  if (!(await confirmDiscardEdits())) {
    rollbackSelectionToRenderedIdentity()
    return
  }
  if (selectedTemplate.value && selectedCaseId.value) {
    await runPreview(false)
  } else {
    clearRenderedDocument()
  }
}

// 有未保存编辑时的丢弃确认（重新渲染会覆盖编辑内容）
async function confirmDiscardEdits() {
  if (!hasUnsavedEdits.value) return true
  try {
    await ElMessageBox.confirm(
      '重新渲染将丢弃当前未保存的编辑内容，是否继续？如需保留请先「保存为草稿」。',
      '有未保存的修改',
      {
        confirmButtonText: '丢弃修改并继续',
        cancelButtonText: '取消',
        type: 'warning',
      }
    )
    return true
  } catch {
    return false
  }
}

async function runPreview(requireConfirmation = true) {
  if (!canGenerate.value) return
  if (requireConfirmation && !(await confirmDiscardEdits())) return false

  const template = selectedTemplate.value
  const caseId = selectedCaseId.value

  // 预览忙碌态由 composable 的 renderLoading（请求序号派生）统一驱动：
  // 重叠预览时仅最新请求持有/释放它，旧请求（stale）不会提前关掉 spinner。
  const result = await renderTemplate(template.id, caseId)

  // 旧请求结果对当前选择无意义，不覆盖 renderIdentity 也不弹错。
  if (result.stale) return false
  if (!result.ok) {
    renderIdentity.value = null
    ElMessage.error('渲染模板失败: ' + result.error)
    return false
  }

  renderIdentity.value = {
    templateId: template.id,
    template,
    caseId,
    requestId: result.requestId,
  }
  return true
}

// 预览
async function handlePreview() {
  await runPreview(true)
}

// 生成文书（创建草稿）
async function handleGenerate() {
  if (!canGenerate.value) return
  if (!(await confirmDiscardEdits())) return

  generating.value = true
  const rendered = await runPreview(false)
  if (!rendered || !renderIdentity.value || !renderResult.value) {
    generating.value = false
    return
  }

  // 创建草稿
  const draftResult = await casyContext.docs.createDraft({
    title: `${selectedTemplate.value.name} - ${selectedCase.value?.caseName || ''}`,
    content: renderResult.value.html,
    caseId: selectedCaseId.value,
    templatePath: selectedTemplate.value.path,
  })

  generating.value = false

  if (draftResult.ok) {
    draftId.value = draftResult.data?.id || null
    lastSavedHtml.value = renderResult.value.html
    ElMessage.success('文书已生成并保存为草稿')
  } else {
    ElMessage.error('创建草稿失败: ' + draftResult.error)
  }
}

// 导出 DOCX：未编辑内容继续走模板保真导出；编辑稿走 Rust 原生 DOCX 生成。
async function handleExport() {
  if (!canExport.value) return

  exporting.value = true
  try {
    let result
    if (isEditedFromRender.value) {
      const document = legalEditorRef.value?.getDocumentJson?.()
      if (!document) {
        ElMessage.error('编辑器内容尚未就绪，无法导出')
        return
      }
      result = await casyContext.docs.exportEditedDocx({
        document,
        title: `${selectedTemplate.value?.name || '文书'} - ${selectedCase.value?.caseName || ''}（已编辑）`,
      })
    } else {
      result = await exportDocx(
        selectedTemplate.value.id,
        selectedCaseId.value
      )
    }
    await handleExportResult(result)
  } finally {
    exporting.value = false
  }
}

async function handleExportResult(result) {
  if (!result.ok) {
    ElMessage.error('导出失败: ' + result.error)
    return
  }
  ElMessage.success(`DOCX 已导出: ${result.data.outputPath}`)
  try {
    await ElMessageBox.confirm('是否打开导出的文件？', '导出成功', {
      confirmButtonText: '打开',
      cancelButtonText: '关闭',
      type: 'success',
    })
    await casyContext.files.open(result.data.outputPath)
  } catch {
    // 用户取消，忽略
  }
}

// 保存为草稿（编辑后内容优先，可在文书工坊中继续编辑）
async function handleSaveDraft() {
  if (!renderResult.value || !renderIdentity.value) return
  if (!identityMatchesSelection.value) {
    ElMessage.error('当前案件或模板与预览结果不一致，请重新预览后再保存')
    return
  }

  savingDraft.value = true
  const payload = {
    title: `${renderIdentity.value.template.name} - ${selectedCase.value?.caseName || ''}`,
    content: editedHtml.value,
    caseId: renderIdentity.value.caseId,
    templatePath: renderIdentity.value.template.path,
  }
  const draftResult = draftId.value
    ? await casyContext.docs.updateDraft(draftId.value, {
        title: payload.title,
        content: payload.content,
        caseId: payload.caseId,
      })
    : await casyContext.docs.createDraft(payload)

  savingDraft.value = false

  if (draftResult.ok) {
    draftId.value = draftResult.data?.id || draftId.value
    lastSavedHtml.value = editedHtml.value
    ElMessage.success('草稿已保存，可在文书工坊中继续编辑')
  } else {
    ElMessage.error('保存草稿失败: ' + draftResult.error)
  }
}

// 放弃编辑修改，回退到原始渲染结果
async function handleRevertEdits() {
  if (!isEditedFromRender.value) return
  try {
    await ElMessageBox.confirm(
      '确定放弃所有编辑修改，恢复为模板渲染的原始内容吗？',
      '放弃修改',
      {
        confirmButtonText: '放弃修改',
        cancelButtonText: '取消',
        type: 'warning',
      }
    )
    editedHtml.value = renderResult.value?.html || ''
    ElMessage.info('已恢复为原始渲染内容')
  } catch {
    // 用户取消，忽略
  }
}

onMounted(() => {
  loadCases()
})
</script>

<style scoped>
.document-gen-view {
  height: 100%;
  overflow: hidden;
}

.gen-layout {
  height: 100%;
}

.gen-left {
  height: 100%;
  overflow: hidden;
}

.gen-right {
  height: 100%;
  overflow-y: auto;
}

.gen-content {
  padding: 16px;
}

.case-selector {
  margin-bottom: 16px;
}

.template-info-card {
  padding: 16px;
  background: var(--gray-50);
  border-radius: 8px;
  margin-bottom: 16px;
}

.info-header {
  gap: 12px;
  margin-bottom: 8px;
}

.info-header h3 {
  margin: 0;
  font-size: 16px;
  color: #303133;
}

.info-desc {
  margin: 0 0 8px;
  font-size: 13px;
  color: #606266;
}

.info-stats {
  display: flex;
  gap: 16px;
  font-size: 12px;
  color: var(--gray-400);
}

.field-preview {
  margin-bottom: 16px;
}

.preview-header {
  margin-bottom: 12px;
}

.preview-header h4 {
  margin: 0;
  font-size: 14px;
  color: #303133;
}

.field-table {
  width: 100%;
}

.field-value.empty {
  color: var(--gray-300);
  font-style: italic;
}

.render-preview {
  margin-bottom: 16px;
  border: 1px solid #ebeef5;
  border-radius: 8px;
  overflow: hidden;
}

.render-preview :deep(.el-tabs__header) {
  margin: 0;
  padding: 0 16px;
  background: #fafafa;
}

.html-preview {
  padding: 16px;
  max-height: 400px;
  overflow-y: auto;
  font-family: 'SimSun', serif;
  line-height: 1.8;
}

.html-preview :deep(p) {
  margin: 8px 0;
  text-indent: 2em;
}

.text-preview {
  padding: 16px;
  max-height: 400px;
  overflow-y: auto;
  font-family: 'Courier New', monospace;
  font-size: 13px;
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-all;
}

/* 所见即所得编辑 tab */
.edit-tab-label {
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.dirty-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--el-color-warning, #e6a23c);
  flex-shrink: 0;
}

.edit-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 8px 16px;
  border-bottom: 1px solid var(--c-border, var(--el-border-color-light));
  background: var(--c-bg-subtle, transparent);
}

.edit-status {
  font-size: 12px;
  color: var(--c-text-secondary, var(--el-text-color-secondary));
  transition: color var(--motion-fast, 120ms) ease;
}

.edit-status.dirty {
  color: var(--el-color-warning, #e6a23c);
  font-weight: 600;
}

.edit-toolbar-actions {
  gap: 8px;
}

.generated-editor-wrapper {
  height: 480px;
  overflow: hidden;
  background: var(--c-bg-card, var(--el-bg-color));
}

.generated-editor-wrapper :deep(.notion-legal-editor-shell) {
  height: 100%;
}

.generated-editor-wrapper :deep(.editor-content-area) {
  padding: 24px 32px 64px;
}

.gen-actions {
  display: flex;
  gap: 12px;
  padding: 16px 0;
  border-top: 1px solid #ebeef5;
}

.empty-state {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 300px;
}
</style>
