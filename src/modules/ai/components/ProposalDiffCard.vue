<script setup lang="ts">
/**
 * 提案 Diff 确认卡片（W1：Cursor 式 AI Diff 确认视图）
 *
 * 渲染一条 pending 提案：标题（工具名 + 目标实体名）、字段级 before→after diff、
 * 过期时间与状态徽标；快捷键 Cmd/Ctrl+Enter 确认、Esc 拒绝。
 * 确认经 approve_ai_proposal 换取一次性 auth_token 后重放原始写操作。
 */
import { ref, computed, onMounted, onBeforeUnmount } from 'vue'
import { ElMessage } from 'element-plus'
import { tauriCallSafe } from '../../../core/tauriBridge'
import {
  Tools,
  Right,
  CircleCheck,
  CircleClose,
  Clock,
  Warning,
} from '../../../shared/icons'
import { aiToolCaller } from '../../../core/ai/tool-caller'
import {
  getProposalPreview,
  rejectProposal,
} from '../../../core/ai/proposals'
import type { ProposalPreviewDto, ProposalStatus } from '../../../core/ai/proposals'

const props = defineProps<{
  proposalId: string
  /** 是否为当前响应快捷键的卡片（面板中仅最后一张提案卡片激活） */
  keyboardActive?: boolean
}>()

const emit = defineEmits<{
  resolved: [status: ProposalStatus]
}>()

// ============================================================
// 状态
// ============================================================
const preview = ref<ProposalPreviewDto | null>(null)
const loading = ref(true)
const acting = ref<'' | 'approve' | 'reject' | 'renew'>('')
const nowTick = ref(Date.now())

let tickTimer: ReturnType<typeof setInterval> | null = null

// ============================================================
// 计算属性
// ============================================================
const status = computed<ProposalStatus>(() => preview.value?.proposal.status ?? 'pending')

const statusMeta = computed(() => {
  const map: Record<ProposalStatus, { label: string; tagType: 'primary' | 'success' | 'warning' | 'info' | 'danger' }> = {
    pending: { label: '待确认', tagType: 'warning' },
    approved: { label: '已授权', tagType: 'primary' },
    executed: { label: '已执行', tagType: 'success' },
    rejected: { label: '已拒绝', tagType: 'info' },
    expired: { label: '已过期', tagType: 'danger' },
  }
  return map[status.value]
})

const toolLabel = computed(() => preview.value?.proposal.toolName ?? '')

const entityTypeLabel = computed(() => {
  const t = preview.value?.proposal.targetEntityType ?? ''
  const map: Record<string, string> = {
    task: '任务',
    tasks: '任务',
    case: '案件',
    cases: '案件',
    knowledge: '知识',
    knowledge_item: '知识',
    knowledge_items: '知识',
    file: '文件',
    case_file: '文件',
    case_files: '文件',
  }
  return map[t] ?? t
})

const diffs = computed(() => preview.value?.fieldDiffs ?? [])

/** 过期倒计时文案 */
const expiresText = computed(() => {
  const p = preview.value?.proposal
  if (!p) return ''
  if (p.status !== 'pending') return ''
  const remainMs = new Date(p.expiresAt.replace(' ', 'T')).getTime() - nowTick.value
  if (remainMs <= 0) return '已过期'
  const minutes = Math.floor(remainMs / 60000)
  const seconds = Math.floor((remainMs % 60000) / 1000)
  return minutes > 0 ? `${minutes} 分 ${seconds} 秒后过期` : `${seconds} 秒后过期`
})

const isPending = computed(() => !!preview.value && status.value === 'pending' && !acting.value)

// ============================================================
// 方法
// ============================================================
async function load() {
  loading.value = true
  try {
    preview.value = await getProposalPreview(props.proposalId)
  } finally {
    loading.value = false
  }
}

function formatValue(v: unknown): string {
  if (v === null || v === undefined || v === '') return '（空）'
  let text: string
  if (typeof v === 'object') {
    try {
      text = JSON.stringify(v)
    } catch {
      text = String(v)
    }
  } else {
    text = String(v)
  }
  return text.length > 160 ? text.slice(0, 160) + '…' : text
}

