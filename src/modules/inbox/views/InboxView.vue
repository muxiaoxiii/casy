<script setup>
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { casyContext } from '../../../core/plugin/context'
import { useInboxStore } from '../../../stores/inbox'
import { useCapture } from '../composables/useCapture'
import { useVoiceNote } from '../composables/useVoiceNote'
import { AI_PROMPTS } from '../../../core/prompts'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  Folder,
  Finished,
  Calendar,
  Collection,
  Briefcase,
  Bell,
  Download,
  Plus,
  Close,
  Message,
  Phone,
  Picture,
  ArrowRight,
  CaretRight,
  UserFilled,
  Timer,
  MagicStick,
  Delete,
  Search,
  Filter,
  Check,
  Microphone,
  ChatDotRound,
  Paperclip,
} from '@element-plus/icons-vue'

const inboxStore = useInboxStore()
const { captureScreenshot, captureClipboard } = useCapture()
const { isRecording, startRecording, stopRecording } = useVoiceNote()

const items = ref([])
const loading = ref(false)
const processing = ref(false)
const structuring = ref(false)
const selectedItemId = ref('')
const sourceFilter = ref('all')
const quickCaptureInputText = ref('')
const casesList = ref([])

// GTD 澄清状态
const clarifyAction = ref('action') // 'action' | 'delegate' | 'wait' | 'someday'
const clarifyContext = ref('@Desk')
const clarifyMatter = ref('')
const clarifyDoWhen = ref('today')
const clarifyDeadline = ref('')
const clarifyNotes = ref('')
const clarifyEstMinutes = ref(15)

// 快速判断与 AI 结果缓存
const quickJudgeResults = ref({})
const aiResults = ref({})

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

const sourceFilters = computed(() => {
  const counts = pendingItems.value.reduce((result, item) => {
    const source = item.sourceType || 'note'
    result[source] = (result[source] || 0) + 1
    return result
  }, {})
  return [
    { value: 'all', label: 'All Inbox', count: pendingItems.value.length, icon: Collection },
    { value: 'email', label: 'Email', count: counts.email || 0, icon: Message },
    { value: 'wechat', label: 'WeChat', count: counts.wechat || 0, icon: ChatDotRound },
    { value: 'note', label: 'Call Notes & 速记', count: counts.note || 0, icon: Phone },
  ]
})

const filteredPendingItems = computed(() => {
  if (sourceFilter.value === 'all') return pendingItems.value
  return pendingItems.value.filter((item) => (item.sourceType || 'note') === sourceFilter.value)
})

const selectedItem = computed(() => {
  const list = filteredPendingItems.value
  return list.find((item) => item.id === selectedItemId.value) || list[0] || null
})



function selectSource(val) {
  sourceFilter.value = val
}

function selectItem(item) {
  selectedItemId.value = item.id
  clarifyNotes.value = item.contentText || ''
  clarifyCaseId.value = item.aiSuggestedCaseId || item.caseId || ''
  const matched = casesList.value.find((c) => c.id === clarifyCaseId.value)
  clarifyMatter.value = matched?.displayName || matched?.caseName || item.caseName || ''
}

function onClarifyCaseChange(caseId) {
  clarifyCaseId.value = caseId
  const matched = casesList.value.find((c) => c.id === caseId)
  clarifyMatter.value = matched?.displayName || matched?.caseName || ''
}

// ============================================================
// 数据加载
// ============================================================
onMounted(async () => {
  await loadItems()
  await loadCases()
  if (filteredPendingItems.value[0]) {
    selectItem(filteredPendingItems.value[0])
  }
})

async function loadItems() {
  loading.value = true
  const result = await casyContext.inbox.list()
  if (result.ok && Array.isArray(result.data)) {
    items.value = result.data
  }
  loading.value = false
}

async function loadCases() {
  const result = await casyContext.cases.list({})
  if (result.ok && Array.isArray(result.data)) {
    casesList.value = result.data
  }
}

