<script setup>
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { casyContext } from '../../../core/plugin/context'
import { useInboxStore } from '../../../stores/inbox'
import { useCapture } from '../composables/useCapture'
import { useVoiceNote } from '../composables/useVoiceNote'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Connection, Paperclip, DocumentCopy, Camera, EditPen, Folder, Finished, Calendar, Collection, Briefcase, Bell, Download } from '@element-plus/icons-vue'

const inboxStore = useInboxStore()
const { captureScreenshot, captureClipboard, startClipboardMonitor } = useCapture()

// 语音速记（设计哲学 §10）：录音 → save_voice_note → add_inbox_item
const { isRecording, recordingTime, startRecording, stopRecording, formatTime } = useVoiceNote()
const micAvailable = ref(true)

async function toggleVoiceNote() {
  if (isRecording.value) {
    stopRecording()
    // 保存后刷新收件箱（onstop 为异步，稍等落库）
    setTimeout(() => loadItems(), 800)
    return
  }
  try {
    await startRecording()
  } catch (err) {
    micAvailable.value = false
    ElMessage.warning('麦克风不可用或无权限，语音速记已禁用')
  }
}
const items = ref([])
const loading = ref(false)
const activeTab = ref('pending')
const showAddNoteDialog = ref(false)
const newNote = ref('')
const processing = ref(false)
const isDragging = ref(false)
const quickCaptureText = ref('')
let dragCounter = 0

// v2.1: 推荐确认弹窗状态（设计哲学 §10：推荐动作 → 一键推送）
const showConfirmDialog = ref(false)
const confirmItem = ref(null)
const confirmRecommendations = ref([])
const confirmAction = ref('file_to_case')
const confirmCaseId = ref('')
const confirmFolder = ref('')
const confirmFileName = ref('')
const casesList = ref([])

// 推荐动作元数据（意图 → 按钮/文案；后端 quick_judge 返回 action 字段）
const ACTION_META = {
  file_to_case: { label: '归入案件', icon: Folder, desc: (rec) => '归档到「' + (rec.targetCaseName || rec.targetCaseId || '案件') + '」的 ' + (rec.targetFolder || '07_其他') + ' 目录' },
  create_task: { label: '转为任务', icon: Finished, desc: (rec) => '创建任务：' + (rec.intent?.taskName || rec.intent?.name || '') },
  create_deadline: { label: '记为期限', icon: Calendar, desc: (rec) => '记期限：' + (rec.intent?.name || '') + (rec.intent?.dueDate ? '（' + rec.intent.dueDate + '）' : '') },
  save_knowledge: { label: '存入知识库', icon: Collection, desc: (rec) => '存入知识库：' + (rec.intent?.title || '') },
  create_case: { label: '新建案件', icon: Briefcase, desc: (rec) => '新建案件：' + (rec.intent?.caseName || '') },
  set_reminder: { label: '设置提醒', icon: Bell, desc: (rec) => '设置提醒：' + (rec.intent?.title || '') + (rec.intent?.remindAt ? '（' + rec.intent.remindAt + '）' : '') },
  service_delivery: { label: '抓取送达文书', icon: Download, desc: (rec) => '抓取送达文书：' + (rec.intent?.caseNo || '') + '（' + (rec.intent?.recipientName || '收件人') + '）' },
}

function getActionMeta(action) {
  return ACTION_META[action] || ACTION_META.file_to_case
}

// 推荐列表：本地规则优先；本地 fallback 时用 AI 意图兜底（§10：AI 兜底增强）
function getRecommendations(item) {
  const judge = quickJudgeResults.value[item.id]
  if (judge?.recommendations?.length) return judge.recommendations
  const aiIntent = aiResults.value[item.id]?.intent
  if (aiIntent?.action) {
    return [{
      action: aiIntent.action,
      targetCaseId: item.aiSuggestedCaseId || null,
      targetCaseName: item.aiSuggestedCaseName || null,
      targetFolder: null,
      intent: aiIntent,
      reason: 'AI 识别为' + (aiIntent.docType || aiIntent.action),
    }]
  }
  return []
}

// v2.1: 快速判断结果缓存
const quickJudgeResults = ref({})

// v2.1: AI 分析结果缓存
const aiResults = ref({})

// v2.1: 拷贝进度
const copyProgress = ref(null)

// 语音转写（transcribe_voice_note，需 OpenAI 兼容 STT；失败显示后端友好文案）
const transcribingIds = ref({})
const AUDIO_EXT_RE = /\.(webm|ogg|mp3|m4a|wav)$/i

