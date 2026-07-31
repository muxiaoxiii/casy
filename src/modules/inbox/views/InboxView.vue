<script setup>
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { tauriCallSafe } from '../../../core/tauriBridge.js'
import { useInboxStore } from '../../../stores/inbox.js'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Connection } from '@element-plus/icons-vue'

const inboxStore = useInboxStore()
const items = ref([])
const loading = ref(false)
const activeTab = ref('pending')
const showAddNoteDialog = ref(false)
const newNote = ref('')
const processing = ref(false)
const isDragging = ref(false)
let dragCounter = 0

const categoryLabels = {
  summons: '传票',
  hearing_notice: '口审通知书',
  judgment: '判决/裁定',
  evidence: '证据',
  legal_provision: '法条',
  holiday_notice: '节假日通知',
  cause_action_update: '案由更新',
  complaint: '起诉状',
  defense: '答辩状',
  examination_opinion: '审查意见通知书',
  opposing_counsel: '对方律师函',
  correspondence: '函件',
  client_instruction: '委托/指示',
  note: '笔记',
  email: '邮件',
  other: '其他',
}

const categoryColors = {
  summons: '#f56c6c',
  hearing_notice: '#e6a23c',
  judgment: '#409eff',
  evidence: '#67c23a',
  legal_provision: '#909399',
  holiday_notice: '#67c23a',
  examination_opinion: '#409eff',
  opposing_counsel: '#e6a23c',
  correspondence: '#909399',
  note: '#909399',
  other: '#c0c4cc',
}

const pendingItems = computed(() => items.value.filter((i) => i.status === 'pending'))
const filedItems = computed(() => items.value.filter((i) => i.status === 'filed'))

onMounted(() => {
  loadItems()
})

async function loadItems() {
  loading.value = true
  const result = await tauriCallSafe('list_inbox_items', { status: null })
  if (result.ok) {
    items.value = result.data || []
  }
  loading.value = false
}

async function addNote() {
  if (!newNote.value.trim()) return
  processing.value = true
  const result = await tauriCallSafe('add_inbox_item', {
    sourceType: 'note',
    contentText: newNote.value,
  })
  processing.value = false
  if (result.ok) {
    ElMessage.success('已添加到收件箱')
    showAddNoteDialog.value = false
    newNote.value = ''
    await loadItems()
  }
}

async function processItem(item) {
  processing.value = true
  const result = await tauriCallSafe('process_inbox_item', { id: item.id })
  processing.value = false
  if (result.ok) {
    ElMessage.success(`已分类：${categoryLabels[result.data.category] || result.data.category}`)
    await loadItems()
  }
}

async function acceptSuggestedCase(item) {
  if (!item.aiSuggestedCaseId) return
  processing.value = true
  const result = await tauriCallSafe('file_inbox_item', {
    itemId: item.id,
    caseId: item.aiSuggestedCaseId,
    category: item.aiCategory || 'other',
  })
  processing.value = false
  if (result.ok) {
    ElMessage.success('已关联到案件')
    await loadItems()
  }
}

async function dismissItem(item) {
  const result = await tauriCallSafe('dismiss_inbox_item', { id: item.id })
  if (result.ok) {
    ElMessage.success('已忽略')
    await loadItems()
  }
}

async function importFile() {
  const { open } = await import('@tauri-apps/plugin-dialog')
  const selected = await open({
    multiple: true,
    filters: [
      { name: '文档', extensions: ['pdf', 'docx', 'doc', 'eml', 'txt'] },
      { name: '图片', extensions: ['jpg', 'jpeg', 'png', 'bmp'] },
    ],
  })
  if (!selected) return

  const files = Array.isArray(selected) ? selected : [selected]
  processing.value = true
  for (const file of files) {
    await tauriCallSafe('add_inbox_item', {
      sourceType: 'file',
      sourcePath: file,
      title: file.split('/').pop(),
    })
  }
  processing.value = false
  ElMessage.success(`已导入 ${files.length} 个文件`)
  await loadItems()
}

function confidenceColor(confidence) {
  if (confidence >= 0.8) return '#67c23a'
  if (confidence >= 0.5) return '#e6a23c'
  return '#f56c6c'
}

