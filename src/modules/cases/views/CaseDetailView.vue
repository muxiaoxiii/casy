<script setup>
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { casyContext } from '../../../core/plugin/context'
import { todayLocalISO } from '../../../shared/utils/date'
import CaseFilesPanel from '../components/CaseFilesPanel.vue'
import CasePersonsPanel from '../../persons/components/CasePersonsPanel.vue'
import WhiteboardEntry from '../../whiteboard/components/WhiteboardEntry.vue'
import { ElMessage } from 'element-plus'
import {
  ArrowLeft, Edit, Calendar, Finished, Document,
  Folder, Collection, Clock, Warning, Check,
  Plus, CaretRight, Connection, Timer, Lock, CircleCheck
} from '../../../shared/icons'
import {
  CIVIL_STATUS_LABELS,
  INVALIDATION_STATUS_LABELS,
  ADMIN_STATUS_LABELS,
} from '../../../types'
import { joinAttorneys } from '../../../core/caseNormalize'
import EmptyState from '../../../shared/components/EmptyState.vue'
import AddRelationDialog from '../components/AddRelationDialog.vue'
import CaseWizard from '../components/CaseWizard.vue'
import CaseAttributes from '../components/CaseAttributes.vue'
import ProcedureBoard from '../components/ProcedureBoard.vue'
import CaseSourceRecords from '../components/CaseSourceRecords.vue'

const route = useRoute()
const router = useRouter()

// ============================================================
// 状态
// ============================================================
const caseData = ref(null)
const loading = ref(false)
const loadError = ref('')
const tasks = ref([])
const hearings = ref([])
const timeline = ref([])
const knowledge = ref([])
const files = ref([])
const relatedCases = ref([])
const validTabs = ['overview','hearings','tasks','tracks','files','persons','whiteboard','timeline','fields']
const activeTab = ref(validTabs.includes(route.query.tab) ? route.query.tab : 'overview') // overview | hearings | tasks | tracks | files | timeline | fields

// 编辑状态
const editingGoal = ref(false)
const goalInput = ref('')

// 庭审弹窗与表单
const showHearingDialog = ref(false)
const hearingForm = ref({
  id: '',
  hearingName: '开庭/口审',
  hearingDate: '',
  court: '',
  venue: '',
  judges: '',
  caseLevel: '',
  contactInfo: '',
  actualStatus: '未开',
  lifecycleStatus: 'scheduled',
  changeReason: '',
})

// 关联案件弹窗
const showAddRelationDialog = ref(false)
const showRelatedWizard = ref(false)
const relatedInitial = computed(() => {
  const c = caseData.value || {}
  return {
    clientName:c.clientName || '', opponentName:c.opponentName || '', ourRole:c.ourRole || '', opponentRole:c.opponentRole || '',
    patentName:c.patentName || '', patentAppNo:c.patentAppNo || '', attorneys:c.attorneys || [],
    track:c.track || 'civil_tort', caseRoute:c.caseRoute || '民事诉讼',
    relatedCases:[{caseId:String(caseId.value),relationType:'cross_reference',label:c.caseName || ''}],
  }
})
async function createRelated(data) {
  const result = await casyContext.cases.create(data)
  if (result.ok) {
    ElMessage.success('关联案件已创建')
    await loadRelations()
    router.push({name:'case-detail',params:{id:result.data.id}})
  }
  return result
}

// ============================================================
// 计算属性
// ============================================================
const caseId = computed(() => route.params.id)

const caseTypeLabel = computed(() => {
  const types = {
    computational: '计算型',
    exploratory: '探索型',
    growth: '成长型',
  }
  return types[caseData.value?.caseType] || '探索型'
})

const trackBadges = computed(() => {
  if (!caseData.value) return []
  const badges = []
  const routeStr = caseData.value.caseRoute || ''

  if ((routeStr.includes('专利无效') || caseData.value.invalidationStatus) && caseData.value.invalidationStatus) {
    badges.push({
      track: '专利无效轨',
      status: caseData.value.invalidationStatus,
      label: INVALIDATION_STATUS_LABELS[caseData.value.invalidationStatus] || caseData.value.invalidationStatus,
      tagClass: 'tag purple',
    })
  }

  if ((routeStr.includes('民事诉讼') || caseData.value.civilStatus) && caseData.value.civilStatus) {
    badges.push({
      track: '民事诉讼轨',
      status: caseData.value.civilStatus,
      label: CIVIL_STATUS_LABELS[caseData.value.civilStatus] || caseData.value.civilStatus,
      tagClass: 'tag blue',
    })
  }

  if ((routeStr.includes('行政诉讼') || caseData.value.adminStatus) && caseData.value.adminStatus) {
    badges.push({
      track: '行政诉讼轨',
      status: caseData.value.adminStatus,
      label: ADMIN_STATUS_LABELS[caseData.value.adminStatus] || caseData.value.adminStatus,
      tagClass: 'tag amber',
    })
  }

  return badges
})

const smartTags = computed(() => {
  if (!caseData.value) return []
  const tags = []
  const allTracks = new Set()
  if (caseData.value.track) allTracks.add(caseData.value.track)
  if (caseData.value.rawTrack) allTracks.add(caseData.value.rawTrack)
  
  let hasCivil = false
  let hasCriminal = false
  
  const checkTrack = (t) => {
    if (!t) return
    const tl = t.toLowerCase()
    if (tl.includes('civil') || tl.includes('民事')) hasCivil = true
    if (tl.includes('criminal') || tl.includes('刑事')) hasCriminal = true
  }
  
  checkTrack(caseData.value.track)
  checkTrack(caseData.value.rawTrack)
  
  relatedCases.value.forEach(rc => {
    if (rc.track) {
      allTracks.add(rc.track)
      checkTrack(rc.track)
    }
  })
  
  if (allTracks.size > 1 || relatedCases.value.length > 0) {
    tags.push({ label: '多轨并行', class: 'tag gradient-purple' })
  }
  if (hasCivil && hasCriminal) {
    tags.push({ label: '民刑交织', class: 'tag gradient-red' })
  }
  
  return tags
})

const taskStats = computed(() => {
  const total = tasks.value.length
  const completed = tasks.value.filter(t => t.completed).length
  const pending = tasks.value.filter(t => !t.completed).length
  const overdue = tasks.value.filter(t => {
    if (t.completed) return false
    const due = t.dueDate || t.deadline
    // 时区（审查 P1-2）：原本按 UTC 取"今天"，东八区凌晨窗口内逾期计数偏一天。
    return due && due < todayLocalISO()
  }).length

  return { total, completed, pending, overdue }
})

