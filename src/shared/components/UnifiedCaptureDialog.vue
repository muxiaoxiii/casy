<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { ElMessage } from 'element-plus'
import {
  Briefcase,
  Calendar,
  Collection,
  DocumentAdd,
  Finished,
  Folder,
  MagicStick,
  Paperclip,
  UploadFilled,
} from '@element-plus/icons-vue'
import { casyContext } from '../../core/plugin/context'
import { parseWhen } from '../nlp/parseWhen'

type CaptureAction = 'auto' | 'create_task' | 'create_event' | 'create_case' | 'create_project' | 'save_knowledge' | 'update_holidays' | 'service_delivery'

interface Recommendation {
  action: string
  reason?: string
  targetCaseId?: string | null
  targetCaseName?: string | null
  targetFolder?: string | null
  intent?: Record<string, unknown> | null
}

const props = withDefaults(defineProps<{
  modelValue: boolean
  initialAction?: CaptureAction
}>(), {
  initialAction: 'auto',
})

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  captured: [payload: { ids: string[]; action: CaptureAction }]
}>()

const text = ref('')
const action = ref<CaptureAction>('auto')
const filePaths = ref<string[]>([])
const dragging = ref(false)
const saving = ref(false)
const stage = ref<'compose' | 'review' | 'done'>('compose')
const capturedIds = ref<string[]>([])
const recommendations = ref<Recommendation[]>([])
const selectedRecommendation = ref<Recommendation | null>(null)

const actionOptions = [
  { value: 'auto', label: '自动判断', icon: MagicStick },
  { value: 'create_task', label: '任务', icon: Finished },
  { value: 'create_event', label: '日程', icon: Calendar },
  { value: 'create_case', label: '案件', icon: Briefcase },
  { value: 'create_project', label: '项目', icon: Folder },
  { value: 'save_knowledge', label: '知识', icon: Collection },
  { value: 'update_holidays', label: '节假日', icon: Calendar },
  { value: 'service_delivery', label: '法院送达', icon: DocumentAdd },
] as const

const canSubmit = computed(() => Boolean(text.value.trim() || filePaths.value.length))

function reset() {
  text.value = ''
  action.value = props.initialAction
  filePaths.value = []
  dragging.value = false
  saving.value = false
  stage.value = 'compose'
  capturedIds.value = []
  recommendations.value = []
  selectedRecommendation.value = null
}

interface CaseItem {
  id: string
  caseName: string
  caseNo?: string
  clientName?: string
}

const caseList = ref<CaseItem[]>([])
const targetCaseId = ref<string>('')

async function loadCases() {
  const result = await casyContext.cases.list({ page: 1, perPage: 300 })
  if (result.ok && Array.isArray(result.data)) {
    caseList.value = result.data.map((c: any) => ({
      id: c.id,
      caseName: c.displayName || c.caseName || c.name || '未命名案件',
      caseNo: c.caseNo || c.case_no || '',
      clientName: c.clientName || c.client_name || '',
    }))
  }
}

function onCaseChange(val: string) {
  targetCaseId.value = val
  if (selectedRecommendation.value) {
    selectedRecommendation.value.targetCaseId = val || null
    if (selectedRecommendation.value.intent) {
      selectedRecommendation.value.intent.caseId = val || null
    }
  }
}

watch(() => props.modelValue, async (visible) => {
  if (!visible) return
  reset()
  loadCases()
  await nextTick()
  document.querySelector<HTMLTextAreaElement>('.unified-capture-dialog textarea')?.focus()
})

watch(() => props.initialAction, (value) => {
  if (props.modelValue) action.value = value
})

function close() {
  emit('update:modelValue', false)
}

async function chooseFiles() {
  const { open } = await import('@tauri-apps/plugin-dialog')
  const selected = await open({ multiple: true, directory: false })
  if (!selected) return
  const paths = Array.isArray(selected) ? selected : [selected]
  filePaths.value = [...new Set([...filePaths.value, ...paths])]
}

function removeFile(path: string) {
  filePaths.value = filePaths.value.filter((item) => item !== path)
}