// ============================================================
// GTD 澄清处理 (Turn to Action)
// ============================================================
async function processCurrentItem(actionType = clarifyAction.value) {
  const item = selectedItem.value
  if (!item) return

  processing.value = true
  if (actionType === 'action') {
    // 转为任务
    await casyContext.tasks.create({
      taskName: item.title,
      description: clarifyNotes.value || item.contentText,
      caseId: clarifyCaseId.value || null,
      caseName: clarifyMatter.value || null,
      startBucket: clarifyDoWhen.value,
      dueDate: clarifyDeadline.value || null,
      estimatedMinutes: clarifyEstMinutes.value,
    })
    ElMessage.success('已转为行动并归入案件业务链条')
  } else if (actionType === 'wait') {
    // 设为等待
    await casyContext.tasks.create({
      taskName: `等外部回复: ${item.title}`,
      description: clarifyNotes.value,
      caseId: clarifyCaseId.value || null,
      caseName: clarifyMatter.value || null,
      taskType: 'waiting',
      startBucket: 'anytime',
    })
    ElMessage.success('已记入外部等待列表')
  } else if (actionType === 'someday') {
    // 放入将来也许
    await casyContext.tasks.create({
      taskName: item.title,
      caseId: clarifyCaseId.value || null,
      caseName: clarifyMatter.value || null,
      startBucket: 'someday',
    })
    ElMessage.success('已归入将来也许清单')
  }

  // 标记收件项完成
  if (item.id) {
    await casyContext.inbox.confirmAction({
      inboxItemId: item.id,
      action: actionType === 'action' ? 'create_task' : 'file_to_case',
      targetCaseId: clarifyCaseId.value || null,
    })
  }

  processing.value = false
  await loadItems()
}

// AI 结构化解析
async function structureWithAI() {
  const item = selectedItem.value
  if (!item || !item.contentText) return
  
  structuring.value = true
  const result = await casyContext.ai.askAi(
    `${AI_PROMPTS.INBOX_STRUCTURING}\n\n[待解析文本]: ${item.contentText}`
  )
  
  structuring.value = false
  if (result.ok && result.text) {
    try {
      // 尝试匹配 JSON
      const jsonStr = result.text.match(/\{[\s\S]*\}/)?.[0] || result.text
      const data = JSON.parse(jsonStr)
      
      if (data.events && data.events.length > 0) {
        clarifyAction.value = 'action'
        clarifyDeadline.value = data.events[0].time || ''
        clarifyNotes.value = `[AI 提取事件]: ${data.events[0].name} @ ${data.events[0].location || '未知地点'}\n` + clarifyNotes.value
      }
      
      if (data.tasks && data.tasks.length > 0) {
        clarifyAction.value = 'action'
        clarifyNotes.value = `[AI 提取任务]:\n${data.tasks.map(t => '- ' + t).join('\n')}\n` + clarifyNotes.value
      }
      
      if (data.notes) {
        clarifyNotes.value += `\n[AI 提取关键信息]: ${data.notes}`
      }
      
      ElMessage.success('AI 结构化解析完成')
    } catch (e) {
      clarifyNotes.value = `[AI 辅助分析]:\n${result.text}\n\n` + clarifyNotes.value
      ElMessage.success('已应用 AI 辅助分析')
    }
  } else {
    ElMessage.error(result.error || 'AI 解析失败')
  }
}

// 快速捕获
async function submitQuickCapture() {
  const text = quickCaptureInputText.value.trim()
  if (!text) return
  processing.value = true
  const result = await casyContext.inbox.add('note', text)
  processing.value = false
  if (result.ok) {
    ElMessage.success('已快速捕获到收件箱')
    quickCaptureInputText.value = ''
    await loadItems()
  }
}

