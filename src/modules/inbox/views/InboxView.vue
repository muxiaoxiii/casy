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

// v2.1: 推荐确认弹窗状态
const showConfirmDialog = ref(false)
const confirmItem = ref(null)
const confirmCaseId = ref('')
const confirmFolder = ref('')
const confirmFileName = ref('')
const casesList = ref([])

// v2.1: 快速判断结果缓存
const quickJudgeResults = ref({})

// v2.1: AI 分析结果缓存
const aiResults = ref({})

// v2.1: 拷贝进度
const copyProgress = ref(null)

const categoryLabels = {
  summons: '传票',
  hearing_notice: '口审通知书',
  judgment: '判决/裁定',
  verdict: '判决书',
  ruling: '裁定书',
  complaint: '起诉状',
  defence: '答辩状/代理词',
  official_notice: '审查意见通知书',
  evidence: '证据材料',
  correspondence: '往来函件',
  legal_provision: '法条/法规',
  holiday_notice: '节假日通知',
  cause_action_update: '案由更新',
  defense: '答辩状',
  examination_opinion: '审查意见通知书',
  opposing_counsel: '对方律师函',
  client_instruction: '委托/指示',
  note: '笔记',
  email: '邮件',
  other: '其他',
}

const categoryColors = {
  summons: '#f56c6c',
  hearing_notice: '#e6a23c',
  judgment: '#409eff',
  verdict: '#409eff',
  ruling: '#409eff',
  complaint: '#e6a23c',
  defence: '#909399',
  official_notice: '#409eff',
  evidence: '#67c23a',
  legal_provision: '#909399',
  holiday_notice: '#67c23a',
  examination_opinion: '#409eff',
  opposing_counsel: '#e6a23c',
  correspondence: '#909399',
  note: '#909399',
  other: '#c0c4cc',
}

const folderOptions = [
  { value: '01_传票', label: '01_传票' },
  { value: '02_证据', label: '02_证据' },
  { value: '03_交文', label: '03_交文' },
  { value: '04_收文', label: '04_收文' },
  { value: '05_内部', label: '05_内部' },
  { value: '06_通信', label: '06_通信' },
  { value: '07_其他', label: '07_其他' },
]

const pendingItems = computed(() => items.value.filter((i) => i.status === 'pending'))
const filedItems = computed(() => items.value.filter((i) => i.status === 'filed'))
const archivedItems = computed(() => items.value.filter((i) => i.status === 'archived' || i.status === 'ignored'))

onMounted(() => {
  loadItems()
  loadCases()
})

async function loadItems() {
  loading.value = true
  const result = await tauriCallSafe('list_inbox_items', { status: null })
  if (result.ok) {
    items.value = result.data || []
    // 对每个 pending 项运行快速判断
    for (const item of items.value) {
      if (item.status === 'pending' && !quickJudgeResults.value[item.id]) {
        runQuickJudge(item.id)
      }
    }
  }
  loading.value = false
}

async function loadCases() {
  const result = await tauriCallSafe('list_cases', {})
  if (result.ok) {
    casesList.value = result.data || []
  }
}

// v2.1: 即时判断
async function runQuickJudge(itemId) {
  const result = await tauriCallSafe('quick_judge_inbox_item', { id: itemId })
  if (result.ok) {
    quickJudgeResults.value[itemId] = result.data
  }
}

// v2.1: AI 分析（带缓存）
async function runAiAnalysis(item) {
  processing.value = true
  const result = await tauriCallSafe('ai_analyze_inbox_item', { id: item.id })
  processing.value = false
  if (result.ok) {
    aiResults.value[item.id] = result.data
    if (result.data.cached) {
      ElMessage.info('已分析过，结果如下（缓存）')
    } else {
      ElMessage.success('AI 分析完成')
    }
  }
}

// v2.1: 打开确认弹窗
function openConfirmDialog(item) {
  const judge = quickJudgeResults.value[item.id]
  confirmItem.value = item
  confirmCaseId.value = judge?.recommendations?.[0]?.targetCaseId || item.aiSuggestedCaseId || ''
  confirmFolder.value = judge?.recommendations?.[0]?.targetFolder || '07_其他'
  confirmFileName.value = item.title || ''
  showConfirmDialog.value = true
}