function onDrop(event: DragEvent) {
  event.preventDefault()
  dragging.value = false
  const files = Array.from(event.dataTransfer?.files || [])
  const paths = files.map((file) => (file as File & { path?: string }).path || file.name).filter(Boolean)
  filePaths.value = [...new Set([...filePaths.value, ...paths])]
}

function explicitRecommendation(value: Exclude<CaptureAction, 'auto'>): Recommendation {
  const content = text.value.trim()
  const parsed = parseWhen(content)
  const base = {
    name: content,
    title: content,
    content,
    taskName: parsed.taskName || content,
    dueDate: parsed.date,
    eventDate: parsed.date,
    startTime: parsed.time,
    caseId: targetCaseId.value || null,
  }
  const labels: Record<string, string> = {
    create_task: '按你的选择创建任务',
    create_event: '按你的选择创建日程',
    create_case: '按你的选择创建案件',
    create_project: '按你的选择创建非案件项目',
    save_knowledge: '按你的选择存入知识库',
    update_holidays: '解析法定节假日与调休安排，确认后更新日历',
    service_delivery: '按你的选择处理法院送达',
  }
  return { action: value, reason: labels[value], intent: base, targetCaseId: targetCaseId.value || null }
}

async function capture() {
  if (!canSubmit.value || saving.value) return
  saving.value = true
  const ids: string[] = []

  try {
    if (text.value.trim()) {
      const result = await casyContext.inbox.add('note', text.value.trim())
      if (!result.ok || !result.data) throw new Error(result.error || '文字捕获失败')
      ids.push(result.data)
    }

    for (const path of filePaths.value) {
      const result = await casyContext.inbox.add('file', undefined, path)
      if (!result.ok || !result.data) throw new Error(result.error || `文件捕获失败：${path}`)
      ids.push(result.data)
    }

    capturedIds.value = ids
    emit('captured', { ids, action: action.value })

    if (!text.value.trim()) {
      stage.value = 'done'
      return
    }

    if (action.value === 'auto') {
      const judged = await casyContext.inbox.quickJudge(ids[0])
      const data = judged.ok ? judged.data as { recommendations?: Recommendation[] } : null
      recommendations.value = data?.recommendations || []
    } else if (action.value === 'update_holidays' || action.value === 'service_delivery') {
      const judged = await casyContext.inbox.quickJudge(ids[0])
      const data = judged.ok ? judged.data as { recommendations?: Recommendation[] } : null
      const matched = data?.recommendations?.find(item => item.action === action.value)
      recommendations.value = [matched || explicitRecommendation(action.value)]
    } else {
      recommendations.value = [explicitRecommendation(action.value)]
    }

    selectedRecommendation.value = recommendations.value[0] || null
    targetCaseId.value = selectedRecommendation.value?.targetCaseId || ''
    stage.value = recommendations.value.length ? 'review' : 'done'
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : String(error))
  } finally {
    saving.value = false
  }
}

async function confirmRecommendation() {
  const recommendation = selectedRecommendation.value
  const inboxItemId = capturedIds.value[0]
  if (!recommendation || !inboxItemId || saving.value) return
  saving.value = true
  const result = await casyContext.inbox.confirmAction({
    inboxItemId,
    action: recommendation.action,
    targetCaseId: targetCaseId.value || recommendation.targetCaseId || null,
    targetCategory: recommendation.targetFolder,
    intent: recommendation.intent,
  })
  saving.value = false
  if (!result.ok) {
    ElMessage.error(result.error || '执行失败，内容仍保留在收件箱')
    return
  }
  stage.value = 'done'
  ElMessage.success('已完成处理并归入业务链条')
}

function fileName(path: string) {
  return path.split(/[\\/]/).pop() || path
}

function actionLabel(value: string) {
  return actionOptions.find((item) => item.value === value)?.label || '建议动作'
}
</script>