async function importFile() {
  const { open } = await import('@tauri-apps/plugin-dialog')
  const selected = await open({ multiple: true })
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

async function dismissItem(item) {
  if (item.id) {
    await casyContext.inbox.dismiss(item.id)
  }
  ElMessage.success('已忽略此项')
  await loadItems()
}
</script>

<template>
  <div class="stitch-inbox-view">
    <!-- ═══ 顶部全局快速捕获条 (Global Capture Bar) ═══ -->
    <div class="quick-capture-hero-bar">
      <div class="capture-input-container">
        <div class="capture-icon-bubble">
          <el-icon :size="16" color="var(--c-primary)"><Plus /></el-icon>
        </div>
        <input
          v-model="quickCaptureInputText"
          class="capture-real-input"
          :placeholder="$t('inbox.capture_placeholder')"
          @keyup.enter="submitQuickCapture"
        />
        <div class="capture-tools">
          <button class="tool-btn" :class="{ recording: isRecording }" title="语音速记" @click="startRecording">
            <el-icon :size="16"><Microphone /></el-icon>
          </button>
          <button class="tool-btn" title="导入文件" @click="importFile">
            <el-icon :size="16"><Paperclip /></el-icon>
          </button>
        </div>
      </div>
    </div>

    <!-- ═══ 三栏澄清台主体 (Split View) ═══ -->
    <div class="inbox-three-columns">
      <!-- ── 左栏：Sources & Status (240px) ── -->
      <aside class="inbox-sources-sidebar">
        <div class="sidebar-section-title">
          <span>{{ $t('inbox.sources') }}</span>
        </div>

        <nav class="sources-nav-list">
          <button
            v-for="src in sourceFilters"
            :key="src.value"
            type="button"
            class="source-nav-item"
            :class="{ active: sourceFilter === src.value }"
            @click="selectSource(src.value)"
          >
            <div class="source-nav-left">
              <el-icon class="source-ico" :size="16"><component :is="src.icon" /></el-icon>
              <span>{{ src.label }}</span>
            </div>
            <span class="source-count-pill">{{ src.count }}</span>
          </button>
        </nav>

        <div class="sidebar-section-title" style="margin-top: 24px;">
          <span>{{ $t('inbox.status') }}</span>
        </div>

        <button class="source-nav-item status-alert-item" :class="{ active: sourceFilter === 'needs-clarify' }" @click="selectSource('all')">
          <div class="source-nav-left">
            <el-icon class="text-warning" :size="16"><Timer /></el-icon>
            <span>Needs Clarification</span>
          </div>
        </button>
      </aside>

      <!-- ── 中栏：Unprocessed Stream (事项流) ── -->
      <main class="inbox-stream-panel">
        <div class="stream-header-row">
          <div>
            <h2 class="section-title">{{ $t('inbox.needs_clarify') }} ({{ items.length }})</h2>
            <p class="stream-subtitle">Unprocessed items requiring your attention.</p>
          </div>
          <div class="stream-actions">
            <button class="btn-clean-ghost" @click="importFile">
              <el-icon :size="14"><Plus /></el-icon>
              <span>New</span>
              <kbd class="mini-kbd">⌘I</kbd>
            </button>
          </div>
        </div>

        <!-- 任务列表 -->
        <div class="stream-card-list">
          <div
            v-for="item in filteredPendingItems"
            :key="item.id"
            class="inbox-stream-card"
            :class="{ selected: selectedItem?.id === item.id }"
            @click="selectItem(item)"
          >
            <!-- 左侧主色条 -->
            <div v-if="selectedItem?.id === item.id" class="card-selected-line" />

            <div class="card-content-wrap">
              <div class="card-top-meta">
                <span class="meta-source-kicker">{{ item.sourceLabel || 'RAW INBOX' }}</span>
                <span class="meta-due-tag">{{ item.dueText || 'Due < 24h' }}</span>
              </div>

              <h3 class="card-item-title">{{ item.title }}</h3>
              <p class="card-item-snippet">{{ item.contentText }}</p>

              <div class="card-bottom-tags">
                <span class="tag-context-pill">
                  <el-icon :size="12"><Finished /></el-icon>
                  {{ item.context || '@Inbox' }}
                </span>
                <span v-if="item.caseName" class="tag-matter-link">
                  <el-icon :size="12"><Briefcase /></el-icon>
                  {{ item.caseName }}
                </span>
              </div>
            </div>
          </div>
        </div>
      </main>

      <!-- ── 右栏：440px Clarify Panel (GTD 澄清详情) ── -->
      <aside v-if="selectedItem" class="inbox-clarify-panel">
        <div class="clarify-panel-header">
          <span class="panel-header-caps">{{ $t('inbox.clarify_task') }}</span>
          <div class="panel-header-btns">
            <button class="btn-icon-primary" title="AI 结构化解析" :disabled="structuring" @click="structureWithAI">
              <el-icon :size="15" :class="{'is-loading': structuring}"><MagicStick /></el-icon>
            </button>
            <button class="btn-icon-danger" title="忽略/删除" @click="dismissItem(selectedItem)">
              <el-icon :size="15"><Delete /></el-icon>
            </button>
          </div>
        </div>

        <div class="clarify-scroll-body">
          <!-- 标题与来源 -->
          <div class="clarify-title-block">
            <textarea
              v-model="selectedItem.title"
              class="clarify-title-input"
              rows="2"
              placeholder="Task title..."
            />
            <div class="capture-meta-origin">
              <span class="origin-dot-danger" />
              <span>Captured via {{ selectedItem.sourceType || 'Email' }} · 3 hours ago</span>
            </div>
          </div>

          <!-- GTD 4 象限行动选择器 -->
          <div class="gtd-action-card">
            <span class="gtd-kicker">Is it actionable?</span>
            <div class="gtd-buttons-grid">
              <button
                class="gtd-quad-btn"
                :class="{ active: clarifyAction === 'action' }"
                @click="clarifyAction = 'action'"
              >
                <el-icon :size="18"><CaretRight /></el-icon>
                <strong>Do It</strong>
              </button>
              <button
                class="gtd-quad-btn"
                :class="{ active: clarifyAction === 'delegate' }"
                @click="clarifyAction = 'delegate'"
              >
                <el-icon :size="18"><UserFilled /></el-icon>
                <strong>Delegate</strong>
              </button>
              <button
                class="gtd-quad-btn"
                :class="{ active: clarifyAction === 'wait' }"
                @click="clarifyAction = 'wait'"
              >
                <el-icon :size="18"><Timer /></el-icon>
                <strong>Defer (Wait)</strong>
              </button>
              <button
                class="gtd-quad-btn"
                :class="{ active: clarifyAction === 'someday' }"
                @click="clarifyAction = 'someday'"
              >
                <el-icon :size="18"><Folder /></el-icon>
                <strong>Someday</strong>
              </button>
            </div>
          </div>

          <!-- 属性表单 -->
          <div class="clarify-form-stack">
            <!-- 上下文标签 Context -->
            <div class="form-group-block">
              <label class="form-label-caps">Context</label>
              <div class="context-tags-row">
                <button
                  class="context-chip"
                  :class="{ active: clarifyContext === '@Calls' }"
                  @click="clarifyContext = '@Calls'"
                >
                  <el-icon :size="13"><Phone /></el-icon>
                  <span>@Calls</span>
                </button>
                <button
                  class="context-chip"
                  :class="{ active: clarifyContext === '@Desk' }"
                  @click="clarifyContext = '@Desk'"
                >
                  <el-icon :size="13"><Finished /></el-icon>
                  <span>@Desk</span>
                </button>
              </div>
            </div>

            <!-- 关联案件 Matter / Project -->
            <div class="form-group-block">
              <div style="display: flex; align-items: center; justify-content: space-between; margin-bottom: 6px;">
                <label class="form-label-caps" style="margin-bottom: 0;">Matter / Project（归属案件）</label>
                <span v-if="clarifyCaseId" style="font-size: 11px; padding: 1px 7px; border-radius: 10px; background: #ecfdf5; color: #059669; border: 1px solid #a7f3d0;">已关联链条</span>
                <span v-else style="font-size: 11px; padding: 1px 7px; border-radius: 10px; background: #fffbeb; color: #d97706; border: 1px solid #fde68a;">全局独立项</span>
              </div>
              <el-select
                v-model="clarifyCaseId"
                filterable
                clearable
                placeholder="搜索并选择关联案件 / 项目..."
                style="width: 100%;"
                @change="onClarifyCaseChange"
              >
                <el-option
                  v-for="c in casesList"
                  :key="c.id"
                  :label="c.caseNo ? `[${c.caseNo}] ${c.displayName || c.caseName}` : (c.displayName || c.caseName)"
                  :value="c.id"
                >
                  <div style="display: flex; align-items: center; justify-content: space-between; width: 100%;">
                    <span style="font-weight: 500;">{{ c.displayName || c.caseName }}</span>
                    <span v-if="c.caseNo" style="font-size: 11px; font-family: monospace; color: var(--c-primary); background: rgba(62,92,154,0.08); padding: 1px 6px; border-radius: 4px;">{{ c.caseNo }}</span>
                  </div>
                </el-option>
              </el-select>
            </div>

            <!-- 双时态时间 Time Horizon (Time-Twin) -->
            <div class="form-group-block">
              <label class="form-label-caps">Time Horizon</label>
              <div class="time-twin-container">
                <div class="time-twin-half">
                  <span class="time-twin-kicker">Do When</span>
                  <div class="time-twin-val">
                    <el-icon :size="14"><Clock /></el-icon>
                    <strong>Today</strong>
                  </div>
                </div>
                <div class="time-twin-divider" />
                <div class="time-twin-half">
                  <span class="time-twin-kicker text-danger">Hard Deadline</span>
                  <div class="time-twin-val text-danger">
                    <el-icon :size="14"><Warning /></el-icon>
                    <strong>TOMORROW 5PM</strong>
                  </div>
                </div>
              </div>
            </div>

            <!-- 备注与预估时间 -->
            <div class="form-group-block">
              <label class="form-label-caps">Notes &amp; Meta</label>
              <div class="notes-meta-box">
                <textarea
                  v-model="clarifyNotes"
                  class="notes-textarea"
                  rows="3"
                  placeholder="补充关键信息、联系方式或初步思考..."
                />
                <div class="notes-meta-foot">
                  <span class="est-tag">
                    <el-icon :size="13"><Timer /></el-icon>
                    Est: 15m
                  </span>
                  <span class="ai-extracted-badge">
                    <span class="ai-mono">AI</span>
                    <span>Extracted</span>
                  </span>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- 底部主操作按钮 (Process & Next ⌘↵) -->
        <div class="clarify-footer-action">
          <button class="btn-clarify-primary" :loading="processing" @click="processCurrentItem('action')">
            <span>{{ $t('inbox.turn_action') }}</span>
            <span class="kbd-sub">⌘↵</span>
          </button>
          <div class="clarify-sub-actions">
            <button class="btn-clarify-ghost" @click="processCurrentItem('wait')">
              <el-icon :size="13"><Clock /></el-icon>
              <span>{{ $t('inbox.wait') }}</span>
            </button>
            <button class="btn-clarify-ghost" @click="processCurrentItem('someday')">
              <el-icon :size="13"><Collection /></el-icon>
              <span>{{ $t('inbox.vault') }}</span>
            </button>
          </div>
        </div>
      </aside>
    </div>
  </div>
</template>

<style scoped>
/* ═══════════════════════════════════════════════════════════
   Stitch Inbox View (v4.0_4 & v4.0_5 Three-Column Workspace)
   ═══════════════════════════════════════════════════════════ */
.stitch-inbox-view {
  display: flex;
  flex-direction: column;
  height: calc(100vh - 64px);
  background: var(--c-bg-page);
  color: var(--c-text);
  overflow: hidden;
}

/* ── 顶部快速捕获条 ───────────────────────────────────────── */
.quick-capture-hero-bar {
  padding: 16px 28px 12px;
  background: var(--c-bg-card);
  border-bottom: 1px solid var(--c-border);
}

.capture-input-container {
  max-width: 960px;
  margin: 0 auto;
  display: flex;
  align-items: center;
  gap: 12px;
  background: var(--c-bg-subtle);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-xl);
  padding: 8px 14px;
  transition: all var(--motion-fast);
}