// v2.1: 确认归档
async function doConfirmArchive() {
  if (!confirmCaseId.value) {
    ElMessage.warning('请选择案件')
    return
  }
  showConfirmDialog.value = false
  processing.value = true

  const result = await tauriCallSafe('confirm_inbox_action', {
    inboxItemId: confirmItem.value.id,
    targetCaseId: confirmCaseId.value,
    targetCategory: confirmFolder.value,
  })
  processing.value = false
  if (result.ok) {
    ElMessage.success('已归档')
    await loadItems()
  }
}

// v2.1: 一键确认（strong 推荐直接执行）
async function quickConfirm(item) {
  const judge = quickJudgeResults.value[item.id]
  if (!judge?.recommendations?.length) {
    openConfirmDialog(item)
    return
  }
  const rec = judge.recommendations[0]
  processing.value = true
  const result = await tauriCallSafe('confirm_inbox_action', {
    inboxItemId: item.id,
    targetCaseId: rec.targetCaseId,
    targetCategory: rec.targetFolder || '07_其他',
  })
  processing.value = false
  if (result.ok) {
    ElMessage.success('已归档')
    await loadItems()
  }
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

// v2.1: 置信度 → 强度等级
function getStrength(confidence) {
  if (confidence >= 0.7) return 'strong'
  if (confidence >= 0.3) return 'candidate'
  return 'fallback'
}

function strengthLabel(strength) {
  if (strength === 'strong') return '⚡ 推荐'
  if (strength === 'candidate') return '📋 候选'
  return '⚠️ 需手动选择'
}

function strengthColor(strength) {
  if (strength === 'strong') return '#67c23a'
  if (strength === 'candidate') return '#e6a23c'
  return '#f56c6c'
}

function confidenceColor(confidence) {
  if (confidence >= 0.7) return '#67c23a'
  if (confidence >= 0.3) return '#e6a23c'
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

function formatFileSize(bytes) {
  if (!bytes) return ''
  if (bytes < 1024) return bytes + ' B'
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB'
  return (bytes / (1024 * 1024)).toFixed(1) + ' MB'
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

// ===== 拷贝进度事件监听 =====
let unlistenFns = []

onMounted(async () => {
  await loadItems()

  try {
    const { listen } = await import('@tauri-apps/api/event')

    unlistenFns.push(
      await listen('file-copy-progress', (event) => {
        copyProgress.value = event.payload
      })
    )

    unlistenFns.push(
      await listen('file-verify-failed', (event) => {
        ElMessage.warning(`文件校验失败: ${event.payload.msg}`)
      })
    )

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

    <!-- 拷贝进度条 -->
    <el-dialog v-model="copyProgress" title="正在拷贝文件..." width="400" :close-on-click-modal="false" :show-close="false">
      <div class="copy-progress-dialog">
        <el-progress :percentage="copyProgress?.percent || 0" :stroke-width="12" />
        <div class="copy-progress-info">
          {{ formatFileSize(copyProgress?.copied) }} / {{ formatFileSize(copyProgress?.total) }}
        </div>
      </div>
    </el-dialog>

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
          <div v-for="item in pendingItems" :key="item.id" class="inbox-card" :class="'strength-' + (getStrength(quickJudgeResults[item.id]?.confidence ?? item.aiConfidence ?? 0))">
            <!-- 置信度指示条 -->
            <div
              class="confidence-bar"
              :style="{ background: confidenceColor(quickJudgeResults[item.id]?.confidence ?? item.aiConfidence ?? 0) }"
            />

            <!-- 头部：图标 + 标题 + 快速判断标签 -->
            <div class="inbox-card-header">
              <span class="inbox-icon">
                {{ item.sourceType === 'file' ? '📄' : item.sourceType === 'email' ? '📧' : '📝' }}
              </span>
              <span class="inbox-title">{{ item.title || '无标题' }}</span>
              <el-tag
                v-if="quickJudgeResults[item.id]?.category || item.aiCategory"
                size="small"
                :style="{ background: categoryColors[quickJudgeResults[item.id]?.category || item.aiCategory], color: 'white' }"
              >
                {{ categoryLabels[quickJudgeResults[item.id]?.category || item.aiCategory] || '其他' }}
              </el-tag>
              <span
                class="strength-badge"
                :style="{ color: strengthColor(getStrength(quickJudgeResults[item.id]?.confidence ?? item.aiConfidence ?? 0)) }"
              >
                {{ strengthLabel(getStrength(quickJudgeResults[item.id]?.confidence ?? item.aiConfidence ?? 0)) }}
              </span>
            </div>

            <!-- 快速判断结果 -->
            <div v-if="quickJudgeResults[item.id]" class="quick-judge-panel">
              <div class="confidence-display">
                <span class="confidence-label">置信度：</span>
                <el-progress
                  :percentage="Math.round((quickJudgeResults[item.id].confidence || 0) * 100)"
                  :stroke-width="6"
                  :color="confidenceColor(quickJudgeResults[item.id].confidence || 0)"
                  style="flex: 1; max-width: 120px;"
                />
                <span class="confidence-value" :style="{ color: confidenceColor(quickJudgeResults[item.id].confidence || 0) }">
                  {{ Math.round((quickJudgeResults[item.id].confidence || 0) * 100) }}%
                </span>
              </div>

              <!-- 推荐列表 -->
              <div v-if="quickJudgeResults[item.id].recommendations?.length" class="recommendations">
                <div
                  v-for="(rec, idx) in quickJudgeResults[item.id].recommendations"
                  :key="idx"
                  class="recommendation-item"
                  :class="{ 'recommendation-default': idx === 0 && quickJudgeResults[item.id].strength === 'strong' }"
                >
                  <span class="rec-icon">{{ idx === 0 && quickJudgeResults[item.id].strength === 'strong' ? '●' : '○' }}</span>
                  <span class="rec-text">
                    归档到「{{ rec.targetCaseName || rec.targetCaseId }}」的 {{ rec.targetFolder || '07_其他' }} 目录
                  </span>
                  <span class="rec-reason">{{ rec.reason }}</span>
                </div>
              </div>
              <div v-else class="no-recommendation">
                ⚠️ 未匹配到案件，请手动选择操作
              </div>
            </div>

            <!-- AI 提取结果（如果已分析） -->
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
              <div v-if="item.aiExtracted.parties?.length" class="extracted-field">
                <span class="field-label">当事人：</span>
                <span class="field-value">{{ formatExtractedField(item.aiExtracted.parties) }}</span>
              </div>
              <div v-if="item.aiExtracted.hearing_date" class="extracted-field">
                <span class="field-label">庭审日期：</span>
                <span class="field-value">{{ item.aiExtracted.hearing_date }}</span>
              </div>
            </div>

            <!-- AI 分析结果缓存提示 -->
            <div v-if="aiResults[item.id]?.cached" class="ai-cached-notice">
              ℹ️ {{ aiResults[item.id].message }}
            </div>

            <!-- 内容预览 -->
            <div v-if="item.contentText" class="inbox-preview">
              {{ item.contentText.slice(0, 200) }}{{ item.contentText.length > 200 ? '...' : '' }}
            </div>

            <!-- 操作按钮 -->
            <div class="inbox-actions">
              <!-- Strong 推荐：一键确认 -->
              <el-button
                v-if="getStrength(quickJudgeResults[item.id]?.confidence ?? 0) === 'strong'"
                size="small"
                type="success"
                @click="quickConfirm(item)"
              >
                ✅ 确认归档
              </el-button>
              <!-- Candidate/Fallback：打开确认弹窗 -->
              <el-button
                v-else
                size="small"
                type="primary"
                @click="openConfirmDialog(item)"
              >
                📁 选择归档
              </el-button>
              <!-- AI 分析按钮 -->
              <el-button
                v-if="quickJudgeResults[item.id]?.aiAvailable !== false"
                size="small"
                type="info"
                :loading="processing"
                @click="runAiAnalysis(item)"
              >
                🤖 {{ item.aiAnalyzed ? '查看AI分析' : 'AI 分析' }}
              </el-button>
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
            <div v-if="item.filedTo" class="filed-info">
              归档到：{{ item.filedTo }} / {{ item.filedAs }}
            </div>
          </div>
        </div>
      </el-tab-pane>

      <el-tab-pane label="已忽略" name="archived">
        <div v-if="!archivedItems.length" class="empty-state">
          <el-empty description="暂无忽略记录" :image-size="60" />
        </div>
        <div v-else class="inbox-list">
          <div v-for="item in archivedItems" :key="item.id" class="inbox-card filed">
            <div class="inbox-card-header">
              <span class="inbox-icon">🚫</span>
              <span class="inbox-title">{{ item.title || '无标题' }}</span>
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

    <!-- v2.1: 推荐确认弹窗 -->
    <el-dialog v-model="showConfirmDialog" title="确认归档" width="560">
      <div v-if="confirmItem" class="confirm-dialog-content">
        <div class="confirm-file-info">
          <span class="confirm-icon">📄</span>
          <span class="confirm-filename">{{ confirmItem.title || '无标题' }}</span>
        </div>

        <el-form label-width="60px" class="confirm-form">
          <el-form-item label="案件">
            <el-select v-model="confirmCaseId" filterable placeholder="选择案件" style="width: 100%;">
              <el-option
                v-for="c in casesList"
                :key="c.id"
                :label="c.displayName || c.display_name || c.caseName || c.case_name"
                :value="c.id"
              />
            </el-select>
          </el-form-item>
          <el-form-item label="目录">
            <el-select v-model="confirmFolder" style="width: 100%;">
              <el-option v-for="f in folderOptions" :key="f.value" :label="f.label" :value="f.value" />
            </el-select>
          </el-form-item>
          <el-form-item label="文件名">
            <el-input v-model="confirmFileName" placeholder="文件名" />
          </el-form-item>
        </el-form>
      </div>
      <template #footer>
        <el-button @click="showConfirmDialog = false">取消</el-button>
        <el-button type="primary" :loading="processing" @click="doConfirmArchive">确认归档</el-button>
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

/* v2.1: 强度边框 */
.inbox-card.strength-strong {
  border-left: 3px solid #67c23a;
}

.inbox-card.strength-candidate {
  border-left: 3px solid #e6a23c;
}

.inbox-card.strength-fallback {
  border-left: 3px solid #f56c6c;
}

.confidence-bar {
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 4px;
}

/* v2.1: 快速判断面板 */
.quick-judge-panel {
  background: #f8f9fa;
  border-radius: 6px;
  padding: 10px 12px;
  margin-bottom: 8px;
}

.confidence-display {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
  font-size: 13px;
}

.confidence-label {
  color: #909399;
  white-space: nowrap;
}

.confidence-value {
  font-weight: 600;
  font-size: 14px;
  min-width: 36px;
  text-align: right;
}

/* v2.1: 推荐列表 */
.recommendations {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.recommendation-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 8px;
  border-radius: 4px;
  font-size: 13px;
  background: #fff;
  border: 1px solid #e8e8e8;
}

.recommendation-default {
  border-color: #67c23a;
  background: #f0f9eb;
}

.rec-icon {
  flex-shrink: 0;
  color: #67c23a;
}

.rec-text {
  flex: 1;
  color: #303133;
}

.rec-reason {
  color: #909399;
  font-size: 12px;
}

.no-recommendation {
  color: #e6a23c;
  font-size: 13px;
}

/* v2.1: 强度标签 */
.strength-badge {
  font-size: 12px;
  font-weight: 500;
  white-space: nowrap;
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

.ai-cached-notice {
  background: #ecf5ff;
  border-radius: 4px;
  padding: 6px 10px;
  margin-bottom: 8px;
  font-size: 12px;
  color: #409eff;
}

.filed-info {
  font-size: 12px;
  color: #909399;
  margin-top: 4px;
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

/* 确认弹窗 */
.confirm-dialog-content {
  padding: 0 8px;
}

.confirm-file-info {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 16px;
  padding: 10px;
  background: #f8f9fa;
  border-radius: 6px;
}

.confirm-icon {
  font-size: 20px;
}

.confirm-filename {
  font-weight: 500;
}

.confirm-form {
  margin-top: 12px;
}

/* 拷贝进度 */
.copy-progress-dialog {
  text-align: center;
  padding: 12px 0;
}

.copy-progress-info {
  margin-top: 12px;
  color: #909399;
  font-size: 13px;
}
</style>