function formatExtractedField(value) {
  if (value == null) return null
  if (typeof value === 'string') return value
  if (Array.isArray(value)) {
    return value
      .map((v) => {
        if (typeof v === 'object' && v.name) return `${v.name}(${v.role || ''})`
        return String(v)
      })
      .join('、')
  }
  if (typeof value === 'object') return JSON.stringify(value)
  return String(value)
}

// ===== 拖拽上传 =====
function onDragEnter(e) {
  e.preventDefault()
  dragCounter++
  isDragging.value = true
}

function onDragOver(e) {
  e.preventDefault()
}

function onDragLeave(e) {
  e.preventDefault()
  dragCounter--
  if (dragCounter === 0) isDragging.value = false
}

async function onDrop(e) {
  e.preventDefault()
  isDragging.value = false
  dragCounter = 0

  const files = e.dataTransfer?.files
  if (!files || files.length === 0) return

  processing.value = true
  let count = 0
  for (const file of files) {
    // 获取文件路径（Tauri 环境）
    const filePath = file.path || file.name
    const result = await tauriCallSafe('add_inbox_item', {
      sourceType: 'file',
      sourcePath: filePath,
      title: file.name,
    })
    if (result.ok) count++
  }
  processing.value = false
  if (count > 0) {
    ElMessage.success(`已导入 ${count} 个文件`)
    await loadItems()
  }
}

// ===== 系统托盘事件监听 =====
let unlistenFns = []

onMounted(async () => {
  await loadItems()

  // 监听托盘事件
  try {
    const { listen } = await import('@tauri-apps/api/event')

    unlistenFns.push(
      await listen('tray:add_file', () => {
        importFile()
      })
    )

    unlistenFns.push(
      await listen('tray:add_note', () => {
        showAddNoteDialog.value = true
      })
    )

    unlistenFns.push(
      await listen('tray:clipboard_to_inbox', async () => {
        try {
          const text = await navigator.clipboard.readText()
          if (text && text.trim()) {
            const result = await tauriCallSafe('add_inbox_item', {
              sourceType: 'clipboard',
              contentText: text,
              title: '剪贴板内容',
            })
            if (result.ok) {
              ElMessage.success('剪贴板内容已添加到收件箱')
              await loadItems()
            }
          } else {
            ElMessage.warning('剪贴板为空')
          }
        } catch (err) {
          ElMessage.error('无法读取剪贴板: ' + err.message)
        }
      })
    )

    unlistenFns.push(
      await listen('inbox:new_item', () => {
        loadItems()
      })
    )
  } catch (err) {
    console.warn('事件监听注册失败:', err)
  }
})

onUnmounted(() => {
  unlistenFns.forEach((fn) => fn())
})
</script>