async function onRenew(){
  if(acting.value)return
  acting.value='renew'
  try {const result=await tauriCallSafe('renew_ai_proposal',{proposalId:props.proposalId});if(!result.ok)throw new Error(result.error);await load();ElMessage.success('已重新核对当前数据，请检查新的差异后再确认变更。')}
  catch(error){ElMessage.error(String(error))}finally{acting.value=''}
}

async function onApprove() {
  if (!isPending.value) return
  acting.value = 'approve'
  try {
    const result = await aiToolCaller.approveProposalAndExecute(props.proposalId)
    await load()
    if (result.ok) {
      ElMessage.success('变更已确认并执行')
    } else {
      ElMessage.error('执行失败：' + (result.error || '未知错误'))
    }
    emit('resolved', status.value)
  } finally {
    acting.value = ''
  }
}

async function onReject() {
  if (!isPending.value) return
  acting.value = 'reject'
  try {
    const ok = await rejectProposal(props.proposalId)
    await load()
    if (ok !== null) {
      ElMessage.info('已拒绝该变更')
    }
    // 提案终态：清理 tool-caller 登记表（防单例 Map 泄漏）
    aiToolCaller.discardProposal(props.proposalId)
    emit('resolved', status.value)
  } finally {
    acting.value = ''
  }
}

/** 快捷键：Cmd/Ctrl+Enter 确认，Esc 拒绝（仅激活卡片且焦点不在聊天输入框时） */
function handleGlobalKeydown(e: KeyboardEvent) {
  if (!props.keyboardActive || !isPending.value) return
  // 有模态弹窗（el-dialog/drawer/message-box 等 overlay）打开时不抢占 Esc/Enter
  if (document.querySelector('.el-overlay')) return
  const target = e.target as HTMLElement | null
  if (target?.closest?.('.chat-input')) return
  // 富文本编辑器（tiptap）内的 Cmd+Enter/Esc 不触发提案操作
  if (target?.closest?.('.ProseMirror, [contenteditable="true"]')) return
  if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
    e.preventDefault()
    onApprove()
  } else if (e.key === 'Escape') {
    e.preventDefault()
    onReject()
  }
}

// ============================================================
// 生命周期
// ============================================================
onMounted(() => {
  load()
  tickTimer = setInterval(() => {
    nowTick.value = Date.now()
  }, 1000)
  window.addEventListener('keydown', handleGlobalKeydown)
})

onBeforeUnmount(() => {
  if (tickTimer) clearInterval(tickTimer)
  window.removeEventListener('keydown', handleGlobalKeydown)
})
</script>