const nextAction = computed(() => {
  return tasks.value.find(t => !t.completed && t.blocked === 0 && t.taskType === 'action') || null
})

// 顺序项目统计
const sequentialTasks = computed(() => {
  return tasks.value.filter(t => t.sequential).sort((a, b) => a.sequenceOrder - b.sequenceOrder)
})

const sequentialTotalCount = computed(() => sequentialTasks.value.length)
const sequentialCompletedCount = computed(() => sequentialTasks.value.filter(t => t.completed).length)

const sequentialCompletionRate = computed(() => {
  if (sequentialTotalCount.value === 0) return 0
  return Math.round((sequentialCompletedCount.value / sequentialTotalCount.value) * 100)
})

// 案件类型差异化指标
const typeMetrics = ref(null)

const metricsCaseType = computed(() => {
  const t = typeMetrics.value?.caseType || typeMetrics.value?.case_type || caseData.value?.caseType || 'generic'
  return ['computational', 'exploratory', 'growth'].includes(t) ? t : 'generic'
})

const metricsTypeLabel = computed(() => {
  const labels = { computational: '计算型', exploratory: '探索型', growth: '成长型', generic: '通用' }
  return labels[metricsCaseType.value]
})

function percentText(v) {
  if (v == null) return '—'
  const p = v <= 1 ? v * 100 : v
  return `${Math.round(p)}%`
}

// ============================================================
// 数据加载
// ============================================================
let detailLoad = 0
async function loadCaseData() {
  const request = ++detailLoad
  loading.value = true;loadError.value=''
  try {
  caseData.value = null
  tasks.value = []; hearings.value = []; timeline.value = []; knowledge.value = []; files.value = []; relatedCases.value = []; typeMetrics.value = null
  await loadCase()
  if (request !== detailLoad || !caseData.value) return
  await Promise.all([
    loadHearings(),
    loadTasks(),
    loadTimeline(),
    loadKnowledge(),
    loadFiles(),
    loadTypeMetrics(),
    loadRelations(),
  ])
  }catch(error){if(request===detailLoad)loadError.value=String(error)}
  finally {if (request === detailLoad) loading.value = false}
}

async function loadTypeMetrics() {
  const id = caseId.value; const request = detailLoad
  const result = await casyContext.cases.caseTypeMetrics(id)
  if (id !== caseId.value || request !== detailLoad) return
  if (result.ok && result.data) {
    typeMetrics.value = result.data
  } else {
    typeMetrics.value = null
  }
}

async function loadCase() {
  const id = caseId.value; const request = detailLoad
  const result = await casyContext.cases.get(id)
  if (id !== caseId.value || request !== detailLoad) return
  if (result.ok) {
    caseData.value = result.data
    goalInput.value = result.data.caseGoal || ''
  }else {caseData.value=null;loadError.value=result.error || '案件不存在或无法读取'}
}

async function loadHearings() {
  const id = caseId.value; const request = detailLoad
  const result = await casyContext.cases.listHearings(id)
  if (id !== caseId.value || request !== detailLoad) return
  if (result.ok) {
    hearings.value = result.data || []
  }
}

async function loadTasks() {
  const id = caseId.value; const request = detailLoad
  const result = await casyContext.tasks.list({ caseId: id })
  if (id !== caseId.value || request !== detailLoad) return
  if (result.ok) {
    tasks.value = result.data || []
  }
}

async function loadTimeline() {
  const id = caseId.value; const request = detailLoad
  const result = await casyContext.cases.timeline(id)
  if (id !== caseId.value || request !== detailLoad) return
  if (result.ok) {
    timeline.value = result.data || []
  }
}

async function loadRelations() {
  const id = caseId.value; const request = detailLoad
  const result = await casyContext.cases.relations(id)
  if (id !== caseId.value || request !== detailLoad) return
  if (result.ok) {
    relatedCases.value = result.data || []
  }
}

async function handleRelationAdded() {
  // 当添加关系并且可能合并数据后，重新加载关联关系以及可能受影响的列表（文件、任务等）
  loadRelations()
  loadFiles()
  loadTasks()
  loadHearings()
}

async function loadKnowledge() {
  const id = caseId.value; const request = detailLoad
  knowledge.value = []
  if (!caseData.value || caseData.value.id !== id) return
  const searchTerms = [
    caseData.value.caseName,
    caseData.value.caseType,
    caseData.value.clientName,
  ].filter(Boolean).join(' ')

  if (searchTerms) {
    const result = await casyContext.knowledge.search(searchTerms)
    if (id !== caseId.value || request !== detailLoad) return
    if (result.ok && result.data) {
      knowledge.value = result.data
    }
  }
}

async function loadFiles() {
  const id = caseId.value; const request = detailLoad
  const result = await casyContext.files.list(id)
  if (id !== caseId.value || request !== detailLoad) return
  if (result.ok) {
    files.value = result.data || []
  }
}

// ============================================================
// 操作
// ============================================================
function goBack() {
  router.push({ name: 'cases', query: { caseId: caseId.value } })
}

async function saveGoal() {
  if (!caseData.value) return
  const id = caseId.value; const goal = goalInput.value
  const result = await casyContext.cases.update(id, { caseGoal: goal })
  if (id !== caseId.value) return
  if (result.ok) {
    caseData.value.caseGoal = goal
    editingGoal.value = false
    ElMessage.success('案件目标已保存')
  }
}

// 庭审 CRUD 操作
function openAddHearing() {
  hearingForm.value = {
    id: '',
    hearingName: '第' + (hearings.value.length + 1) + '次开庭 / 口审',
    hearingDate: '',
    court: caseData.value?.court || '',
    venue: '',
    judges: caseData.value?.judgePanel || '',
    caseLevel: caseData.value?.caseLevel || '',
    contactInfo: caseData.value?.clerk || '',
    actualStatus: '未开',
  lifecycleStatus: 'scheduled',
  changeReason: '',
  }
  showHearingDialog.value = true
}

function openEditHearing(h) {
  hearingForm.value = {
    id: h.id,
    hearingName: h.hearingName || h.hearingRecord || '开庭/口审',
    hearingDate: h.hearingDate,
    court: h.court || '',
    venue: h.venue || '',
    judges: h.judges || '',
    caseLevel: h.caseLevel || '',
    contactInfo: h.contactInfo || '',
    actualStatus: h.actualStatus || '未开',
    lifecycleStatus: h.lifecycleStatus || (h.actualStatus==='已开'?'held':'scheduled'),
    changeReason: '',
  }
  showHearingDialog.value = true
}