<template>
  <el-dialog
    :model-value="modelValue"
    width="min(680px, calc(100vw - 32px))"
    append-to-body
    class="unified-capture-dialog custom-glass-dialog"
    :show-close="false"
    @close="close"
  >
    <template #header>
      <div class="capture-heading">
        <div>
          <h2>统一捕获</h2>
          <p>先进入收件箱，由本地规则判断；需要时再调用 AI 增强。</p>
        </div>
        <kbd>⌘I</kbd>
      </div>
    </template>

    <div v-if="stage === 'compose'" class="capture-composer">
      <div
        class="capture-surface"
        :class="{ dragging }"
        @dragenter.prevent="dragging = true"
        @dragover.prevent
        @dragleave.prevent="dragging = false"
        @drop="onDrop"
      >
        <textarea
          v-model="text"
          rows="6"
          placeholder="输入想法、任务、日程、案件信息或法院短信，也可以拖入文件……"
          @keydown.meta.enter.prevent="capture"
          @keydown.ctrl.enter.prevent="capture"
        />
        <div v-if="filePaths.length" class="file-list">
          <button v-for="path in filePaths" :key="path" class="file-chip" type="button" @click="removeFile(path)">
            <el-icon><Paperclip /></el-icon>
            <span>{{ fileName(path) }}</span>
            <span class="remove-mark">移除</span>
          </button>
        </div>
        <button class="drop-action" type="button" @click="chooseFiles">
          <el-icon><UploadFilled /></el-icon>
          选择文件或拖到此处
        </button>
      </div>

      <div class="intent-row" aria-label="处理方式">
        <button
          v-for="option in actionOptions"
          :key="option.value"
          type="button"
          :class="['intent-option', { active: action === option.value }]"
          @click="action = option.value"
        >
          <el-icon><component :is="option.icon" /></el-icon>
          {{ option.label }}
        </button>
      </div>

      <div class="capture-note">
        “自动判断”只给出建议，不会静默创建或移动资料。
      </div>
    </div>

    <div v-else-if="stage === 'review'" class="review-stage">
      <div class="stage-kicker">捕获分析完成</div>
      <h3>确认这次捕获的落地方向与归属</h3>

      <!-- 1. 意图落地动作选择 -->
      <div class="review-section-title">1. 选择落地方向</div>
      <div class="recommendations-list">
        <button
          v-for="recommendation in recommendations"
          :key="recommendation.action + recommendation.reason"
          type="button"
          :class="['recommendation-row', { active: selectedRecommendation === recommendation }]"
          @click="selectedRecommendation = recommendation"
        >
          <span class="recommendation-title">{{ actionLabel(recommendation.action) }}</span>
          <span class="recommendation-reason">
            {{ recommendation.reason || '根据输入内容推荐' }}
            <small v-if="recommendation.action === 'update_holidays' && recommendation.intent">
              {{ recommendation.intent.year }} 年 · 放假 {{ (recommendation.intent.holidays as unknown[] || []).length }} 天 · 调休上班 {{ (recommendation.intent.workdays as unknown[] || []).length }} 天
            </small>
          </span>
        </button>
      </div>

      <!-- 2. 核心案件/项目链条归属确认 (防止产生孤立散落节点) -->
      <div v-if="selectedRecommendation?.action !== 'update_holidays'" class="case-binding-block">
        <div class="case-binding-header">
          <div class="cb-title">
            <el-icon><Briefcase /></el-icon>
            <span>2. 归属案件 / 项目（核心链条锚点）</span>
          </div>
          <span v-if="targetCaseId" class="cb-badge matched">
            已锚定案件
          </span>
          <span v-else class="cb-badge unlinked">
            全局独立项
          </span>
        </div>

        <el-select
          v-model="targetCaseId"
          filterable
          clearable
          placeholder="搜索并选择关联案件 / 项目（输入案号、当事人或案名）"
          class="case-select-input"
          @change="onCaseChange"
        >
          <el-option
            v-for="c in caseList"
            :key="c.id"
            :label="c.caseNo ? `[${c.caseNo}] ${c.caseName}` : c.caseName"
            :value="c.id"
          >
            <div class="case-option-item">
              <span class="co-name">{{ c.caseName }}</span>
              <span v-if="c.clientName" class="co-client">{{ c.clientName }}</span>
              <span v-if="c.caseNo" class="co-no">{{ c.caseNo }}</span>
            </div>
          </el-option>
        </el-select>

        <div v-if="targetCaseId" class="case-binding-tip matched-tip">
          ✨ 确认后将自动挂载至该案的脉络链条（任务、文书与动态点阵图）。
        </div>
        <div v-else class="case-binding-tip orphan-tip">
          💡 提示：若未选择案件，此项将作为全局独立项保存。建议选择关联案件以形成完整业务脉络链条，避免产生孤立节点。
        </div>
      </div>

      <p class="review-safety">不确认也没关系，原始内容已经安全保存在收件箱。</p>
    </div>

    <div v-else class="done-stage">
      <el-icon :size="30"><Finished /></el-icon>
      <h3>已保存</h3>
      <p>内容已进入可追溯流程，可以在收件箱继续查看和调整。</p>
    </div>

    <template #footer>
      <div class="capture-footer">
        <button class="button-secondary" type="button" @click="close">
          {{ stage === 'compose' ? '取消' : '关闭' }}
        </button>
        <button
          v-if="stage === 'compose'"
          class="button-primary"
          type="button"
          :disabled="!canSubmit || saving"
          @click="capture"
        >
          {{ saving ? '正在保存…' : '捕获并判断' }}
        </button>
        <button
          v-else-if="stage === 'review'"
          class="button-primary"
          type="button"
          :disabled="!selectedRecommendation || saving"
          @click="confirmRecommendation"
        >
          {{ saving ? '正在执行…' : '确认执行' }}
        </button>
      </div>
    </template>
  </el-dialog>
