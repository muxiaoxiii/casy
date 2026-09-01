<script setup>
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { casyContext } from '../../../core/plugin/context'
import {
  completeTaskOptimistic, restoreTaskOptimistic,
  deleteTaskOptimistic, snoozeTaskWithUndo
} from '../../../core/taskActions'
import { ElMessage, ElMessageBox } from 'element-plus'
import { useFiltersStore } from '../../../stores/filters'
import {
  Plus, Clock, Calendar, Star, Folder,
  ArrowRight, Delete, Edit, More, RefreshRight,
  Box, List, Timer, Files, Check, Search,
  Menu, Grid, Collection, Select, Close, Location,
  Opportunity, Warning, TrendCharts, CircleCheck
} from '@element-plus/icons-vue'
import { useTasksStore } from '../../../stores/tasks'
import PerspectiveManager from '../components/PerspectiveManager.vue'
import TaskRow from '../components/TaskRow.vue'
import AreasDialog from '../components/AreasDialog.vue'
import TodayResetDialog from '../components/TodayResetDialog.vue'
import StateFeedback from '../../../shared/components/StateFeedback.vue'
import { VueDraggable } from 'vue-draggable-plus'
import { formatDate } from '../utils/taskDisplay'
import { registerShortcut } from '../../../shared/keyboard'
import { parseWhen } from '../../../shared/nlp/parseWhen'

// ============================================================
// 1. 状态管理 (直连真实 SQLite 数据库)
// ============================================================
const tasks = ref([])
const cases = ref([])
const areas = ref([])
const loading = ref(false)

// 当前激活的标签页/透视
// 'all' | 'inbox' | 'today' | 'upcoming' | 'multiday' | 'next' | 'waiting' | 'matrix' | 'bycase' | 'completed' | (customId)
const activePerspective = ref('all')

// 任务搜索与过滤
const searchQuery = ref('')
const selectedContextFilter = ref('all')
const selectedCaseFilter = ref('all')

// 抽屉与详情编辑
const showDrawer = ref(false)
const editingTask = ref(null)
const editForm = ref({
  taskName: '',
  description: '',
  deadline: '',
  priority: 'normal',
  caseId: '',
  taskType: 'action',
  startDate: '',
  dueDate: '',
  dueTime: '',
  waitingFor: '',
  followUpDate: '',
  context: '',
  flagged: false,
  areaId: '',
  estimatedMinutes: null,
  startBucket: 'anytime',
})

// 弹窗状态
const showCreateDialog = ref(false)
const showAreasDialog = ref(false)
const showTodayReset = ref(false)
const showPerspectiveManager = ref(false)
const editingPerspective = ref(null)

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
  { key: 'matrix', label: '四象限', icon: Grid, color: '#E6A23C', desc: '重要与紧急度决策看板' },
  { key: 'bycase', label: '按案件', icon: Folder, color: '#409EFF', desc: '按关联案件聚合分类' },
  { key: 'completed', label: '已完成', icon: CircleCheck, color: '#67C23A', desc: '历史归档与复盘' },
]

