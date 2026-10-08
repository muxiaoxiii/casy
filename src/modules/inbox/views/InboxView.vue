<script setup>
import UiDataState from '../../../shared/ui/UiDataState.vue'
import EmptyState from '../../../shared/components/EmptyState.vue'
import HolidayImportReview from '../../../shared/components/HolidayImportReview.vue'
import { useRouter } from 'vue-router'
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { safeListen } from '../../../core/tauriEvents'
import { casyContext } from '../../../core/plugin/context'
import { AI_PROMPTS } from '../../../core/prompts'
import { todayLocalISO } from '../../../shared/utils/date'
import { useVoiceNote } from '../composables/useVoiceNote'
import { useCapture } from '../composables/useCapture'
import { isTauriRuntime } from '../../../core/mockData'
import { ElMessage } from 'element-plus'
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
  Clock,
  Warning,
  Refresh,
} from '../../../shared/icons'

const {isRecording,isSaving:voiceSaving,recordingTime,startRecording,stopRecording,formatTime}=useVoiceNote()
const {captureClipboard}=useCapture()
async function captureText(){if(await captureClipboard())await loadItems()}
async function toggleRecording(){if(isRecording.value)stopRecording();else try{await startRecording()}catch(error){ElMessage.error(`无法录音：${error}`)}}
watch(voiceSaving,(saving,wasSaving)=>{if(wasSaving && !saving)void loadItems()})

const router = useRouter()
const holidayDraft = ref(null)
const holidayBusy = ref(false)
const receipt = ref(null)
const holidayCandidate = computed(() => /节假日|放假|调休|"holidays"/.test(selectedItem.value?.contentText || ''))
let receiptRequest = 0
async function confirmHolidays() {
  if (!holidayDraft.value || holidayBusy.value || processing.value || !selectedItem.value) return
  processing.value = true
  try {
    const result = await casyContext.inbox.confirmAction({ inboxItemId: selectedItem.value.id, action: 'update_holidays', intent: { ...holidayDraft.value } })
    if (!result.ok) throw new Error(result.error || '节假日写入失败')
    ElMessage.success(`已写入日历：放假 ${result.data.holidaysCount} 天，补班 ${result.data.workdaysCount} 天`)
    await loadItems()
    router.push({ path: '/calendar', query: { view: 'year', date: `${result.data.year}-01-01` } })
  } catch (error) { ElMessage.error(String(error)) }
  finally { processing.value = false }
}
const items = ref([])
const loading = ref(false)
const processing = ref(false)
const structuring = ref(false)
const selectedItemId = ref('')
const sourceFilter = ref('all')
const quickCaptureInputText = ref('')
const casesList = ref([])
const loadError = ref('')
const countError = ref('')
const caseError = ref('')
const statusFilter = ref('pending')
const clarifyTitle = ref('')
const clarifyWaitingFor = ref('')
const clarifyFollowUp = ref('')
const nativeFiles = isTauriRuntime()

// GTD 澄清状态
const clarifyAction = ref('action') // 'action' | 'delegate' | 'wait' | 'someday'
const clarifyContext = ref('office')
const clarifyMatter = ref('')
const clarifyCaseId = ref('')
const clarifyDoWhen = ref('today')
const clarifyDeadline = ref('')
const clarifyNotes = ref('')
const clarifyEstMinutes = ref(null)

const pendingItems = computed(() => items.value.filter((i) => i.status === 'pending'))
const filedItems = computed(() => items.value.filter((i) => i.status === 'filed'))
// 待处理总数走后端聚合：列表分页/截断时徽标仍然真实
const pendingCount = ref(0)

