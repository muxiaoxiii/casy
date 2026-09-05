<script setup>
import { ref, computed, watch, onMounted, onBeforeUnmount, nextTick } from 'vue'
import { casyContext } from '../../../core/plugin/context'
import { aiToolCaller } from '../../../core/ai/tool-caller'
import { tauriCallSafe } from '../../../core/tauriBridge'
import ProposalDiffCard from './ProposalDiffCard.vue'
import { ElMessage } from 'element-plus'
import {
  ChatDotRound,
  Position,
  Loading,
  Tools,
  CircleCheck,
  CircleClose,
  Document,
  Collection,
  Briefcase,
  List,
  Link,
} from '@element-plus/icons-vue'

// ============================================================
// 状态
// ============================================================
const messages = ref([])
const inputMessage = ref('')
const loading = ref(false)
const showToolCalls = ref(false)

// 可用提供商与模型（来自插件系统，初始化完成后自动刷新）
const providers = ref([])
const availableModels = ref([])
const selectedProvider = ref('')
const selectedModel = ref('')
let offAiConfigured = null
watch(selectedProvider, () => {
  const provider = casyContext.getProviders().find(p => p.id === selectedProvider.value)
  availableModels.value = (provider?.models || []).map(m => ({ model: m.id, label: m.name }))
  selectedModel.value = provider?.models[0]?.id || ''
})

// ── @ 引用沙箱（W1）────────────────────────────────────────
const inputRef = ref(null)
/** 输入框上方的引用 chips：{ kind, id, title } */
const refChips = ref([])
/** @ 触发选择器状态 */
const mention = ref({ visible: false, query: '', results: [], index: 0, start: 0, end: 0 })
let mentionTimer = null
let mentionSearchSeq = 0

const REF_KIND_META = {
  file: { label: '文件', icon: Document },
  knowledge: { label: '知识', icon: Collection },
  task: { label: '任务', icon: List },
  case: { label: '案件', icon: Briefcase },
}

function refIcon(kind) {
  return REF_KIND_META[kind]?.icon || Link
}

function refKindLabel(kind) {
  return REF_KIND_META[kind]?.label || kind
}

// ============================================================
// 计算属性
// ============================================================
const toolDefinitions = computed(() => {
  return casyContext.getToolDefinitions()
})

/** 最后一张提案卡片的消息下标（仅它响应快捷键） */
const lastProposalIndex = computed(() => {
  return messages.value.reduce((acc, m, i) => (m.role === 'proposal' ? i : acc), -1)
})

// ============================================================
// 生命周期
// ============================================================
onMounted(() => {
  loadModels()
  offAiConfigured = casyContext.on('ai:configured', ({ activeId }) => {
    selectedProvider.value = activeId || ''
    loadModels()
  })
  addSystemMessage()
  // 插件系统异步初始化：就绪后刷新提供商/模型 + 重建欢迎语（避免时序竞态）
  const off = casyContext.on('plugins:ready', () => {
    loadModels()
    // 若欢迎语还是空工具清单，则重建
    if (messages.value.length === 1 && messages.value[0].role === 'system') {
      messages.value[0].content = buildIntro()
    }
  })
  setTimeout(() => {
    loadModels()
    off()
  }, 800)
})

onBeforeUnmount(() => {
  offAiConfigured?.()
  if (mentionTimer) clearTimeout(mentionTimer)
})

// ============================================================
// 函数
// ============================================================
function loadModels() {
  const list = casyContext.getProviders()
  providers.value = list.map(p => ({ id: p.id, name: p.name, mode: p.mode }))
  if (!list.some(p => p.id === selectedProvider.value)) selectedProvider.value = list[0]?.id || ''
  const provider = list.find(p => p.id === selectedProvider.value)
  availableModels.value = (provider?.models || []).map(m => ({ model: m.id, label: m.name }))
  if (!availableModels.value.some(m => m.model === selectedModel.value)) selectedModel.value = availableModels.value[0]?.model || ''
}

function buildIntro() {
  const tools = casyContext.getToolDefinitions()
  const toolLines = tools.map(t => `- ${t.name}: ${t.description}`).join('\n')
  return '我是 Casy AI 助手，可以帮你查询案件、管理任务、搜索知识库。\n\n' +
    '我可以调用以下工具：\n' +
    (toolLines || '（工具加载中…）') + '\n\n' +
    '输入 @ 可以引用文件、知识、任务或案件作为上下文。请告诉我你需要什么帮助。'
}

function addSystemMessage() {
  messages.value.push({
    role: 'system',
    content: buildIntro(),
    timestamp: new Date(),
  })
}