/** 判断收件项是否关联音频（宽松：source_path 命中音频扩展名，或语音类来源） */
function isAudioItem(item) {
  const p = item.sourcePath || item.source_path || ''
  if (AUDIO_EXT_RE.test(p)) return true
  return item.sourceType === 'voice' || item.sourceType === 'audio'
}

async function transcribeItem(item) {
  if (transcribingIds.value[item.id]) return
  transcribingIds.value = { ...transcribingIds.value, [item.id]: true }
  const result = await casyContext.inbox.transcribeVoiceNote(item.id)
  transcribingIds.value = { ...transcribingIds.value, [item.id]: false }
  if (result.ok) {
    ElMessage.success('转写完成')
    // 转写文本已由后端写回 content_text，刷新列表
    await loadItems()
  } else {
    ElMessage.warning(result.error || '转写失败')
  }
}

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
  const result = await casyContext.inbox.list()
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
  const result = await casyContext.cases.list({})
  if (result.ok) {
    casesList.value = result.data || []
  }
}

// v2.1: 即时判断
async function runQuickJudge(itemId) {
  const result = await casyContext.inbox.quickJudge(itemId)
  if (result.ok) {
    quickJudgeResults.value[itemId] = result.data
  }
}

// v2.1: AI 分析（带缓存）
async function runAiAnalysis(item) {
  processing.value = true
  const result = await casyContext.inbox.aiAnalyze(item.id)
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
  confirmRecommendations.value = getRecommendations(item)
  confirmAction.value = getRecommendations(item)[0]?.action || 'file_to_case'
  confirmCaseId.value = judge?.recommendations?.[0]?.targetCaseId || item.aiSuggestedCaseId || ''
  confirmFolder.value = judge?.recommendations?.[0]?.targetFolder || '07_其他'
  confirmFileName.value = item.title || ''
  showConfirmDialog.value = true
}

// v2.1: 确认推荐动作（设计哲学 §10：推荐按钮 → 一键自行推送）
async function doConfirmArchive() {
  const rec = confirmRecommendations.value.find(r => r.action === confirmAction.value) || confirmRecommendations.value[0]
  if (rec?.action === 'file_to_case' && !confirmCaseId.value) {
    ElMessage.warning('请选择案件')
    return
  }
  showConfirmDialog.value = false
  processing.value = true

  const result = await casyContext.inbox.confirmAction({
    inboxItemId: confirmItem.value.id,
    targetCaseId: rec?.action === 'file_to_case' ? (confirmCaseId.value || rec?.targetCaseId) : null,
    targetCategory: rec?.action === 'file_to_case' ? (confirmFolder.value || rec?.targetFolder) : null,
    action: rec?.action || 'file_to_case',
    intent: rec?.intent || null,
  })
  processing.value = false
  if (result.ok) {
    ElMessage.success(getActionMeta(rec?.action).label + '成功')
    await loadItems()
  }
}

// v2.1: 一键确认（strong 推荐直接执行：按意图自行推送）
async function quickConfirm(item) {
  const recs = getRecommendations(item)
  if (!recs.length) {
    openConfirmDialog(item)
    return
  }
  const rec = recs[0]
  processing.value = true
  const result = await casyContext.inbox.confirmAction({
    inboxItemId: item.id,
    targetCaseId: rec.targetCaseId || null,
    targetCategory: rec.targetFolder || null,
    action: rec.action,
    intent: rec.intent || null,
  })
  processing.value = false
  if (result.ok) {
    ElMessage.success(getActionMeta(rec.action).label + '成功')
    await loadItems()
  }
}

// v2.1: 拒绝推荐（设计哲学 §10：推荐拒绝 → 学习信号）
async function rejectRecommendation(item) {
  const recs = getRecommendations(item)
  if (!recs.length) return
  
  // 弹出原因选择（可选）
  try {
    const { value: reason } = await ElMessageBox.prompt(
      '请选择不采纳原因（可选，帮助改进推荐）',
      '不采纳推荐',
      {
        confirmButtonText: '确认',
        cancelButtonText: '跳过',
        inputPlaceholder: '如：信息不准确 / 不需要处理 / 手动处理更好',
        inputType: 'textarea',
      }
    )
    
    processing.value = true
    const result = await casyContext.inbox.rejectRecommendation({
      inboxItemId: item.id,
      action: recs[0].action,
      reason: reason || null,
      intent: recs[0].intent || null,
    })
    processing.value = false
    
    if (result.ok) {
      ElMessage.success('已记录反馈，推荐将优化')
      // 清除该条目的推荐缓存，避免重复显示
      delete quickJudgeResults.value[item.id]
      delete aiResults.value[item.id]
    }
  } catch {
    // 用户点击"跳过"，直接记录拒绝但不带原因
    processing.value = true
    await casyContext.inbox.rejectRecommendation({
      inboxItemId: item.id,
      action: recs[0].action,
      reason: null,
      intent: recs[0].intent || null,
    })
    processing.value = false
    ElMessage.success('已记录反馈')
    delete quickJudgeResults.value[item.id]
    delete aiResults.value[item.id]
  }
}