.capture-input-container:focus-within {
  border-color: var(--c-primary);
  background: var(--c-bg-card);
  box-shadow: var(--shadow-sm);
}

.capture-icon-bubble {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  background: var(--c-primary-light);
  display: grid;
  place-items: center;
  flex-shrink: 0;
}

.capture-real-input {
  flex: 1;
  border: none;
  background: transparent;
  outline: none;
  font-size: 14px;
  color: var(--c-text);
}

.capture-tools {
  display: flex;
  align-items: center;
  gap: 6px;
}

.tool-btn {
  width: 28px;
  height: 28px;
  border-radius: 6px;
  border: none;
  background: transparent;
  color: var(--slate-gray-light);
  cursor: pointer;
  display: grid;
  place-items: center;
  transition: all var(--motion-fast);
}

.tool-btn:hover {
  background: var(--c-bg-hover);
  color: var(--c-text);
}

.tool-btn.recording {
  color: var(--status-risk);
  background: var(--bg-risk-weak);
}

/* ── 三栏主体布局 ─────────────────────────────────────────── */
.inbox-three-columns {
  display: flex;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

/* ── 左栏：Sources (240px) ────────────────────────────────── */
.inbox-sources-sidebar {
  width: 230px;
  border-right: 1px solid var(--c-border);
  background: var(--c-bg-card);
  padding: 18px 14px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex-shrink: 0;
}

.sidebar-section-title {
  font-size: 10.5px;
  font-weight: 700;
  color: var(--slate-gray-light);
  text-transform: uppercase;
  letter-spacing: 0.8px;
  padding: 0 8px 4px;
}

.sources-nav-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.source-nav-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 10px;
  border-radius: var(--c-radius-lg);
  border: none;
  background: transparent;
  color: var(--c-text-regular);
  font-size: 13px;
  cursor: pointer;
  transition: all var(--motion-fast);
}

