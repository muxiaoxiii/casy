<script setup lang="ts">
import { computed, nextTick, ref, watch, onMounted, onUnmounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
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
  Close,
  Check,
} from '../icons'
import { casyContext } from '../../core/plugin/context'
import { parseWhen } from '../nlp/parseWhen'
import type { IpcJsonObject } from '../../types/ipc'
import { isTauriRuntime } from '../../core/mockData'
import HolidayImportReview from './HolidayImportReview.vue'
import type { HolidayDraft } from '../holidayNotice'
import CaseWizard from '../../modules/cases/components/CaseWizard.vue'
import { newIntake, parseIntakeText } from '../../modules/cases/components/caseIntake'
import { useRouter } from 'vue-router'
const router = useRouter()
const nativeFiles = isTauriRuntime()
const caseLoading = ref(false)
const holidayDraft = ref<HolidayDraft | null>(null)
const holidayBusy = ref(false)
const reviewTitle = ref('')
const reviewDate = ref('')
const reviewTime = ref('')
const caseWizardOpen = ref(false)
const caseInitial = ref<Record<string, unknown>>({})

type CaptureAction = 'auto' | 'create_task' | 'create_event' | 'create_case' | 'create_project' | 'save_knowledge' | 'update_holidays' | 'service_delivery'