// 快速捕获（回车即入袋，设计哲学 §10.2）
async function quickCapture() {
  if (!quickCaptureText.value.trim()) return
  processing.value = true
  const result = await casyContext.inbox.add('note', quickCaptureText.value)
  processing.value = false
  if (result.ok) {
    ElMessage.success('已捕获到收件箱')
    quickCaptureText.value = ''
    await loadItems()
  }
}

// 截屏捕获（设计哲学 §10）
async function handleScreenshot() {
  const result = await captureScreenshot()
  if (result) {
    ElMessage.success('截屏已捕获到收件箱')
    await loadItems()
  } else {
    ElMessage.error('截屏失败')
  }
}

// 从剪贴板粘贴
async function pasteFromClipboard() {
  try {
    const text = await navigator.clipboard.readText()
    if (text && text.trim()) {
      processing.value = true
      const result = await casyContext.inbox.add('paste', text)
      processing.value = false
      if (result.ok) {
        ElMessage.success('剪贴板内容已捕获')
        await loadItems()
      }
    } else {
      ElMessage.warning('剪贴板为空')
    }
  } catch (err) {
    ElMessage.error('无法读取剪贴板')
  }
}

async function addNote() {
  if (!newNote.value.trim()) return
  processing.value = true
  const result = await casyContext.inbox.add('note', newNote.value)
  processing.value = false
  if (result.ok) {
    ElMessage.success('已添加到收件箱')
    showAddNoteDialog.value = false
    newNote.value = ''
    await loadItems()
  }
}

async function dismissItem(item) {
  const result = await casyContext.inbox.dismiss(item.id)
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
    await casyContext.inbox.add('file', null, file)
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

// ===== 批量 AI 分类（事件驱动进度，失败回退轮询） =====
const batch = ref({ total: 0, processed: 0, failed: 0, active: 0, currentItem: null, running: false })
const batchPaused = ref(false) // paused 不在进度 payload 中，由前端本地跟踪
const batchStarting = ref(false)
let batchPollTimer = null

const batchPercent = computed(() => {
  if (!batch.value.total) return 0
  return Math.min(100, Math.round(((batch.value.processed + batch.value.failed) / batch.value.total) * 100))
})

function applyBatchProgress(p) {
  if (!p) return
  batch.value = {
    total: p.total || 0,
    processed: p.processed || 0,
    failed: p.failed || 0,
    active: p.active || 0,
    currentItem: p.currentItem ?? null,
    running: !!p.running,
  }
  if (!p.running) batchPaused.value = false
}

function onBatchFinished(payload) {
  batch.value.running = false
  batchPaused.value = false
  const ok = payload?.processed ?? batch.value.processed
  const fail = payload?.failed ?? batch.value.failed
  if (fail > 0) {
    ElMessage.warning(`处理完成：成功 ${ok}，失败 ${fail}`)
  } else {
    ElMessage.success(`处理完成：成功 ${ok}，失败 ${fail}`)
  }
  loadItems()
}

async function startBatch() {
  batchStarting.value = true
  const result = await casyContext.inbox.startBatch()
  batchStarting.value = false
  if (result.ok) {
    batchPaused.value = false
    applyBatchProgress(result.data)
  } else {
    ElMessage.error(result.error || '启动批量分类失败')
  }
}

async function pauseBatch() {
  const result = await casyContext.inbox.pauseBatch()
  if (result.ok) {
    batchPaused.value = true
  } else {
    ElMessage.error(result.error || '暂停失败')
  }
}

async function resumeBatch() {
  const result = await casyContext.inbox.resumeBatch()
  if (result.ok) {
    batchPaused.value = false
  } else {
    ElMessage.error(result.error || '恢复失败')
  }
}

async function cancelBatch() {
  const result = await casyContext.inbox.cancelBatch()
  if (result.ok) {
    batchPaused.value = false
  } else {
    ElMessage.error(result.error || '取消失败')
  }
}

// 事件监听失败时的降级：500ms 轮询 get_inbox_progress
function startBatchPolling() {
  if (batchPollTimer) return
  let wasRunning = batch.value.running
  batchPollTimer = setInterval(async () => {
    const result = await casyContext.inbox.getBatchProgress()
    if (!result.ok) return
    const nowRunning = !!result.data?.running
    applyBatchProgress(result.data)
    if (wasRunning && !nowRunning) onBatchFinished(result.data)
    wasRunning = nowRunning
  }, 500)
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
    const result = await casyContext.inbox.add('file', null, filePath)
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
            const result = await casyContext.inbox.add('clipboard', text)
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

  // 批量 AI 分类：先取一次当前进度（页面打开时批次可能已在跑）
  const progressResult = await casyContext.inbox.getBatchProgress()
  if (progressResult.ok) applyBatchProgress(progressResult.data)

  // 批量进度事件监听；失败时回退 500ms 轮询
  try {
    const { listen } = await import('@tauri-apps/api/event')

    unlistenFns.push(
      await listen('inbox:batch-progress', (event) => {
        applyBatchProgress(event.payload)
      })
    )

    unlistenFns.push(
      await listen('inbox:batch-finished', (event) => {
        onBatchFinished(event.payload)
      })
    )
  } catch (err) {
    console.warn('批量进度事件监听失败，回退轮询:', err)
    startBatchPolling()
  }
})