const sourceFilters = computed(() => {
  const currentItems = statusFilter.value === 'pending' ? pendingItems.value : filedItems.value
  const counts = currentItems.reduce((result, item) => {
    const source = item.sourceType || 'note'
    result[source] = (result[source] || 0) + 1
    return result
  }, {})
  return [
    { value: 'all', label: '全部', count: currentItems.length, icon: Collection },
    { value: 'email', label: '邮件', count: counts.email || 0, icon: Message },
    { value: 'wechat', label: '微信', count: counts.wechat || 0, icon: ChatDotRound },
    { value: 'note', label: '速记', count: counts.note || 0, icon: Phone },
    { value: 'file', label: '文件', count: counts.file || 0, icon: Paperclip },
  ]
})

const filteredPendingItems = computed(() => {
  const list = statusFilter.value === 'pending' ? pendingItems.value : filedItems.value
  if (sourceFilter.value === 'all') return list
  return list.filter((item) => (item.sourceType || 'note') === sourceFilter.value)
})

const selectedItem = computed(() => {
  const list = filteredPendingItems.value
  return list.find((item) => item.id === selectedItemId.value) || list[0] || null
})

watch(() => [selectedItem.value?.id, selectedItem.value?.status], async ([id, status]) => {
  const request = ++receiptRequest
  receipt.value = null; holidayDraft.value = null
  if (!id || status !== 'filed') return
  const result = await casyContext.inbox.actionResult(id)
  if (request === receiptRequest && result.ok) receipt.value = result.data
})



function selectSource(val) {
  sourceFilter.value = val
}

function selectItem(item) {
  selectedItemId.value = item.id
  resetClarification(item)
}
function resetClarification(item) {
  if (!item) return
  clarifyTitle.value = item.title || ''
  clarifyAction.value = 'action'
  clarifyContext.value = 'office'
  clarifyDoWhen.value = 'today'
  clarifyDeadline.value = ''
  clarifyWaitingFor.value = ''
  clarifyFollowUp.value = ''
  clarifyEstMinutes.value = null
  clarifyNotes.value = item.contentText || ''
  clarifyCaseId.value = item.linkedCaseId || item.aiSuggestedCaseId || item.caseId || ''
  const matched = casesList.value.find((c) => c.id === clarifyCaseId.value)
  clarifyMatter.value = matched?.displayName || matched?.caseName || item.caseName || ''
}
watch(() => selectedItem.value?.id, () => resetClarification(selectedItem.value))
function sourceLabel(type) { return ({ email: '邮件', wechat: '微信', note: '速记', file: '文件' }[type] || '收件项') }
function capturedAt(item) { return item.createdAt ? String(item.createdAt).replace('T', ' ').slice(0, 16) : '' }

function onClarifyCaseChange(caseId) {
  clarifyCaseId.value = caseId
  const matched = casesList.value.find((c) => c.id === caseId)
  clarifyMatter.value = matched?.displayName || matched?.caseName || ''
}

// ============================================================
// 数据加载
// ============================================================
safeListen('inbox:new_item', () => { void loadItems() })
onMounted(async () => {
  window.addEventListener('casy:inbox-changed', loadItems)
  await loadItems()
  await loadCases()
  if (filteredPendingItems.value[0]) {
    selectItem(filteredPendingItems.value[0])
  }
})
onUnmounted(() => window.removeEventListener('casy:inbox-changed', loadItems))

async function loadItems() {
  loading.value = true
  loadError.value = ''
  const result = await casyContext.inbox.list()
  if (result.ok && Array.isArray(result.data)) {
    items.value = result.data
  } else {
    loadError.value = result.error || '收件箱加载失败'
  }
  const count = await casyContext.inbox.count('pending')
  if (count.ok && typeof count.data === 'number') { pendingCount.value = count.data; countError.value = '' }
  else countError.value = count.error || '待处理数量读取失败'
  loading.value = false
}

async function loadCases() {
  // 分页取全量（审查 N10）：原来 list({}) 只用后端默认一页，案件下拉被截断在 50 条
  const result = await casyContext.cases.listAll()
  if (result.ok) {
    casesList.value = result.data || []
    caseError.value = ''
  } else {
    // 失败反馈：否则澄清面板的案件下拉静默空白，用户无法归卷
    caseError.value = result.error || '案件加载失败'
  }
}