async function sendMessage() {
  const content = inputMessage.value.trim()
  if (!content || loading.value) return
  closeMention()

  // 引用 chips → context_refs（随本轮发送，发送后清空）
  const sentRefs = refChips.value.slice()
  const contextRefs = sentRefs.map(c => ({ kind: c.kind, id: c.id }))
  refChips.value = []

  // 添加用户消息
  messages.value.push({
    role: 'user',
    content,
    refs: sentRefs,
    timestamp: new Date(),
  })

  inputMessage.value = ''
  loading.value = true

  try {
    // 设置模型
    aiToolCaller.setModel(selectedProvider.value, selectedModel.value)

    // 构建消息历史（仅真实对话内容；提案卡片不进入历史）
    const history = messages.value
      .filter(m => (m.role === 'user' || m.role === 'assistant') && m.content)
      .map(m => ({
        role: m.role,
        content: m.content,
      }))

    // 调用 AI（带工具调用 + @ 引用沙箱）
    const result = await aiToolCaller.chatWithTools(
      [{ role: 'system', content: '你是 Casy AI 助手，帮助律师管理案件、任务和知识库。' }, ...history],
      { autoConfirm: false, contextRefs }
    )

    // 添加 AI 响应
    messages.value.push({
      role: 'assistant',
      content: result.content,
      toolCalls: result.toolCalls,
      toolResults: result.toolResults,
      usedRefs: result.usedRefs,
      timestamp: new Date(),
    })

    // AI 产生的写操作 → 提案 Diff 确认卡片（就地审批，不跳转）
    for (const p of result.proposals || []) {
      messages.value.push({
        role: 'proposal',
        proposalId: p.id,
        timestamp: new Date(),
      })
    }

    // 如果有工具调用结果，添加工具调用详情
    if (result.toolCalls.length > 0) {
      messages.value.push({
        role: 'system',
        content: `执行了 ${result.toolCalls.length} 个工具调用`,
        toolCallDetails: result.toolCalls.map((tc, i) => ({
          name: tc.name,
          params: tc.params,
          result: result.toolResults[i],
        })),
        timestamp: new Date(),
      })
    }
  } catch (error) {
    console.error('AI error:', error)
    const msg = error instanceof Error ? error.message : String(error)
    messages.value.push({
      role: 'assistant',
      content: '抱歉，发生了错误：' + msg,
      timestamp: new Date(),
    })
  } finally {
    loading.value = false
    scrollToBottom()
  }
}

/** 提案卡片审批完成后的跟进提示 */
function onProposalResolved(status) {
  const text = {
    executed: '变更已执行。',
    rejected: '变更已拒绝。',
    expired: '提案已过期。',
    approved: '提案已授权。',
  }[status]
  if (!text) return
  messages.value.push({ role: 'system', content: text, timestamp: new Date() })
  scrollToBottom()
}

// ============================================================
// @ 引用选择器
// ============================================================
function getTextarea() {
  return inputRef.value?.textarea || null
}

/** 输入/光标移动后同步 @ 触发状态 */
function syncMentionState() {
  const ta = getTextarea()
  if (!ta) return
  const caret = ta.selectionStart ?? 0
  const before = inputMessage.value.slice(0, caret)
  const atIdx = before.lastIndexOf('@')
  if (atIdx < 0) return closeMention()
  // '@' 前必须是行首或空白，避免误伤邮箱等文本
  if (atIdx > 0 && !/\s/.test(before[atIdx - 1])) return closeMention()
  const query = before.slice(atIdx + 1)
  if (/\s/.test(query)) return closeMention()
  mention.value.visible = true
  mention.value.query = query
  mention.value.start = atIdx
  mention.value.end = caret
  scheduleMentionSearch(query)
}

function scheduleMentionSearch(query) {
  if (mentionTimer) clearTimeout(mentionTimer)
  mentionTimer = setTimeout(() => runMentionSearch(query), 200)
}