</template>

<style scoped>
.capture-heading {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 20px;
}

.capture-heading h2 {
  margin: 0;
  color: var(--c-text);
  font-size: 18px;
  line-height: 1.3;
}

.capture-heading p {
  margin: 5px 0 0;
  color: var(--c-text-secondary);
  font-size: 13px;
}

.capture-heading kbd {
  border: 1px solid var(--c-border);
  border-radius: 5px;
  background: var(--c-bg-subtle);
  color: var(--c-text-secondary);
  padding: 3px 7px;
  font-size: 11px;
}

.capture-surface {
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 12px;
  background: var(--c-bg-card);
  box-shadow: inset 0 2px 4px rgba(0,0,0,0.02);
  transition: all var(--motion-fast);
}

.capture-surface:focus-within,
.capture-surface.dragging {
  border-color: var(--c-primary);
  background: #fff;
  box-shadow: 0 0 0 3px rgba(62, 92, 154, 0.15), inset 0 2px 4px rgba(0,0,0,0.02);
}

.capture-surface textarea {
  box-sizing: border-box;
  width: 100%;
  min-height: 120px;
  resize: vertical;
  border: 0;
  outline: 0;
  background: transparent;
  color: var(--c-text);
  padding: 16px;
  font-size: 15px;
  line-height: 1.6;
}

.capture-surface textarea::placeholder {
  color: #858c99;
}

.drop-action {
  display: flex;
  align-items: center;
  gap: 7px;
  width: 100%;
  border: 0;
  border-top: 1px solid var(--c-border-light);
  background: transparent;
  color: var(--c-text-secondary);
  padding: 10px 14px;
  cursor: pointer;
  font: 12.5px inherit;
}

.drop-action:hover {
  color: var(--c-primary);
  background: var(--c-bg-subtle);
}

.file-list {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  padding: 0 12px 10px;
}

.file-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  max-width: 100%;
  border: 1px solid var(--c-border);
  border-radius: 6px;
  background: var(--c-bg-subtle);
  color: var(--c-text-regular);
  padding: 5px 8px;
  cursor: pointer;
}