<template>
  <div class="proposal-diff-card" :class="`pdc-${status}`">
    <!-- 头部：工具名 + 目标实体 + 状态徽标 -->
    <div class="pdc-header">
      <el-icon class="pdc-header-icon"><Tools /></el-icon>
      <span class="pdc-title">{{ toolLabel }}</span>
      <span v-if="preview?.targetEntityName" class="pdc-target">
        {{ entityTypeLabel }} · {{ preview.targetEntityName }}
      </span>
      <el-tag size="small" :type="statusMeta.tagType" effect="light" class="pdc-status-tag">
        {{ statusMeta.label }}
      </el-tag>
    </div>

    <!-- 加载中 -->
    <div v-if="loading" class="pdc-loading">加载提案预览…</div>

    <template v-else-if="preview">
      <!-- 字段级 Diff -->
      <div v-if="diffs.length > 0" class="pdc-diff">
        <div v-for="d in diffs" :key="d.field" class="pdc-row" :class="{ unchanged: !d.changed }">
          <div class="pdc-field">{{ d.field }}</div>
          <div class="pdc-values">
            <span class="pdc-before" :class="{ empty: d.before === null || d.before === undefined }">
              {{ formatValue(d.before) }}
            </span>
            <el-icon class="pdc-arrow"><Right /></el-icon>
            <span class="pdc-after">{{ formatValue(d.after) }}</span>
          </div>
        </div>
      </div>
      <div v-else class="pdc-no-diff">无字段级变更明细</div>

      <!-- 底部：过期时间 + 操作/终态 -->
      <div class="pdc-footer">
        <span v-if="status === 'pending'" class="pdc-expires">
          <el-icon><Clock /></el-icon>
          {{ expiresText }}
        </span>
        <span v-else class="pdc-terminal">
          <el-icon v-if="status === 'executed'"><CircleCheck /></el-icon>
          <el-icon v-else-if="status === 'rejected'"><CircleClose /></el-icon>
          <el-icon v-else><Warning /></el-icon>
          {{ statusMeta.label }}
        </span>

        <el-button v-if="status==='pending' || status==='expired'" size="small" :disabled="!!acting" @click="onRenew">重新核对并续期（不执行）</el-button>
        <div v-if="status === 'pending'" class="pdc-actions">
          <el-button size="small" :loading="acting === 'reject'" @click="onReject">
            拒绝
            <kbd class="pdc-kbd">Esc</kbd>
          </el-button>
          <el-button
            size="small"
            type="primary"
            :loading="acting === 'approve'"
            @click="onApprove"
          >
            确认变更
            <kbd class="pdc-kbd">⌘/Ctrl↵</kbd>
          </el-button>
        </div>
      </div>
    </template>

    <!-- 加载失败 -->
    <div v-else class="pdc-loading">提案不存在或已被清理</div>
  </div>
</template>

<style scoped>
.proposal-diff-card {
  max-width: 88%;
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  overflow: hidden;
  font-size: 13px;
}

.pdc-pending {
  border-color: var(--c-warning);
}

.pdc-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background: var(--c-bg-subtle);
  border-bottom: 1px solid var(--c-border-light);
}

.pdc-header-icon {
  color: var(--c-primary);
  flex-shrink: 0;
}

.pdc-title {
  font-weight: 600;
  color: var(--c-text-heading);
  font-family: var(--font-mono);
  font-size: 12px;
}

.pdc-target {
  color: var(--c-text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
  min-width: 0;
}

.pdc-status-tag {
  flex-shrink: 0;
}

.pdc-loading {
  padding: 16px 12px;
  color: var(--c-text-secondary);
}

.pdc-diff {
  padding: 6px 0;
}

.pdc-row {
  display: flex;
  gap: 12px;
  padding: 6px 12px;
  border-bottom: 1px solid var(--c-border-lighter);
}

.pdc-row:last-child {
  border-bottom: none;
}

.pdc-row.unchanged {
  opacity: 0.55;
}

.pdc-field {
  width: 120px;
  flex-shrink: 0;
  color: var(--c-text-secondary);
  font-family: var(--font-mono);
  font-size: 12px;
  padding-top: 1px;
  overflow: hidden;
  text-overflow: ellipsis;
}

.pdc-values {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 1;
  min-width: 0;
  flex-wrap: wrap;
}

.pdc-before {
  background: var(--c-danger-light);
  color: var(--c-danger);
  text-decoration: line-through;
  border-radius: var(--c-radius-sm);
  padding: 1px 6px;
  word-break: break-all;
}

.pdc-before.empty {
  text-decoration: none;
  background: transparent;
  color: var(--c-text-placeholder);
}

.pdc-arrow {
  color: var(--c-text-placeholder);
  flex-shrink: 0;
}

.pdc-after {
  background: var(--c-success-light);
  color: var(--c-success);
  border-radius: var(--c-radius-sm);
  padding: 1px 6px;
  word-break: break-all;
}

.pdc-no-diff {
  padding: 12px;
  color: var(--c-text-placeholder);
}

.pdc-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 8px 12px;
  border-top: 1px solid var(--c-border-light);
  background: var(--c-bg-subtle);
}

.pdc-expires,
.pdc-terminal {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  color: var(--c-text-secondary);
}

.pdc-actions {
  display: flex;
  gap: 8px;
}

.pdc-kbd {
  margin-left: 6px;
  font-family: var(--font-mono);
  font-size: 10px;
  opacity: 0.65;
}
</style>