onUnmounted(() => {
  unlistenFns.forEach((fn) => fn())
  if (batchPollTimer) {
    clearInterval(batchPollTimer)
    batchPollTimer = null
  }
})
</script>

<template>
  <div class="inbox-page">
    <!-- 快速捕获条（设计哲学 §10.2：极简输入，回车即入袋） -->
    <div class="capture-bar">
      <textarea
        v-model="quickCaptureText"
        placeholder="有什么想法、材料、待办？先记下来，稍后厘清…（回车即入袋）"
        @keydown.enter.exact.prevent="quickCapture"
        rows="1"
      />
      <div class="capture-bar-tools">
        <button class="capture-btn" @click="quickCapture" :disabled="!quickCaptureText.trim()">
          快速捕获 <kbd>⌘I</kbd>
        </button>
        <button class="capture-btn" @click="importFile" title="导入文件">
          <el-icon :size="13"><Paperclip /></el-icon> 文件
        </button>
        <button class="capture-btn" @click="pasteFromClipboard" title="粘贴剪贴板">
          <el-icon :size="13"><DocumentCopy /></el-icon> 粘贴
        </button>
        <button class="capture-btn" @click="handleScreenshot" title="截屏捕获">
          <el-icon :size="13"><Camera /></el-icon> 截屏
        </button>
        <button class="capture-btn" @click="showAddNoteDialog = true" title="添加笔记">
          <el-icon :size="13"><EditPen /></el-icon> 笔记
        </button>
        <button
          class="capture-btn voice-btn"
          :class="{ recording: isRecording }"
          :disabled="!micAvailable"
          :title="micAvailable ? (isRecording ? '点击停止并保存' : '语音速记') : '麦克风不可用或无权限'"
          @click="toggleVoiceNote"
        >
          <span v-if="isRecording" class="rec-dot"></span>
          {{ isRecording ? `录音中 ${formatTime(recordingTime)}` : '语音' }}
        </button>
        <span v-if="!micAvailable" class="capture-hint">麦克风不可用，语音速记已禁用</span>
        <span v-else class="capture-hint">也可直接拖文件到这里</span>
      </div>
    </div>

    <!-- 收件箱状态概览 -->
    <div class="inbox-hero">
      <div class="inbox-hero-left">
        <span class="inbox-hero-title">📥 收件箱 · 大口袋</span>
        <span class="inbox-hero-sub">所有想法、材料先进口袋——先捕获，后厘清</span>
      </div>
      <div class="inbox-hero-stats">
        <div class="hero-stat">
          <span class="hero-stat-num red">{{ pendingItems.length }}</span>
          <span class="hero-stat-label">待厘清</span>
        </div>
        <div class="hero-stat">
          <span class="hero-stat-num">{{ filedItems.length }}</span>
          <span class="hero-stat-label">已归档</span>
        </div>
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

    <!-- 批量 AI 分类 -->
    <div class="batch-panel">
      <div class="batch-main">
        <div class="batch-header">
          <span class="batch-title">批量 AI 分类</span>
          <span v-if="!batch.running" class="batch-sub">待处理 {{ pendingItems.length }} 条</span>
          <span v-else-if="batchPaused" class="batch-status paused">已暂停</span>
          <span v-else class="batch-status">处理中</span>
        </div>
        <template v-if="batch.running">
          <el-progress :percentage="batchPercent" :stroke-width="8" color="#4C8067" />
          <div class="batch-detail">
            <span>{{ batch.processed + batch.failed }} / {{ batch.total }}</span>
            <span v-if="batch.failed > 0" class="batch-failed">失败 {{ batch.failed }}</span>
            <span v-if="batch.currentItem" class="batch-current">正在处理：{{ batch.currentItem }}</span>
            <span class="batch-active">并发 {{ batch.active }}</span>
          </div>
        </template>
      </div>
      <div class="batch-actions">
        <el-button
          v-if="!batch.running"
          type="primary"
          size="small"
          :disabled="!pendingItems.length"
          :loading="batchStarting"
          @click="startBatch"
        >
          开始批量分类
        </el-button>
        <template v-else-if="!batchPaused">
          <el-button size="small" @click="pauseBatch">暂停</el-button>
          <el-button size="small" type="danger" plain @click="cancelBatch">取消</el-button>
        </template>
        <template v-else>
          <el-button size="small" type="primary" @click="resumeBatch">恢复</el-button>
          <el-button size="small" type="danger" plain @click="cancelBatch">取消</el-button>
        </template>
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

              <!-- 推荐列表（本地规则优先，AI 兜底） -->
              <div v-if="getRecommendations(item).length" class="recommendations">
                <div
                  v-for="(rec, idx) in getRecommendations(item)"
                  :key="idx"
                  class="recommendation-item"
                  :class="{ 'recommendation-default': idx === 0 && getStrength(quickJudgeResults[item.id]?.confidence ?? 0) === 'strong' }"
                >
                  <span class="rec-icon">{{ idx === 0 && getStrength(quickJudgeResults[item.id]?.confidence ?? 0) === 'strong' ? '●' : '○' }}</span>
                  <span class="rec-text">{{ getActionMeta(rec.action).desc(rec) }}</span>
                  <span class="rec-reason">{{ rec.reason }}</span>
                </div>
              </div>
              <div v-else class="no-recommendation">
                未识别意图，请手动选择操作
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
                <el-icon :size="13"><component :is="getActionMeta(getRecommendations(item)[0]?.action).icon" /></el-icon>
                {{ getActionMeta(getRecommendations(item)[0]?.action).label }}
              </el-button>
              <!-- Candidate/Fallback：打开确认弹窗 -->
              <el-button
                v-else
                size="small"
                type="primary"
                @click="openConfirmDialog(item)"
              >
                <el-icon :size="13"><component :is="getActionMeta(getRecommendations(item)[0]?.action).icon" /></el-icon>
                {{ getActionMeta(getRecommendations(item)[0]?.action).label }}
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
              <!-- 不采纳按钮（设计哲学 §10：推荐拒绝 → 学习信号） -->
              <el-button
                v-if="getRecommendations(item).length > 0"
                size="small"
                type="warning"
                plain
                @click="rejectRecommendation(item)"
              >
                不采纳
              </el-button>
              <!-- 语音转写按钮（音频条目） -->
              <el-button
                v-if="isAudioItem(item)"
                size="small"
                :loading="!!transcribingIds[item.id]"
                @click="transcribeItem(item)"
              >
                转写
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
    <el-dialog v-model="showConfirmDialog" title="确认动作" width="560">
      <div v-if="confirmItem" class="confirm-dialog-content">
        <div class="confirm-file-info">
          <span class="confirm-icon">📄</span>
          <span class="confirm-filename">{{ confirmItem.title || '无标题' }}</span>
        </div>

        <!-- 推荐动作选择（设计哲学 §10：判断意图 → 推荐按钮 → 确认推送） -->
        <div v-if="confirmRecommendations.length > 0" class="confirm-actions">
          <div
            v-for="(rec, idx) in confirmRecommendations"
            :key="idx"
            class="confirm-action-item"
            :class="{ active: confirmAction === rec.action }"
            @click="confirmAction = rec.action"
          >
            <el-radio :model-value="confirmAction" :value="rec.action" class="confirm-action-radio">
              <span class="action-label">{{ getActionMeta(rec.action).label }}</span>
              <span class="action-desc">{{ getActionMeta(rec.action).desc(rec) }}</span>
            </el-radio>
          </div>
        </div>

        <el-form v-if="confirmAction === 'file_to_case'" label-width="60px" class="confirm-form">
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
        <el-button type="primary" :loading="processing" @click="doConfirmArchive">
          {{ getActionMeta(confirmAction).label }}
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.inbox-page {
  max-width: 900px;
  margin: 0 auto;
}

