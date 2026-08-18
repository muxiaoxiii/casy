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
                :label="`${c.caseName || c.caseNo || c.id}`"
                :value="c.id"
              >
                <span>{{ c.caseName || c.caseNo || c.id }}</span>
                <span style="float: right; color: #8492a6; font-size: 12px">
                  {{ c.clientName }}
                </span>
              </el-option>
            </el-select>
          </div>

          <!-- 模板信息 -->
          <div v-if="selectedTemplate" class="template-info-card">
            <div class="info-header">
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
            <div class="preview-header">
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
                <div class="html-preview" v-html="renderResult.html"></div>
              </el-tab-pane>
              <el-tab-pane label="纯文本" name="text">
                <pre class="text-preview">{{ renderResult.text }}</pre>
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
              :loading="previewing"
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
              :disabled="!canGenerate"
              @click="handleCreateDraft"
            >
              <el-icon><EditPen /></el-icon>
              创建草稿
            </el-button>
          </div>

          <!-- 空状态 -->
          <div
            v-if="!selectedTemplate || !selectedCaseId"
            class="empty-state"
          >
            <el-empty description="请选择模板和案件" :image-size="80">
              <template #description>
                <p v-if="!selectedTemplate">👈 请先选择一个模板</p>
                <p v-else>请选择一个案件</p>
              </template>
            </el-empty>
          </div>
        </div>
      </el-col>
    </el-row>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Document, View, Download, EditPen } from '@element-plus/icons-vue'
import { tauriCallSafe } from '../../../core/tauriBridge'
import {
  useDocsyBridge,
  mapCaseToTemplate,
  mapToFieldRows,
} from '../composables/useDocsyBridge.js'
import TemplateBrowser from './TemplateBrowser.vue'

const {
  loading: bridgeLoading,
  renderResult,
  renderTemplate,
  exportDocx,
} = useDocsyBridge()

// 状态
const selectedTemplate = ref(null)
const selectedCaseId = ref(null)
const cases = ref([])
const fieldFilter = ref('')
const previewTab = ref('html')
const generating = ref(false)
const previewing = ref(false)
const exporting = ref(false)

// 计算属性
const canGenerate = computed(
  () => selectedTemplate.value && selectedCaseId.value
)

const canExport = computed(
  () => canGenerate.value && renderResult.value
)

const selectedCase = computed(() =>
  cases.value.find((c) => c.id === selectedCaseId.value)
)

// 字段行数据
const fieldRows = computed(() => {
  if (!selectedCase.value) return []
  const values = mapCaseToTemplate(selectedCase.value)
  return mapToFieldRows(values)
})

// 过滤后的字段行
const filteredFieldRows = computed(() => {
  if (!fieldFilter.value) return fieldRows.value
  const lower = fieldFilter.value.toLowerCase()
  return fieldRows.value.filter(
    (r) =>
      r.field.toLowerCase().includes(lower) ||
      r.value.toLowerCase().includes(lower)
  )
})

// 加载案件列表
async function loadCases() {
  const result = await tauriCallSafe('list_cases', { page: 1, perPage: 500 })
  if (result.ok) {
    cases.value = result.data?.items || []
  }
}

// 模板选择回调
function onTemplateSelect(tpl) {
  // 如果已选择案件，自动预览
  if (selectedCaseId.value) {
    handlePreview()
  }
}

// 案件选择回调
function onCaseChange() {
  // 如果已选择模板，自动预览
  if (selectedTemplate.value && selectedCaseId.value) {
    handlePreview()
  }
}

// 预览
async function handlePreview() {
  if (!canGenerate.value) return

  previewing.value = true
  await renderTemplate(selectedTemplate.value.id, selectedCaseId.value)
  previewing.value = false
}

// 生成文书（创建草稿）
async function handleGenerate() {
  if (!canGenerate.value) return

  generating.value = true

  // 先渲染模板
  const result = await renderTemplate(
    selectedTemplate.value.id,
    selectedCaseId.value
  )

  if (!result.ok) {
    generating.value = false
    ElMessage.error('渲染模板失败: ' + result.error)
    return
  }

  // 创建草稿
  const draftResult = await tauriCallSafe('create_draft', {
    title: `${selectedTemplate.value.name} - ${selectedCase.value?.caseName || ''}`,
    content: result.data.html,
    caseId: selectedCaseId.value,
    templatePath: selectedTemplate.value.path,
  })

  generating.value = false

  if (draftResult.ok) {
    ElMessage.success('文书已生成并保存为草稿')
  } else {
    ElMessage.error('创建草稿失败: ' + draftResult.error)
  }
}

// 导出 DOCX
async function handleExport() {
  if (!canExport.value) return

  exporting.value = true

  const result = await exportDocx(
    selectedTemplate.value.id,
    selectedCaseId.value
  )

  exporting.value = false

  if (result.ok) {
    ElMessage.success(`DOCX 已导出: ${result.data.outputPath}`)
    // 询问是否打开文件
    try {
      await ElMessageBox.confirm('是否打开导出的文件？', '导出成功', {
        confirmButtonText: '打开',
        cancelButtonText: '关闭',
        type: 'success',
      })
      // 打开文件
      await tauriCallSafe('open_path', { path: result.data.outputPath })
    } catch {
      // 用户取消，忽略
    }
  } else {
    ElMessage.error('导出失败: ' + result.error)
  }
}

// 创建草稿（不渲染，直接跳转编辑）
async function handleCreateDraft() {
  if (!canGenerate.value) return

  const draftResult = await tauriCallSafe('create_draft', {
    title: `${selectedTemplate.value.name} - ${selectedCase.value?.caseName || ''}`,
    content: '',
    caseId: selectedCaseId.value,
    templatePath: selectedTemplate.value.path,
  })

  if (draftResult.ok) {
    ElMessage.success('草稿已创建，可在文书工坊中编辑')
  } else {
    ElMessage.error('创建草稿失败: ' + draftResult.error)
  }
}

// 字段类型标签
function fieldTypeLabel(type) {
  const labels = {
    text: '文本',
    date: '日期',
    party_list: '当事人',
    checkbox: '勾选',
    radio_group: '单选',
  }
  return labels[type] || type
}

function fieldTypeTag(type) {
  const tags = {
    text: '',
    date: 'warning',
    party_list: 'success',
    checkbox: 'info',
    radio_group: 'danger',
  }
  return tags[type] || ''
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
  background: #f5f7fa;
  border-radius: 8px;
  margin-bottom: 16px;
}

.info-header {
  display: flex;
  align-items: center;
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
  color: #909399;
}

.field-preview {
  margin-bottom: 16px;
}

.preview-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
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
  color: #c0c4cc;
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