const hearingSaving = ref(false)
async function saveHearing() {
  if(hearingSaving.value)return
  const idCase=caseId.value
  const form={...hearingForm.value}
  if (!form.hearingDate) {
    ElMessage.warning('请选择开庭/口审时间')
    return
  }
  hearingSaving.value=true
  try {
  if (form.id) {
    const res = await casyContext.cases.updateHearing(form.id, {
      hearingName: form.hearingName,
      hearingDate: form.hearingDate,
      court: form.court,
      venue: form.venue,
      judges: form.judges,
      caseLevel: form.caseLevel,
      contactInfo: form.contactInfo,
      actualStatus: form.lifecycleStatus==='held'?'已开':'未开',
      lifecycleStatus: form.lifecycleStatus,
      changeReason: form.changeReason,
    })
    if(idCase!==caseId.value)return
    if (res.ok) {
      ElMessage.success('庭审排期已更新')
      showHearingDialog.value = false
      await loadHearings()
      await loadCase()
      await loadTimeline()
    } else {
      ElMessage.error(res.error || '更新失败')
    }
  } else {
    const res = await casyContext.cases.createHearing({
      caseId: idCase,
      hearingName: form.hearingName,
      hearingDate: form.hearingDate,
      court: form.court,
      venue: form.venue,
      judges: form.judges,
      caseLevel: form.caseLevel,
      contactInfo: form.contactInfo,
      actualStatus: form.lifecycleStatus==='held'?'已开':'未开',
      lifecycleStatus: form.lifecycleStatus,
      changeReason: form.changeReason,
    })
    if(idCase!==caseId.value)return
    if (res.ok) {
      ElMessage.success('已添加开庭/口审排期')
      showHearingDialog.value = false
      await loadHearings()
      await loadCase()
      await loadTimeline()
    } else {
      ElMessage.error(res.error || '创建失败')
    }
  }
  } finally {hearingSaving.value=false}
}

async function deleteHearing(h) {
  const res = await casyContext.cases.deleteHearing(h.id)
  if (res.ok) {
    ElMessage.success('庭审记录已删除')
    await loadHearings()
  } else {
    ElMessage.error(res.error || '删除失败')
  }
}

async function toggleHearingStatus(h) {
  const nextStatus = h.actualStatus === '已开' ? '未开' : '已开'
  const res = await casyContext.cases.updateHearing(h.id, { actualStatus: nextStatus })
  if (res.ok) {
    h.actualStatus = nextStatus
    h.lifecycleStatus = nextStatus==='已开'?'held':'scheduled'
    ElMessage.success(`开庭状态已变更为: ${nextStatus}`)
  }
}


async function toggleTaskComplete(task) {
  const result = await casyContext.tasks.toggle(task.id)
  if (result.ok) {
    task.completed = task.completed ? 0 : 1
    ElMessage.success(task.completed ? '已完成' : '已恢复')
    if (task.completed && task.sequential) {
      await unlockNextTask(task)
    }
    await loadTasks()
  }
}

async function unlockNextTask(completedTask) {
  const nextTask = tasks.value.find(t =>
    t.caseId === caseId.value &&
    t.sequential &&
    t.blocked &&
    t.sequenceOrder > completedTask.sequenceOrder
  )

  if (nextTask) {
    // 后端失败时不得本地置 blocked=0 或提示成功——校验 res.ok 后再更新本地。
    const res = await casyContext.tasks.update({
      id: nextTask.id,
      blocked: 0,
    })
    if (!res.ok) return ElMessage.error(res.error || '解锁后续步骤失败')
    nextTask.blocked = 0
    ElMessage.success(`已解锁后续步骤：${nextTask.taskName}`)
  }
}

function openTaskEdit(task) {
  router.push({ name: 'tasks', query: { edit: task.id } })
}

function addNewTask() {
  router.push({ name: 'tasks', query: { capture: 'task', caseId: caseId.value } })
}

function formatDate(dateStr) {
  if (!dateStr) return ''
  const date = new Date(dateStr)
  return date.toLocaleDateString('zh-CN', { month: 'short', day: 'numeric' })
}

let unsubscribeUpdated = null
onMounted(() => {
  loadCaseData()
  unsubscribeUpdated = casyContext.on('case:updated', (payload) => {
    const p = payload || {}
    if (!p.id || p.id === caseId.value) {
      loadTimeline()
      loadCase()
      loadHearings()
    }
  })
})

watch(caseId, () => {
  showHearingDialog.value = false; showAddRelationDialog.value = false; showRelatedWizard.value = false; editingGoal.value = false
  loadCaseData()
})
watch(() => route.query.tab, tab => { activeTab.value = validTabs.includes(tab) ? tab : 'overview' })
watch(activeTab, tab => {if(route.query.tab!==tab)router.replace({query:{...route.query,tab}});if(tab==='timeline')void loadTimeline()})

onUnmounted(() => {
  ++detailLoad
  if (unsubscribeUpdated) unsubscribeUpdated()
})
</script>