interface Recommendation {
  action: string
  reason?: string
  targetCaseId?: string | null
  targetCaseName?: string | null
  targetFolder?: string | null
  intent?: IpcJsonObject | null
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
const confirmed = ref(false)
const capturedIds = ref<string[]>([])
const savedInputs = new Map<string, string>()
const actionResult = ref<IpcJsonObject | null>(null)
const recommendations = ref<Recommendation[]>([])
const selectedRecommendation = ref<Recommendation | null>(null)
const selectedRecommendationIndex = computed({
  get: () => selectedRecommendation.value ? recommendations.value.indexOf(selectedRecommendation.value) : -1,
  set: (index: number) => { selectedRecommendation.value = recommendations.value[index] || null },
})

const actionOptions = [
  { value: 'auto', label: '自动判断', icon: MagicStick },
  { value: 'create_task', label: '任务', icon: Finished },
  { value: 'create_event', label: '日程', icon: Calendar },
  { value: 'create_case', label: '案件', icon: Briefcase },
  { value: 'save_knowledge', label: '知识', icon: Collection },
  { value: 'update_holidays', label: '节假日', icon: Calendar },
  { value: 'service_delivery', label: '法院送达', icon: DocumentAdd },
] as const

const canSubmit = computed(() => Boolean(text.value.trim() || filePaths.value.length))

function reset() {
  holidayDraft.value = null
  holidayBusy.value = false
  targetCaseId.value = ''
  text.value = ''
  action.value = props.initialAction
  filePaths.value = []
  dragging.value = false
  saving.value = false
  stage.value = 'compose'
  confirmed.value = false
  capturedIds.value = []
  savedInputs.clear()
  actionResult.value = null
  caseWizardOpen.value = false
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
let caseRequestId = 0

async function loadCases(query = '') {
  const requestId = ++caseRequestId
  caseLoading.value = true
  const result = await casyContext.cases.list({ page: 1, perPage: 100, search: query || null })
  if (requestId !== caseRequestId) return
  caseLoading.value = false
  if (result.ok && result.data) {
    caseList.value = result.data.items.map(c => ({
      id: c.id,
      caseName: c.caseName,
      caseNo: c.caseNo || '',
      clientName: c.clientName || '',
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

async function close() {
  if (saving.value) return
  if (stage.value === 'compose' && canSubmit.value) {
    try {
      await ElMessageBox.confirm('尚未完成捕获，关闭后未保存的输入将被丢弃。', '关闭捕获', { confirmButtonText: '丢弃并关闭', cancelButtonText: '继续编辑', type: 'warning' })
    } catch { return }
  }
  emit('update:modelValue', false)
}

async function chooseFiles() {
  if (!nativeFiles) return
  try {
  const { open } = await import('@tauri-apps/plugin-dialog')
  const selected = await open({ multiple: true, directory: false })
  if (!selected) return
  const paths = Array.isArray(selected) ? selected : [selected]
  filePaths.value = [...new Set([...filePaths.value, ...paths])]
  } catch (error) { ElMessage.error(error instanceof Error ? error.message : '无法选择文件') }
}

function removeFile(path: string) {
  filePaths.value = filePaths.value.filter((item) => item !== path)
}

function onDrop(event: DragEvent) {
  event.preventDefault()
  dragging.value = false
  if (saving.value) return
  const files = Array.from(event.dataTransfer?.files || [])
  const paths = files.map((file) => (file as File & { path?: string }).path).filter((path): path is string => Boolean(path?.startsWith('/')))
  if (files.length && !paths.length && !nativeFiles) ElMessage.warning('请在桌面应用中添加文件')
  filePaths.value = [...new Set([...filePaths.value, ...paths])]
}

function onNativeDrop(event: Event) {
  if (!props.modelValue || stage.value !== 'compose' || saving.value) return
  const paths = (event as CustomEvent<{ paths?: string[] }>).detail?.paths || []
  filePaths.value = [...new Set([...filePaths.value, ...paths])]
}
onMounted(() => window.addEventListener('casy:file-drop', onNativeDrop))
onUnmounted(() => window.removeEventListener('casy:file-drop', onNativeDrop))
watch(selectedRecommendation, async recommendation => {
  const intent = recommendation?.intent
  reviewTitle.value = String((recommendation?.action === 'create_case' ? intent?.caseName : recommendation?.action === 'create_task' ? intent?.taskName : intent?.title) || intent?.name || text.value)
  reviewDate.value = String(intent?.dueDate || intent?.eventDate || '')
  reviewTime.value = String(intent?.dueTime || intent?.startTime || '')
  if (!recommendation) return
  targetCaseId.value = recommendation.targetCaseId || ''
  const id = targetCaseId.value
  if (id && !caseList.value.some(item => item.id === id)) {
    const result = await casyContext.cases.get(id)
    if (result.ok && result.data && selectedRecommendation.value === recommendation && !caseList.value.some(item => item.id === id)) {
      caseList.value.push(result.data)
    }
  }
})
async function openInbox() { await close(); router.push('/inbox') }

function openCreated() {
  const result = actionResult.value
  const caseId = (result?.case as IpcJsonObject | undefined)?.id
  const knowledgeId = result?.knowledgeId
  close()
  if (typeof caseId === 'string') router.push({ name: 'case-detail', params: { id: caseId } })
  else if (typeof knowledgeId === 'string') router.push({ path: '/knowledge', query: { select: knowledgeId } })
  else router.push(selectedRecommendation.value?.action === 'create_event' ? '/calendar' : '/tasks')
}

function explicitRecommendation(value: Exclude<CaptureAction, 'auto'>): Recommendation {
  const content = text.value.trim()
  const parsed = parseWhen(content)
  const base = {
    name: content,
    title: value === 'create_event' ? parsed.taskName || content : content,
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
  const intent = value === 'create_case' ? { ...base, ...parseIntakeText(content).fields } : base
  return { action: value, reason: labels[value], intent: intent as IpcJsonObject, targetCaseId: targetCaseId.value || null }
}

async function capture() {
  if (!canSubmit.value || saving.value) return
  saving.value = true
  const ids: string[] = []

  try {
    if (text.value.trim()) {
      const key = `note:${text.value.trim()}`
      if (!savedInputs.has(key)) {
        const result = await casyContext.inbox.add('note', text.value.trim())
        if (!result.ok || !result.data) throw new Error(result.error || '文字捕获失败')
        savedInputs.set(key, result.data)
      }
      ids.push(savedInputs.get(key)!)
    }

    for (const path of filePaths.value) {
      const key = `file:${path}`
      if (!savedInputs.has(key)) {
        const result = await casyContext.inbox.add('file', undefined, path)
        if (!result.ok || !result.data) throw new Error(result.error || `文件捕获失败：${path}`)
        savedInputs.set(key, result.data)
      }
      ids.push(savedInputs.get(key)!)
    }

    capturedIds.value = ids
    window.dispatchEvent(new Event('casy:inbox-changed'))
    emit('captured', { ids, action: action.value })

    if (!text.value.trim()) {
      stage.value = 'done'
      return
    }

    if (action.value === 'auto') {
      const judged = await casyContext.inbox.quickJudge(ids[0])
      if (!judged.ok) throw new Error(judged.error || '原文已保存，暂时无法生成处理建议，请重试')
      const data = judged.ok ? judged.data as { recommendations?: Recommendation[] } : null
      recommendations.value = (data?.recommendations || []).filter(item => item.action !== 'create_project')
    } else if (action.value === 'update_holidays' || action.value === 'service_delivery') {
      const judged = await casyContext.inbox.quickJudge(ids[0])
      const data = judged.ok ? judged.data as { recommendations?: Recommendation[] } : null
      const matched = data?.recommendations?.find(item => item.action === action.value)
      recommendations.value = [matched || explicitRecommendation(action.value)]
    } else {
      recommendations.value = [explicitRecommendation(action.value)]
    }

    selectedRecommendation.value = recommendations.value[0] || null
    targetCaseId.value = selectedRecommendation.value?.targetCaseId || targetCaseId.value
    stage.value = recommendations.value.length ? 'review' : 'done'
  } catch (error) {
    if (ids.length) {
      capturedIds.value = ids
      window.dispatchEvent(new Event('casy:inbox-changed'))
    }
    ElMessage.error(error instanceof Error ? error.message : String(error))
  } finally {
    saving.value = false
  }
}

async function confirmRecommendation() {
  const recommendation = selectedRecommendation.value
  const inboxItemId = capturedIds.value[0]
  if (!recommendation || !inboxItemId || saving.value || holidayBusy.value) return
  if (recommendation.action === 'update_holidays' && !holidayDraft.value) { ElMessage.error('请先核对有效的节假日日期预览'); return }
  if (recommendation.action === 'create_case') {
    const initial = { ...newIntake(), ...recommendation.intent, ...parseIntakeText(text.value).fields }
    initial.caseName = reviewTitle.value.trim()
    initial.notes ||= text.value
    if (typeof initial.thirdParties === 'string') {
      try { initial.thirdParties = JSON.parse(initial.thirdParties) } catch { initial.thirdParties = [] }
    }
    if (typeof initial.attorneys === 'string') initial.attorneys = initial.attorneys.split(/[、,，;；]/).filter(Boolean)
    caseInitial.value = initial
    caseWizardOpen.value = true
    return
  }
  if (['create_task', 'create_event', 'create_case', 'save_knowledge'].includes(recommendation.action) && !reviewTitle.value.trim()) {
    ElMessage.warning('请填写标题')
    return
  }
  if (recommendation.action === 'create_event' && !reviewDate.value) {
    ElMessage.warning('请选择日程日期')
    return
  }
  saving.value = true
  try {
  const intent: IpcJsonObject = recommendation.action === 'update_holidays'
    ? { ...holidayDraft.value! } : { ...recommendation.intent, caseId: targetCaseId.value || null }
  if (['create_task', 'create_event', 'create_case', 'save_knowledge'].includes(recommendation.action)) {
    Object.assign(intent, { taskName: reviewTitle.value.trim(), title: reviewTitle.value.trim(), name: reviewTitle.value.trim() })
  }
  if (recommendation.action === 'create_case') intent.caseName = reviewTitle.value.trim()
  if (recommendation.action === 'create_task') Object.assign(intent, { dueDate: reviewDate.value || null, dueTime: reviewTime.value || null })
  if (recommendation.action === 'create_event') Object.assign(intent, { eventDate: reviewDate.value || null, startTime: reviewTime.value || null })
  const result = await casyContext.inbox.confirmAction({
    inboxItemId,
    action: recommendation.action,
    targetCaseId: targetCaseId.value || null,
    targetCategory: recommendation.targetFolder,
    intent,
  })
  if (!result.ok) {
    ElMessage.error(result.error || '执行失败，内容仍保留在收件箱')
    return
  }
  await fileCapturedAttachments(result.data as IpcJsonObject, targetCaseId.value)
  confirmed.value = true
  actionResult.value = result.data as IpcJsonObject
  stage.value = 'done'
  window.dispatchEvent(new Event('casy:inbox-changed'))
  ElMessage.success('已完成处理')
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : '执行失败，内容仍保留在收件箱')
  } finally {
    saving.value = false
  }
}

async function submitCapturedCase(payload: IpcJsonObject) {
  const result = await casyContext.inbox.confirmAction({ inboxItemId: capturedIds.value[0], action: 'create_case', intent: payload })
  if (result.ok) {
    await fileCapturedAttachments(result.data as IpcJsonObject)
    confirmed.value = true
    actionResult.value = result.data as IpcJsonObject
    stage.value = 'done'
    window.dispatchEvent(new Event('casy:inbox-changed'))
  }
  return result
}

async function fileCapturedAttachments(result: IpcJsonObject, selectedCase = '') {
  const caseId = (result?.case as IpcJsonObject | undefined)?.id || selectedCase
  if (typeof caseId !== 'string' || !caseId) return
  for (const id of capturedIds.value.slice(1)) {
    const filed = await casyContext.inbox.confirmAction({ inboxItemId: id, action: 'file_to_case', targetCaseId: caseId, targetCategory: 'other' })
    if (!filed.ok) throw new Error(filed.error || '附件归档失败，已保存的内容保留，请重试')
  }
}

function fileName(path: string) {
  return path.split(/[\\/]/).pop() || path
}

function actionLabel(value: string) {
  return actionOptions.find((item) => item.value === value)?.label || '建议动作'
}
</script>

<template>
  <el-dialog :model-value="modelValue && !caseWizardOpen" width="min(600px, calc(100vw - 24px))" append-to-body class="unified-capture-dialog"
    :show-close="false" :close-on-click-modal="false" :close-on-press-escape="!saving" :before-close="close">
    <template #header>
      <div class="capture-heading">
        <h2>{{ stage === 'compose' ? '快速捕获' : stage === 'review' ? '确认处理' : '已保存' }}</h2>
        <el-button :icon="Close" text circle aria-label="关闭捕获" title="关闭" :disabled="saving" @click="close" />
      </div>
    </template>
    <div v-if="stage === 'compose'" class="capture-composer">
      <div class="capture-surface" :class="{ dragging }" @dragenter.prevent="dragging = true" @dragover.prevent @dragleave.prevent="dragging = false" @drop="onDrop">
        <textarea v-model="text" aria-label="捕获内容" rows="5" :disabled="saving" placeholder="记录待办、想法或待整理的材料…" @keydown.meta.enter.prevent="capture" @keydown.ctrl.enter.prevent="capture" />
        <div v-if="filePaths.length" class="file-list">
          <div v-for="path in filePaths" :key="path" class="file-chip">
            <el-icon><Paperclip /></el-icon><span :title="fileName(path)">{{ fileName(path) }}</span>
            <el-button :icon="Close" text circle :aria-label="'移除 ' + fileName(path)" :disabled="saving" @click="removeFile(path)" />
          </div>
        </div>
        <div class="attachment-toolbar">
          <el-button :icon="Paperclip" text :disabled="!nativeFiles || saving" :title="nativeFiles ? '添加附件' : '请在桌面应用中添加文件'" @click="chooseFiles">添加附件</el-button>
          <span v-if="filePaths.length">{{ filePaths.length }} 个附件</span>
        </div>
      </div>
      <div class="capture-fields">
        <label><span>处理方式</span><el-select v-model="action" aria-label="处理方式" :disabled="saving">
          <el-option v-for="option in actionOptions" :key="option.value" :value="option.value" :label="option.label"><el-icon><component :is="option.icon" /></el-icon> {{ option.label }}</el-option>
        </el-select></label>
        <label v-if="['create_task', 'create_event', 'save_knowledge'].includes(action)"><span>关联案件</span>
          <el-select v-model="targetCaseId" aria-label="关联案件" filterable remote :remote-method="loadCases" :loading="caseLoading" clearable placeholder="未关联" :disabled="saving">
            <el-option v-for="c in caseList" :key="c.id" :value="c.id" :label="c.caseName" />
          </el-select>
        </label>
      </div>
    </div>
    <div v-else-if="stage === 'review'" class="review-stage">
      <div class="saved-status"><el-icon><Check /></el-icon>原始内容已存入收件箱</div>
      <el-radio-group v-model="selectedRecommendationIndex" class="recommendations-list" aria-label="处理建议">
        <el-radio v-for="(recommendation, index) in recommendations" :key="index" :value="index" :disabled="saving">{{ actionLabel(recommendation.action) }}</el-radio>
      </el-radio-group>
      <p v-if="selectedRecommendation?.reason" class="recommendation-reason">{{ selectedRecommendation.reason }}</p>
      <HolidayImportReview v-if="selectedRecommendation?.action === 'update_holidays'" :content="text" :disabled="saving" @change="holidayDraft = $event" @busy="holidayBusy = $event" />
      <el-form label-position="top" class="review-form">
        <el-form-item v-if="['create_task', 'create_event', 'create_case', 'save_knowledge'].includes(selectedRecommendation?.action || '')" label="标题">
          <el-input v-model="reviewTitle" aria-label="标题" :disabled="saving" />
        </el-form-item>
        <div v-if="['create_task', 'create_event'].includes(selectedRecommendation?.action || '')" class="review-time">
          <el-form-item :label="selectedRecommendation?.action === 'create_event' ? '日程日期' : '截止日期'"><el-date-picker v-model="reviewDate" aria-label="日期" type="date" value-format="YYYY-MM-DD" placeholder="未设置" :disabled="saving" /></el-form-item>
          <el-form-item label="时间"><el-time-picker v-model="reviewTime" aria-label="时间" format="HH:mm" value-format="HH:mm" clearable placeholder="未设置" :disabled="saving" /></el-form-item>
        </div>
        <el-form-item v-if="!['update_holidays', 'create_case'].includes(selectedRecommendation?.action || '')" label="关联案件">
          <el-select v-model="targetCaseId" filterable remote :remote-method="loadCases" :loading="caseLoading" clearable placeholder="未关联案件" :disabled="saving" @change="onCaseChange">
            <el-option v-for="c in caseList" :key="c.id" :value="c.id" :label="c.caseNo ? c.caseName + ' · ' + c.caseNo : c.caseName" />
          </el-select>
        </el-form-item>
      </el-form>
    </div>
    <div v-else class="done-stage">
      <el-icon :size="32"><Check /></el-icon><h3>{{ confirmed ? '处理完成' : '已存入收件箱' }}</h3>
      <p v-if="confirmed && capturedIds.length > 1 && !targetCaseId && !actionResult?.case">{{ capturedIds.length - 1 }} 个附件待整理</p>
      <el-button v-if="confirmed && ['create_task','create_event','create_case','save_knowledge'].includes(selectedRecommendation?.action || '')" type="primary" @click="openCreated">查看{{ actionLabel(selectedRecommendation?.action || '') }}</el-button>
      <template v-if="confirmed && actionResult?.action === 'holidays_updated'">
        <p>已写入日历：放假 {{ actionResult.holidaysCount }} 天，补班 {{ actionResult.workdaysCount }} 天。<span v-if="actionResult.changedDates === 0">这些日期与现有日历一致，无新增变动。</span><span v-else-if="actionResult.changedDates != null">实际更新 {{ actionResult.changedDates }} 个日期。</span></p>
        <el-button type="primary" @click="close(); router.push({ path: '/calendar', query: { view: 'year', date: `${actionResult.year}-01-01` } })">查看导入后的日历</el-button>
      </template>
      <el-button text @click="openInbox">查看收件箱</el-button>
    </div>
    <template #footer>
      <div class="capture-footer">
        <el-button :disabled="saving" @click="close">{{ stage === 'compose' ? '取消' : stage === 'review' ? '稍后处理' : '关闭' }}</el-button>
        <el-button v-if="stage === 'compose'" type="primary" :disabled="!canSubmit" :loading="saving" @click="capture">{{ action === 'auto' ? '存入收件箱' : '继续' }}</el-button>
        <el-button v-else-if="stage === 'review'" type="primary" :disabled="!selectedRecommendation || holidayBusy || (selectedRecommendation.action === 'update_holidays' && !holidayDraft)" :loading="saving" @click="confirmRecommendation">{{ selectedRecommendation?.action === 'create_case' ? '完善案件信息' : '确认处理' }}</el-button>
      </div>
    </template>
  </el-dialog>
  <CaseWizard v-if="caseWizardOpen" v-model="caseWizardOpen" title="新建案件" :initial-case="caseInitial" :submit="submitCapturedCase" />
</template>

<style scoped>
.capture-heading { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
.capture-heading h2 { font-size: 18px; line-height: 1.4; color: var(--c-text-heading); margin: 0; }
.capture-surface { border: 1px solid var(--c-border-strong); border-radius: 8px; background: var(--c-bg-card); transition: border-color var(--motion-fast), box-shadow var(--motion-fast); }
.capture-surface:focus-within, .capture-surface.dragging { border-color: var(--c-primary); box-shadow: 0 0 0 2px var(--c-primary-light); }
.capture-surface textarea { display: block; width: 100%; min-height: 160px; max-height: 320px; resize: vertical; border: 0; outline: 0; background: transparent; color: var(--c-text); padding: 16px; font: inherit; font-size: 14px; line-height: 1.7; }
.capture-surface textarea::placeholder { color: var(--c-text-secondary); }
.attachment-toolbar { display: flex; justify-content: space-between; align-items: center; gap: 8px; padding: 4px 8px; border-top: 1px solid var(--c-border-light); }
.attachment-toolbar > span { font-size: 12px; color: var(--c-text-secondary); }
.file-list { padding: 0 12px 10px; display: flex; flex-direction: column; gap: 4px; }
.file-chip { display: flex; align-items: center; gap: 8px; padding: 2px 8px; border-radius: 4px; background: var(--c-bg-subtle); color: var(--c-text-regular); font-size: 12px; }
.file-chip > span { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.capture-fields, .review-time { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 16px; margin-top: 20px; }
.capture-fields label { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
.capture-fields label > span { font-size: 12px; color: var(--c-text-secondary); }
.saved-status { display: flex; align-items: center; gap: 8px; color: var(--c-success); font-size: 13px; padding: 0 0 16px; border-bottom: 1px solid var(--c-border); }
.recommendations-list { margin-top: 12px; }
.recommendation-reason { color: var(--c-text-secondary); font-size: 12px; line-height: 1.6; margin: 0 0 16px; }
.review-time { margin: 0; }
.review-form :deep(.el-date-editor), .review-form :deep(.el-select) { width: 100%; }
.done-stage { padding: 24px 0; text-align: center; color: var(--c-success); }
.done-stage h3 { font-size: 16px; color: var(--c-text); margin: 12px 0; }
.capture-footer { display: flex; justify-content: flex-end; gap: 8px; padding-top: 12px; border-top: 1px solid var(--c-border); }
@media (max-width: 480px) { .capture-fields, .review-time { grid-template-columns: minmax(0, 1fr); gap: 12px; } }
</style>
<style>
.unified-capture-dialog { max-height: calc(100dvh - min(15vh, 48px) - 16px); margin-top: min(15vh, 48px); display: flex; flex-direction: column; }
.unified-capture-dialog > .el-dialog__body { min-height: 0; overflow-y: auto; }
.unified-capture-dialog > .el-dialog__header, .unified-capture-dialog > .el-dialog__footer { flex-shrink: 0; }
</style>