async function runMentionSearch(query) {
  const q = query.trim()
  if (!q) {
    mention.value.results = []
    mention.value.index = 0
    return
  }
  // 竞态防护：只接受最新一次搜索的响应（旧响应丢弃）
  const reqId = ++mentionSearchSeq
  // 候选来源：全局搜索（知识+文件 FTS）/ 任务搜索 / 案件搜索；单路失败不影响其他
  const [gs, ts, cs] = await Promise.all([
    tauriCallSafe('global_search', { query: q }),
    tauriCallSafe('search_tasks', { query: q }),
    tauriCallSafe('search_cases', { query: q }),
  ])
  if (reqId !== mentionSearchSeq) return
  const results = []
  const seen = new Set()
  const push = (kind, id, title, sub) => {
    if (!id || !title) return
    const key = kind + ':' + id
    if (seen.has(key)) return
    seen.add(key)
    results.push({ kind, id, title, sub: sub || '' })
  }
  if (gs.ok && Array.isArray(gs.data)) {
    for (const r of gs.data.slice(0, 6)) {
      push(r.itemType === 'file' ? 'file' : 'knowledge', r.id, r.title, r.category)
    }
  }
  if (ts.ok && Array.isArray(ts.data)) {
    for (const t of ts.data.slice(0, 5)) {
      push('task', t.id, t.taskName, t.dueDate ? '截止 ' + t.dueDate : '')
    }
  }
  if (cs.ok && Array.isArray(cs.data)) {
    for (const c of cs.data.slice(0, 5)) {
      push('case', c.id, c.caseName, c.clientName)
    }
  }
  mention.value.results = results.slice(0, 12)
  mention.value.index = 0
}

function handleKeydown(e) {
  if (!mention.value.visible) return
  const total = mention.value.results.length
  if (e.key === 'ArrowDown' && total > 0) {
    e.preventDefault()
    mention.value.index = (mention.value.index + 1) % total
  } else if (e.key === 'ArrowUp' && total > 0) {
    e.preventDefault()
    mention.value.index = (mention.value.index - 1 + total) % total
  } else if (e.key === 'Enter' && !e.ctrlKey && !e.metaKey) {
    e.preventDefault()
    if (total > 0) {
      pickMention(mention.value.results[mention.value.index])
    } else {
      closeMention()
    }
  } else if (e.key === 'Escape') {
    e.preventDefault()
    e.stopPropagation()
    closeMention()
  }
}

function pickMention(item) {
  if (!item) return
  const { start, end } = mention.value
  // 移除输入框中的 @query 文本，引用以 chip 形式承载
  inputMessage.value = inputMessage.value.slice(0, start) + inputMessage.value.slice(end)
  if (!refChips.value.some(c => c.kind === item.kind && c.id === item.id)) {
    refChips.value.push({ kind: item.kind, id: item.id, title: item.title })
  }
  closeMention()
  nextTick(() => {
    const ta = getTextarea()
    if (ta) {
      ta.focus()
      ta.setSelectionRange(start, start)
    }
  })
}

function closeMention() {
  mention.value.visible = false
  mention.value.results = []
  mention.value.index = 0
}

function removeChip(index) {
  refChips.value.splice(index, 1)
}

function scrollToBottom() {
  nextTick(() => {
    const container = document.querySelector('.chat-messages')
    if (container) {
      container.scrollTop = container.scrollHeight
    }
  })
}

function formatTime(date) {
  if (!date) return ''
  return new Date(date).toLocaleTimeString('zh-CN', {
    hour: '2-digit',
    minute: '2-digit'
  })
}

function clearChat() {
  messages.value = []
  refChips.value = []
  closeMention()
  addSystemMessage()
}
</script>