<template>
  <div class="case-detail-container" v-loading="loading">
    <!-- 顶部返回导航 -->
    <div class="detail-header">
      <button class="btn-back" @click="goBack">
        <el-icon :size="14"><ArrowLeft /></el-icon>
        返回案件列表
      </button>
    </div>

    <el-alert v-if="loadError" :title="loadError" type="error" :closable="false" />
    <!-- ═══ 案件概要 Hero Card ═══ -->
    <div v-if="caseData" class="case-hero-card">
      <div class="hero-left">
        <h1 class="case-title">{{ caseData.caseName }}</h1>
        <div class="case-subtitle">
          <span v-if="caseData.caseNo" class="sub-item mono">{{ caseData.caseNo }}</span>
          <span v-if="caseData.patentNo" class="sub-item mono">{{ caseData.patentNo }}</span>
          <span v-if="caseData.clientName" class="sub-item">客户：{{ caseData.clientName }}</span>
          <span v-if="caseData.court" class="sub-item">受理机构：{{ caseData.court }}</span>
        </div>
        <div class="hero-tags">
          <span v-for="b in trackBadges" :key="b.track" :class="b.tagClass">
            {{ b.track }} · {{ b.label }}
          </span>
          <span v-for="t in smartTags" :key="t.label" :class="t.class">
            {{ t.label }}
          </span>
          <span class="tag solid-blue">{{ caseTypeLabel }}</span>
        </div>
      </div>

      <!-- 进度环 (5/8) -->
      <div class="hero-donut-wrap">
        <div class="donut-outer">
          <div class="donut-inner">
            <span class="donut-num">{{ taskStats.completed }}/{{ taskStats.total || 0 }}</span>
            <span class="donut-sub">推进率</span>
          </div>
        </div>
      </div>
    </div>

    <!-- ═══ 下一步行动 Hero Card（原则一：顺序项目唯一入口） ═══ -->
    <div v-if="nextAction" class="next-card">
      <div class="play" @click="openTaskEdit(nextAction)">
        <el-icon :size="14" color="#FFFFFF"><CaretRight /></el-icon>
      </div>
      <div class="info">
        <div class="n">{{ nextAction.taskName }}</div>
        <div class="s">
          当前唯一可推进 · 完成后自动解锁后续步骤
          <template v-if="nextAction.estimatedMinutes"> · 预计 {{ nextAction.estimatedMinutes }} 分钟</template>
        </div>
      </div>
      <button class="btn-sm primary" @click="openTaskEdit(nextAction)">开始</button>
      <button class="btn-sm" @click="toggleTaskComplete(nextAction)">完成</button>
    </div>

    <!-- ═══ 详情 Tab 切换栏 ═══ -->
    <div class="detail-tabs">
      <button type="button"
        class="dtab"
        :class="{ active: activeTab === 'overview' }"
        @click="activeTab = 'overview'"
      >
        项目总览
      </button>
      <button type="button"
        class="dtab"
        :class="{ active: activeTab === 'hearings' }"
        @click="activeTab = 'hearings'"
      >
        开庭口审 · {{ hearings.length }}
      </button>
      <button type="button"
        class="dtab"
        :class="{ active: activeTab === 'tasks' }"
        @click="activeTab = 'tasks'"
      >
        任务待办 · {{ tasks.length }}
      </button>
      <button type="button"
        class="dtab"
        :class="{ active: activeTab === 'tracks' }"
        @click="activeTab = 'tracks'"
      >
        程序事项 · 收转文与期限
      </button>
      <button type="button"
        class="dtab"
        :class="{ active: activeTab === 'files' }"
        @click="activeTab = 'files'"
      >
        案卷 · {{ files.length }}
      </button>
      <button type="button"
        class="dtab"
        :class="{ active: activeTab === 'persons' }"
        @click="activeTab = 'persons'"
      >
        实体对象
      </button>
      <button type="button"
        class="dtab"
        :class="{ active: activeTab === 'whiteboard' }"
        @click="activeTab = 'whiteboard'"
      >
        事实白板
      </button>
      <button type="button"
        class="dtab"
        :class="{ active: activeTab === 'timeline' }"
        @click="activeTab = 'timeline'"
      >
        动态轨迹 · {{ timeline.length }}
      </button>
      <button type="button"
        class="dtab"
        :class="{ active: activeTab === 'fields' }"
        @click="activeTab = 'fields'"
      >
        全量属性与字段
      </button>
    </div>

    <!-- ═══ Tab 1: 项目总览 ═══ -->
    <div v-if="activeTab === 'overview'" class="tab-pane">
      <div class="overview-grid">
        <!-- 左列：里程碑与目标 -->
        <div class="card">
          <div class="ch">
            <span class="t">里程碑与顺序项目</span>
            <span class="s">{{ sequentialCompletedCount }}/{{ sequentialTotalCount }} 步完成</span>
          </div>
          <div class="sep"></div>

          <!-- 案件目标 -->
          <div class="goal-box">
            <div class="goal-header">
              <span class="goal-label">案件目标</span>
              <button v-if="!editingGoal" class="btn-text" @click="editingGoal = true">编辑</button>
            </div>
            <div v-if="editingGoal" class="goal-edit">
              <input
                v-model="goalInput"
                class="goal-input"
                placeholder="概括本案核心目标..."
                @keyup.enter="saveGoal"
              />
              <div class="goal-ops">
                <button class="btn-sm" @click="editingGoal = false">取消</button>
                <button class="btn-sm primary" @click="saveGoal">保存</button>
              </div>
            </div>
            <div v-else class="goal-text">
              {{ caseData?.caseGoal || '点击编辑设置案件核心目标' }}
            </div>
          </div>

          <!-- 顺序步骤序列 -->
          <div v-if="sequentialTasks.length" class="seq-list">
            <div
              v-for="task in sequentialTasks"
              :key="task.id"
              class="seq-item"
              :class="{
                done: task.completed,
                locked: task.blocked,
                active: !task.completed && !task.blocked,
              }"
            >
              <div class="seq-check" @click="toggleTaskComplete(task)">
                <el-icon v-if="task.completed" :size="12"><Check /></el-icon>
                <el-icon v-else-if="task.blocked" :size="12"><Lock /></el-icon>
              </div>
              <div class="seq-content" @click="openTaskEdit(task)">
                <span class="seq-name">{{ task.taskName }}</span>
                <span v-if="task.blocked" class="seq-hint">等待前置步骤完成</span>
              </div>
              <span v-if="task.completed" class="tag green">已完成</span>
              <span v-else-if="task.blocked" class="tag text-3">已锁定</span>
              <span v-else class="tag solid-blue">当前推进</span>
            </div>
          </div>
          <EmptyState
            v-else
            type="tasks"
            compact
            title="暂无顺序项目步骤"
            description="可在任务管理中为本案添加阶段步骤"
          />
        </div>

        <!-- 右列：统计与指标 -->
        <div class="vstack">
          <!-- 任务统计 -->
          <div class="card">
            <div class="ch">
              <span class="t">任务统计</span>
              <span class="s">全案看板</span>
            </div>
            <div class="sep"></div>
            <div class="task-stats-row">
              <div class="stat-cell" @click="router.push({ name: 'tasks', query: { caseId } })">
                <span class="num">{{ taskStats.total }}</span>
                <span class="lbl">总计</span>
              </div>
              <div class="stat-cell" @click="router.push({ name: 'tasks', query: { caseId } })">
                <span class="num">{{ taskStats.pending }}</span>
                <span class="lbl">待办</span>
              </div>
              <div class="stat-cell" @click="router.push({ name: 'tasks', query: { caseId } })">
                <span class="num text-success">{{ taskStats.completed }}</span>
                <span class="lbl">已完成</span>
              </div>
              <div class="stat-cell" @click="router.push({ name: 'tasks', query: { caseId } })">
                <span class="num" :class="{ 'text-danger': taskStats.overdue > 0 }">{{ taskStats.overdue }}</span>
                <span class="lbl">逾期</span>
              </div>
            </div>
          </div>

          <!-- 差异化指标卡片 -->
          <div v-if="typeMetrics" class="card">
            <div class="ch">
              <span class="t">{{ metricsTypeLabel }}效能指标</span>
            </div>
            <div class="sep"></div>
            <div class="metrics-list">
              <template v-if="metricsCaseType === 'computational'">
                <div class="m-row">
                  <span class="m-k">按时完成率</span>
                  <span class="m-v">{{ percentText(typeMetrics.onTimeRate) }}</span>
                </div>
                <div class="m-row">
                  <span class="m-k">逾期项</span>
                  <span class="m-v text-danger">{{ typeMetrics.overdueCount ?? 0 }}</span>
                </div>
              </template>
              <template v-else-if="metricsCaseType === 'exploratory'">
                <div class="m-row">
                  <span class="m-k">阶段推进</span>
                  <span class="m-v">{{ typeMetrics.trackTransitions90d ?? 0 }} 次</span>
                </div>
                <div class="m-row">
                  <span class="m-k">顺序解锁</span>
                  <span class="m-v">{{ typeMetrics.blockedResolved ?? 0 }}/{{ typeMetrics.blockedTotal ?? 0 }}</span>
                </div>
              </template>
              <template v-else>
                <div class="m-row">
                  <span class="m-k">总完成率</span>
                  <span class="m-v">{{ percentText(typeMetrics.completionRate) }}</span>
                </div>
              </template>
            </div>
          </div>

          <!-- 关联资源快速跳转 -->
          <div class="card">
            <div class="ch">
              <span class="t">关联资源</span>
            </div>
            <div class="sep"></div>
            <div class="resource-links">
              <div class="res-item" @click="activeTab = 'files'">
                <el-icon :size="16" class="res-ico blue"><Folder /></el-icon>
                <span class="res-name">卷宗文件</span>
                <span class="res-count">{{ files.length }}</span>
              </div>
              <div class="res-item" @click="router.push({ name: 'knowledge' })">
                <el-icon :size="16" class="res-ico purple"><Collection /></el-icon>
                <span class="res-name">关联知识</span>
                <span class="res-count">{{ knowledge.length }}</span>
              </div>
            </div>
          </div>

          <!-- 关联案件快速跳转 -->
          <div class="card" v-if="relatedCases.length > 0 || true">
            <div class="ch" style="display: flex; justify-content: space-between; align-items: center;">
              <span class="t">关联案件 ({{ relatedCases.length }})</span>
              <el-button link type="primary" size="small" :icon="Plus" @click="showRelatedWizard = true">新建关联案</el-button>
              <el-button link type="primary" size="small" @click="showAddRelationDialog = true">
                <el-icon><Plus /></el-icon> 添加关联
              </el-button>
            </div>
            <div class="sep"></div>
            <div class="related-cases-list">
              <div 
                v-for="rc in relatedCases" 
                :key="rc.relationId"
                class="related-case-item"
                @click="router.push({name:'case-detail',params:{id:rc.caseId}})"
                style="cursor: pointer; padding: 12px; margin-bottom: 8px; background: var(--bg-hover); border-radius: 6px; display: flex; flex-direction: column; gap: 4px;"
              >
                <span style="font-size: 13px; font-weight: 500; color: var(--text-1);">{{ rc.caseName }}</span>
                <div style="display: flex; gap: 8px; align-items: center;">
                  <span class="tag sm" style="background: rgba(var(--primary-rgb), 0.1); color: var(--primary);">{{ rc.track }}</span>
                  <span class="tag sm" style="background: var(--bg-card); color: var(--text-3); border: 1px solid var(--border-light);">{{ rc.relationType }}</span>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- ═══ Tab 2: 程序期限与统筹 ═══ -->
    <div v-if="activeTab === 'tracks'" class="tab-pane">
      <ProcedureBoard :case-id="String(caseId)" @open-case="router.push({name:'case-detail',params:{id:$event},query:{tab:'tracks'}})" />
    </div>

    <!-- ═══ Tab 3: 案卷管理 ═══ -->
    <div v-if="activeTab === 'files'" class="tab-pane">
      <CaseFilesPanel :case-id="caseId" :case-no="caseData?.caseNo" />
    </div>

    <!-- ═══ Tab: 实体对象（W6 · Capacities 式单一事实源） ═══ -->
    <div v-if="activeTab === 'persons'" class="tab-pane">
      <CasePersonsPanel :case-id="caseId" />
    </div>

    <!-- ═══ Tab: 事实白板（W7 · LiquidText 式事实节点网络） ═══ -->
    <div v-if="activeTab === 'whiteboard'" class="tab-pane">
      <WhiteboardEntry :case-id="caseId" />
    </div>

    <!-- ═══ Tab 4: 动态轨迹 ═══ -->
    <div v-if="activeTab === 'timeline'" class="tab-pane">
      <div class="card">
        <div class="ch">
          <span class="t">动态轨迹</span>
          <span class="s">案件生命周期全记录</span>
        </div>
        <div class="sep"></div>
        <div v-if="timeline.length" class="timeline-stream">
          <div v-for="(item, idx) in timeline" :key="idx" class="t-row">
            <div class="t-date">{{ item.eventDate || item.date || item.createdAt?.slice(5, 10) }}</div>
            <div class="t-track"><div class="t-dot"></div></div>
            <div class="t-body">
              <div class="t-head">
                <span class="t-title">{{ item.title || item.action || '动态' }}</span>
                <span v-if="item.author" class="t-author">{{ item.author }}</span>
              </div>
              <div v-if="item.detail || item.description" class="t-desc">{{ item.detail || item.description }}</div>
              <el-button v-if="item.sourceTable==='procedure_events' || item.sourceTable==='hearings'" text @click="activeTab=item.sourceTable==='hearings'?'hearings':'tracks'">查看 / 维护原记录</el-button>
            </div>
          </div>
        </div>
        <div v-else class="empty-hint">暂无动态记录</div>
      </div>
    </div>
    <!-- ═══ Tab: 开庭与口审排期 ═══ -->
    <div v-if="activeTab === 'hearings'" class="tab-pane">
      <div class="card">
        <div class="ch" style="display: flex; justify-content: space-between; align-items: center;">
          <div>
            <span class="t">开庭与口审排期记录</span>
            <span class="s">历次出庭/口审信息沉淀 (共 {{ hearings.length }} 次)</span>
          </div>
          <button class="btn-sm primary" @click="openAddHearing">
            <el-icon><Plus /></el-icon>
            <span>添加开庭/口审</span>
          </button>
        </div>
        <div class="sep"></div>

        <div v-if="hearings.length" class="hearings-table-wrap">
          <table class="data-table">
            <thead>
              <tr>
                <th style="width: 18%">庭审/口审轮次</th>
                <th style="width: 16%">开庭时间</th>
                <th style="width: 16%">审理法院/机构</th>
                <th style="width: 14%">合议庭/法官</th>
                <th style="width: 14%">法庭/地点</th>
                <th style="width: 10%">状态</th>
                <th style="width: 12%">操作</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="h in hearings" :key="h.id">
                <td>
                  <strong>{{ h.hearingName || h.hearingRecord || '开庭/口审' }}</strong>
                  <div v-if="h.caseLevel" class="text-xs text-text-muted">{{ h.caseLevel }}</div>
                  <small v-if="h.changeReason">变更依据：{{ h.changeReason }}</small>
                </td>
                <td>
                  <div class="font-mono text-primary font-semibold">{{ h.hearingDate }}</div>
                </td>
                <td>{{ h.court || caseData?.court || '—' }}</td>
                <td>{{ h.judges || caseData?.judgePanel || '—' }}</td>
                <td>{{ h.venue || '—' }}</td>
                <td>
                  <el-tag
                    :type="h.actualStatus === '已开' ? 'success' : 'warning'"
                    size="small"
                    style="cursor: pointer"
                    @click="openEditHearing(h)"
                  >
                    {{ ({scheduled:'已排期',held:'已开庭',postponed:'延期待定',cancelled:'已取消'})[h.lifecycleStatus] || h.actualStatus || '待开庭' }}
                  </el-tag>
                </td>
                <td>
                  <div class="row-ops">
                    <button class="btn-text" @click="openEditHearing(h)">编辑</button>
                    <button class="btn-text" @click="activeTab='tracks'">收转文与修订记录</button>
                  </div>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
        <EmptyState
          v-else
          type="calendar"
          compact
          title="暂无开庭排期"
          description="点击右上角「添加开庭/口审」录入庭审信息"
        />
      </div>
    </div>

    <!-- ═══ Tab: 任务与待办事项 ═══ -->
    <div v-if="activeTab === 'tasks'" class="tab-pane">
      <div class="card">
        <div class="ch" style="display: flex; justify-content: space-between; align-items: center;">
          <div>
            <span class="t">本案关联任务与待办</span>
            <span class="s">{{ taskStats.completed }}/{{ taskStats.total }} 项已完成</span>
          </div>
          <button class="btn-sm primary" @click="addNewTask">
            <el-icon><Plus /></el-icon>
            <span>添加任务</span>
          </button>
        </div>
        <div class="sep"></div>

        <div v-if="tasks.length" class="tasks-table-wrap">
          <table class="data-table">
            <thead>
              <tr>
                <th style="width: 8%">状态</th>
                <th style="width: 36%">任务名称与详情</th>
                <th style="width: 14%">优先级</th>
                <th style="width: 16%">截止日期</th>
                <th style="width: 14%">责任人</th>
                <th style="width: 12%">操作</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="t in tasks" :key="t.id" :class="{ 'row-done': t.completed }">
                <td>
                  <el-checkbox
                    :model-value="Boolean(t.completed)"
                    @change="toggleTaskComplete(t)"
                  />
                </td>
                <td>
                  <div class="task-title" :class="{ done: t.completed }">{{ t.taskName }}</div>
                  <div v-if="t.description" class="task-desc">{{ t.description }}</div>
                </td>
                <td>
                  <el-tag
                    v-if="t.priority === 'urgent_important'"
                    type="danger"
                    size="small"
                  >
                    紧急且重要
                  </el-tag>
                  <el-tag
                    v-else-if="t.priority === 'important'"
                    type="warning"
                    size="small"
                  >
                    重要
                  </el-tag>
                  <el-tag
                    v-else-if="t.priority === 'urgent'"
                    type="danger"
                    size="small"
                  >
                    紧急
                  </el-tag>
                  <el-tag v-else size="small" type="info">普通</el-tag>
                </td>
                <td>
                  <span class="font-mono text-xs">{{ t.deadline || t.dueDate || '—' }}</span>
                </td>
                <td>{{ t.assignee || '—' }}</td>
                <td>
                  <div class="row-ops">
                    <button class="btn-text" @click="openTaskEdit(t)">编辑</button>
                  </div>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
        <EmptyState
          v-else
          type="tasks"
          compact
          title="本案暂无关联任务"
          description="点击右上角「添加任务」创建办案待办"
        />
      </div>
    </div>

    <div v-if="activeTab === 'fields'" class="tab-pane">
      <CaseAttributes v-if="caseData" :case-data="caseData" @saved="loadCase" />
      <CaseSourceRecords :case-id="String(caseId)" />
    </div>

    <!-- 庭审排期新增/编辑弹窗 -->
    <el-dialog
      v-model="showHearingDialog"
      :title="hearingForm.id ? '编辑开庭/口审排期' : '新增开庭/口审排期'"
      width="540px"
      destroy-on-close
    >
      <el-form label-width="110px" size="small">
        <el-form-item label="庭审轮次/名称" required>
          <el-input v-model="hearingForm.hearingName" placeholder="例如: 一审第一次开庭 / 口头审理" />
        </el-form-item>
        <el-form-item label="开庭/口审时间" required>
          <el-date-picker
            v-model="hearingForm.hearingDate"
            type="datetime"
            placeholder="选择日期"
            value-format="YYYY-MM-DD HH:mm:ss"
            style="width: 100%"
          />
        </el-form-item>
        <el-form-item label="审理法院/机构">
          <el-input v-model="hearingForm.court" placeholder="审理法院名称" />
        </el-form-item>
        <el-form-item label="法庭/地点">
          <el-input v-model="hearingForm.venue" placeholder="例如: 第五法庭 / 线上腾讯会议" />
        </el-form-item>
        <el-form-item label="承办法官/合议庭">
          <el-input v-model="hearingForm.judges" placeholder="法官姓名" />
        </el-form-item>
        <el-form-item label="案件阶段/审级">
          <el-input v-model="hearingForm.caseLevel" placeholder="例如: 一审 / 二审 / 阶段" />
        </el-form-item>
        <el-form-item label="书记员/联系方式">
          <el-input v-model="hearingForm.contactInfo" placeholder="联系电话或书记员姓名" />
        </el-form-item>
