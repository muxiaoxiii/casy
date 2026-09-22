<script setup>
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { useRoute, useRouter, onBeforeRouteLeave } from 'vue-router'
import { observeChanges } from '../../../core/observeChanges'
import { casyContext } from '../../../core/plugin/context'
import { deleteTaskOptimistic, completeTaskOptimistic, restoreTaskOptimistic, snoozeTaskWithUndo, canUndo, undoLast } from '../../../core/taskActions'
import { ElMessage, ElMessageBox } from 'element-plus'
import { useFiltersStore } from '../../../stores/filters'
import {
  Plus, Clock, Calendar, Star, Folder,
  ArrowRight, Delete, Edit, More, RefreshRight,
  Box, List, Timer, Files, Check, Search,
  Menu, Grid, Collection, Select, Close, Location,
  Opportunity, Warning, TrendCharts, CircleCheck, AlarmClock, RefreshLeft
} from '../../../shared/icons'
import { useTasksStore } from '../../../stores/tasks'
import PerspectiveManager from '../components/PerspectiveManager.vue'
import TaskQuickEditor from '../components/TaskQuickEditor.vue'
import TaskRow from '../components/TaskRow.vue'
import AreasDialog from '../components/AreasDialog.vue'
import TodayResetDialog from '../components/TodayResetDialog.vue'
import StateFeedback from '../../../shared/components/StateFeedback.vue'
import { VueDraggable } from 'vue-draggable-plus'
import { formatDate } from '../utils/taskDisplay'
import { registerShortcut } from '../../../shared/keyboard'
import { parseWhen } from '../../../shared/nlp/parseWhen'
import { tauriCall } from '../../../core/tauriBridge'
import { emptyEditForm, toEditForm, toSavePayload } from '../utils/taskForm'
import {
  applyTaskCardFilters,
  buildCaseGroupSections,
  buildChildrenMap,
  buildGtdStats,
  buildMatrixQuadrants,
  tasksForPerspective,
  topLevelPerspectiveTasks,
} from '../utils/taskFilter'

// ============================================================
// 1. 状态管理 (直连真实 SQLite 数据库)
// ============================================================
const tasks = ref([])
const cases = ref([])
const areas = ref([])
const loading = ref(false)
const savingTask = ref(false)
const busyTasks = new Set()
const undoAvailable = computed(canUndo)
const undoBusy = ref(false)

// 当前激活的标签页/透视
// 'all' | 'inbox' | 'today' | 'upcoming' | 'multiday' | 'next' | 'waiting' | 'matrix' | 'bycase' | 'completed' | (customId)
const activePerspective = ref('today')
const inspectedTaskId = ref('')
const quickEditors = ref([])
const showAllPerspectives = ref(false)

// 任务搜索与过滤
const searchQuery = ref('')
const selectedContextFilter = ref('all')
const selectedCaseFilter = ref('all')

// 抽屉与详情编辑
const showDrawer = ref(false)
const editingTask = ref(null)
const editForm = ref(emptyEditForm())

// 弹窗状态
const showCreateDialog = ref(false)
const showAreasDialog = ref(false)
const showTodayReset = ref(false)
const showPerspectiveManager = ref(false)
const editingPerspective = ref(null)

// W2 推迟到…对话框
const showDeferDialog = ref(false)
const deferTargetTask = ref(null)
const deferDate = ref('')
const deferSaving = ref(false)

// 快速捕获输入条
const captureInput = ref('')
const capturing = ref(false)
const captureInputRef = ref(null)

// 子任务管理
const expandedParents = ref(new Set())
const newChildText = ref({})

// Store
const tasksStore = useTasksStore()
const filtersStore = useFiltersStore()
const route = useRoute()
const router = useRouter()
const customPerspectives = computed(() => tasksStore.customPerspectives)
const savedFilters = computed(() => filtersStore.filters)

let unregisterKeys = []

// ============================================================
// 2. 丰富全景透视标签定义 (Rich Perspectives)
// ============================================================
const perspectives = [
  { key: 'all', label: '全部待办', icon: List, color: 'var(--c-primary)', desc: '全量待办任务工作台' },
  { key: 'inbox', label: '收件箱', icon: Box, color: '#9BA2AF', desc: '未分类待厘清任务' },
  { key: 'today', label: '今日专注', icon: Calendar, color: '#B4554F', desc: '今日必须推进的重点' },
  { key: 'upcoming', label: '计划排期', icon: Timer, color: '#3E5C9A', desc: '有明确截止或开始日期的任务' },
  { key: 'multiday', label: '跨天专项', icon: TrendCharts, color: '#6C6A9C', desc: '多日连续阶段性任务' },
  { key: 'next', label: '随时行动', icon: ArrowRight, color: '#4C8067', desc: '无依赖可立即执行' },
  { key: 'waiting', label: '等待追踪', icon: Clock, color: '#B0823A', desc: '等待对方回复或委派跟进' },
  { key: 'review', label: '待回顾', icon: RefreshRight, color: 'var(--c-info)', desc: '已到回顾日期的未完成任务' },
  { key: 'deferred', label: '已推迟', icon: AlarmClock, color: '#5B7A9E', desc: '推迟到未来日期的任务，到期自动回归' },
  { key: 'matrix', label: '四象限', icon: Grid, color: '#E6A23C', desc: '重要与紧急度决策看板' },
  { key: 'bycase', label: '按案件', icon: Folder, color: '#409EFF', desc: '按关联案件聚合分类' },
  { key: 'completed', label: '已完成', icon: CircleCheck, color: '#67C23A', desc: '历史归档与复盘' },
]

watch(() => route.query.tab, tab => {
  if (typeof tab === 'string' && perspectives.some(p => p.key === tab)) activePerspective.value = tab
}, { immediate: true })
const metricLabel = computed(() => ({ dueOrOverdue: '到期与逾期', dueToday: '今日到期', waitingOverdue: '等待超时' }[route.query.metric] || ''))
function clearMetric() {
  const { metric, ...query } = route.query
  router.replace({ query })
}

const primaryPerspectiveKeys = ['inbox', 'today', 'upcoming', 'next', 'waiting', 'completed']
const primaryPerspectives = computed(() => primaryPerspectiveKeys.map(key => perspectives.find(p => p.key === key)))
const secondaryPerspectives = computed(() => perspectives.filter(p => !primaryPerspectiveKeys.includes(p.key)))
const currentPerspective = computed(() => perspectives.find(p => p.key === activePerspective.value) || { label: customPerspectives.value.find(p => p.id === activePerspective.value)?.name || '任务', desc: '按你的方式组织工作' })
const capturePreview = computed(() => parseWhen(captureInput.value))
async function flushQuickEditor() { return (await quickEditors.value?.[0]?.saveIfDirty()) !== false }
onBeforeRouteLeave(flushQuickEditor)
async function inspectTask(task) { if (await flushQuickEditor()) inspectedTaskId.value = inspectedTaskId.value === task.id ? '' : task.id }
async function selectCase(id) { if (!(await flushQuickEditor())) return; selectedCaseFilter.value = id; await switchPerspective('all') }
function advancedTask(task) { inspectedTaskId.value = ''; openDrawer(tasks.value.find(t => t.id === task.id) || task) }

const priorityOptions = [
  { value: 'urgent_important', label: '重要且紧急', color: '#f56c6c' },
  { value: 'important', label: '重要不紧急', color: '#e6a23c' },
  { value: 'urgent', label: '紧急不重要', color: '#409eff' },
  { value: 'normal', label: '普通', color: '#909399' },
]