.source-nav-item:hover {
  background: var(--c-bg-hover);
  color: var(--c-text);
}

.source-nav-item.active {
  background: var(--c-bg-selected);
  color: var(--c-primary);
  font-weight: 600;
}

.source-nav-left {
  display: flex;
  align-items: center;
  gap: 8px;
}

.source-count-pill {
  font-family: var(--font-mono);
  font-size: 11px;
  padding: 1px 6px;
  border-radius: var(--c-radius-full);
  background: var(--c-bg-subtle);
  color: var(--slate-gray-light);
}

.source-nav-item.active .source-count-pill {
  background: var(--c-primary-light);
  color: var(--c-primary);
}

.status-alert-item {
  color: var(--status-warning);
}

/* ── 中栏：Unprocessed Stream ─────────────────────────────── */
.inbox-stream-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  background: var(--c-bg-page);
  border-right: 1px solid var(--c-border);
  min-width: 0;
  overflow-y: auto;
}

.stream-header-row {
  padding: 20px 24px 14px;
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  border-bottom: 1px solid var(--c-border-light);
}

.stream-main-title {
  font-size: 18px;
  font-weight: 700;
  color: var(--c-text-heading);
  margin: 0;
}

.stream-subtitle {
  font-size: 12px;
  color: var(--slate-gray-light);
  margin: 2px 0 0;
}