<el-form-item label="改期 / 延期 / 取消的通知或决定依据"><el-input v-model="hearingForm.changeReason" type="textarea" placeholder="申请延期不等于获准；仅有申请时保留原排期。多次开庭分别新建记录。" /></el-form-item>
        <el-form-item label="出庭状态">
          <el-select v-model="hearingForm.lifecycleStatus"><el-option value="scheduled" label="已排期"/><el-option value="held" label="已开庭"/><el-option v-if="hearingForm.id" value="postponed" label="延期待定（已获准，等待新排期）"/><el-option v-if="hearingForm.id" value="cancelled" label="已取消（有正式依据）"/></el-select>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button size="small" @click="showHearingDialog = false">取消</el-button>
        <el-button type="primary" size="small" :loading="hearingSaving" @click="saveHearing">保存</el-button>
      </template>
    </el-dialog>
    <AddRelationDialog
      v-model="showAddRelationDialog"
      :currentCaseId="caseId"
      @relation-added="handleRelationAdded"
    />
    <CaseWizard v-model="showRelatedWizard" :initial-case="relatedInitial" :submit="createRelated" />
  </div>
</template>

<style scoped>
.dtab{border:0;background:transparent;font:inherit}
/* ============================================================
   案件详情 · Slate 设计规范
   ============================================================ */