<template>
  <div class="inbox-page">
    <div class="toolbar">
      <h3>📥 收件箱</h3>
      <div class="toolbar-actions">
        <el-button size="small" @click="importFile" :loading="processing">📎 导入文件</el-button>
        <el-button size="small" @click="showAddNoteDialog = true">📝 添加笔记</el-button>
        <el-button size="small" @click="loadItems">刷新</el-button>
      </div>
    </div>

    <!-- 拖拽上传区 -->
    <div
      class="drop-zone"
      :class="{ 'drop-zone-active': isDragging }"
      @dragenter="onDragEnter"
      @dragover="onDragOver"
      @dragleave="onDragLeave"
      @drop="onDrop"
    >
      <div v-if="isDragging" class="drop-hint">
        <span class="drop-icon">📥</span>
        <span>松开鼠标，导入文件到收件箱</span>
      </div>
      <div v-else class="drop-idle">
        <span class="drop-icon">📎</span>
        <span>拖拽文件到此处快速导入</span>
      </div>
    </div>

    <el-tabs v-model="activeTab">
      <el-tab-pane label="待处理" name="pending">
        <div v-if="loading" class="skeleton-wrapper">
          <el-skeleton :rows="4" animated>
            <template #template>
              <div v-for="i in 3" :key="i" class="skeleton-inbox-card">
                <div class="skeleton-inbox-header">
                  <el-skeleton-item variant="circle" style="width: 24px; height: 24px;" />
                  <el-skeleton-item variant="text" style="width: 40%; height: 20px;" />
                  <el-skeleton-item variant="text" style="width: 15%; height: 20px;" />
                </div>
                <el-skeleton-item variant="rect" style="width: 100%; height: 40px; margin: 8px 0;" />
                <div style="display: flex; justify-content: flex-end; gap: 8px;">
                  <el-skeleton-item variant="button" style="width: 60px; height: 28px;" />
                  <el-skeleton-item variant="button" style="width: 60px; height: 28px;" />
                </div>
              </div>
            </template>
          </el-skeleton>
        </div>
        <div v-else-if="!pendingItems.length" class="empty-state">
          <el-empty description="收件箱是空的" :image-size="60">
            <el-button @click="importFile">导入文件</el-button>
          </el-empty>
        </div>
        <div v-else class="inbox-list">
          <div v-for="item in pendingItems" :key="item.id" class="inbox-card">
            <div v-if="item.aiConfidence != null" class="confidence-bar" :style="{ background: confidenceColor(item.aiConfidence) }" />
            <div class="inbox-card-header">
              <span class="inbox-icon">
                {{ item.sourceType === 'file' ? '📄' : item.sourceType === 'email' ? '📧' : '📝' }}
              </span>
              <span class="inbox-title">{{ item.title || '无标题' }}</span>
              <el-tag
                v-if="item.aiCategory"
                size="small"
                :style="{ background: categoryColors[item.aiCategory], color: 'white' }"
              >
                {{ categoryLabels[item.aiCategory] || item.aiCategory }}
              </el-tag>
              <span v-if="item.aiConfidence != null" class="confidence" :style="{ color: confidenceColor(item.aiConfidence) }">
                {{ Math.round(item.aiConfidence * 100) }}%
              </span>
            </div>
            <!-- AI 提取结果详情 -->
            <div v-if="item.aiCategory" class="ai-details">
              <span class="ai-detail-label">AI 分类：</span>
              <el-tag size="small" :style="{ background: categoryColors[item.aiCategory], color: 'white' }">
                {{ categoryLabels[item.aiCategory] || item.aiCategory }}
              </el-tag>
              <span v-if="item.aiConfidence != null" class="ai-confidence-text" :style="{ color: confidenceColor(item.aiConfidence) }">
                置信度 {{ Math.round(item.aiConfidence * 100) }}%
              </span>
            </div>
            <!-- AI 提取的结构化信息 -->
            <div v-if="item.aiExtracted" class="ai-extracted-info">
              <div v-if="item.aiExtracted.case_no" class="extracted-field">
                <span class="field-label">案号：</span>
                <span class="field-value">{{ item.aiExtracted.case_no }}</span>
              </div>
              <div v-if="item.aiExtracted.court" class="extracted-field">
                <span class="field-label">法院：</span>
                <span class="field-value">{{ item.aiExtracted.court }}</span>
              </div>
              <div v-if="item.aiExtracted.patent_no" class="extracted-field">
                <span class="field-label">专利号：</span>
                <span class="field-value">{{ item.aiExtracted.patent_no }}</span>
              </div>
              <div v-if="item.aiExtracted.parties && item.aiExtracted.parties.length > 0" class="extracted-field">
                <span class="field-label">当事人：</span>
                <span class="field-value">{{ formatExtractedField(item.aiExtracted.parties) }}</span>
              </div>
              <div v-if="item.aiExtracted.hearing_date" class="extracted-field">
                <span class="field-label">庭审日期：</span>
                <span class="field-value">{{ item.aiExtracted.hearing_date }}</span>
              </div>
              <div v-if="item.aiExtracted.deadline" class="extracted-field">
                <span class="field-label">期限：</span>
                <span class="field-value deadline-warn">{{ item.aiExtracted.deadline }}</span>
              </div>
            </div>
            <!-- AI 建议关联的案件 -->
            <div v-if="item.aiSuggestedCaseId" class="ai-suggestion">
              <el-icon><Connection /></el-icon>
              <span>AI 建议关联案件</span>
              <el-button size="small" type="success" @click="acceptSuggestedCase(item)">
                接受关联
              </el-button>
            </div>
            <div v-if="item.contentText" class="inbox-preview">
              {{ item.contentText.slice(0, 200) }}{{ item.contentText.length > 200 ? '...' : '' }}
            </div>
            <div class="inbox-actions">
              <el-button size="small" type="primary" @click="processItem(item)">重新分析</el-button>
              <el-button size="small" @click="dismissItem(item)">忽略</el-button>
            </div>
          </div>
        </div>
      </el-tab-pane>

      <el-tab-pane label="已归档" name="filed">
        <div v-if="!filedItems.length" class="empty-state">
          <el-empty description="暂无归档记录" :image-size="60" />
        </div>
        <div v-else class="inbox-list">
          <div v-for="item in filedItems" :key="item.id" class="inbox-card filed">
            <div class="inbox-card-header">
              <span class="inbox-icon">✅</span>
              <span class="inbox-title">{{ item.title || '无标题' }}</span>
              <el-tag size="small" type="info">{{ categoryLabels[item.aiCategory] || '已归档' }}</el-tag>
            </div>
          </div>
        </div>
      </el-tab-pane>
    </el-tabs>

    <!-- 添加笔记弹窗 -->
    <el-dialog v-model="showAddNoteDialog" title="添加笔记" width="500">
      <el-input v-model="newNote" type="textarea" :rows="5" placeholder="输入笔记内容..." />
      <template #footer>
        <el-button @click="showAddNoteDialog = false">取消</el-button>
        <el-button type="primary" :loading="processing" @click="addNote">添加</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.inbox-page {
  max-width: 900px;
  margin: 0 auto;
}

.toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
}

.toolbar h3 {
  margin: 0;
}

.toolbar-actions {
  display: flex;
  gap: 8px;
}

/* 拖拽上传区 */
.drop-zone {
  border: 2px dashed #dcdfe6;
  border-radius: 8px;
  padding: 20px;
  text-align: center;
  margin-bottom: 16px;
  transition: all 0.3s;
  cursor: pointer;
}

.drop-zone:hover {
  border-color: #409eff;
  background: #f5f7ff;
}

.drop-zone-active {
  border-color: #409eff;
  background: #ecf5ff;
  transform: scale(1.01);
}

.drop-hint,
.drop-idle {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  color: #909399;
  font-size: 14px;
}

.drop-zone-active .drop-hint {
  color: #409eff;
  font-weight: 500;
}

.drop-icon {
  font-size: 20px;
}

.loading-state, .empty-state {
  text-align: center;
  padding: 40px;
  color: #666;
}

.inbox-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.inbox-card {
  border: 1px solid #e0e0e0;
  border-radius: 8px;
  padding: 12px;
  transition: box-shadow 0.2s;
  position: relative;
  overflow: hidden;
}

.inbox-card:hover {
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

.inbox-card.filed {
  opacity: 0.7;
}

.confidence-bar {
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 4px;
}

.ai-details {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: #909399;
  margin-bottom: 6px;
  flex-wrap: wrap;
}

.ai-detail-label {
  color: #909399;
}

.ai-confidence-text {
  font-weight: 500;
  font-size: 12px;
}

.ai-summary {
  color: #606266;
  font-size: 12px;
}

.ai-extracted-info {
  background: #f8f9fa;
  border-radius: 6px;
  padding: 8px 12px;
  margin-bottom: 8px;
  font-size: 12px;
}

.extracted-field {
  display: flex;
  gap: 8px;
  margin-bottom: 4px;
}

.extracted-field:last-child {
  margin-bottom: 0;
}

.field-label {
  color: #909399;
  min-width: 60px;
}

.field-value {
  color: #303133;
}

.deadline-warn {
  color: #e6a23c;
  font-weight: 500;
}

.ai-suggestion {
  display: flex;
  align-items: center;
  gap: 8px;
  background: #ecf5ff;
  border-radius: 6px;
  padding: 8px 12px;
  margin-bottom: 8px;
  font-size: 13px;
  color: #409eff;
}

.ai-suggestion .el-icon {
  font-size: 16px;
}

.inbox-card-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}

.inbox-icon {
  font-size: 18px;
}

.inbox-title {
  flex: 1;
  font-weight: 500;
}

.confidence {
  font-size: 12px;
  font-weight: 500;
}

.inbox-preview {
  font-size: 13px;
  color: #666;
  margin-bottom: 8px;
  white-space: pre-wrap;
  line-height: 1.5;
}

.inbox-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}

.skeleton-wrapper {
  padding: 16px 0;
}

.skeleton-inbox-card {
  border: 1px solid #e0e0e0;
  border-radius: 8px;
  padding: 12px;
  margin-bottom: 12px;
}

.skeleton-inbox-header {
  display: flex;
  align-items: center;
  gap: 8px;
}
</style>