const priorityOptions = [
  { value: 'urgent_important', label: '重要且紧急 (第一象限)', color: '#f56c6c' },
  { value: 'important', label: '重要不紧急 (第二象限)', color: '#e6a23c' },
  { value: 'urgent', label: '紧急不重要 (第三象限)', color: '#409eff' },
  { value: 'normal', label: '普通/不紧急不重要 (第四象限)', color: '#909399' },
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
const todayStr = computed(() => {
  const d = new Date()
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${y}-${m}-${day}`
})

const gtdStats = computed(() => {
  const today = todayStr.value
  const uncompleted = tasks.value.filter(t => !t.completed)
  const completedList = tasks.value.filter(t => !!t.completed)

  return {
    all: uncompleted.length,
    inbox: uncompleted.filter(t => t.startBucket === 'inbox' || (!t.dueDate && !t.startDate && !t.caseId)).length,
    today: uncompleted.filter(t => t.startBucket === 'today' || (t.startDate && t.startDate <= today) || (t.dueDate && t.dueDate === today)).length,
    upcoming: uncompleted.filter(t => t.dueDate || t.startDate || t.deadline).length,
    multiday: uncompleted.filter(t => t.startDate && t.dueDate && t.startDate !== t.dueDate).length,
    next: uncompleted.filter(t => t.taskType === 'action' || !t.taskType).length,
    waiting: uncompleted.filter(t => t.taskType === 'waiting' || !!t.waitingFor).length,
    matrix: uncompleted.length,
    bycase: uncompleted.filter(t => !!t.caseId).length,
    completed: completedList.length,
  }
})

// ============================================================
// 4. 当前透视任务流过滤 (Dynamic Filtering)
// ============================================================
const gtdTasks = computed(() => {
  const today = todayStr.value
  const q = searchQuery.value.trim().toLowerCase()
  let list = []

  switch (activePerspective.value) {
    case 'all':
      list = tasks.value.filter(t => !t.completed)
      break

    case 'inbox':
      list = tasks.value.filter(t => !t.completed && (t.startBucket === 'inbox' || (!t.dueDate && !t.startDate && !t.caseId)))
      break

    case 'today':
      list = tasks.value.filter(t => !t.completed && (t.startBucket === 'today' || (t.startDate && t.startDate <= today) || (t.dueDate && t.dueDate === today)))
      break

    case 'upcoming':
      list = tasks.value.filter(t => !t.completed && (t.dueDate || t.startDate || t.deadline))
      list.sort((a, b) => (a.dueDate || a.startDate || '9999').localeCompare(b.dueDate || b.startDate || '9999'))
      break

    case 'multiday':
      list = tasks.value.filter(t => !t.completed && t.startDate && t.dueDate && t.startDate !== t.dueDate)
      break

    case 'next':
      list = tasks.value.filter(t => !t.completed && (t.taskType === 'action' || !t.taskType))
      break

    case 'waiting':
      list = tasks.value.filter(t => !t.completed && (t.taskType === 'waiting' || !!t.waitingFor))
      break

    case 'completed':
      list = tasks.value.filter(t => !!t.completed)
      break

    case 'matrix':
    case 'bycase':
      list = tasks.value.filter(t => !t.completed)
      break

    default:
      // 自定义透视
      list = tasksStore.getTasksByCustomPerspective(activePerspective.value)
      break
  }

  // 搜索关键字过滤
  if (q) {
    list = list.filter(t => t.taskName?.toLowerCase().includes(q) || t.description?.toLowerCase().includes(q) || getCaseName(t.caseId)?.toLowerCase().includes(q))
  }

  // 上下文过滤
  if (selectedContextFilter.value !== 'all') {
    list = list.filter(t => t.context === selectedContextFilter.value)
  }

  // 案件过滤
  if (selectedCaseFilter.value !== 'all') {
    list = list.filter(t => t.caseId === selectedCaseFilter.value)
  }

  return list
})

// 案件分组视图
const caseGroupSections = computed(() => {
  const map = new Map()
  const unassigned = []

  for (const t of gtdTasks.value) {
    if (t.caseId) {
      if (!map.has(t.caseId)) {
        map.set(t.caseId, {
          caseId: t.caseId,
          caseName: getCaseName(t.caseId) || '未知案件',
          tasks: [],
        })
      }
      map.get(t.caseId).tasks.push(t)
    } else {
      unassigned.push(t)
    }
  }

  const list = Array.from(map.values())
  if (unassigned.length) {
    list.push({
      caseId: '__unassigned__',
      caseName: '律所通用 / 未指定案件',
      tasks: unassigned,
    })
  }
  return list
})

// 四象限看板数据
const matrixQuadrants = computed(() => {
  const uncompleted = tasks.value.filter(t => !t.completed)
  return {
    q1: {
      key: 'urgent_important',
      title: '重要且紧急 (Do First)',
      desc: '诉讼举证截止、明日开庭准备、紧急保全',
      color: '#f56c6c',
      tasks: uncompleted.filter(t => t.priority === 'urgent_important'),
    },
    q2: {
      key: 'important',
      title: '重要不紧急 (Schedule)',
      desc: '起草长篇辩护词、战略推演、客户深度维系',
      color: '#e6a23c',
      tasks: uncompleted.filter(t => t.priority === 'important' || (!t.priority && (t.startDate && t.dueDate && t.startDate !== t.dueDate))),
    },
    q3: {
      key: 'urgent',
      title: '紧急不重要 (Delegate)',
      desc: '调取常规档案、法庭文书盖章送达、助理跟进',
      color: '#409eff',
      tasks: uncompleted.filter(t => t.priority === 'urgent' || t.taskType === 'waiting'),
    },
    q4: {
      key: 'normal',
      title: '普通 / 不紧急 (Someday)',
      desc: '模板整理、行业合规资讯查阅、备忘归档',
      color: '#909399',
      tasks: uncompleted.filter(t => t.priority === 'normal' || !t.priority),
    },
  }
})

// 子任务映射
const childrenMap = computed(() => {
  const m = new Map()
  for (const t of tasks.value) {
    if (!t.parentId) continue
    if (!m.has(t.parentId)) m.set(t.parentId, [])
    m.get(t.parentId).push(t)
  }
  return m
})

// ============================================================
// 5. 数据加载与持久化
// ============================================================
onMounted(async () => {
  await loadData()
  filtersStore.loadFilters('tasks')

  unregisterKeys.push(
    registerShortcut('meta+t', () => captureInputRef.value?.focus(), { description: '聚焦快速捕获' }),
    registerShortcut('ctrl+t', () => captureInputRef.value?.focus(), { description: '聚焦快速捕获' })
  )
})

onUnmounted(() => {
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

async function loadTasks() {
  // 加载包含已完成在内的全量任务以支持已完成归档透视
  const result = await casyContext.tasks.list({})
  if (result.ok && Array.isArray(result.data)) {
    tasks.value = result.data
  }
}

async function loadCases() {
  const result = await casyContext.cases.list({})
  if (result.ok && Array.isArray(result.data)) {
    cases.value = result.data
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
function switchPerspective(key) {
  activePerspective.value = key
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
  if (e.metaKey || e.ctrlKey) {
    quickCapture(true)
  } else {
    quickCapture(false)
  }
}

// 完成/取消完成任务
async function toggleComplete(task) {
  const newDone = !task.completed
  await casyContext.tasks.update({ id: task.id, completed: newDone ? 1 : 0 })
  ElMessage.success(newDone ? '任务已完成' : '已恢复为待办')
  await loadTasks()
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

  await casyContext.tasks.update({ id: task.id, priority: priorityKey })
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
  editForm.value = {
    taskName: task.taskName || '',
    description: task.description || '',
    deadline: task.deadline || '',
    priority: task.priority || 'normal',
    caseId: task.caseId || '',
    taskType: task.taskType || 'action',
    startDate: task.startDate || '',
    dueDate: task.dueDate || task.deadline || '',
    dueTime: task.dueTime || '',
    waitingFor: task.waitingFor || '',
    followUpDate: task.followUpDate || '',
    context: task.context || '',
    flagged: task.flagged === 1,
    areaId: task.areaId || '',
    estimatedMinutes: task.estimatedMinutes || null,
    startBucket: task.startBucket || 'anytime',
  }
  showDrawer.value = true
}

// 保存任务编辑
async function saveTask() {
  if (!editForm.value.taskName.trim()) {
    ElMessage.warning('请输入任务名称')
    return
  }

  const data = {
    id: editingTask.value.id,
    ...editForm.value,
    flagged: editForm.value.flagged ? 1 : 0,
  }

  const result = await casyContext.tasks.update(data)
  if (result.ok) {
    ElMessage.success('任务已更新')
    showDrawer.value = false
    await loadTasks()
  } else {
    ElMessage.error(result.error || '保存失败')
  }
}

// 删除任务
async function deleteTask(task) {
  try {
    await ElMessageBox.confirm(`确定删除任务「${task.taskName}」吗？`, '删除确认', {
      confirmButtonText: '删除',
      cancelButtonText: '取消',
      type: 'warning',
    })
    await casyContext.tasks.remove(task.id)
    ElMessage.success('已删除任务')
    if (editingTask.value?.id === task.id) {
      showDrawer.value = false
    }
    await loadTasks()
  } catch {}
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

  await casyContext.tasks.create({
    taskName: text,
    parentId: parentTask.id,
    caseId: parentTask.caseId || null,
    completed: 0,
    startBucket: parentTask.startBucket || 'anytime',
  })
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
    <!-- ═══ 1. 顶部 Header 栏 ═══ -->
    <div class="tasks-top-header">
      <div class="header-titles">
        <h1 class="tasks-heading">任务管理工作台</h1>
        <span class="tasks-sub-hint">支持全景视角、四象限决策看板、GTD 流程与跨天专项排期</span>
      </div>

      <div class="header-actions">
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

        <button class="btn-action-primary" @click="showCreateDialog = true">
          <el-icon :size="14"><Plus /></el-icon>
          <span>新建任务</span>
        </button>
      </div>
    </div>

    <!-- ═══ 2. 丰富全景透视标签栏 (Full Dynamic Perspective Tabs) ═══ -->
    <div class="perspective-tabs-scroll-bar">
      <!-- 基础内置与全景标签 -->
      <button
        v-for="p in perspectives"
        :key="p.key"
        class="perspective-tab-pill"
        :class="{ active: activePerspective === p.key }"
        @click="switchPerspective(p.key)"
        :title="p.desc"
      >
        <el-icon :size="14" class="tab-icon"><component :is="p.icon" /></el-icon>
        <span class="tab-title">{{ p.label }}</span>
        <span class="tab-badge" :style="{ backgroundColor: p.color }">
          {{ gtdStats[p.key] ?? 0 }}
        </span>
      </button>

      <!-- 自定义透视 -->
      <div
        v-for="cp in customPerspectives"
        :key="cp.id"
        class="perspective-tab-pill custom"
        :class="{ active: activePerspective === cp.id }"
        @click="switchPerspective(cp.id)"
      >
        <el-icon :size="14"><Files /></el-icon>
        <span class="tab-title">{{ cp.name }}</span>
        <span class="tab-badge" :style="{ backgroundColor: cp.color || '#409eff' }">
          {{ getCustomPerspectiveCount(cp.id) }}
        </span>
        <el-dropdown trigger="click" @command="(cmd) => handlePerspectiveCommand(cmd, cp)" @click.stop>
          <el-icon class="more-icon"><More /></el-icon>
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item command="edit"><el-icon><Edit /></el-icon> 编辑</el-dropdown-item>
              <el-dropdown-item command="delete" divided><el-icon><Delete /></el-icon> 删除</el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
      </div>

      <!-- 新建自定义透视 -->
      <button class="perspective-tab-pill create-btn" @click="openCreatePerspective">
        <el-icon :size="14"><Plus /></el-icon>
        <span>新建透视</span>
      </button>
    </div>

    <!-- ═══ 3. 快速自然语言捕获栏 ═══ -->
    <div class="capture-bar-wrapper">
      <div class="capture-real-box">
        <el-icon class="cap-icon" :size="16"><Plus /></el-icon>
        <input
          ref="captureInputRef"
          v-model="captureInput"
          placeholder="记下新待办… (Enter 快速保存，⌘+Enter 直达今日，支持“明天下午3点 @法庭 #张三案”)"
          class="cap-input"
          @keydown.enter="onCaptureKeydown"
        />
        <div class="cap-right-btns">
          <button v-if="captureInput.trim()" class="btn-cap-submit" @click="quickCapture(false)">
            添加待办
          </button>
        </div>
      </div>
    </div>

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
          <TaskRow
            v-for="task in cg.tasks"
            :key="task.id"
            :task="task"
            :perspective="activePerspective"
            :snooze-options="snoozeOptions"
            :resolve-case-name="getCaseName"
            :resolve-area-name="getAreaName"
            :has-children="childrenMap.has(task.id)"
            :expanded="expandedParents.has(task.id)"
            @toggle="toggleComplete"
            @open="openDrawer"
            @delete="deleteTask"
            @toggle-expand="toggleExpand"
          />
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
            @open="openDrawer"
            @delete="deleteTask"
            @toggle-expand="toggleExpand"
          />

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
          :empty-text="activePerspective === 'completed' ? '暂无已完成归档记录' : '当前视角下暂无任务，输入上方输入框快速记录'"
        />
      </div>
    </div>

    <!-- ═══ 5. 任务编辑详情抽屉 (Task Detail Drawer) ═══ -->
    <el-drawer
      v-model="showDrawer"
      title="任务详细信息"
      size="480px"
      destroy-on-close
    >
      <div v-if="editingTask" class="drawer-body">
        <div class="form-item">
          <label>任务名称</label>
          <input v-model="editForm.taskName" class="form-input" placeholder="输入任务名称..." />
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
          <button class="btn-danger-del" @click="deleteTask(editingTask)">
            <el-icon><Delete /></el-icon>
            <span>删除</span>
          </button>
          <div class="drawer-right-btns">
            <button class="btn-cancel" @click="showDrawer = false">取消</button>
            <button class="btn-primary" @click="saveTask">保存修改</button>
          </div>
        </div>
      </template>
    </el-drawer>

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
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 700;
  padding: 1px 6px;
  border-radius: 10px;
  color: #fff;
}

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
</style>