.case-detail-container {
  max-width: 1320px;
  margin: 0 auto;
  padding: 16px 20px 32px;
}

.detail-header {
  margin-bottom: 12px;
}

.btn-back {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  background: transparent;
  border: none;
  font-size: 13px;
  color: var(--c-text-secondary);
  cursor: pointer;
  padding: 4px 0;
  transition: color var(--motion-fast) var(--ease-out);
}

.btn-back:hover {
  color: var(--c-primary);
}

/* ── 案件概要 Hero ─────────────────────────────────────────── */
.case-hero-card {
  display: flex;
  align-items: flex-start;
  gap: 16px;
  background: var(--c-surface);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  padding: 16px 18px;
  margin-bottom: 14px;
  box-shadow: var(--shadow-sm);
}

.hero-left {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
}

.case-title {
  font-size: 18px;
  font-weight: 700;
  color: var(--c-text-heading);
  letter-spacing: -0.2px;
  margin: 0;
}

.case-subtitle {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
  font-size: 12px;
  color: var(--c-text-secondary);
}

.sub-item.mono {
  font-family: var(--font-mono);
}

.hero-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 4px;
}

.hero-donut-wrap {
  flex-shrink: 0;
}

.donut-outer {
  width: 54px;
  height: 54px;
  border-radius: 50%;
  background: conic-gradient(var(--c-primary) 65%, var(--gray-200) 0);
  display: flex;
  align-items: center;
  justify-content: center;
}