// ============================================================
// GTD 澄清处理 (Turn to Action)
// ============================================================
async function fileCurrentItem(action) {
  const item=selectedItem.value
  if(!item || processing.value)return
  if(action==='file_to_case' && !clarifyCaseId.value)return ElMessage.warning('请先选择归属案件')
  processing.value=true
  try {
    const result=await casyContext.inbox.confirmAction({inboxItemId:item.id,action,targetCaseId:clarifyCaseId.value || null,intent:{title:clarifyTitle.value || item.title,content:clarifyNotes.value || item.contentText || ''}})
    if(!result.ok)throw new Error(result.error || '处理失败')
    ElMessage.success(action==='file_to_case'?'已归卷至案件':'已保存到知识库')
    await loadItems()
  } catch(error){ElMessage.error(String(error))}finally{processing.value=false}
}

async function processCurrentItem(actionType = clarifyAction.value) {
  const item = selectedItem.value
  if (!item || processing.value) return
  if (!clarifyTitle.value.trim()) return ElMessage.warning('请填写任务标题')

  processing.value = true

  const taskIntent = {
    taskName: clarifyTitle.value.trim(),
    description: clarifyNotes.value || item.contentText || '',
    taskType: actionType === 'delegate' || actionType === 'wait' ? 'waiting' : 'action',
    startBucket: actionType === 'someday' ? 'someday' : clarifyDoWhen.value,
    dueDate: clarifyDeadline.value || null,
    startDate: clarifyDoWhen.value === 'today' ? todayLocalISO() : null,
    waitingFor: ['delegate', 'wait'].includes(actionType) ? clarifyWaitingFor.value.trim() || null : null,
    followUpDate: ['delegate', 'wait'].includes(actionType) ? clarifyFollowUp.value || null : null,
    context: clarifyContext.value || null,
    estimatedMinutes: clarifyEstMinutes.value || null,
  }

  const confirmResult = await casyContext.inbox.confirmAction({
    inboxItemId: item.id,
    action: 'create_task',
    targetCaseId: clarifyCaseId.value || null,
    intent: taskIntent,
  })
  if (!confirmResult.ok) {
    processing.value = false
    ElMessage.error(confirmResult.error || '操作失败，收件项未被归档')
    return
  }

  const successMessages = {
    action: '已创建任务',
    delegate: '已建立委派跟踪任务',
    wait: '已记入外部等待列表',
    someday: '已归入将来也许清单',
  }
  ElMessage.success(successMessages[actionType] || '处理完成')

  processing.value = false
  await loadItems()
  if (filteredPendingItems.value[0]) selectItem(filteredPendingItems.value[0])
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
  if (!text || processing.value) return
  processing.value = true
  const result = await casyContext.inbox.add('note', text)
  processing.value = false
  if (result.ok) {
    ElMessage.success('已快速捕获到收件箱')
    quickCaptureInputText.value = ''
    await loadItems()
    statusFilter.value = 'pending'
    sourceFilter.value = 'all'
    selectedItemId.value = result.data || ''
  } else {
    ElMessage.error(result.error || '捕获失败')
  }
}

async function importFile() {
  if (!nativeFiles || processing.value) return
  try {
  const { open } = await import('@tauri-apps/plugin-dialog')
  const selected = await open({ multiple: true })
  if (!selected) return

  const files = Array.isArray(selected) ? selected : [selected]
  processing.value = true
  let okCount = 0
  const errors = []
  for (const file of files) {
    const res = await casyContext.inbox.add('file', null, file)
    if (res.ok) okCount++
    else errors.push(res.error || file)
  }
  processing.value = false
  // 汇总真实结果：部分失败时不再虚假报成功
  if (errors.length && okCount === 0) {
    return ElMessage.error(`导入失败：${errors.join('；')}`)
  }
  if (errors.length) {
    ElMessage.warning(`成功 ${okCount} 个，失败 ${errors.length} 个：${errors.join('；')}`)
  } else {
    ElMessage.success(`已导入 ${files.length} 个文件`)
  }
  await loadItems()
  } catch (error) {
    processing.value = false
    ElMessage.error(error instanceof Error ? error.message : '文件导入失败')
  }
}