.file-chip span:first-of-type {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.remove-mark {
  color: var(--c-text-secondary);
  font-size: 11px;
}

.intent-row {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 12px;
}

.intent-option {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-height: 32px;
  border: 1px solid var(--c-border);
  border-radius: 20px;
  background: var(--c-bg-subtle);
  color: var(--c-text-secondary);
  padding: 6px 12px;
  cursor: pointer;
  font-size: 13px;
  font-weight: 500;
  transition: all var(--motion-fast);
}

.intent-option:hover {
  background: var(--c-bg-hover);
  color: var(--c-text);
}

.intent-option.active {
  border-color: var(--c-primary);
  background: var(--c-primary-light);
  color: var(--c-primary);
  font-weight: 600;
}

.capture-note,
.review-safety {
  margin: 10px 0 0;
  color: var(--c-text-secondary);
  font-size: 12px;
}

.stage-kicker {
  color: var(--c-primary);
  font-size: 12px;
  font-weight: 600;
}

.review-stage h3,
.done-stage h3 {
  margin: 5px 0 14px;
  color: var(--c-text);
  font-size: 17px;
}

.review-section-title {
  margin: 12px 0 6px;
  color: var(--c-text-secondary);
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 0.5px;
  text-transform: uppercase;
}

.recommendations-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.case-binding-block {
  margin-top: 18px;
  padding: 14px 16px;
  border-radius: 10px;
  background: var(--c-bg-subtle, #f8fafc);
  border: 1px solid var(--c-border, #e2e8f0);
}

.case-binding-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 10px;
}

.cb-title {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--c-text);
  font-size: 13px;
  font-weight: 600;
}

.cb-badge {
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 12px;
  font-weight: 500;
}

.cb-badge.matched {
  background: #ecfdf5;
  color: #059669;
  border: 1px solid #a7f3d0;
}

.cb-badge.unlinked {
  background: #fffbeb;
  color: #d97706;
  border: 1px solid #fde68a;
}

.case-select-input {
  width: 100%;
}

.case-option-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  width: 100%;
}

.co-name {
  font-weight: 500;
  color: var(--c-text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.co-client {
  font-size: 12px;
  color: var(--c-text-secondary);
}

.co-no {
  font-size: 11px;
  font-family: var(--font-mono, monospace);
  color: var(--c-primary, #3e5c9a);
  background: rgba(62, 92, 154, 0.08);
  padding: 1px 6px;
  border-radius: 4px;
}

.case-binding-tip {
  margin-top: 8px;
  font-size: 12px;
  line-height: 1.5;
}

.matched-tip {
  color: #059669;
}

.orphan-tip {
  color: #b45309;
}

.recommendation-row {
  display: grid;
  grid-template-columns: 110px 1fr;
  gap: 12px;
  width: 100%;
  margin-top: 7px;
  border: 1px solid var(--c-border);
  border-radius: 8px;
  background: #fff;
  padding: 11px 12px;
  text-align: left;
  cursor: pointer;
}

.recommendation-row:hover,
.recommendation-row.active {
  border-color: var(--c-primary);
  background: #f6f8fc;
}

.recommendation-title {
  color: var(--c-text);
  font-weight: 600;
}

.recommendation-reason {
  color: var(--c-text-secondary);
}

.recommendation-reason small {
  display: block;
  margin-top: 4px;
  color: var(--c-text);
  font-size: 12px;
}

.done-stage {
  padding: 26px 12px;
  text-align: center;
  color: var(--c-primary);
}

.done-stage p {
  margin: 0;
  color: var(--c-text-secondary);
}

.capture-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

.button-primary,
.button-secondary {
  min-height: 34px;
  border-radius: 7px;
  padding: 7px 15px;
  cursor: pointer;
  font: 13px inherit;
  white-space: nowrap;
}

.button-primary {
  border: 1px solid var(--c-primary);
  background: var(--c-primary);
  color: #fff;
}

.button-primary:disabled {
  border-color: #aab4c7;
  background: #aab4c7;
  cursor: not-allowed;
}

.button-secondary {
  border: 1px solid var(--c-border);
  background: #fff;
  color: var(--c-text-regular);
}

@media (max-width: 680px) {
  .intent-row {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .recommendation-row {
    grid-template-columns: 1fr;
    gap: 4px;
  }
}
</style>