.donut-inner {
  width: 42px;
  height: 42px;
  border-radius: 50%;
  background: var(--c-surface);
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
}

.donut-num {
  font-size: 11px;
  font-weight: 700;
  color: var(--c-primary);
  line-height: 1;
}

.donut-sub {
  font-size: 8px;
  color: var(--c-text-secondary);
  transform: scale(0.9);
}

/* ── 下一步行动 Hero ───────────────────────────────────────── */
.next-card {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 14px;
  border-radius: var(--c-radius-lg);
  background: var(--c-primary-light);
  border: 1px solid var(--c-primary-lighter);
  margin-bottom: 14px;
}

.next-card .play {
  width: 32px;
  height: 32px;
  border-radius: 50%;
  background: var(--c-primary);
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  cursor: pointer;
}

.next-card .info {
  flex: 1;
  min-width: 0;
}

.next-card .info .n {
  font-size: 13px;
  font-weight: 600;
  color: var(--c-text);
}

.next-card .info .s {
  font-size: 11.5px;
  color: var(--c-primary);
  margin-top: 2px;
}

/* ── Detail Tabs ───────────────────────────────────────────── */
.detail-tabs {
  display: flex;
  gap: 6px;
  border-bottom: 1px solid var(--c-border);
  margin-bottom: 16px;
  padding-bottom: 0;
}

.dtab {
  padding: 8px 14px;
  font-size: 13px;
  font-weight: 500;
  color: var(--c-text-regular);
  cursor: pointer;
  border-bottom: 2px solid transparent;
  margin-bottom: -1px;
  transition: all var(--motion-fast) var(--ease-out);
}

.dtab:hover {
  color: var(--c-text);
}

.dtab.active {
  color: var(--c-primary);
  border-bottom-color: var(--c-primary);
  font-weight: 600;
}

/* ── Tab Pane ──────────────────────────────────────────────── */
.tab-pane {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.overview-grid {
  display: grid;
  grid-template-columns: 1.5fr 1fr;
  gap: 14px;
  align-items: start;
}

.vstack {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

/* ── 卡片 ──────────────────────────────────────────────────── */
.card {
  background: var(--c-surface);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  padding: 14px;
  box-shadow: var(--shadow-sm);
}

.card .ch {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.card .ch .t {
  font-size: 13px;
  font-weight: 600;
  color: var(--c-text-heading);
}

.card .ch .s {
  font-size: 11px;
  color: var(--c-text-secondary);
}

.card .sep {
  height: 1px;
  background: var(--c-border-light);
  margin: 10px 0;
}

/* ── 目标框 ────────────────────────────────────────────────── */
.goal-box {
  background: var(--gray-50);
  border: 1px solid var(--c-border-light);
  border-radius: var(--c-radius);
  padding: 10px 12px;
  margin-bottom: 12px;
}

.goal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 4px;
}

.goal-label {
  font-size: 11px;
  font-weight: 600;
  color: var(--c-text-secondary);
}

.btn-text {
  background: transparent;
  border: none;
  font-size: 11px;
  color: var(--c-primary);
  cursor: pointer;
}

.goal-text {
  font-size: 12.5px;
  color: var(--c-text);
  line-height: 1.5;
}

.goal-input {
  width: 100%;
  padding: 6px 8px;
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius);
  font-size: 12px;
  outline: none;
  background: #fff;
}

.goal-ops {
  display: flex;
  justify-content: flex-end;
  gap: 6px;
  margin-top: 6px;
}

/* ── 顺序步骤列表 ──────────────────────────────────────────── */
.seq-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.seq-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border: 1px solid var(--c-border-light);
  border-radius: var(--c-radius);
  background: var(--c-surface);
  transition: all var(--motion-fast) var(--ease-out);
}

.seq-item.active {
  border-color: var(--c-primary-lighter);
  background: var(--c-primary-light);
}

.seq-item.locked {
  opacity: 0.65;
}

.seq-check {
  width: 18px;
  height: 18px;
  border-radius: 50%;
  border: 1.5px solid var(--gray-300);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  flex-shrink: 0;
}

.seq-item.done .seq-check {
  background: var(--c-success);
  border-color: var(--c-success);
  color: #fff;
}

.seq-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 2px;
  cursor: pointer;
  min-width: 0;
}

.seq-name {
  font-size: 12.5px;
  font-weight: 500;
  color: var(--c-text);
}