async function dismissItem(item) {
  if (item.id) {
    const res = await casyContext.inbox.dismiss(item.id)
    // 后端失败：报错并退出，不弹成功
    if (!res.ok) return ElMessage.error(res.error || '忽略失败')
  }
  ElMessage.success('已忽略此项')
  await loadItems()
}
</script>

<template>
  <div class="inbox-page">
    <header class="inbox-header">
      <div><h1>收件箱</h1><span>{{ pendingCount }} 项待处理</span></div>
      <el-button :icon="Refresh" circle :loading="loading" title="刷新收件箱" aria-label="刷新收件箱" @click="loadItems" />
    </header>
    <el-alert v-if="countError" :title="`待处理数量读取失败：${countError}`" type="error" :closable="false"><el-button text @click="loadItems">重试</el-button></el-alert>
    <form class="quick-capture" @submit.prevent="submitQuickCapture">
      <el-icon><Plus /></el-icon>
      <el-button v-if="nativeFiles" :icon="Microphone" :type="isRecording?'danger':'default'" :disabled="voiceSaving" @click="toggleRecording">{{ isRecording ? `停止 ${formatTime(recordingTime)}` : voiceSaving ? '保存录音…' : '录音' }}</el-button>
      <el-button v-if="nativeFiles" :disabled="processing" @click="captureText">粘贴文字</el-button>
      <input v-model="quickCaptureInputText" aria-label="快速记录" placeholder="记录一件待办或想法…" :disabled="processing" />
      <el-button :icon="Paperclip" text circle :disabled="!nativeFiles || processing" :title="nativeFiles ? '导入文件' : '请在桌面应用中导入文件'" aria-label="导入文件" @click="importFile" />
      <el-button :icon="ArrowRight" type="primary" native-type="submit" :disabled="!quickCaptureInputText.trim()" :loading="processing" aria-label="存入收件箱" title="存入收件箱" />
    </form>
    <div class="inbox-layout">
      <section class="inbox-list">
        <div class="list-controls">
          <el-radio-group v-model="statusFilter" size="small" aria-label="处理状态">
            <el-radio-button value="pending">待处理 {{ pendingItems.length }}</el-radio-button>
            <el-radio-button value="filed">已处理 {{ filedItems.length }}</el-radio-button>
          </el-radio-group>
          <nav class="source-filters" aria-label="收件来源">
            <button v-for="source in sourceFilters" :key="source.value" type="button" :class="{ active: sourceFilter === source.value }" :aria-pressed="sourceFilter === source.value" @click="selectSource(source.value)">{{ source.label }}<span>{{ source.count }}</span></button>
          </nav>
        </div>
        <div class="item-list">
          <UiDataState :loading="loading" :error="loadError" :count="filteredPendingItems.length" :filtered="sourceFilter !== 'all'"
            :empty-title="statusFilter === 'pending' ? '暂无待处理事项' : '暂无已处理事项'"
            empty-description="新增收件会在这里显示，可从上方输入或添加文件。" @retry="loadItems" @clear="selectSource('all')">
          <button v-for="item in filteredPendingItems" :key="item.id" class="inbox-item" :class="{ selected: selectedItem?.id === item.id }" :aria-pressed="selectedItem?.id === item.id" @click="selectItem(item)">
            <span class="item-meta"><span>{{ sourceLabel(item.sourceType) }}</span><span v-if="capturedAt(item)">{{ capturedAt(item) }}</span></span>
            <strong>{{ item.title || '未命名收件项' }}</strong>
            <span class="item-snippet">{{ item.contentText || item.sourcePath || item.filePath || '附件' }}</span>
            <span v-if="item.caseName" class="item-case"><el-icon><Briefcase /></el-icon>{{ item.caseName }}</span>
          </button>
          </UiDataState>
        </div>
      </section>
      <section v-if="selectedItem" class="clarify-panel">
        <header class="clarify-header">
          <span>{{ selectedItem.status === 'pending' ? '整理收件项' : '已处理收件项' }}</span>
          <div v-if="selectedItem.status === 'pending'">
            <el-button :icon="MagicStick" text circle :loading="structuring" :disabled="processing" title="AI 提取待办信息" aria-label="AI 提取待办信息" @click="structureWithAI" />
            <el-button :icon="Delete" text circle :disabled="processing" title="忽略此项" aria-label="忽略此项" @click="dismissItem(selectedItem)" />
          </div>
        </header>
        <div class="clarify-body">
          <template v-if="selectedItem.status === 'pending'">
            <HolidayImportReview v-if="holidayCandidate" :content="selectedItem.contentText" :disabled="processing" @change="holidayDraft = $event" @busy="holidayBusy = $event" />
            <el-button v-if="holidayCandidate" type="primary" :disabled="!holidayDraft || holidayBusy || processing" @click="confirmHolidays">确认节假日并更新日历</el-button>
            <el-form label-position="top" :disabled="processing" @submit.prevent>
              <el-form-item label="任务标题"><el-input v-model="clarifyTitle" type="textarea" :autosize="{ minRows: 1, maxRows: 3 }" aria-label="任务标题" /></el-form-item>
              <el-form-item label="处理方式">
                <el-radio-group v-model="clarifyAction" class="action-options" aria-label="处理方式">
                  <el-radio-button value="action">行动</el-radio-button><el-radio-button value="delegate">委派</el-radio-button><el-radio-button value="wait">等待</el-radio-button><el-radio-button value="someday">将来也许</el-radio-button>
                </el-radio-group>
              </el-form-item>
              <el-form-item label="关联案件">
                <el-select v-model="clarifyCaseId" filterable clearable placeholder="未关联案件" aria-label="关联案件" @change="onClarifyCaseChange">
                  <el-option v-for="item in casesList" :key="item.id" :value="item.id" :label="item.caseNo ? item.caseName + ' · ' + item.caseNo : item.caseName" />
                </el-select>
                <el-alert v-if="caseError" :title="`案件列表读取失败：${caseError}`" type="error" :closable="false"><el-button text @click="loadCases">重试</el-button></el-alert>
              </el-form-item>
              <div class="form-grid ui-grid">
                <el-form-item label="开始安排">
                  <el-select v-model="clarifyDoWhen" aria-label="开始安排" :disabled="clarifyAction === 'someday'">
                    <el-option label="今天" value="today" /><el-option label="随时" value="anytime" /><el-option label="将来也许" value="someday" />
                  </el-select>
                </el-form-item>
                <el-form-item label="截止日期"><el-date-picker v-model="clarifyDeadline" type="date" value-format="YYYY-MM-DD" placeholder="未设置" aria-label="截止日期" /></el-form-item>
              </div>
              <div v-if="['delegate', 'wait'].includes(clarifyAction)" class="form-grid">
                <el-form-item :label="clarifyAction === 'delegate' ? '委派给' : '等待对象'"><el-input v-model="clarifyWaitingFor" placeholder="未指定" aria-label="等待对象" /></el-form-item>
                <el-form-item label="跟进日期"><el-date-picker v-model="clarifyFollowUp" type="date" value-format="YYYY-MM-DD" placeholder="未设置" aria-label="跟进日期" /></el-form-item>
              </div>
              <div class="form-grid ui-grid">
                <el-form-item label="执行场景"><el-select v-model="clarifyContext" aria-label="执行场景"><el-option label="办公室" value="office" /><el-option label="电话" value="phone" /><el-option label="法庭" value="court" /><el-option label="电脑" value="computer" /><el-option label="外出" value="outside" /></el-select></el-form-item>
                <el-form-item label="预估时长（分钟）"><el-input-number v-model="clarifyEstMinutes" :min="0" :max="1440" :step="15" controls-position="right" placeholder="未填写" aria-label="预估时长" /></el-form-item>
              </div>
              <el-form-item label="备注"><el-input v-model="clarifyNotes" type="textarea" :autosize="{ minRows: 4, maxRows: 10 }" aria-label="备注" /></el-form-item>
            </el-form>
          </template>
          <template v-else><div v-if="receipt?.action === 'holidays_updated'" class="holiday-receipt"><strong>日历写入回执</strong><p>{{ receipt.year }} 年 · 放假 {{ receipt.holidaysCount }} 天 · 补班 {{ receipt.workdaysCount }} 天</p><p v-if="receipt.changedDates === 0">导入日期与已有日历一致，无新增变动。</p><p v-else-if="receipt.changedDates != null">实际更新 {{ receipt.changedDates }} 个日期。</p><p v-if="receipt.holidays?.length">放假：{{ receipt.holidays.join('、') }}</p><p v-if="receipt.workdays?.length">补班：{{ receipt.workdays.join('、') }}</p><el-button @click="router.push({ path: '/calendar', query: { view: 'year', date: `${receipt.year}-01-01` } })">查看日历</el-button></div><h2>{{ selectedItem.title }}</h2><p class="original-content">{{ selectedItem.contentText || selectedItem.sourcePath || selectedItem.filePath }}</p></template>
        </div>
        <footer v-if="selectedItem.status === 'pending'" class="clarify-footer ui-row ui-row--between">
          <span>{{ sourceLabel(selectedItem.sourceType) }}</span>
          <el-button v-if="selectedItem.sourcePath" :disabled="processing || !clarifyCaseId" @click="fileCurrentItem('file_to_case')">归卷至案件</el-button>
          <el-button :disabled="processing" @click="fileCurrentItem('save_knowledge')">沉淀知识库</el-button>
          <el-button type="primary" :loading="processing" :disabled="!clarifyTitle.trim()" @click="processCurrentItem()">{{ clarifyAction === 'delegate' ? '建立委派任务' : clarifyAction === 'wait' ? '转为等待任务' : clarifyAction === 'someday' ? '归入将来也许' : '转为任务' }}</el-button>
        </footer>
      </section>
      <section v-else class="clarify-empty"><EmptyState type="custom" compact hide-action title="暂无待整理的内容" /></section>
    </div>
  </div>