/* 快速捕获条（设计哲学 §10.2） */
.capture-bar {
  background: #FFFFFF;
  border: 1px solid #E0E3E9;
  border-radius: 8px;
  padding: 12px 14px;
  margin-bottom: 12px;
}

.capture-bar textarea {
  width: 100%;
  border: none;
  outline: none;
  resize: none;
  font-size: 13.5px;
  line-height: 1.6;
  background: transparent;
  color: #1F2430;
  font-family: inherit;
  min-height: 22px;
}

.capture-bar textarea::placeholder {
  color: #9BA2AF;
}

.capture-bar-tools {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 8px;
  padding-top: 8px;
  border-top: 1px dashed #EEF0F3;
}

.capture-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 4px 10px;
  border-radius: 6px;
  border: 1px solid #E0E3E9;
  background: #FFFFFF;
  font-size: 12px;
  color: #4B5160;
  cursor: pointer;
  font-family: inherit;
  transition: all 0.15s;
}

.capture-btn:hover {
  border-color: #CDD2DB;
  background: #F6F7F9;
}

.capture-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* 语音速记录制态 */
.voice-btn.recording {
  border-color: #B4554F;
  color: #B4554F;
  background: #F6EDEC;
}

.rec-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #B4554F;
  animation: rec-pulse 1s ease-in-out infinite;
}