.seq-hint {
  font-size: 10.5px;
  color: var(--c-text-secondary);
}

/* ── 任务统计格子 ──────────────────────────────────────────── */
.task-stats-row {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 8px;
}

.stat-cell {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 8px;
  background: var(--gray-50);
  border-radius: var(--c-radius);
  cursor: pointer;
  transition: background var(--motion-fast) var(--ease-out);
}

.stat-cell:hover {
  background: var(--c-bg-hover);
}

.stat-cell .num {
  font-size: 16px;
  font-weight: 700;
  color: var(--c-text-heading);
}

.stat-cell .lbl {
  font-size: 10.5px;
  color: var(--c-text-secondary);
  margin-top: 2px;
}

/* ── 指标行 ────────────────────────────────────────────────── */
.metrics-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.m-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 12px;
}

.m-k {
  color: var(--c-text-regular);
}

.m-v {
  font-weight: 600;
  font-family: var(--font-mono);
}

/* ── 资源链接 ──────────────────────────────────────────────── */
.resource-links {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.res-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border-radius: var(--c-radius);
  cursor: pointer;
  transition: background var(--motion-fast) var(--ease-out);
}

.res-item:hover {
  background: var(--c-bg-hover);
}

.res-name {
  flex: 1;
  font-size: 12.5px;
  color: var(--c-text);
}

.res-count {
  font-size: 11px;
  color: var(--c-text-secondary);
  font-family: var(--font-mono);
}

.res-ico.blue { color: var(--c-primary); }
.res-ico.purple { color: var(--c-info); }

/* ── 程序期限与统筹 ──────────────────────────────────────────────── */
.tracks-container {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.track-panel {
  border: 1px solid var(--c-border-light);
  border-radius: var(--c-radius);
  padding: 12px 14px;
}

.track-panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
}

.tp-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--c-text);
}

.track-stepper {
  display: flex;
  align-items: center;
  gap: 10px;
}

.tp-step {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: var(--c-text-secondary);
}

.tp-step.active {
  color: var(--c-text);
  font-weight: 500;
}

.tp-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--c-primary);
}

.tp-dot.muted {
  background: var(--gray-300);
}

.tp-line {
  flex: 1;
  height: 1px;
  background: var(--c-border);
}

/* ── 动态轨迹时间线 ────────────────────────────────────────── */
.timeline-stream {
  display: flex;
  flex-direction: column;
}

.t-row {
  display: flex;
  gap: 12px;
  min-height: 38px;
}

.t-date {
  width: 48px;
  font-size: 11px;
  color: var(--c-text-secondary);
  font-family: var(--font-mono);
  text-align: right;
  padding-top: 2px;
  flex-shrink: 0;
}

.t-track {
  width: 12px;
  display: flex;
  justify-content: center;
  position: relative;
  flex-shrink: 0;
}

.t-track::before {
  content: "";
  position: absolute;
  top: 0;
  bottom: 0;
  left: 50%;
  width: 1px;
  background: var(--c-border);
}

.t-row:first-child .t-track::before { top: 6px; }
.t-row:last-child .t-track::before { height: 6px; }

.t-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--c-primary);
  margin-top: 6px;
  z-index: 1;
}

.t-body {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding-bottom: 10px;
}

.t-summary {
  font-size: 12.5px;
  color: var(--c-text);
  font-weight: 500;
}

.t-detail {
  font-size: 11px;
  color: var(--c-text-secondary);
}

/* ── 药丸标签 ──────────────────────────────────────────────── */
.tag {
  display: inline-flex;
  align-items: center;
  height: 18px;
  padding: 0 7px;
  border-radius: 999px;
  font-size: 10px;
  font-weight: 500;
  white-space: nowrap;
}

.tag.blue { background: var(--c-primary-light); color: var(--c-primary); }
.tag.purple { background: var(--c-info-light); color: var(--c-info); }
.tag.green { background: var(--c-success-light); color: var(--c-success); }
.tag.amber { background: var(--c-warning-light); color: var(--c-warning); }
.tag.red { background: var(--c-danger-light); color: var(--c-danger); }
.tag.solid-blue { background: var(--c-primary); color: #fff; }
.tag.text-3 { background: var(--gray-100); color: var(--c-text-secondary); }

.btn-sm {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 24px;
  padding: 0 9px;
  border-radius: var(--c-radius);
  font-size: 11px;
  font-weight: 500;
  cursor: pointer;
  border: 1px solid var(--c-border);
  background: var(--c-surface);
  color: var(--c-text-regular);
  font-family: inherit;
  transition: all var(--motion-fast) var(--ease-out);
}

.btn-sm:hover {
  border-color: var(--gray-400);
  color: var(--c-text);
}

.btn-sm.primary {
  background: var(--c-primary);
  color: #fff;
  border-color: transparent;
}

.btn-sm.primary:hover {
  background: var(--c-primary-hover);
}

.text-danger { color: var(--c-danger) !important; }
.text-success { color: var(--c-success) !important; }

/* ── 表格与列表布局 ────────────────────────────────────────── */
.hearings-table-wrap,
.tasks-table-wrap {
  overflow-x: auto;
}

.data-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 12.5px;
  text-align: left;
}

.data-table th {
  padding: 8px 12px;
  background: var(--c-bg-subtle, #f8fafc);
  color: var(--c-text-secondary, #64748b);
  font-weight: 600;
  border-bottom: 1px solid var(--c-border);
  font-size: 11.5px;
}

.data-table td {
  padding: 10px 12px;
  border-bottom: 1px solid var(--c-border-light, #f1f5f9);
  vertical-align: middle;
}

.data-table tr:hover td {
  background: var(--c-bg-hover, #f8fafc);
}

.data-table tr.row-done td {
  opacity: 0.65;
}

.task-title {
  font-weight: 500;
  color: var(--c-text-main, #1e293b);
}

.task-title.done {
  text-decoration: line-through;
  color: var(--c-text-muted, #94a3b8);
}

.task-desc {
  font-size: 11px;
  color: var(--c-text-muted, #64748b);
  margin-top: 2px;
}

.row-ops {
  display: flex;
  gap: 8px;
  align-items: center;
}

/* ── 全量字段网格编辑 ──────────────────────────────────────── */
.fields-grid-layout {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 16px 24px;
}

.field-item {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.field-item.full-width {
  grid-column: 1 / -1;
}

.field-label {
  font-size: 11.5px;
  font-weight: 600;
  color: var(--c-text-secondary, #64748b);
}

.field-val {
  font-size: 13px;
  color: var(--c-text-main, #1e293b);
  padding: 4px 0;
  min-height: 24px;
}
</style>