const contextOptions = [
  { value: 'office', label: '@办公室', icon: Location },
  { value: 'phone', label: '@电话沟通', icon: Clock },
  { value: 'court', label: '@法庭开庭', icon: Folder },
  { value: 'computer', label: '@电脑起草', icon: Edit },
  { value: 'outside', label: '@外出办理', icon: Location },
]

const snoozeOptions = [
  { value: 'tonight', label: '今晚' },
  { value: 'tomorrow', label: '明天' },
  { value: 'weekend', label: '周末' },
  { value: 'next_week', label: '下周' },
]

// ============================================================
// 3. 统计计数计算 (Real Counts)
// ============================================================
const clockNow = ref(new Date())
let dayTimer
const todayStr = computed(() => {
  const d = clockNow.value
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${y}-${m}-${day}`
})

const gtdStats = computed(() => buildGtdStats(tasks.value, todayStr.value))

// ============================================================
// 4. 当前透视任务流过滤 (Dynamic Filtering)
// ============================================================
const filteredTasks = computed(() => applyTaskCardFilters(
  tasksForPerspective(tasks.value, activePerspective.value, {
    todayStr: todayStr.value,
    getCustomTasks: (id) => tasksStore.getTasksByCustomPerspective(id),
  }),
  {
    searchQuery: searchQuery.value,
    contextFilter: selectedContextFilter.value,
    caseFilter: selectedCaseFilter.value,
    resolveCaseName: getCaseName,
    metric: route.query.metric,
    todayStr: todayStr.value,
  },
))

// Preserve independently actionable children when their parent is outside this perspective/filter.
const gtdTasks = computed(() => {
  if(activePerspective.value==='bycase')return filteredTasks.value
  return topLevelPerspectiveTasks(filteredTasks.value)
})

// 案件分组视图
const caseGroupSections = computed(() => buildCaseGroupSections(gtdTasks.value, getCaseName))

// 四象限看板数据
const matrixQuadrants = computed(() => buildMatrixQuadrants(tasks.value))

// 子任务映射
const childrenMap = computed(() => buildChildrenMap(tasks.value))

// ============================================================
// 5. 数据加载与持久化
// ============================================================
onMounted(async () => {
  dayTimer = window.setInterval(() => { clockNow.value = new Date() }, 60000)
  await loadData()
  filtersStore.loadFilters('tasks')

  // 消费 ?edit=<taskId>（证据链接/通知中心跳转约定）：定位并打开任务抽屉
  const editId = route.query.edit
  if (editId) {
    const target = tasks.value.find(t => t.id === editId)
    if (target) {
      openDrawer(target)
    } else {
      ElMessage.warning('未找到对应任务（可能已被删除）')
    }
    const { edit, ...query } = route.query
    router.replace({ query })
  }

  unregisterKeys.push(
    registerShortcut('meta+t', () => captureInputRef.value?.focus(), { description: '聚焦快速捕获' }),
    registerShortcut('ctrl+t', () => captureInputRef.value?.focus(), { description: '聚焦快速捕获' }),
    // 撤销上一步删除/稍后等任务操作：接通 taskActions 的 Undo 栈
    registerShortcut('meta+z', () => undoTaskAction(), { description: '撤销上一步任务操作' }),
    registerShortcut('ctrl+z', () => undoTaskAction(), { description: '撤销上一步任务操作' })
  )
})

onUnmounted(() => {
  if (dayTimer) window.clearInterval(dayTimer)
  unregisterKeys.forEach(fn => fn())
})

async function loadData() {
  loading.value = true
  await Promise.all([
    loadTasks(),
    loadCases(),
    loadAreas(),
  ])
  loading.value = false
}
onUnmounted(observeChanges(casyContext, ['task', 'case', 'inbox'], loadData))

let tasksRequest = 0
async function loadTasks() {
  const request = ++tasksRequest
  // 加载包含已完成在内的全量任务以支持已完成归档透视
  const result = await casyContext.tasks.list({})
  if (request === tasksRequest && result.ok && Array.isArray(result.data)) {
    tasks.value = result.data
  }
}

async function loadCases() {
  const result = await casyContext.cases.list({})
  if (result.ok) {
    cases.value = Array.isArray(result.data)
      ? result.data
      : (Array.isArray(result.data?.items) ? result.data.items : [])
  }
}

async function loadAreas() {
  const result = await casyContext.tasks.areas()
  if (result.ok && Array.isArray(result.data)) {
    areas.value = result.data
  }
}

function getCaseName(caseId) {
  if (!caseId) return ''
  const c = cases.value.find(item => item.id === caseId)
  return c ? (c.caseName || c.caseNo) : ''
}

function getAreaName(areaId) {
  if (!areaId) return ''
  const a = areas.value.find(item => item.id === areaId)
  return a ? a.name : ''
}

// 切换透视
async function switchPerspective(key) {
  if (!(await flushQuickEditor())) return
  if (metricLabel.value) clearMetric()
  activePerspective.value = key
  inspectedTaskId.value = ''
  tasksStore.activePerspective = key
}

// 快速捕获任务
async function quickCapture(isTodayOnly = false) {
  const text = captureInput.value.trim()
  if (!text || capturing.value) return
  capturing.value = true

  const { taskName, date, time } = parseWhen(text)
  const data = {
    taskName: taskName || text,
    caseId: selectedCaseFilter.value === 'all' ? null : selectedCaseFilter.value,
    startDate: date || (isTodayOnly ? todayStr.value : null),
    dueDate: date || (isTodayOnly ? todayStr.value : null),
    dueTime: time || null,
    startBucket: isTodayOnly ? 'today' : (date ? 'anytime' : 'inbox'),
    taskType: 'action',
    priority: 'normal',
    completed: 0,
  }

  const result = await casyContext.tasks.create(data)
  capturing.value = false
  if (result.ok) {
    ElMessage.success(`已记录任务：「${data.taskName}」`)
    captureInput.value = ''
    await loadTasks()
  } else {
    ElMessage.error(result.error || '创建失败')
  }
}

function onCaptureKeydown(e) {
  if (e.isComposing) return
  if (e.metaKey || e.ctrlKey) {
    quickCapture(true)
  } else {
    quickCapture(false)
  }
}

// 完成/取消完成任务
async function toggleComplete(task) {
  if (busyTasks.has(task.id)) return
  busyTasks.add(task.id)
  try {
    const ok = task.completed ? await restoreTaskOptimistic(task) : await completeTaskOptimistic(task)
    if (ok) await loadTasks()
  } finally {
    busyTasks.delete(task.id)
  }
}

async function undoTaskAction() {
  if (undoBusy.value) return
  undoBusy.value = true
  try { if (await undoLast()) await loadTasks() }
  finally { undoBusy.value = false }
}

async function markReviewed(task) {
  if (busyTasks.has(task.id)) return
  busyTasks.add(task.id)
  try {
    const result = await casyContext.tasks.update({ id: task.id, lastReviewDate: todayStr.value, nextReviewDate: null })
    if (result.ok) await loadTasks()
    else ElMessage.error(result.error || '回顾保存失败')
  } finally { busyTasks.delete(task.id) }
}

// 更改任务象限优先级 (拖拽落位)
async function onDropToQuadrant(e, priorityKey) {
  e.preventDefault()
  let task = null
  if (e.dataTransfer) {
    try {
      task = JSON.parse(e.dataTransfer.getData('application/json'))
    } catch {}
  }
  if (!task || !task.id) return

  const result = await casyContext.tasks.update({ id: task.id, priority: priorityKey })
  if (!result.ok) {
    ElMessage.error(result.error || '移动任务失败')
    return
  }
  ElMessage.success(`已将「${task.taskName}」移动至对应象限`)
  await loadTasks()
}

function onDragStartTask(e, task) {
  if (e.dataTransfer) {
    e.dataTransfer.effectAllowed = 'move'
    e.dataTransfer.setData('application/json', JSON.stringify(task))
  }
}

function onDragOver(e) {
  e.preventDefault()
}

// 打开编辑抽屉
function openDrawer(task) {
  editingTask.value = task
  editForm.value = toEditForm(task)
  showDrawer.value = true
}

function openNewTask() {
  editingTask.value = { id: null, taskName: '' }
  editForm.value = emptyEditForm('inbox')
  showDrawer.value = true
}

// 保存任务编辑
async function saveTask() {
  if (savingTask.value) return
  if (!editForm.value.taskName.trim()) {
    ElMessage.warning('请输入任务名称')
    return
  }

  const data = toSavePayload(editingTask.value.id, editForm.value)

  savingTask.value = true
  try {
  const result = editingTask.value.id
    ? await casyContext.tasks.update(data)
    : await casyContext.tasks.create(data)
  if (result.ok) {
    ElMessage.success(editingTask.value.id ? '任务已更新' : '任务已创建')
    showDrawer.value = false
    await loadTasks()
  } else {
    ElMessage.error(result.error || '保存失败')
  }
  } finally { savingTask.value = false }
}

async function moveTaskToday(task) {
  const result = await casyContext.tasks.update({
    id: task.id,
    startBucket: 'today',
    startDate: todayStr.value,
  })
  if (result.ok) {
    ElMessage.success('已移至今日')
    await loadTasks()
  } else ElMessage.error(result.error || '操作失败')
}

async function markTaskWaiting(task) {
  const result = await casyContext.tasks.update({ id: task.id, taskType: 'waiting' })
  if (result.ok) {
    ElMessage.success('已标记为等待')
    await loadTasks()
  } else ElMessage.error(result.error || '操作失败')
}

async function snoozeTask(task, option) {
  if (busyTasks.has(task.id)) return
  busyTasks.add(task.id)
  try {
    if (await snoozeTaskWithUndo(task, option, snoozeOptions.find(item => item.value === option)?.label || option)) await loadTasks()
  } finally { busyTasks.delete(task.id) }
}

function triageTask(task) {
  openDrawer(task)
}

// 删除任务（乐观 + Undo）：本地立即移除、失败回滚、Cmd/Ctrl+Z 可撤销
async function deleteTask(task) {
  let confirmed = false
  try {
    await ElMessageBox.confirm(`确定删除任务「${task.taskName}」吗？`, '删除确认', {
      confirmButtonText: '删除',
      cancelButtonText: '取消',
      type: 'warning',
    })
    confirmed = true
  } catch { /* 用户取消：属预期 */ }
  if (!confirmed) return

  const wasEditing = editingTask.value?.id === task.id
  const ok = await deleteTaskOptimistic(task, {
    // 乐观阶段：从当前列表移除；若是抽屉中正在编辑的任务，同时关闭抽屉
    remove: () => {
      const idx = tasks.value.findIndex(t => t.id === task.id)
      if (idx !== -1) tasks.value.splice(idx, 1)
      if (editingTask.value?.id === task.id) showDrawer.value = false
    },
    // 回滚（删除失败）或撤销（Undo）时：把任务还原进列表
    restore: () => {
      if (!tasks.value.some(t => t.id === task.id)) {
        tasks.value.push(task)
      }
    },
  })

  if (!ok) {
    // 删除失败：若此前抽屉正在编辑该任务，重开抽屉供继续编辑
    if (wasEditing && editingTask.value?.id === task.id && !showDrawer.value) {
      openDrawer(task)
    }
    return
  }
  ElMessage.success('已删除任务（⌘/Ctrl+Z 可撤销）')
}

// ============================================================
// W2 推迟到…（OmniFocus 式 Defer Date）
// ============================================================
function openDeferDialog(task) {
  deferTargetTask.value = task
  // 默认明天；已推迟任务回显当前推迟日
  if (task.deferUntil) {
    deferDate.value = task.deferUntil
  } else {
    const d = new Date()
    d.setDate(d.getDate() + 1)
    const p = (n) => String(n).padStart(2, '0')
    deferDate.value = `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`
  }
  showDeferDialog.value = true
}

function disablePastDates(date) {
  const today = new Date()
  today.setHours(0, 0, 0, 0)
  return date.getTime() < today.getTime()
}

async function confirmDefer() {
  if (!deferTargetTask.value || !deferDate.value || deferSaving.value) return
  deferSaving.value = true
  const ok = await tauriCall('defer_task', {
    taskId: deferTargetTask.value.id,
    until: deferDate.value,
  }, { errorMessage: '推迟失败' })
  deferSaving.value = false
  if (ok !== null) {
    ElMessage.success(`已推迟至 ${formatDate(deferDate.value) || deferDate.value}，到期自动回归今日`)
    showDeferDialog.value = false
    deferTargetTask.value = null
    await loadTasks()
  }
}

async function undeferTask(task) {
  const ok = await tauriCall('clear_task_defer', { taskId: task.id }, { errorMessage: '操作失败' })
  if (ok !== null) {
    ElMessage.success(`「${task.taskName}」已结束推迟`)
    await loadTasks()
  }
}

function toggleExpand(task) {
  if (expandedParents.value.has(task.id)) {
    expandedParents.value.delete(task.id)
  } else {
    expandedParents.value.add(task.id)
  }
}

async function addChild(parentTask) {
  const text = (newChildText.value[parentTask.id] || '').trim()
  if (!text) return

  const result = await casyContext.tasks.create({
    taskName: text,
    parentId: parentTask.id,
    caseId: parentTask.caseId || null,
    completed: 0,
    startBucket: parentTask.startBucket || 'anytime',
  })
  // 后端失败：报错并退出，不清空输入、不弹成功，避免虚假成功
  if (!result.ok) return ElMessage.error(result.error || '添加子任务失败')
  newChildText.value[parentTask.id] = ''
  expandedParents.value.add(parentTask.id)
  ElMessage.success('已添加子任务')
  await loadTasks()
}

// 自定义透视操作
function openCreatePerspective() {
  editingPerspective.value = null
  showPerspectiveManager.value = true
}

function handlePerspectiveCommand(command, perspective) {
  if (command === 'edit') {
    editingPerspective.value = perspective
    showPerspectiveManager.value = true
  } else if (command === 'delete') {
    ElMessageBox.confirm('确定删除此透视？', '确认', { type: 'warning' }).then(() => {
      tasksStore.deleteCustomPerspective(perspective.id)
      ElMessage.success('透视已删除')
      activePerspective.value = 'all'
    }).catch(() => {})
  }
}

function handlePerspectiveSave(data) {
  if (editingPerspective.value) {
    tasksStore.updateCustomPerspective(editingPerspective.value.id, data)
    ElMessage.success('透视已更新')
  } else {
    tasksStore.saveCustomPerspective(data)
    ElMessage.success('透视已创建')
  }
  showPerspectiveManager.value = false
  editingPerspective.value = null
}

function getCustomPerspectiveCount(perspectiveId) {
  return tasksStore.getTasksByCustomPerspective(perspectiveId).length
}
</script>

<template>
  <div class="stitch-tasks-workspace">
    <aside class="task-navigation" aria-label="任务导航">
      <div class="task-nav-brand"><span class="task-nav-mark">✓</span><div><strong>行动</strong><small>给重要的事留出空间</small></div></div>
      <nav class="task-primary-nav" aria-label="常用清单">
        <button v-for="p in primaryPerspectives" :key="p.key" type="button" :class="{ selected: activePerspective === p.key }" :aria-pressed="activePerspective === p.key" @click="switchPerspective(p.key)">
          <el-icon :style="{ color: p.color }"><component :is="p.icon" /></el-icon><span>{{ p.label }}</span><small>{{ gtdStats[p.key] || '' }}</small>
        </button>
      </nav>
      <button class="task-nav-section" type="button" :aria-expanded="showAllPerspectives" @click="showAllPerspectives = !showAllPerspectives">更多视角 <span>{{ showAllPerspectives ? '−' : '+' }}</span></button>
      <nav v-if="showAllPerspectives || secondaryPerspectives.some(p => p.key === activePerspective)" class="task-primary-nav" aria-label="更多视角">
        <button v-for="p in secondaryPerspectives" :key="p.key" type="button" :class="{ selected: activePerspective === p.key }" @click="switchPerspective(p.key)"><el-icon><component :is="p.icon" /></el-icon><span>{{ p.label }}</span><small>{{ gtdStats[p.key] || '' }}</small></button>
      </nav>
      <div class="task-nav-section">案件 <button type="button" @click="showAreasDialog = true" title="管理领域">管理领域</button></div>
      <nav class="task-primary-nav task-case-nav" aria-label="按案件筛选">
        <button type="button" :class="{ selected: selectedCaseFilter === 'all' }" @click="selectCase('all')"><el-icon><Folder /></el-icon><span>全部案件</span></button>
        <button v-for="c in cases" :key="c.id" type="button" :class="{ selected: selectedCaseFilter === c.id }" @click="selectCase(c.id)"><span class="case-dot"/><span>{{ c.caseName || c.caseNo }}</span></button>
      </nav>
      <div v-if="customPerspectives.length" class="task-nav-section">我的清单</div>
      <nav class="task-primary-nav" aria-label="自定义清单">
        <div v-for="cp in customPerspectives" :key="cp.id" class="custom-nav-row"><button type="button" :class="{ selected: activePerspective === cp.id }" @click="switchPerspective(cp.id)"><el-icon><Files /></el-icon><span>{{ cp.name }}</span></button><el-dropdown trigger="click" @command="cmd => handlePerspectiveCommand(cmd, cp)"><button type="button" :aria-label="'管理' + cp.name">···</button><template #dropdown><el-dropdown-menu><el-dropdown-item command="edit">编辑</el-dropdown-item><el-dropdown-item command="delete">删除</el-dropdown-item></el-dropdown-menu></template></el-dropdown></div>
      </nav>
      <button type="button" class="task-new-list" @click="openCreatePerspective">＋ 新建自定义清单</button>
      <button type="button" class="task-nav-calendar" @click="router.push('/calendar')"><el-icon><Calendar /></el-icon> 在日历中安排时间 <span>↗</span></button>
    </aside>
    <main class="task-focus-pane">
      <div class="task-mobile-filters"><select :value="activePerspective" aria-label="选择任务视角" @change="switchPerspective($event.target.value)"><option v-for="p in perspectives" :key="p.key" :value="p.key">{{ p.label }}</option><option v-for="p in customPerspectives" :key="p.id" :value="p.id">{{ p.name }}</option></select><select :value="selectedCaseFilter" aria-label="筛选关联案件" @change="selectCase($event.target.value)"><option value="all">全部案件</option><option v-for="c in cases" :key="c.id" :value="c.id">{{ c.caseName || c.caseNo }}</option></select></div>
    <!-- ═══ 1. 顶部 Header 栏 ═══ -->
    <div class="tasks-top-header">
      <div class="header-titles">
        <p class="task-page-eyebrow">{{ new Date().toLocaleDateString('zh-CN', { month: 'long', day: 'numeric', weekday: 'long' }) }}</p><h1 class="tasks-heading">{{ currentPerspective.label }}</h1><p class="task-page-description">{{ currentPerspective.desc }}</p>
      </div>

      <div class="header-actions">
        <el-button :icon="RefreshLeft" :disabled="!undoAvailable || undoBusy" aria-label="撤销任务操作" title="撤销任务操作" @click="undoTaskAction" />
        <!-- 快速搜索 -->
        <div class="task-search-box">
          <el-icon class="search-icon" :size="14"><Search /></el-icon>
          <input v-model="searchQuery" placeholder="搜索任务名称、案件、备注..." class="search-real-input" />
          <el-icon v-if="searchQuery" class="clear-icon" :size="12" @click="searchQuery = ''"><Close /></el-icon>
        </div>

        <button class="btn-action-ghost" @click="showAreasDialog = true">
          <el-icon :size="14"><Folder /></el-icon>
          <span>领域</span>
        </button>

        <button class="btn-action-primary" @click="openNewTask">
          <el-icon :size="14"><Plus /></el-icon>
          <span>新建任务</span>
        </button>
      </div>
    </div>

    <!-- ═══ 2. 丰富全景透视标签栏 (Full Dynamic Perspective Tabs) ═══ -->
    <el-tag v-if="metricLabel" class="metric-filter" closable @close="clearMetric">{{ metricLabel }}</el-tag>
    <!-- ═══ 3. 快速自然语言捕获栏 ═══ -->
    <div class="capture-bar-wrapper">
      <div class="capture-real-box">
        <el-icon class="cap-icon" :size="16"><Plus /></el-icon>
        <input
          ref="captureInputRef"
          v-model="captureInput"
          placeholder="记下下一步… 例如：明天下午3点 核对证据目录"
          aria-label="快速添加任务"
          :disabled="capturing"
          class="cap-input"
          @keydown.enter="onCaptureKeydown"
        />
        <div class="cap-right-btns"><kbd v-if="!captureInput">↵</kbd>
          <button v-if="captureInput.trim()" class="btn-cap-submit" @click="quickCapture(false)">
            添加待办
          </button>
        </div>
      </div>
    </div>

    <div v-if="captureInput.trim()" class="capture-interpretation"><span>{{ capturePreview.taskName }}</span><small>{{ capturePreview.date || '收件箱' }} {{ capturePreview.time || '' }}</small><small>⌘ / Ctrl + Enter 放入今天</small></div>

    <!-- ═══ 4. 主工作区分发渲染 (根据当前激活 Tab 渲染专属视图) ═══ -->

    <!-- A. 四象限决策看板 (Matrix View) -->
    <div v-if="activePerspective === 'matrix'" class="matrix-board-grid">
      <div
        v-for="(quad, qKey) in matrixQuadrants"
        :key="qKey"
        class="quadrant-card"
        @dragover="onDragOver"
        @drop="onDropToQuadrant($event, quad.key)"
      >
        <div class="quad-header" :style="{ borderTopColor: quad.color }">
          <div>
            <h3 class="quad-title" :style="{ color: quad.color }">{{ quad.title }}</h3>
            <p class="quad-sub">{{ quad.desc }}</p>
          </div>
          <span class="quad-count-pill">{{ quad.tasks.length }}</span>
        </div>

        <div class="quad-tasks-list">
          <div
            v-for="task in quad.tasks"
            :key="task.id"
            class="quad-task-item"
            draggable="true"
            @dragstart="onDragStartTask($event, task)"
            @click="openDrawer(task)"
          >
            <button class="quad-check-btn" :class="{ checked: task.completed }" @click.stop="toggleComplete(task)">
              <el-icon v-if="task.completed" :size="11"><Check /></el-icon>
            </button>
            <div class="quad-task-info">
              <strong class="quad-task-name">{{ task.taskName }}</strong>
              <div class="quad-task-meta">
                <span v-if="task.caseId" class="q-case">{{ getCaseName(task.caseId) }}</span>
                <span v-if="task.dueDate" class="q-due">{{ task.dueDate }}</span>
              </div>
            </div>
          </div>

          <div v-if="!quad.tasks.length" class="quad-empty">
            <span>暂无此类任务 · 可拖拽任务落入此象限</span>
          </div>
        </div>
      </div>
    </div>

    <!-- B. 按案件分组视图 (By Matter View) -->
    <div v-else-if="activePerspective === 'bycase'" class="case-groups-stream">
      <div v-for="cg in caseGroupSections" :key="cg.caseId" class="case-group-card">
        <div class="cg-header">
          <div class="cg-title-row">
            <el-icon class="cg-icon"><Folder /></el-icon>
            <h3 class="cg-title">{{ cg.caseName }}</h3>
          </div>
          <span class="cg-count-tag">{{ cg.tasks.length }} 项待办</span>
        </div>

        <div class="cg-tasks-list">
          <template v-for="task in cg.tasks" :key="task.id"><TaskRow
            :task="task"
            :perspective="activePerspective"
            :snooze-options="snoozeOptions"
            :resolve-case-name="getCaseName"
            :resolve-area-name="getAreaName"
            :has-children="childrenMap.has(task.id)"
            :expanded="expandedParents.has(task.id)"
            @toggle="toggleComplete"
            @open="inspectTask"
            @delete="deleteTask"
            @triage="triageTask"
            @move-today="moveTaskToday"
            @mark-waiting="markTaskWaiting"
            @snooze="snoozeTask"
            @toggle-expand="toggleExpand"
            @defer="openDeferDialog"
            @undefer="undeferTask"
            @reviewed="markReviewed"
            @follow-up="openDrawer"
          />
          <TaskQuickEditor ref="quickEditors" v-if="inspectedTaskId === task.id" :key="'edit-' + task.id" :task="task" :cases="cases" @close="inspectedTaskId = ''" @saved="loadTasks" @advanced="advancedTask(task)" />
          </template>
        </div>
      </div>
      <StateFeedback
        v-if="loading || !caseGroupSections.length"
        :state="loading ? 'loading' : 'empty'"
        empty-text="暂无案件相关待办"
      />
    </div>

    <!-- C. 常规待办 / GTD / 跨天 / 已完成列表视图 -->
    <div v-else class="tasks-main-list-card">
      <div class="list-section-header">
        <div class="lsh-left">
          <h2 class="lsh-title">
            {{ perspectives.find(p => p.key === activePerspective)?.label || '任务列表' }}
          </h2>
          <span class="lsh-sub">{{ perspectives.find(p => p.key === activePerspective)?.desc || '' }}</span>
        </div>
        <span class="lsh-badge">{{ gtdTasks.length }} 项</span>
      </div>

      <div class="task-rows-stack">
        <template v-for="task in gtdTasks" :key="task.id">
          <TaskRow
            :task="task"
            :perspective="activePerspective"
            :snooze-options="snoozeOptions"
            :resolve-case-name="getCaseName"
            :resolve-area-name="getAreaName"
            :has-children="childrenMap.has(task.id)"
            :expanded="expandedParents.has(task.id)"
            @toggle="toggleComplete"
            @open="inspectTask"
            @delete="deleteTask"
            @triage="triageTask"
            @move-today="moveTaskToday"
            @mark-waiting="markTaskWaiting"
            @snooze="snoozeTask"
            @toggle-expand="toggleExpand"
            @defer="openDeferDialog"
            @undefer="undeferTask"
            @reviewed="markReviewed"
            @follow-up="openDrawer"
          />
          <TaskQuickEditor ref="quickEditors" v-if="inspectedTaskId === task.id" :key="'edit-' + task.id" :task="task" :cases="cases" @close="inspectedTaskId = ''" @saved="loadTasks" @advanced="advancedTask(task)" />

          <!-- 展开子任务 -->
          <div v-if="expandedParents.has(task.id)" class="subtasks-container">
            <div
              v-for="child in (childrenMap.get(task.id) || [])"
              :key="'sub-' + child.id"
              class="subtask-row"
            >
              <button class="sub-check-btn" :class="{ checked: child.completed }" @click.stop="toggleComplete(child)">
                <el-icon v-if="child.completed" :size="10"><Check /></el-icon>
              </button>
              <span class="sub-name" :class="{ struck: child.completed }">{{ child.taskName }}</span>
              <el-icon class="sub-del-icon" @click="deleteTask(child)"><Delete /></el-icon>
            </div>

            <div class="subtask-add-row">
              <input
                v-model="newChildText[task.id]"
                placeholder="+ 添加子步骤/待办，按 Enter 提交"
                class="subtask-input"
                @keyup.enter="addChild(task)"
              />
            </div>
          </div>
        </template>

        <StateFeedback
          v-if="loading || !gtdTasks.length"
          :state="loading ? 'loading' : 'empty'"
          :empty-text="activePerspective === 'completed' ? '暂无已完成归档记录' : activePerspective === 'deferred' ? '暂无推迟中的任务，可在任务菜单选择「推迟到…」' : '当前视角下暂无任务，输入上方输入框快速记录'"
        />
      </div>
    </div>

    </main>

    <!-- ═══ 5. 任务编辑详情抽屉 (Task Detail Drawer) ═══ -->
    <el-drawer
      v-model="showDrawer"
      :title="editingTask?.id ? '任务详细信息' : '新建任务（可直接关联案件）'"
      size="min(480px, 100vw)"
      :close-on-click-modal="!savingTask"
      :close-on-press-escape="!savingTask"
      :show-close="!savingTask"
      destroy-on-close
    >
      <div v-if="editingTask" class="drawer-body">
        <div class="form-item">
          <label for="task-edit-name">任务名称</label>
          <input id="task-edit-name" v-model="editForm.taskName" class="form-input" placeholder="输入任务名称..." />
        </div>

        <div class="form-row">
          <div class="form-item">
            <label for="task-edit-type">任务类型</label>
            <select id="task-edit-type" v-model="editForm.taskType" class="form-select">
              <option value="action">行动</option><option value="waiting">等待</option><option value="deadline">期限</option>
            </select>
          </div>
          <div class="form-item">
            <label for="task-edit-bucket">开始安排</label>
            <select id="task-edit-bucket" v-model="editForm.startBucket" class="form-select">
              <option value="inbox">待整理</option><option value="today">今天</option><option value="anytime">随时</option><option value="someday">将来也许</option>
            </select>
          </div>
        </div>

        <div class="form-row">
          <div class="form-item">
            <label>开始日期 (Do When)</label>
            <input v-model="editForm.startDate" type="date" class="form-input" />
          </div>
          <div class="form-item">
            <label>截止日期 (Deadline)</label>
            <input v-model="editForm.dueDate" type="date" class="form-input" />
          </div>
        </div>

        <div class="form-row">
          <div class="form-item">
            <label>四象限优先级</label>
            <select v-model="editForm.priority" class="form-select">
              <option v-for="po in priorityOptions" :key="po.value" :value="po.value">
                {{ po.label }}
              </option>
            </select>
          </div>
          <div class="form-item">
            <label>预估工时 (分钟)</label>
            <input v-model.number="editForm.estimatedMinutes" type="number" min="0" step="15" class="form-input" />
          </div>
        </div>

        <div class="form-row">
          <div class="form-item"><label>截止时间</label><input v-model="editForm.dueTime" type="time" class="form-input" aria-label="截止时间" /></div>
          <div class="form-item"><label>重复</label><select v-model="editForm.recurrenceRule" class="form-select" aria-label="重复">
            <option value="">不重复</option><option value="daily">每天</option><option value="weekdays">每个工作日</option>
            <option v-for="(day,index) in ['周一','周二','周三','周四','周五','周六','周日']" :key="day" :value="`weekly:${index + 1}`">每{{ day }}</option>
            <option v-for="day in 31" :key="day" :value="`monthly:${day}`">每月 {{ day }} 日</option>
          </select></div>
        </div>
        <div class="form-row">
          <div class="form-item"><label>等待对象</label><input v-model="editForm.waitingFor" class="form-input" aria-label="等待对象" /></div>
          <div class="form-item"><label>跟进日期</label><input v-model="editForm.followUpDate" type="date" class="form-input" aria-label="跟进日期" /></div>
        </div>
        <div class="form-item"><label>下次回顾</label><input v-model="editForm.nextReviewDate" type="date" class="form-input" aria-label="下次回顾" /></div>

        <div class="form-item">
          <label>关联案件 (Matter)</label>
          <select v-model="editForm.caseId" class="form-select">
            <option value="">（无关联案件）</option>
            <option v-for="c in cases" :key="c.id" :value="c.id">
              {{ c.caseName || c.caseNo }}
            </option>
          </select>
        </div>

        <div class="form-item">
          <label>推迟到 (Defer Until)</label>
          <div class="defer-field-row">
            <input v-model="editForm.deferUntil" type="date" class="form-input" />
            <button
              v-if="editForm.deferUntil"
              type="button"
              class="btn-defer-clear"
              @click="editForm.deferUntil = ''"
            >
              清除
            </button>
          </div>
        </div>

        <div class="form-item">
          <label>上下文场景 (Context)</label>
          <div class="context-pill-group">
            <button
              v-for="ctx in contextOptions"
              :key="ctx.value"
              class="ctx-pill-btn"
              :class="{ active: editForm.context === ctx.value }"
              @click="editForm.context = (editForm.context === ctx.value ? '' : ctx.value)"
            >
              {{ ctx.label }}
            </button>
          </div>
        </div>

        <div class="form-item">
          <label>备注与案情要点</label>
          <textarea v-model="editForm.description" rows="4" class="form-textarea" placeholder="记录事项细节、会见要点或草案说明..." />
        </div>
      </div>

      <template #footer>
        <div class="drawer-footer">
          <button v-if="editingTask?.id" class="btn-danger-del" :disabled="savingTask" @click="deleteTask(editingTask)">
            <el-icon><Delete /></el-icon>
            <span>删除</span>
          </button>
          <div class="drawer-right-btns">
            <button class="btn-cancel" :disabled="savingTask" @click="showDrawer = false">取消</button>
            <button class="btn-primary" :disabled="savingTask" @click="saveTask">{{ savingTask ? '保存中...' : editingTask?.id ? '保存修改' : '创建任务' }}</button>
          </div>
        </div>
      </template>
    </el-drawer>

    <!-- W2 推迟到…对话框 -->
    <el-dialog
      v-model="showDeferDialog"
      title="推迟到…"
      width="360px"
      destroy-on-close
    >
      <div class="defer-dialog-body">
        <p class="defer-dialog-desc">
          「{{ deferTargetTask?.taskName }}」在到期前将从「今日专注」隐藏，到期当天自动回归。
        </p>
        <el-date-picker
          v-model="deferDate"
          type="date"
          value-format="YYYY-MM-DD"
          placeholder="选择回归日期"
          :disabled-date="disablePastDates"
          style="width: 100%"
        />
      </div>
      <template #footer>
        <div class="drawer-right-btns">
          <button class="btn-cancel" @click="showDeferDialog = false">取消</button>
          <button class="btn-primary" :disabled="!deferDate || deferSaving" @click="confirmDefer">
            {{ deferSaving ? '推迟中…' : '确定推迟' }}
          </button>
        </div>
      </template>
    </el-dialog>

    <!-- 弹窗组件 -->
    <AreasDialog v-model="showAreasDialog" />
    <PerspectiveManager
      v-model="showPerspectiveManager"
      :perspective="editingPerspective"
      @save="handlePerspectiveSave"
    />
  </div>
</template>

<style scoped>
.metric-filter { align-self: flex-start; }
/* ═══════════════════════════════════════════════════════════
   Stitch Unified Task Management Styles
   ═══════════════════════════════════════════════════════════ */
.stitch-tasks-workspace {
  max-width: 1440px;
  margin: 0 auto;
  padding: 20px 24px 40px;
  display: flex;
  flex-direction: column;
  gap: 16px;
  color: var(--c-text);
  font-family: var(--font-family);
  min-height: calc(100vh - 80px);
}

/* ── 1. 顶部 Header ───────────────────────────────────────── */
.tasks-top-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  flex-wrap: wrap;
}

.tasks-heading {
  font-size: 22px;
  font-weight: 700;
  color: var(--c-text-heading);
  margin: 0;
}

.tasks-sub-hint {
  font-size: 12px;
  color: var(--slate-gray-light);
  margin-top: 2px;
  display: block;
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 10px;
}

.task-search-box {
  position: relative;
  width: 260px;
}

.search-icon {
  position: absolute;
  left: 10px;
  top: 50%;
  transform: translateY(-50%);
  color: var(--slate-gray-light);
}

.clear-icon {
  position: absolute;
  right: 10px;
  top: 50%;
  transform: translateY(-50%);
  color: var(--slate-gray-light);
  cursor: pointer;
}

.search-real-input {
  width: 100%;
  height: 34px;
  padding: 0 28px 0 30px;
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  font-size: 12px;
  outline: none;
  color: var(--c-text);
  box-shadow: var(--shadow-sm);
}

.search-real-input:focus { border-color: var(--c-primary); }

.btn-action-ghost {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 34px;
  padding: 0 12px;
  border-radius: var(--c-radius-lg);
  border: 1px solid var(--c-border);
  background: var(--c-bg-card);
  color: var(--c-text-regular);
  font-size: 12px;
  cursor: pointer;
}

.btn-action-ghost:hover { background: var(--c-bg-hover); color: var(--c-text); }

.btn-action-primary {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 34px;
  padding: 0 16px;
  border-radius: var(--c-radius-lg);
  border: none;
  background: var(--c-primary);
  color: var(--c-primary-contrast);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  box-shadow: var(--shadow-sm);
}

/* ── 2. 丰富全景透视标签栏 ───────────────────────────────── */
.perspective-tabs-scroll-bar {
  display: flex;
  gap: 8px;
  padding: 8px;
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-xl);
  box-shadow: var(--shadow-sm);
  overflow-x: auto;
}

.perspective-tab-pill {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  border-radius: var(--c-radius-lg);
  border: 1px solid transparent;
  background: transparent;
  color: var(--c-text-secondary);
  font-size: 12.5px;
  font-weight: 500;
  cursor: pointer;
  white-space: nowrap;
  transition: all var(--motion-fast);
}

.perspective-tab-pill:hover {
  background: var(--c-bg-hover);
  color: var(--c-text);
}

.perspective-tab-pill.active {
  background: var(--c-primary-light);
  color: var(--c-primary);
  font-weight: 700;
  border-color: color-mix(in srgb, var(--c-primary) 30%, transparent);
}

.tab-badge {
  background: var(--c-bg-subtle);
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 700;
  padding: 1px 6px;
  border-radius: 6px;
  color: var(--c-text-regular);
}
.perspective-tab-pill.active .tab-badge { background: var(--c-primary); color: var(--c-primary-contrast); }

.perspective-tab-pill.create-btn {
  border: 1px dashed var(--c-border);
  color: var(--slate-gray-light);
}

.perspective-tab-pill.create-btn:hover {
  border-color: var(--c-primary);
  color: var(--c-primary);
}

.more-icon {
  margin-left: 2px;
  font-size: 12px;
  opacity: 0.6;
}

.more-icon:hover { opacity: 1; }

/* ── 3. 自然语言捕获栏 ───────────────────────────────────── */
.capture-bar-wrapper {
  margin-bottom: 4px;
}

.capture-real-box {
  display: flex;
  align-items: center;
  gap: 10px;
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-xl);
  padding: 0 16px;
  height: 42px;
  box-shadow: var(--shadow-sm);
}

.capture-real-box:focus-within {
  border-color: var(--c-primary);
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--c-primary) 20%, transparent);
}

.cap-icon { color: var(--slate-gray-light); }

.cap-input {
  flex: 1;
  border: none;
  background: transparent;
  outline: none;
  font-size: 13px;
  color: var(--c-text);
}

.btn-cap-submit {
  padding: 4px 12px;
  border-radius: var(--c-radius);
  border: none;
  background: var(--c-primary);
  color: var(--c-primary-contrast);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
}

/* ── 4. A. 四象限看板 ────────────────────────────────────── */
.matrix-board-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 16px;
}

.quadrant-card {
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-xl);
  box-shadow: var(--shadow-sm);
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-height: 280px;
}

.quad-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  border-top: 3.5px solid transparent;
  padding-top: 8px;
}

.quad-title {
  font-size: 15px;
  font-weight: 700;
  margin: 0;
}

.quad-sub {
  font-size: 11px;
  color: var(--slate-gray-light);
  margin: 2px 0 0;
}

.quad-count-pill {
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 700;
  background: var(--c-bg-subtle);
  padding: 2px 8px;
  border-radius: 10px;
}

.quad-tasks-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex: 1;
}

.quad-task-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  border-radius: var(--c-radius-lg);
  border: 1px solid var(--c-border);
  background: var(--c-bg-page);
  cursor: grab;
  transition: all var(--motion-fast);
}

.quad-task-item:hover {
  background: var(--c-bg-hover);
  border-color: var(--c-primary);
  transform: translateY(-1px);
}

.quad-check-btn {
  width: 16px;
  height: 16px;
  border-radius: 4px;
  border: 1.5px solid var(--c-border-strong);
  background: transparent;
  cursor: pointer;
  display: grid;
  place-items: center;
  color: #fff;
  flex-shrink: 0;
}

.quad-check-btn.checked {
  background: var(--c-primary);
  border-color: var(--c-primary);
}

.quad-task-info {
  flex: 1;
  min-width: 0;
}

.quad-task-name {
  font-size: 12.5px;
  color: var(--c-text-heading);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.quad-task-meta {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 10.5px;
  margin-top: 2px;
}

.q-case { color: var(--c-primary); }
.q-due { color: var(--slate-gray-light); font-family: var(--font-mono); }

.quad-empty {
  padding: 30px 0;
  text-align: center;
  font-size: 11.5px;
  color: var(--slate-gray-light);
}

/* ── 4. B. 按案件分组 ────────────────────────────────────── */
.case-groups-stream {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.case-group-card {
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-xl);
  box-shadow: var(--shadow-sm);
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.cg-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-bottom: 8px;
  border-bottom: 1px solid var(--c-border);
}

.cg-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.cg-icon { color: var(--c-primary); }

.cg-title {
  font-size: 15px;
  font-weight: 700;
  margin: 0;
}

.cg-count-tag {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--slate-gray-light);
}

.cg-tasks-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

/* ── 4. C. 常规列表卡片 ──────────────────────────────────── */
.tasks-main-list-card {
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-xl);
  box-shadow: var(--shadow-sm);
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.list-section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--c-border);
}

.lsh-title {
  font-size: 17px;
  font-weight: 700;
  margin: 0;
}

.lsh-sub {
  font-size: 11.5px;
  color: var(--slate-gray-light);
  margin-top: 2px;
  display: block;
}

.lsh-badge {
  font-family: var(--font-mono);
  font-size: 11px;
  font-weight: 700;
  background: var(--c-primary-light);
  color: var(--c-primary);
  padding: 2px 8px;
  border-radius: 10px;
}

.task-rows-stack {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.subtasks-container {
  margin-left: 28px;
  padding-left: 12px;
  border-left: 2px solid var(--c-border);
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-top: -2px;
  margin-bottom: 6px;
}

.subtask-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 8px;
  background: var(--c-bg-page);
  border-radius: var(--c-radius);
  font-size: 12px;
}

.sub-check-btn {
  width: 14px;
  height: 14px;
  border-radius: 3px;
  border: 1.5px solid var(--c-border-strong);
  background: transparent;
  cursor: pointer;
  display: grid;
  place-items: center;
  color: #fff;
}

.sub-check-btn.checked {
  background: var(--c-primary);
  border-color: var(--c-primary);
}

.sub-name { flex: 1; }
.sub-name.struck { text-decoration: line-through; color: var(--slate-gray-light); }

.sub-del-icon {
  font-size: 12px;
  color: var(--slate-gray-light);
  cursor: pointer;
}

.sub-del-icon:hover { color: var(--status-risk); }

.subtask-input {
  width: 100%;
  background: transparent;
  border: 1px dashed var(--c-border);
  border-radius: var(--c-radius);
  padding: 4px 8px;
  font-size: 11.5px;
  outline: none;
}

.empty-placeholder {
  padding: 48px 0;
  text-align: center;
  color: var(--slate-gray-light);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
}

/* ── 5. 抽屉样式 ─────────────────────────────────────────── */
.drawer-body {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.form-item {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex: 1;
}

.form-item label {
  font-size: 12px;
  font-weight: 600;
  color: var(--c-text-heading);
}

.form-row {
  display: flex;
  gap: 12px;
}

.form-input,
.form-select,
.form-textarea {
  width: 100%;
  padding: 8px 10px;
  border-radius: var(--c-radius);
  border: 1px solid var(--c-border);
  background: var(--c-bg-page);
  color: var(--c-text);
  font-size: 12.5px;
  outline: none;
}

.form-input:focus,
.form-select:focus,
.form-textarea:focus {
  border-color: var(--c-primary);
}

.context-pill-group {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.ctx-pill-btn {
  padding: 4px 10px;
  border-radius: var(--c-radius-lg);
  border: 1px solid var(--c-border);
  background: var(--c-bg-page);
  color: var(--c-text-secondary);
  font-size: 11.5px;
  cursor: pointer;
}

.ctx-pill-btn.active {
  background: var(--c-primary-light);
  border-color: var(--c-primary);
  color: var(--c-primary);
  font-weight: 600;
}

.drawer-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
}

.btn-danger-del {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 6px 12px;
  border-radius: var(--c-radius-lg);
  border: 1px solid var(--status-risk);
  background: var(--bg-risk-weak);
  color: var(--status-risk);
  font-size: 12px;
  cursor: pointer;
}

.drawer-right-btns {
  display: flex;
  gap: 8px;
}

.btn-cancel {
  padding: 6px 14px;
  border-radius: var(--c-radius-lg);
  border: 1px solid var(--c-border);
  background: var(--c-bg-subtle);
  color: var(--c-text);
  font-size: 12px;
  cursor: pointer;
}

.btn-primary {
  padding: 6px 16px;
  border-radius: var(--c-radius-lg);
  border: none;
  background: var(--c-primary);
  color: var(--c-primary-contrast);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
}

.btn-primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* ── W2 推迟到… ─────────────────────────────────────────── */
.defer-field-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.btn-defer-clear {
  flex-shrink: 0;
  padding: 6px 10px;
  border-radius: var(--c-radius);
  border: 1px solid var(--c-border);
  background: var(--c-bg-subtle);
  color: var(--c-text-secondary);
  font-size: 11.5px;
  cursor: pointer;
  transition: color var(--motion-fast) var(--ease-out), border-color var(--motion-fast) var(--ease-out);
}

.btn-defer-clear:hover {
  color: var(--status-risk);
  border-color: var(--status-risk);
}

.defer-hint {
  font-size: 11px;
  color: var(--slate-gray-light);
}

.defer-dialog-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.defer-dialog-desc {
  margin: 0;
  font-size: 12px;
  line-height: 1.6;
  color: var(--c-text-secondary);
}

@media (max-width: 600px) {
  .stitch-tasks-workspace { padding: 20px 16px 32px; }
  .header-actions { width: 100%; display: grid; grid-template-columns: 34px 1fr auto; }
  .header-actions > .el-button { width: 34px; padding: 0; }
  .task-search-box { grid-column: 1 / -1; grid-row: 2; width: 100%; }
  .btn-action-ghost { justify-self: start; }
  .btn-action-ghost, .btn-action-primary { white-space: nowrap; }
}
@container (max-width: 700px) {
  .header-actions { flex-wrap: wrap; width: 100%; gap: 8px; }
  .task-search-box { flex: 1; min-width: 140px; width: auto; }
  .filter-toolbar-row { flex-wrap: wrap; }
}
</style>

<style scoped>
.stitch-tasks-workspace { display: grid; grid-template-columns: 214px minmax(0, 1fr); padding: 0; gap: 0; height: 100%; min-height: 0; background: var(--c-bg-card); border: 1px solid var(--c-border); border-radius: 12px; overflow: hidden; }
.task-navigation { display: flex; flex-direction: column; gap: 3px; padding: 26px 14px 18px; background: var(--c-bg); border-right: 1px solid var(--c-border); overflow-y: auto; }
.task-nav-brand { display: flex; gap: 12px; align-items: center; padding: 2px 12px 26px; }
.task-nav-mark { display: grid; place-items: center; width: 30px; height: 30px; color: var(--c-primary); background: var(--c-primary-light); border-radius: 9px; font-size: 20px; }
.task-nav-brand strong { display: block; font-size: 18px; letter-spacing: .08em; }
.task-nav-brand small { display: block; font-size: 10px; color: var(--c-text-secondary); margin-top: 5px; }
.task-navigation button { font: inherit; color: var(--c-text); background: transparent; border: 0; cursor: pointer; border-radius: 6px; }
.task-primary-nav { display: flex; flex-direction: column; gap: 3px; }
.task-primary-nav button { width: 100%; display: flex; align-items: center; gap: 10px; text-align: left; padding: 10px 12px; font-size: 13px; min-width: 0; }
.task-primary-nav button > span { overflow: hidden; white-space: nowrap; text-overflow: ellipsis; flex: 1; }
.task-primary-nav button > small { color: var(--c-text-secondary); font-size: 11px; font-variant-numeric: tabular-nums; }
.task-primary-nav button:hover { background: var(--c-bg-hover); }
.task-primary-nav button.selected { background: var(--c-primary-light); color: var(--c-primary); font-weight: 600; }
.task-navigation .task-nav-section { display: flex; justify-content: space-between; padding: 22px 12px 8px; color: var(--c-text-secondary); font-size: 11px; text-align: left; }
.task-nav-section button { font-size: 10px; color: var(--c-text-secondary); }
.task-primary-nav .case-dot { flex: 0 0 6px; height: 6px; border-radius: 50%; background: var(--c-primary); margin: 0 4px; opacity: .6; }
.task-case-nav { max-height: 260px; overflow: auto; }
.custom-nav-row { display: flex; align-items: center; }
.custom-nav-row > button { flex: 1; }
.task-navigation .task-new-list { font-size: 12px; text-align: left; padding: 14px 12px; color: var(--c-text-secondary); }
.task-navigation .task-nav-calendar { margin-top: auto; display: flex; align-items: center; gap: 8px; padding: 18px 6px 0; font-size: 11px; color: var(--c-text-secondary); border-radius: 0; border-top: 1px solid var(--c-border); }
.task-focus-pane { padding: 40px clamp(24px, 4vw, 64px) 32px; overflow-y: auto; min-width: 0; }
.tasks-top-header { display: flex; align-items: flex-start; gap: 20px; margin-bottom: 32px; flex-wrap: wrap; }
.task-page-eyebrow { margin: 0 0 12px; font-size: 11px; color: var(--c-text-secondary); letter-spacing: .08em; }
.tasks-heading { font-size: 30px; font-weight: 650; letter-spacing: -.04em; line-height: 1.2; }
.task-page-description { font-size: 12px; color: var(--c-text-secondary); margin: 12px 0 0; }
.header-actions { gap: 8px; margin-left: auto; flex-wrap: wrap; }
.header-actions > .btn-action-ghost { display: none; }
.task-search-box { width: 160px; }
.capture-bar-wrapper { padding: 0 0 18px; margin: 0; }
.capture-real-box { background: var(--c-bg); border: 1px solid var(--c-border); box-shadow: none; border-radius: 8px; min-height: 48px; }
.cap-input { font-size: 13px; }
.cap-right-btns kbd { color: var(--c-text-secondary); font-size: 15px; }
.capture-interpretation { display: flex; flex-wrap: wrap; align-items: center; gap: 12px; padding: 0 4px 20px; color: var(--c-primary); font-size: 12px; }
.capture-interpretation small { color: var(--c-text-secondary); font-size: 11px; }
.tasks-main-list-card { background: transparent; border: 0; box-shadow: none; border-radius: 0; margin-top: 8px; }
.list-section-header { padding: 12px 0; border-bottom: 1px solid var(--c-border); }
.lsh-title { font-size: 12px; font-weight: 600; }
.lsh-sub { display: none; }
.lsh-badge { background: transparent; font-size: 11px; }
.task-rows-stack { padding: 8px 0; gap: 0; }
.task-focus-pane :deep(.task-card) { border: 0; border-bottom: 1px solid var(--c-border); border-radius: 0; box-shadow: none; padding: 15px 4px; background: transparent; }
.task-focus-pane :deep(.task-card:hover) { background: var(--c-bg-hover); transform: none; }
.task-focus-pane :deep(.task-name-text) { font-size: 14px; font-weight: 450; }
.task-focus-pane :deep(.task-check) { width: 18px; height: 18px; border-radius: 6px; border-width: 1.5px; }
.task-focus-pane :deep(.task-meta) { font-size: 11px; margin-top: 5px; }
.task-navigation button:focus-visible { outline: 2px solid var(--c-primary); outline-offset: 2px; }
@media (max-width: 1050px) { .stitch-tasks-workspace { grid-template-columns: 184px minmax(0, 1fr); } .task-focus-pane { padding: 28px 24px; } .header-actions { margin-left: 0; } }
@media (max-width: 650px) {
  .stitch-tasks-workspace { display: flex; flex-direction: column; overflow: auto; }
  .task-navigation { flex: 0 0 auto; padding: 10px; border-right: 0; border-bottom: 1px solid var(--c-border); overflow: visible; }
  .task-nav-brand, .task-nav-section, .task-case-nav, .task-new-list, .task-nav-calendar, .custom-nav-row { display: none !important; }
  .task-primary-nav { flex-direction: row; overflow-x: auto; }
  .task-primary-nav button { flex: 0 0 auto; width: auto; padding: 9px 12px; }
  .task-primary-nav:has(.custom-nav-row) { display: none; }
  .task-focus-pane { padding: 22px 16px; overflow: visible; }
  .tasks-top-header { margin-bottom: 20px; }
  .tasks-heading { font-size: 25px; }
}
</style>
<style scoped>
.task-mobile-filters { display: none; }
@media (max-width:650px) { .task-mobile-filters { display: flex; gap: 8px; margin-bottom: 20px; } .task-mobile-filters select { width: 50%; min-width: 0; color: var(--c-text); background: var(--c-bg-card); border: 1px solid var(--c-border); border-radius: 6px; padding: 8px; } }
</style>