@keyframes rec-pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.3; }
}

.capture-btn kbd {
  font-family: 'SF Mono', Menlo, monospace;
  font-size: 10px;
  border: 1px solid #E0E3E9;
  border-radius: 3px;
  padding: 0 4px;
  background: #F6F7F9;
}

.capture-hint {
  margin-left: auto;
  font-size: 11px;
  color: #9BA2AF;
}

/* 收件箱状态概览 */
.inbox-hero {
  display: flex;
  align-items: center;
  gap: 16px;
  background: #FFFFFF;
  border: 1px solid #E0E3E9;
  border-radius: 8px;
  padding: 12px 18px;
  margin-bottom: 12px;
}

.inbox-hero-left {
  flex: 1;
}

.inbox-hero-title {
  font-size: 15px;
  font-weight: 700;
  color: #1F2430;
  display: flex;
  align-items: center;
  gap: 8px;
}

.inbox-hero-sub {
  font-size: 12px;
  color: #9BA2AF;
  margin-top: 2px;
}

.inbox-hero-stats {
  display: flex;
  gap: 22px;
  align-items: center;
}

.hero-stat {
  text-align: center;
}

.hero-stat-num {
  font-size: 18px;
  font-weight: 700;
  font-family: 'SF Mono', Menlo, monospace;
  line-height: 1.1;
  color: #1F2430;
}

.hero-stat-num.red {
  color: #B4554F;
}

.hero-stat-label {
  font-size: 11px;
  color: #9BA2AF;
  margin-top: 2px;
}

/* 拖拽上传区 */
.drop-zone {
  border: 2px dashed #CDD2DB;
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

/* 批量 AI 分类 */
.batch-panel {
  display: flex;
  align-items: center;
  gap: 16px;
  background: #FFFFFF;
  border: 1px solid #E0E3E9;
  border-radius: 8px;
  padding: 12px 18px;
  margin-bottom: 12px;
}

.batch-main {
  flex: 1;
  min-width: 0;
}

.batch-header {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 6px;
}

.batch-title {
  font-size: 13.5px;
  font-weight: 600;
  color: #1F2430;
}

.batch-sub {
  font-size: 12px;
  color: #9BA2AF;
}

.batch-status {
  font-size: 12px;
  color: #3E5C9A;
}

.batch-status.paused {
  color: #B4554F;
}

.batch-detail {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-top: 6px;
  font-size: 12px;
  color: #9BA2AF;
  font-family: 'SF Mono', Menlo, monospace;
}

.batch-failed {
  color: #B4554F;
}

.batch-current {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: inherit;
  color: #4B5160;
}

.batch-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
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