<template>
  <div class="ai-chat-panel">
    <!-- 顶部工具栏 -->
    <div class="chat-toolbar">
      <div class="model-selector">
        <el-select
          v-model="selectedProvider"
          size="small"
          style="width: 120px"
          placeholder="提供商"
        >
          <el-option
            v-for="p in providers"
            :key="p.id"
            :label="p.name"
            :value="p.id"
          />
        </el-select>

        <el-select
          v-model="selectedModel"
          size="small"
          style="width: 150px"
          placeholder="模型"
        >
          <el-option
            v-for="m in availableModels"
            :key="m.model"
            :label="m.label"
            :value="m.model"
          />
        </el-select>
      </div>

      <div class="toolbar-actions">
        <el-switch
          v-model="showToolCalls"
          size="small"
          active-text="显示工具调用"
          inactive-text=""
        />
        <el-button size="small" @click="clearChat">清空</el-button>
      </div>
    </div>

    <!-- 消息列表 -->
    <div class="chat-messages">
      <div
        v-for="(msg, index) in messages"
        :key="index"
        :class="['message', `message-${msg.role}`]"
      >
        <!-- 系统消息 -->
        <div v-if="msg.role === 'system'" class="system-message">
          <div class="system-content" style="white-space: pre-wrap">{{ msg.content }}</div>

          <!-- 工具调用详情 -->
          <div v-if="msg.toolCallDetails && showToolCalls" class="tool-call-details">
            <div
              v-for="(tc, i) in msg.toolCallDetails"
              :key="i"
              class="tool-call-item"
            >
              <div class="tool-call-header">
                <el-icon><Tools /></el-icon>
                <span class="tool-name">{{ tc.name }}</span>
              </div>
              <div class="tool-call-params">
                <pre>{{ JSON.stringify(tc.params, null, 2) }}</pre>
              </div>
              <div class="tool-call-result">
                <el-icon v-if="tc.result?.ok !== false" color="#67C23A"><CircleCheck /></el-icon>
                <el-icon v-else color="#F56C6C"><CircleClose /></el-icon>
                <span>{{ tc.result?.ok !== false ? '成功' : '失败' }}</span>
              </div>
            </div>
          </div>
        </div>

        <!-- 用户消息 -->
        <div v-else-if="msg.role === 'user'" class="user-message">
          <div class="message-content">{{ msg.content }}</div>
          <!-- 随消息发出的引用 -->
          <div v-if="msg.refs?.length" class="msg-refs">
            <el-tag
              v-for="r in msg.refs"
              :key="r.kind + r.id"
              size="small"
              effect="plain"
              class="ref-chip readonly"
            >
              <el-icon><component :is="refIcon(r.kind)" /></el-icon>
              <span>{{ r.title }}</span>
            </el-tag>
          </div>
          <div class="message-time">{{ formatTime(msg.timestamp) }}</div>
        </div>

        <!-- AI 消息 -->
        <div v-else-if="msg.role === 'assistant'" class="assistant-message">
          <div class="message-content">{{ msg.content }}</div>

          <!-- 引用来源 -->
          <div v-if="msg.usedRefs?.length" class="msg-refs">
            <span class="msg-refs-label">引用来源</span>
            <el-tag
              v-for="r in msg.usedRefs"
              :key="r.kind + r.id"
              size="small"
              effect="plain"
              class="ref-chip readonly"
            >
              <el-icon><component :is="refIcon(r.kind)" /></el-icon>
              <span>{{ r.title }}</span>
            </el-tag>
          </div>

          <!-- 工具调用摘要 -->
          <div v-if="msg.toolCalls?.length > 0 && showToolCalls" class="tool-calls-summary">
            <el-tag size="small" type="info">
              调用了 {{ msg.toolCalls.length }} 个工具
            </el-tag>
          </div>

          <div class="message-time">{{ formatTime(msg.timestamp) }}</div>
        </div>

        <!-- 提案 Diff 确认卡片（W1） -->
        <div v-else-if="msg.role === 'proposal'" class="proposal-message">
          <ProposalDiffCard
            :proposal-id="msg.proposalId"
            :keyboard-active="index === lastProposalIndex"
            @resolved="onProposalResolved"
          />
        </div>
      </div>

      <!-- 加载中 -->
      <div v-if="loading" class="message message-loading">
        <el-icon class="loading-icon"><Loading /></el-icon>
        <span>思考中...</span>
      </div>
    </div>

    <!-- 输入区域 -->
    <div class="chat-input-area">
      <!-- 引用 chips -->
      <div v-if="refChips.length > 0" class="ref-chips">
        <el-tag
          v-for="(c, i) in refChips"
          :key="c.kind + c.id"
          size="small"
          closable
          class="ref-chip"
          @close="removeChip(i)"
        >
          <el-icon><component :is="refIcon(c.kind)" /></el-icon>
          <span>{{ c.title }}</span>
        </el-tag>
        <span class="ref-chips-hint">已引用 {{ refChips.length }} 项上下文</span>
      </div>

      <div class="chat-input">
        <el-input
          ref="inputRef"
          v-model="inputMessage"
          type="textarea"
          :rows="2"
          placeholder="输入消息，@ 可引用文件 / 知识 / 任务 / 案件"
          @input="syncMentionState"
          @keyup="syncMentionState"
          @click="syncMentionState"
          @keydown="handleKeydown"
          @keydown.enter.ctrl="sendMessage"
          @keydown.enter.meta="sendMessage"
        />
        <el-button
          type="primary"
          :icon="Position"
          :loading="loading"
          @click="sendMessage"
        >
          发送
        </el-button>
      </div>

      <!-- @ 引用候选浮层 -->
      <div v-if="mention.visible" class="mention-popover">
        <div v-if="mention.results.length > 0" class="mention-list">
          <div
            v-for="(item, i) in mention.results"
            :key="item.kind + item.id"
            class="mention-item"
            :class="{ active: i === mention.index }"
            @mousedown.prevent="pickMention(item)"
            @mouseenter="mention.index = i"
          >
            <el-icon class="mention-icon"><component :is="refIcon(item.kind)" /></el-icon>
            <span class="mention-title">{{ item.title }}</span>
            <span v-if="item.sub" class="mention-sub">{{ item.sub }}</span>
            <span class="mention-kind">{{ refKindLabel(item.kind) }}</span>
          </div>
        </div>
        <div v-else class="mention-empty">
          {{ mention.query ? '无匹配结果' : '输入以搜索 文件 / 知识 / 任务 / 案件' }}
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.ai-chat-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--c-bg-card);
  border-radius: 8px;
  overflow: hidden;
}