.btn-clean-ghost {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  border-radius: var(--c-radius-lg);
  border: 1px solid var(--c-border);
  background: var(--c-bg-card);
  color: var(--c-text-regular);
  font-size: 12.5px;
  cursor: pointer;
  transition: all var(--motion-fast);
}

.btn-clean-ghost:hover {
  border-color: var(--c-primary);
  color: var(--c-primary);
}

.mini-kbd {
  font-family: var(--font-mono);
  font-size: 9.5px;
  padding: 1px 4px;
  border: 1px solid var(--c-border);
  border-radius: 3px;
  background: var(--c-bg-subtle);
}

.stream-card-list {
  padding: 18px 24px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.inbox-stream-card {
  position: relative;
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-xl);
  padding: 16px;
  cursor: pointer;
  box-shadow: var(--shadow-sm);
  transition: transform 0.25s cubic-bezier(0.2, 0.8, 0.2, 1), box-shadow 0.2s ease, border-color 0.2s ease;
  overflow: hidden;
}

.inbox-stream-card:hover {
  transform: translateY(-2px);
  border-color: var(--c-border-strong);
  box-shadow: 0 6px 16px rgba(0, 0, 0, 0.06);
}

.inbox-stream-card.selected {
  border-color: var(--c-primary);
  box-shadow: var(--shadow-md);
}