</template>

<style scoped>
.inbox-page { display: flex; flex-direction: column; height: 100%; min-height: 0; max-width: 1440px; margin: 0 auto; padding: 28px 32px 0; color: var(--c-text); }
.inbox-header { display: flex; justify-content: space-between; align-items: center; gap: 16px; margin-bottom: 20px; }
.inbox-header h1 { font-size: 24px; line-height: 1.4; margin: 0 0 4px; }
.inbox-header span { color: var(--c-text-secondary); font-size: 12px; }
.quick-capture { display: flex; align-items: center; gap: 10px; padding: 6px 8px 6px 14px; background: var(--c-bg-card); border: 1px solid var(--c-border-strong); border-radius: 8px; margin-bottom: 24px; }
.quick-capture:focus-within { border-color: var(--c-primary); box-shadow: 0 0 0 2px var(--c-primary-light); }
.quick-capture > .el-icon { color: var(--c-text-secondary); }
.quick-capture input { min-width: 0; flex: 1; border: 0; background: transparent; color: var(--c-text); outline: 0; font: inherit; padding: 4px 0; }
.quick-capture input::placeholder { color: var(--c-text-secondary); }
.quick-capture .el-button + .el-button { margin-left: 0; }
.inbox-layout { display: grid; grid-template-columns: minmax(250px, .85fr) minmax(340px, 1.15fr); grid-template-rows: minmax(0, 1fr); flex: 1; min-height: 0; border-top: 1px solid var(--c-border); }
.inbox-list { min-width: 0; display: flex; flex-direction: column; border-right: 1px solid var(--c-border); }
.list-controls { padding: 20px 20px 0 0; }
.source-filters { display: flex; gap: 14px; overflow-x: auto; margin-top: 16px; }
.source-filters button { display: inline-flex; align-items: center; gap: 5px; flex-shrink: 0; padding: 6px 0 10px; border: 0; border-bottom: 2px solid transparent; background: transparent; color: var(--c-text-secondary); font: inherit; font-size: 12px; cursor: pointer; }
.source-filters button.active { color: var(--c-primary); border-bottom-color: var(--c-primary); }
.source-filters button span { font-variant-numeric: tabular-nums; }
.item-list { overflow-y: auto; flex: 1; min-height: 0; padding-right: 20px; }
.inbox-item { display: flex; flex-direction: column; gap: 7px; width: 100%; border: 0; border-bottom: 1px solid var(--c-border-light); background: transparent; padding: 16px 12px; color: var(--c-text); font: inherit; text-align: left; cursor: pointer; transition: background var(--motion-fast); }
.inbox-item:hover { background: var(--c-bg-hover); }
.inbox-item.selected { background: var(--c-primary-light); box-shadow: inset 3px 0 var(--c-primary); }
.item-meta { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 8px; font-size: 11px; color: var(--c-text-secondary); }
.inbox-item strong { font-size: 14px; font-weight: 600; overflow-wrap: anywhere; }
.item-snippet { display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; color: var(--c-text-secondary); font-size: 12px; line-height: 1.7; overflow-wrap: anywhere; }
.item-case { display: flex; align-items: center; gap: 5px; color: var(--c-primary); font-size: 11px; overflow-wrap: anywhere; }
.clarify-panel { display: flex; flex-direction: column; min-width: 0; min-height: 0; padding-left: 24px; }
.clarify-header { display: flex; align-items: center; justify-content: space-between; min-height: 58px; flex-shrink: 0; gap: 8px; font-size: 13px; font-weight: 600; }
.clarify-body { overflow-y: auto; min-height: 0; flex: 1; padding-right: 4px; }
.form-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 16px; }
.clarify-body :deep(.el-date-editor), .clarify-body :deep(.el-input-number), .clarify-body :deep(.el-select) { width: 100%; }
.action-options { display: flex; flex-wrap: wrap; }
.clarify-footer { gap: 12px; padding: 16px 0; flex-shrink: 0; border-top: 1px solid var(--c-border); background: var(--c-bg-page); }
.clarify-footer > span { color: var(--c-text-secondary); font-size: 12px; }
.clarify-empty { display: flex; flex-direction: column; justify-content: center; align-items: center; gap: 12px; color: var(--c-text-secondary); font-size: 13px; }
.original-content { white-space: pre-wrap; overflow-wrap: anywhere; line-height: 1.8; }
.clarify-body h2 { font-size: 18px; margin: 16px 0; }
.empty-state { font-size: 13px; padding: 32px 0; }
@container (max-width: 700px) {
  .inbox-page { height: auto; padding: 20px 16px 0; }
  .inbox-layout { grid-template-columns: minmax(0, 1fr); grid-template-rows: auto; }
  .inbox-list { border: 0; }
  .list-controls, .item-list { padding-right: 0; }
  .item-list { max-height: 280px; flex: none; }
  .clarify-panel { padding-left: 0; border-top: 1px solid var(--c-border); }
  .clarify-empty { min-height: 160px; }
  .clarify-footer { position: sticky; bottom: 0; }
}

</style>