.chat-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid var(--c-border);
  background: var(--c-bg-subtle);
}

.model-selector {
  display: flex;
  gap: 8px;
}

.toolbar-actions {
  display: flex;
  align-items: center;
  gap: 12px;
}

.chat-messages {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
}

.message {
  margin-bottom: 16px;
}

.system-message {
  background: var(--c-bg-subtle);
  border-radius: 8px;
  padding: 12px;
  font-size: 13px;
  color: var(--gray-500);
  white-space: pre-wrap;
}

.user-message {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
}

.user-message .message-content {
  background: var(--c-primary-container);
  color: var(--c-on-primary-container);
  border-radius: 8px 8px 0 8px;
  padding: 8px 12px;
  max-width: 80%;
}

.assistant-message {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
}

.assistant-message .message-content {
  background: var(--c-bg-subtle);
  color: var(--c-text);
  border-radius: 8px 8px 8px 0;
  padding: 8px 12px;
  max-width: 80%;
  white-space: pre-wrap;
}

.proposal-message {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
}

.msg-refs {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 6px;
}

.msg-refs-label {
  font-size: 11px;
  color: var(--c-text-placeholder);
}

.message-time {
  font-size: 11px;
  color: var(--gray-400);
  margin-top: 4px;
}

.tool-calls-summary {
  margin-top: 8px;
}

.tool-call-details {
  margin-top: 12px;
  border-top: 1px solid var(--c-border);
  padding-top: 12px;
}

.tool-call-item {
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: 6px;
  padding: 8px;
  margin-bottom: 8px;
}

.tool-call-header {
  display: flex;
  align-items: center;
  gap: 6px;
  font-weight: 500;
  margin-bottom: 4px;
}

.tool-name {
  color: var(--c-primary);
}

.tool-call-params {
  background: var(--c-bg-subtle);
  border-radius: 4px;
  padding: 4px 8px;
  font-size: 12px;
  overflow-x: auto;
}

.tool-call-params pre {
  margin: 0;
  font-family: var(--font-mono);
}

.tool-call-result {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-top: 4px;
  font-size: 12px;
}

.message-loading {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--gray-500);
}

.loading-icon {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

/* ── 输入区域（含 @ 引用） ─────────────────────────────── */
.chat-input-area {
  position: relative;
  border-top: 1px solid var(--c-border);
}

.ref-chips {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
  padding: 8px 12px 0;
}

.ref-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  max-width: 240px;
}

.ref-chip :deep(span),
.ref-chip span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ref-chip.readonly {
  cursor: default;
}

.ref-chips-hint {
  font-size: 11px;
  color: var(--c-text-placeholder);
}

.chat-input {
  display: flex;
  gap: 8px;
  padding: 12px;
}

.chat-input .el-textarea {
  flex: 1;
}

/* ── @ 候选浮层 ────────────────────────────────────────── */
.mention-popover {
  position: absolute;
  left: 12px;
  right: 12px;
  bottom: 100%;
  margin-bottom: 4px;
  background: var(--c-bg-elevated);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.12);
  max-height: 280px;
  overflow-y: auto;
  z-index: 10;
}

.mention-list {
  padding: 4px;
}

.mention-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 10px;
  border-radius: var(--c-radius);
  cursor: pointer;
  font-size: 13px;
}

.mention-item.active {
  background: var(--c-primary-light);
}

.mention-icon {
  color: var(--c-primary);
  flex-shrink: 0;
}

.mention-title {
  color: var(--c-text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
  min-width: 0;
}

.mention-sub {
  font-size: 11px;
  color: var(--c-text-placeholder);
  flex-shrink: 0;
  max-width: 160px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.mention-kind {
  font-size: 11px;
  color: var(--c-text-secondary);
  background: var(--c-bg-subtle);
  border-radius: var(--c-radius-sm);
  padding: 1px 6px;
  flex-shrink: 0;
}

.mention-empty {
  padding: 14px 12px;
  font-size: 12px;
  color: var(--c-text-placeholder);
  text-align: center;
}
</style>