.card-selected-line {
  position: absolute;
  top: 0;
  left: 0;
  width: 4px;
  height: 100%;
  background: var(--c-primary);
}

.card-content-wrap {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.card-top-meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.meta-source-kicker {
  font-family: var(--font-mono);
  font-size: 10.5px;
  font-weight: 600;
  color: var(--slate-gray-light);
  text-transform: uppercase;
}

.meta-due-tag {
  font-family: var(--font-mono);
  font-size: 10.5px;
  font-weight: 600;
  padding: 2px 6px;
  border-radius: 4px;
  background: var(--bg-risk-weak);
  color: var(--status-risk);
}

.card-item-title {
  font-size: 14px;
  font-weight: 700;
  color: var(--c-text-heading);
  margin: 2px 0 0;
}

.card-item-snippet {
  font-size: 12.5px;
  color: var(--c-text-regular);
  line-height: 1.45;
  margin: 0;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.card-bottom-tags {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 6px;
}

.tag-context-pill {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 4px;
  background: var(--c-bg-subtle);
  color: var(--slate-gray-light);
}

.tag-matter-link {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  color: var(--c-primary);
}

/* ── 右栏：440px Clarify Panel ────────────────────────────── */
.inbox-clarify-panel {
  width: 440px;
  background: var(--c-bg-card);
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  overflow: hidden;
}

.clarify-panel-header {
  padding: 16px 22px;
  border-bottom: 1px solid var(--c-border);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.panel-header-caps {
  font-size: 11px;
  font-weight: 700;
  color: var(--slate-gray-light);
  text-transform: uppercase;
  letter-spacing: 0.8px;
}

.btn-icon-danger {
  border: none;
  background: transparent;
  color: var(--slate-gray-light);
  cursor: pointer;
  padding: 4px;
  border-radius: 4px;
}

.btn-icon-danger:hover {
  color: var(--status-risk);
  background: var(--bg-risk-weak);
}

.clarify-scroll-body {
  flex: 1;
  overflow-y: auto;
  padding: 20px 22px;
  display: flex;
  flex-direction: column;
  gap: 22px;
}

.clarify-title-block {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.clarify-title-input {
  width: 100%;
  border: none;
  background: transparent;
  outline: none;
  font-size: 16px;
  font-weight: 700;
  color: var(--c-text-heading);
  line-height: 1.35;
  resize: none;
  padding: 0;
}

.capture-meta-origin {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11.5px;
  color: var(--slate-gray-light);
}

.origin-dot-danger {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--status-risk);
}

/* GTD 按钮组 */
.gtd-action-card {
  background: var(--c-bg-subtle);
  border-radius: var(--c-radius-xl);
  padding: 14px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.gtd-kicker {
  font-size: 10.5px;
  font-weight: 700;
  color: var(--slate-gray-light);
  text-transform: uppercase;
}

.gtd-buttons-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 8px;
}

.gtd-quad-btn {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 4px;
  padding: 10px;
  border-radius: var(--c-radius-lg);
  border: 1px solid var(--c-border);
  background: var(--c-bg-card);
  color: var(--c-text-regular);
  cursor: pointer;
  box-shadow: var(--shadow-sm);
  transition: transform 0.2s ease, box-shadow 0.2s ease, border-color 0.2s ease, background 0.2s ease, color 0.2s ease;
}

.gtd-quad-btn:hover {
  transform: translateY(-1px);
  border-color: var(--c-primary);
  color: var(--c-primary);
  box-shadow: var(--shadow-md);
}

.gtd-quad-btn.active {
  border-color: var(--c-primary);
  background: var(--c-primary-light);
  color: var(--c-primary);
}

.gtd-quad-btn strong {
  font-size: 12px;
}

/* 表单字段 */
.clarify-form-stack {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.form-group-block {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.form-label-caps {
  font-size: 10.5px;
  font-weight: 700;
  color: var(--slate-gray-light);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.context-tags-row {
  display: flex;
  gap: 8px;
}

.context-chip {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 5px 12px;
  border-radius: var(--c-radius-lg);
  border: 1px solid var(--c-border);
  background: var(--c-bg-card);
  color: var(--c-text-regular);
  font-size: 12px;
  cursor: pointer;
}

.context-chip.active {
  border-color: var(--c-primary);
  background: var(--c-primary);
  color: var(--c-primary-contrast);
}

.matter-search-field {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-radius: var(--c-radius-lg);
  border: 1px solid var(--c-border);
  background: var(--c-bg-card);
}

.field-input-clean {
  flex: 1;
  border: none;
  background: transparent;
  outline: none;
  font-size: 12.5px;
  color: var(--c-text);
}

/* 双时态 */
.time-twin-container {
  display: flex;
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  background: var(--c-bg-card);
  padding: 8px;
}

.time-twin-half {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 0 6px;
}

.time-twin-divider {
  width: 1px;
  background: var(--c-border);
  margin: 0 4px;
}

.time-twin-kicker {
  font-family: var(--font-mono);
  font-size: 9.5px;
  text-transform: uppercase;
  color: var(--slate-gray-light);
}

.time-twin-val {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 12px;
}

/* 备注 */
.notes-meta-box {
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  background: var(--c-bg-card);
  padding: 10px;
}

.notes-textarea {
  width: 100%;
  border: none;
  background: transparent;
  outline: none;
  font-size: 12.5px;
  color: var(--c-text);
  resize: none;
}

.notes-meta-foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-top: 1px solid var(--c-border-light);
  padding-top: 6px;
  margin-top: 4px;
}

.est-tag {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  color: var(--slate-gray-light);
}

.ai-extracted-badge {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 10.5px;
  color: var(--slate-gray-light);
  border: 1px dashed var(--c-border);
  padding: 1px 5px;
  border-radius: 3px;
}

.ai-mono {
  font-family: var(--font-mono);
  font-weight: 700;
}

/* 底部操作 */
.clarify-footer-action {
  padding: 16px 22px;
  border-top: 1px solid var(--c-border);
  background: var(--c-bg-card);
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.btn-clarify-primary {
  width: 100%;
  padding: 10px;
  border-radius: var(--c-radius-lg);
  border: none;
  background: var(--c-primary);
  color: var(--c-primary-contrast);
  font-size: 13.5px;
  font-weight: 700;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  box-shadow: var(--shadow-sm);
  transition: all var(--motion-fast);
}

.btn-clarify-primary:hover {
  background: var(--c-primary-hover);
}

.kbd-sub {
  font-family: var(--font-mono);
  font-size: 10px;
  opacity: 0.8;
}

.clarify-sub-actions {
  display: flex;
  gap: 8px;
}

.btn-clarify-ghost {
  flex: 1;
  padding: 6px;
  border-radius: 6px;
  border: 1px solid var(--c-border);
  background: var(--c-bg-card);
  color: var(--c-text-regular);
  font-size: 12px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 4px;
}

.btn-clarify-ghost:hover {
  border-color: var(--c-border-strong);
  background: var(--c-bg-hover);
}

@media (max-width: 1100px) {
  .inbox-sources-sidebar { display: none; }
  .inbox-clarify-panel { width: 360px; }
}

@media (max-width: 800px) {
  .inbox-three-columns { flex-direction: column; }
  .inbox-clarify-panel { width: 100%; }
}
</style>
