<script setup>
import { ref, computed, onMounted, watch } from 'vue'
import { tauriCallSafe } from '../../../core/tauriBridge.js'
import { ElMessage, ElMessageBox } from 'element-plus'
import { useFiltersStore } from '../../../stores/filters'
import {
  Plus, Check, Clock, Calendar, Star, Folder,
  ArrowRight, Delete, Edit, More, RefreshRight,
  Box, List, Timer, Collection, Lock
} from '@element-plus/icons-vue'
import { useTasksStore } from '../../../stores/tasks'

// ============================================================
// 原有状态（保留）
// ============================================================
const tasks = ref([])
const loading = ref(false)
const showCreateDialog = ref(false)

// Things 3 式快速捕获输入条
const captureInput = ref('')
const capturing = ref(false)
const captureInputRef = ref(null)

const newTask = ref({
  taskName: '',
  description: '',
  deadline: '',
  priority: 'normal',
  caseId: '',
  // GTD 新增字段
  taskType: 'action',
  startDate: '',
  areaId: '',
  context: '',
})

// 任务编辑抽屉（保留）
const showDrawer = ref(false)
const editingTask = ref(null)
const editForm = ref({
  taskName: '',
  description: '',
  deadline: '',
  priority: 'normal',
  caseId: '',
  // GTD 新增字段
  taskType: 'action',
  startDate: '',
  dueDate: '',
  waitingFor: '',
  followUpDate: '',
  context: '',
  flagged: false,
  areaId: '',
  estimatedMinutes: null,
  startBucket: 'anytime',
})
const savingTask = ref(false)
const caseSearchQuery = ref('')
const caseSearchResults = ref([])
const searchingCases = ref(false)

// ============================================================
// GTD 新增状态
// ============================================================
const cases = ref([])
const areas = ref([])

// GTD 透视（纯透视工作台，无四象限模式）
const activePerspective = ref('inbox')
const tasksStore = useTasksStore()

// 厘清对话框
const showTriageDialog = ref(false)
const triagingTask = ref(null)
const triageForm = ref({
  taskType: 'action',
  caseId: '',
  areaId: '',
  startDate: '',
  dueDate: '',
  context: '',
})

// ============================================================
// 原有常量（保留）
// ============================================================
const priorityOptions = [
  { value: 'urgent_important', label: '重要紧急', color: '#f56c6c' },
  { value: 'important', label: '重要不紧急', color: '#e6a23c' },
  { value: 'urgent', label: '紧急不重要', color: '#409eff' },
  { value: 'normal', label: '普通', color: '#909399' },
]

const priorityLabels = {
  urgent_important: '重要紧急',
  important: '重要不紧急',
  urgent: '紧急不重要',
  normal: '普通',
}

// GTD 透视定义
const perspectives = [
  { key: 'inbox', label: '收件箱', icon: Box, color: '#9BA2AF', desc: '先捕获，稍后厘清' },
  { key: 'today', label: '今天', icon: Calendar, color: '#B4554F', desc: '今日聚焦' },
  { key: 'upcoming', label: '计划中', icon: Timer, color: '#3E5C9A', desc: '有 When 日期的任务' },
  { key: 'next', label: '随时', icon: ArrowRight, color: '#4C8067', desc: '按上下文分组' },
  { key: 'waiting', label: '等待', icon: Clock, color: '#B0823A', desc: '追踪委派' },
  { key: 'review', label: '回顾', icon: RefreshRight, color: '#6C6A9C', desc: 'GTD Reflect' },
  { key: 'someday', label: '某天', icon: Folder, color: '#9BA2AF', desc: '灵感池' },
]

// 任务类型选项
const taskTypeOptions = [
  { value: 'action', label: '行动' },
  { value: 'waiting', label: '等待' },
  { value: 'delegated', label: '委派' },
  { value: 'someday', label: '某天' },
]

// 上下文选项
const contextOptions = [
  { value: 'office', label: '@办公室' },
  { value: 'phone', label: '@电话' },
  { value: 'court', label: '@法院' },
  { value: 'computer', label: '@电脑' },
  { value: 'outside', label: '@外出' },
]

// ============================================================
// 计算属性
// ============================================================

// GTD 透视过滤
const gtdTasks = computed(() => {
  const today = new Date().toISOString().split('T')[0]
  
  switch (activePerspective.value) {
    case 'inbox':
      return tasks.value.filter(t => t.startBucket === 'inbox' && !t.completed)
    
    case 'next':
      return tasks.value.filter(t =>
        !t.completed &&
        t.taskType === 'action' &&
        (t.blocked === 0 || !t.caseId)
      )

    case 'upcoming':
      return tasks.value.filter(t =>
        !t.completed &&
        t.taskType === 'action' &&
        (t.startDate || t.dueDate || t.deadline)
      ).sort((a, b) => {
        const da = a.startDate || a.dueDate || a.deadline || '9999'
        const db = b.startDate || b.dueDate || b.deadline || '9999'
        return da.localeCompare(db)
      })
    
    case 'waiting':
      return tasks.value.filter(t => 
        !t.completed && t.taskType === 'waiting'
      )
    
    case 'today':
      return tasks.value.filter(t => 
        !t.completed && 
        (t.startBucket === 'today' || 
         (t.startDate && t.startDate <= today))
      ).sort((a, b) => (a.todayIndex || 0) - (b.todayIndex || 0))
    
    case 'review':
      return tasks.value.filter(t => 
        !t.completed && t.nextReviewDate && t.nextReviewDate <= today
      )
    
    case 'someday':
      return tasks.value.filter(t => 
        !t.completed && t.startBucket === 'someday'
      )
    
    default:
      return tasks.value.filter(t => !t.completed)
  }
})

// 案件分组（Headings）：随时/计划中按案件分组，其余透视单组平铺
const gtdGroups = computed(() => {
  const groups = new Map()
  for (const t of gtdTasks.value) {
    const key = t.caseId || '__none__'
    if (!groups.has(key)) {
      groups.set(key, {
        key,
        caseId: t.caseId,
        caseName: t.caseId ? getCaseName(t.caseId) || '' : '',
        tasks: [],
      })
    }
    groups.get(key).tasks.push(t)
  }
  return Array.from(groups.values()).sort((a, b) => {
    if (!a.caseId && b.caseId) return 1
    if (a.caseId && !b.caseId) return -1
    return (a.caseName || '').localeCompare(b.caseName || '', 'zh')
  })
})

const gtdSections = computed(() => {
  const persp = activePerspective.value
  if (persp === 'next' || persp === 'upcoming') {
    return gtdGroups.value
  }
  return [{ key: '__flat__', caseId: null, caseName: '', tasks: gtdTasks.value, flat: true }]
})

// GTD 统计
const gtdStats = computed(() => {
  const today = new Date().toISOString().split('T')[0]
  return {
    inbox: tasks.value.filter(t => t.startBucket === 'inbox' && !t.completed).length,
    today: tasks.value.filter(t => !t.completed && (t.startBucket === 'today' || (t.startDate && t.startDate <= today))).length,
    upcoming: tasks.value.filter(t => !t.completed && t.taskType === 'action' && (t.startDate || t.dueDate || t.deadline)).length,
    next: tasks.value.filter(t => !t.completed && t.taskType === 'action' && (t.blocked === 0 || !t.caseId)).length,
    waiting: tasks.value.filter(t => !t.completed && t.taskType === 'waiting').length,
    review: tasks.value.filter(t => !t.completed && t.nextReviewDate && t.nextReviewDate <= today).length,
    someday: tasks.value.filter(t => !t.completed && t.startBucket === 'someday').length,
  }
})

// ============================================================
// 生命周期
// ============================================================
onMounted(() => {
  loadData()
  filtersStore.loadFilters('tasks')
  // 从路由 query 恢复透视；未指定时沿用 store（首页入口可指定）
  const urlParams = new URLSearchParams(window.location.search)
  const perspective = urlParams.get('perspective')
  if (perspective) {
    activePerspective.value = perspective
  } else if (tasksStore.activePerspective) {
    activePerspective.value = tasksStore.activePerspective
  }
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
  const result = await tauriCallSafe('list_tasks', { filter: { completed: false } })
  if (result.ok) {
    tasks.value = result.data || []
  }
}

async function loadCases() {
  const result = await tauriCallSafe('list_cases', { filter: {} })
  if (result.ok) {
    cases.value = result.data || []
  }
}

async function loadAreas() {
  const result = await tauriCallSafe('list_areas', {})
  if (result.ok) {
    areas.value = result.data || []
  }
}

// ============================================================
// 原有函数（保留）
// ============================================================
// 点击圆圈直接完成/恢复，不再弹确认框
async function toggleComplete(task) {
  const result = await tauriCallSafe('toggle_task', { id: task.id })
  if (result.ok) {
    task.completed = task.completed ? 0 : 1
    ElMessage.success(task.completed ? '已完成' : '已恢复')
  }
}

async function deleteTask(task) {
  try {
    await ElMessageBox.confirm('确定删除此任务？', '确认', { type: 'warning' })
    const result = await tauriCallSafe('delete_task', { id: task.id })
    if (result.ok) {
      tasks.value = tasks.value.filter((t) => t.id !== task.id)
      ElMessage.success('已删除')
    }
  } catch {
    // 取消
  }
}

async function createTask() {
  if (!newTask.value.taskName.trim()) {
    ElMessage.warning('请输入任务名称')
    return
  }
  const data = {
    ...newTask.value,
    startBucket: 'inbox', // 默认进收件箱
  }
  const result = await tauriCallSafe('create_task', { data })
  if (result.ok) {
    ElMessage.success('任务已创建')
    showCreateDialog.value = false
    newTask.value = { 
      taskName: '', description: '', deadline: '', priority: 'normal', caseId: '',
      taskType: 'action', startDate: '', areaId: '', context: ''
    }
    await loadTasks()
  }
}

function openDrawer(task) {
  editingTask.value = task
  editForm.value = {
    taskName: task.taskName || '',
    description: task.description || '',
    deadline: task.deadline || '',
    priority: task.priority || 'normal',
    caseId: task.caseId || '',
    // GTD 字段
    taskType: task.taskType || 'action',
    startDate: task.startDate || '',
    dueDate: task.dueDate || task.deadline || '',
    waitingFor: task.waitingFor || '',
    followUpDate: task.followUpDate || '',
    context: task.context || '',
    flagged: task.flagged === 1,
    areaId: task.areaId || '',
    estimatedMinutes: task.estimatedMinutes || null,
    startBucket: task.startBucket || 'anytime',
  }
  caseSearchQuery.value = ''
  caseSearchResults.value = []
  showDrawer.value = true
}

async function saveTask() {
  if (!editForm.value.taskName.trim()) {
    ElMessage.warning('请输入任务名称')
    return
  }
  savingTask.value = true
  const data = {
    id: editingTask.value.id,
    ...editForm.value,
    flagged: editForm.value.flagged ? 1 : 0,
  }
  const result = await tauriCallSafe('update_task', { data })
  savingTask.value = false
  if (result.ok) {
    ElMessage.success('任务已更新')
    showDrawer.value = false
    await loadTasks()
  }
}

async function searchCases(query) {
  if (!query || query.trim().length < 1) {
    caseSearchResults.value = []
    return
  }
  searchingCases.value = true
  const result = await tauriCallSafe('search_cases', { query: query.trim(), limit: 10 })
  searchingCases.value = false
  if (result.ok) {
    caseSearchResults.value = result.data || []
  }
}

function selectCase(caseItem) {
  editForm.value.caseId = caseItem.id
  caseSearchQuery.value = caseItem.caseName || caseItem.case_name || ''
  caseSearchResults.value = []
}

function clearCaseSelection() {
  editForm.value.caseId = ''
  caseSearchQuery.value = ''
  caseSearchResults.value = []
}

function daysUntil(deadline) {
  if (!deadline) return null
  const d = new Date(deadline)
  const today = new Date()
  today.setHours(0, 0, 0, 0)
  return Math.ceil((d - today) / (1000 * 60 * 60 * 24))
}

function isOverdue(deadline) {
  const days = daysUntil(deadline)
  return days !== null && days < 0
}

// ============================================================
// GTD 新增函数
// ============================================================
// 切换透视
function switchPerspective(key) {
  activePerspective.value = key
  // 同步到 store（首页入口可直达指定透视）
  tasksStore.activePerspective = key
  // 更新 URL
  const url = new URL(window.location)
  window.history.replaceState({}, '', url)
}

// ============================================================
// 已保存筛选（设计哲学 §9：视图状态可保存复用，entity_type='tasks'）
// ============================================================
const filtersStore = useFiltersStore()
const savedFilters = computed(() => filtersStore.filters)

async function saveCurrentFilter() {
  let name = ''
  try {
    const { value } = await ElMessageBox.prompt('为当前视图状态命名', '保存筛选', {
      confirmButtonText: '保存',
      cancelButtonText: '取消',
      inputPlaceholder: '如：GTD-等待、四象限总览',
    })
    name = (value || '').trim()
  } catch {
    return // 用户取消
  }
  if (!name) return
  const result = await filtersStore.saveFilter({
    module: 'tasks',
    name,
    filter: { perspective: activePerspective.value },
  })
  if (result.ok) {
    ElMessage.success('筛选已保存')
  } else {
    ElMessage.error(result.error || '保存失败')
  }
}

function applySavedFilter(sf) {
  const f = sf.filter || {}
  // 历史筛选可能带旧 viewMode 字段：一律收敛到 GTD 透视
  if (f.perspective && f.perspective !== activePerspective.value) switchPerspective(f.perspective)
}

async function deleteSavedFilter(sf) {
  const result = await filtersStore.deleteFilter(sf.id)
  if (result.ok) {
    ElMessage.success('已删除')
  } else {
    ElMessage.error(result.error || '删除失败')
  }
}

// 厘清任务
function openTriage(task) {
  triagingTask.value = task
  triageForm.value = {
    taskType: task.taskType || 'action',
    caseId: task.caseId || '',
    areaId: task.areaId || '',
    startDate: task.startDate || '',
    dueDate: task.dueDate || task.deadline || '',
    context: task.context || '',
  }
  showTriageDialog.value = true
}

async function submitTriage() {
  if (!triagingTask.value) return
  
  const data = {
    id: triagingTask.value.id,
    taskType: triageForm.value.taskType,
    caseId: triageForm.value.caseId || null,
    areaId: triageForm.value.areaId || null,
    startDate: triageForm.value.startDate || null,
    dueDate: triageForm.value.dueDate || null,
    context: triageForm.value.context || null,
    startBucket: triageForm.value.taskType === 'someday' ? 'someday' : 'anytime',
  }
  
  const result = await tauriCallSafe('update_task', { data })
  if (result.ok) {
    ElMessage.success('已厘清')
    showTriageDialog.value = false
    await loadTasks()
  }
}

// 移动到今日
async function moveToToday(task) {
  const result = await tauriCallSafe('update_task', {
    data: {
      id: task.id,
      startBucket: 'today',
      todayIndex: gtdStats.value.today,
    }
  })
  if (result.ok) {
    ElMessage.success('已移至今日')
    await loadTasks()
  }
}

// 标记为等待
async function markAsWaiting(task) {
  const result = await tauriCallSafe('update_task', {
    data: {
      id: task.id,
      taskType: 'waiting',
    }
  })
  if (result.ok) {
    ElMessage.success('已标记为等待')
    await loadTasks()
  }
}

// 获取案件名称
function getCaseName(caseId) {
  const c = cases.value.find(c => c.id === caseId)
  return c ? c.caseName : ''
}

// 获取领域名称
function getAreaName(areaId) {
  const a = areas.value.find(a => a.id === areaId)
  return a ? a.name : ''
}

// 获取任务类型标签
function getTaskTypeLabel(type) {
  const labels = { action: '行动', waiting: '等待', delegated: '委派', someday: '某天' }
  return labels[type] || type
}

// 获取任务类型颜色
function getTaskTypeColor(type) {
  const colors = { action: '#409EFF', waiting: '#E6A23C', delegated: '#909399', someday: '#909399' }
  return colors[type] || '#909399'
}

// 格式化日期
function formatDate(dateStr) {
  if (!dateStr) return ''
  const date = new Date(dateStr)
  const today = new Date()
  const tomorrow = new Date(today)
  tomorrow.setDate(tomorrow.getDate() + 1)
  
  if (dateStr === today.toISOString().split('T')[0]) return '今天'
  if (dateStr === tomorrow.toISOString().split('T')[0]) return '明天'
  
  return date.toLocaleDateString('zh-CN', { month: 'short', day: 'numeric' })
}

// 计算等待天数
function getWaitingDays(task) {
  if (!task.followUpDate) return 0
  const today = new Date()
  const followUp = new Date(task.followUpDate)
  return Math.ceil((today - followUp) / (1000 * 60 * 60 * 24))
}

// 催办功能
function openFollowUp(task) {
  // 打开编辑抽屉，聚焦到跟进日期
  openDrawer(task)
  // 可以设置一个标志，让编辑抽屉知道是催办操作
  // 这里简单处理，直接打开编辑抽屉
}

// ============================================================
// Things 3 式快速捕获（自然语言解析）
// ============================================================
function toDateStr(d) {
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return y + '-' + m + '-' + day
}

function todayStr() {
  return toDateStr(new Date())
}

// 解析快速捕获文本：识别开头的日期词（今天/明天/后天/周X/下周X/MM-DD）
// 与时间词（HH:MM / X点 / X点半），解析为 startDate/dueDate
// 返回 { taskName, startDate, dueDate, startBucket }；解析失败时 startDate 为 null
function parseCaptureText(raw) {
  let text = (raw || '').trim()
  if (!text) return null
  const now = new Date()
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate())
  let parsedDate = null

  // 1) 相对日期词（必须出现在开头）
  const rel = text.match(/^(今天|明天|后天)/)
  if (rel) {
    const offset = rel[1] === '今天' ? 0 : rel[1] === '明天' ? 1 : 2
    const d = new Date(today)
    d.setDate(d.getDate() + offset)
    parsedDate = toDateStr(d)
    text = text.slice(rel[1].length).trim()
  } else {
    // 2) 周X / 下周X（本周或下周的最近一次）
    const week = text.match(/^下周?([一二三四五六日天])/)
    if (week) {
      const wd = { 一: 1, 二: 2, 三: 3, 四: 4, 五: 5, 六: 6, 日: 0, 天: 0 }[week[1]]
      const delta = (wd - today.getDay() + 7) % 7
      const d = new Date(today)
      d.setDate(d.getDate() + delta + (week[0].startsWith('下周') ? 7 : 0))
      parsedDate = toDateStr(d)
      text = text.slice(week[0].length).trim()
    } else {
      // 3) MM-DD（今年内已过则顺延到明年）
      const md = text.match(/^(\d{1,2})[-/](\d{1,2})/)
      if (md) {
        const m = parseInt(md[1], 10)
        const day = parseInt(md[2], 10)
        let d = new Date(today.getFullYear(), m - 1, day)
        if (d < today) d = new Date(today.getFullYear() + 1, m - 1, day)
        parsedDate = toDateStr(d)
        text = text.slice(md[0].length).trim()
      }
    }
  }

  // 4) 时间词：HH:MM 或 X点/X点半（仅识别并从前缀剥离；任务无时间字段）
  text = text.replace(/^(\d{1,2}[:：]\d{2}|[0-9一二两三四五六七八九十]{1,3}点(半|整)?)\s*/, '')
  if (!text) text = raw.trim()

  const isToday = parsedDate === todayStr()
  return {
    taskName: text,
    startDate: parsedDate,
    dueDate: parsedDate,
    startBucket: !parsedDate ? 'inbox' : isToday ? 'today' : 'upcoming',
  }
}

// 回车快速捕获；Ctrl/Cmd+回车直接进今日
async function quickCapture(forceToday = false) {
  const text = captureInput.value.trim()
  if (!text || capturing.value) return
  const parsed = parseCaptureText(text)
  capturing.value = true
  let data
  if (forceToday) {
    data = {
      taskName: parsed ? parsed.taskName : text,
      startDate: todayStr(),
      dueDate: todayStr(),
      startBucket: 'today',
      taskType: 'action',
    }
  } else if (parsed && parsed.startDate) {
    data = {
      taskName: parsed.taskName,
      startDate: parsed.startDate,
      dueDate: parsed.dueDate,
      startBucket: parsed.startBucket,
      taskType: 'action',
    }
  } else {
    // 解析失败或仅时间词：标题=原文，进收件箱
    data = {
      taskName: text,
      startBucket: 'inbox',
      taskType: 'action',
    }
  }
  const result = await tauriCallSafe('create_task', { data })
  capturing.value = false
  if (result.ok) {
    ElMessage.success('已捕获')
    captureInput.value = ''
    await loadTasks()
  }
}

function onCaptureKeydown(e) {
  e.preventDefault()
  if (e.ctrlKey || e.metaKey) quickCapture(true)
  else quickCapture(false)
}

// ============================================================
// Review 回顾闭环
// ============================================================
// 下周日的日期（今天为周日则取 7 天后）
function nextSundayStr() {
  const d = new Date()
  const day = d.getDay()
  const diff = (7 - day) % 7 || 7
  const target = new Date(d.getFullYear(), d.getMonth(), d.getDate() + diff)
  return toDateStr(target)
}

// 标记已回顾：lastReviewDate=今天，nextReviewDate=下周日
async function markReviewed(task) {
  const result = await tauriCallSafe('update_task', {
    data: {
      id: task.id,
      lastReviewDate: todayStr(),
      nextReviewDate: nextSundayStr(),
    },
  })
  if (result.ok) {
    ElMessage.success('已标记回顾')
    await loadTasks()
  }
}

// ============================================================
// 快捷键
// ============================================================
function handleKeydown(e) {
  // Ctrl/Cmd+T: 聚焦快速捕获输入框
  if ((e.metaKey || e.ctrlKey) && e.key === 't') {
    e.preventDefault()
    if (captureInputRef.value) captureInputRef.value.focus()
  }
}

onMounted(() => {
  document.addEventListener('keydown', handleKeydown)
})

import { onUnmounted } from 'vue'
onUnmounted(() => {
  document.removeEventListener('keydown', handleKeydown)
})
</script>

<template>
  <div class="tasks-page">
    <!-- 工具栏 -->
    <div class="toolbar">
      <div class="toolbar-left">
        <h3>任务管理</h3>
        <span class="shortcut-hint">Ctrl/Cmd+T 快速捕获</span>
      </div>
      
      <div class="toolbar-right">
        <!-- 已保存筛选（设计哲学 §9） -->
        <el-dropdown v-if="savedFilters.length > 0" trigger="click">
          <el-button size="small">
            已存筛选
          </el-button>
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item v-for="sf in savedFilters" :key="sf.id">
                <div class="saved-filter-item" @click="applySavedFilter(sf)">
                  <span>{{ sf.name }}</span>
                  <el-button size="small" type="danger" text @click.stop="deleteSavedFilter(sf)">删除</el-button>
                </div>
              </el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
        <el-button size="small" text type="primary" @click="saveCurrentFilter">保存筛选</el-button>

        <el-button type="primary" size="small" @click="showCreateDialog = true">
          <el-icon><Plus /></el-icon>
          新建任务
        </el-button>
      </div>
    </div>

    <!-- Things 3 式快速捕获条（常驻，无弹窗） -->
    <div class="capture-bar">
      <el-input
        ref="captureInputRef"
        v-model="captureInput"
        placeholder="快速捕获：输入任务，回车进收件箱，Ctrl/Cmd+回车进今日"
        clearable
        :disabled="capturing"
        @keydown.enter="onCaptureKeydown"
      >
        <template #prefix>
          <el-icon><Plus /></el-icon>
        </template>
      </el-input>
      <div class="capture-hint">
        <span>回车快速捕获</span>
        <span class="capture-hint-divider">·</span>
        <span>Ctrl/Cmd+回车直接进今日</span>
      </div>
    </div>

    <!-- GTD 透视标签（唯一视图模式） -->
    <div class="perspective-tabs">
      <div
        v-for="p in perspectives"
        :key="p.key"
        :class="['tab-item', { active: activePerspective === p.key }]"
        @click="switchPerspective(p.key)"
      >
        <el-icon :size="14"><component :is="p.icon" /></el-icon>
        <span class="tab-label">{{ p.label }}</span>
        <span class="tab-count" :style="{ backgroundColor: p.color }">
          {{ gtdStats[p.key] }}
        </span>
      </div>
    </div>
    <!-- 加载骨架 -->
    <!-- 加载骨架 -->
    <div v-if="loading" class="skeleton-wrapper">
      <el-skeleton :rows="6" animated>
        <template #template>
          <div class="skeleton-rows">
            <div v-for="i in 6" :key="i" class="skeleton-task-row">
              <el-skeleton-item variant="circle" style="width: 18px; height: 18px;" />
              <el-skeleton-item variant="text" style="width: 60%; height: 16px;" />
              <el-skeleton-item variant="text" style="width: 20%; height: 16px;" />
            </div>
          </div>
        </template>
      </el-skeleton>
    </div>

    <!-- GTD 透视视图（唯一视图，无四象限） -->
    <div v-else class="gtd-view">
      <!-- 空状态 -->
      <div v-if="gtdTasks.length === 0" class="empty-state">
        <el-icon :size="48" color="#C0C4CC"><component :is="perspectives.find(p => p.key === activePerspective)?.icon || List" /></el-icon>
        <p>{{ activePerspective === 'inbox' ? '收件箱为空，一切井然有序' : '暂无任务' }}</p>
        <el-button v-if="activePerspective === 'inbox'" type="primary" @click="showCreateDialog = true">
          捕获第一个任务
        </el-button>
      </div>

      <!-- 任务列表：随时/计划中按案件分组（Headings），其余透视平铺 -->
      <template v-else>
        <div v-for="section in gtdSections" :key="section.key" class="task-group">
          <!-- 分组小节标题 -->
          <div v-if="!section.flat" class="group-header">
            <el-icon :size="13"><Folder /></el-icon>
            <span class="group-name">{{ section.caseName || '无案件' }}</span>
            <span class="group-count">{{ section.tasks.length }}</span>
          </div>

          <div class="task-list">
            <div
              v-for="task in section.tasks"
              :key="task.id"
              :class="['task-card', {
                overdue: isOverdue(task.dueDate || task.deadline),
                'due-soon': task.dueSoon === 1,
                flagged: task.flagged === 1,
                blocked: task.blocked === 1
              }]"
            >
              <!-- 完成圆圈：点击直接完成/恢复 -->
              <div
                class="task-check"
                :class="{ done: task.completed === 1 }"
                @click="toggleComplete(task)"
              >
                <el-icon v-if="task.completed === 1"><Check /></el-icon>
              </div>

              <!-- 任务内容 -->
              <div class="task-content" @click="openDrawer(task)">
                <div class="task-title">
                  <span>{{ task.taskName }}</span>
                  <el-tag 
                    v-if="task.taskType !== 'action'" 
                    :color="getTaskTypeColor(task.taskType)"
                    size="small"
                    effect="dark"
                  >
                    {{ getTaskTypeLabel(task.taskType) }}
                  </el-tag>
                </div>
                
                <div class="task-meta">
                  <!-- 案件关联 -->
                  <span v-if="task.caseId" class="meta-item case">
                    <el-icon><Folder /></el-icon>
                    {{ getCaseName(task.caseId) }}
                  </span>
                  
                  <!-- 领域 -->
                  <span v-if="task.areaId" class="meta-item area">
                    <el-icon><Collection /></el-icon>
                    {{ getAreaName(task.areaId) }}
                  </span>
                  
                  <!-- 旗标 -->
                  <span v-if="task.flagged === 1" class="meta-item flagged">
                    <el-icon color="#F59E0B"><Star /></el-icon>
                  </span>
                  
                  <!-- 截止日期 -->
                  <span v-if="task.dueDate || task.deadline" class="meta-item deadline" :class="{ overdue: isOverdue(task.dueDate || task.deadline) }">
                    <el-icon><Calendar /></el-icon>
                    {{ formatDate(task.dueDate || task.deadline) }}
                  </span>
                  
                  <!-- 预计耗时 -->
                  <span v-if="task.estimatedMinutes" class="meta-item estimated">
                    <el-icon><Timer /></el-icon>
                    {{ task.estimatedMinutes }}分钟
                  </span>
                  
                  <!-- 等待信息 -->
                  <span v-if="task.taskType === 'waiting' && task.waitingFor" class="meta-item waiting" :class="{ 'waiting-warning': getWaitingDays(task) > 3 }">
                    <el-icon><Clock /></el-icon>
                    等 {{ task.waitingFor }}
                    <span v-if="getWaitingDays(task) > 0" class="waiting-days">
                      ({{ getWaitingDays(task) }}天)
                    </span>
                    <el-button 
                      v-if="getWaitingDays(task) > 3" 
                      size="small" 
                      type="warning" 
                      plain 
                      class="follow-up-btn"
                      @click.stop="openFollowUp(task)"
                    >
                      催办
                    </el-button>
                  </span>
                  
                  <!-- 上下文 -->
                  <span v-if="task.context" class="meta-item context">
                    @{{ task.context }}
                  </span>
                  
                  <!-- 顺序项目锁定 -->
                  <span v-if="task.blocked === 1" class="meta-item blocked">
                    <el-icon><Lock /></el-icon>
                    已锁定
                  </span>
                </div>
              </div>

              <!-- 操作按钮 -->
              <div class="task-actions">
                <!-- Review 回顾闭环：标记已回顾（下周日再回顾） -->
                <template v-if="activePerspective === 'review'">
                  <span v-if="task.nextReviewDate" class="review-info">下次回顾 {{ task.nextReviewDate }}</span>
                  <el-button
                    size="small"
                    type="primary"
                    plain
                    class="review-btn"
                    @click="markReviewed(task)"
                  >
                    已回顾
                  </el-button>
                </template>
                <el-dropdown trigger="click">
                  <el-button :icon="More" circle size="small" />
                  <template #dropdown>
                    <el-dropdown-menu>
                      <el-dropdown-item @click="openDrawer(task)" :icon="Edit">
                        编辑
                      </el-dropdown-item>
                      <el-dropdown-item 
                        v-if="activePerspective === 'inbox'" 
                        @click="openTriage(task)" 
                        :icon="ArrowRight"
                      >
                        厘清
                      </el-dropdown-item>
                      <el-dropdown-item 
                        v-if="activePerspective !== 'today'" 
                        @click="moveToToday(task)" 
                        :icon="Calendar"
                      >
                        移至今日
                      </el-dropdown-item>
                      <el-dropdown-item 
                        v-if="task.taskType !== 'waiting'" 
                        @click="markAsWaiting(task)" 
                        :icon="Clock"
                      >
                        标记等待
                      </el-dropdown-item>
                      <el-dropdown-item 
                        @click="deleteTask(task)" 
                        :icon="Delete"
                        divided
                      >
                        删除
                      </el-dropdown-item>
                    </el-dropdown-menu>
                  </template>
                </el-dropdown>
              </div>
            </div>
          </div>
        </div>
      </template>
    </div>

    <!-- 新建任务弹窗（增强版） -->
    <el-dialog v-model="showCreateDialog" title="新建任务" width="500">
      <el-form label-width="80px" size="small">
        <el-form-item label="任务名称" required>
          <el-input v-model="newTask.taskName" placeholder="记下你脑中的想法..." />
        </el-form-item>
        <el-form-item label="描述">
          <el-input v-model="newTask.description" type="textarea" :rows="2" />
        </el-form-item>
        
        <div style="display: flex; gap: 12px;">
          <el-form-item label="任务类型" style="flex: 1;">
            <el-select v-model="newTask.taskType" style="width: 100%">
              <el-option v-for="opt in taskTypeOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
            </el-select>
          </el-form-item>
          <el-form-item label="优先级" style="flex: 1;">
            <el-select v-model="newTask.priority" style="width: 100%">
              <el-option v-for="opt in priorityOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
            </el-select>
          </el-form-item>
        </div>
        
        <div style="display: flex; gap: 12px;">
          <el-form-item label="开始日期" style="flex: 1;">
            <el-date-picker v-model="newTask.startDate" type="date" value-format="YYYY-MM-DD" style="width: 100%" />
          </el-form-item>
          <el-form-item label="截止日期" style="flex: 1;">
            <el-date-picker v-model="newTask.deadline" type="date" value-format="YYYY-MM-DD" style="width: 100%" />
          </el-form-item>
        </div>
        
        <div style="display: flex; gap: 12px;">
          <el-form-item label="关联案件" style="flex: 1;">
            <el-select v-model="newTask.caseId" placeholder="选择案件" clearable filterable style="width: 100%">
              <el-option v-for="c in cases" :key="c.id" :label="c.caseName" :value="c.id" />
            </el-select>
          </el-form-item>
          <el-form-item label="领域" style="flex: 1;">
            <el-select v-model="newTask.areaId" placeholder="选择领域" clearable style="width: 100%">
              <el-option v-for="a in areas" :key="a.id" :label="a.name" :value="a.id" />
            </el-select>
          </el-form-item>
        </div>
        
        <el-form-item label="上下文">
          <el-select v-model="newTask.context" placeholder="选择上下文" clearable style="width: 100%">
            <el-option v-for="opt in contextOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
          </el-select>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showCreateDialog = false">取消</el-button>
        <el-button type="primary" @click="createTask">创建</el-button>
      </template>
    </el-dialog>

    <!-- 厘清对话框 -->
    <el-dialog v-model="showTriageDialog" title="厘清任务" width="500" :close-on-click-modal="false">
      <!-- 任务预览 -->
      <div v-if="triagingTask" class="triage-preview">
        <div class="triage-preview-header">
          <strong>{{ triagingTask.taskName }}</strong>
          <span v-if="triagingTask.flagged === 1" class="triage-flagged">
            <el-icon color="#F59E0B"><Star /></el-icon>
          </span>
        </div>
        <p v-if="triagingTask.description" class="triage-description">{{ triagingTask.description }}</p>
        <div class="triage-meta">
          <span v-if="triagingTask.deadline" class="triage-deadline">
            <el-icon><Calendar /></el-icon>
            {{ formatDate(triagingTask.deadline) }}
          </span>
          <span v-if="triagingTask.estimatedMinutes" class="triage-estimated">
            <el-icon><Timer /></el-icon>
            {{ triagingTask.estimatedMinutes }}分钟
          </span>
        </div>
      </div>
      
      <el-divider />
      
      <el-form :model="triageForm" label-position="top" size="small">
        <el-form-item label="这是什么？">
          <div class="triage-type-cards">
            <div 
              v-for="opt in taskTypeOptions" 
              :key="opt.value"
              :class="['triage-type-card', { active: triageForm.taskType === opt.value }]"
              @click="triageForm.taskType = opt.value"
            >
              <div class="type-icon">
                <el-icon v-if="opt.value === 'action'"><ArrowRight /></el-icon>
                <el-icon v-else-if="opt.value === 'waiting'"><Clock /></el-icon>
                <el-icon v-else-if="opt.value === 'delegated'"><Folder /></el-icon>
                <el-icon v-else><Folder /></el-icon>
              </div>
              <div class="type-label">{{ opt.label }}</div>
              <div class="type-desc">
                <span v-if="opt.value === 'action'">需要我亲自做</span>
                <span v-else-if="opt.value === 'waiting'">等待他人完成</span>
                <span v-else-if="opt.value === 'delegated'">已委派给他人</span>
                <span v-else>暂不处理</span>
              </div>
            </div>
          </div>
        </el-form-item>
        
        <div style="display: flex; gap: 12px;">
          <el-form-item label="关联案件" style="flex: 1;">
            <el-select v-model="triageForm.caseId" placeholder="选择案件" clearable filterable style="width: 100%">
              <el-option v-for="c in cases" :key="c.id" :label="c.caseName" :value="c.id" />
            </el-select>
          </el-form-item>
          <el-form-item label="领域" style="flex: 1;">
            <el-select v-model="triageForm.areaId" placeholder="选择领域" clearable style="width: 100%">
              <el-option v-for="a in areas" :key="a.id" :label="a.name" :value="a.id" />
            </el-select>
          </el-form-item>
        </div>
        
        <div style="display: flex; gap: 12px;">
          <el-form-item label="开始日期" style="flex: 1;">
            <el-date-picker v-model="triageForm.startDate" type="date" value-format="YYYY-MM-DD" style="width: 100%" />
          </el-form-item>
          <el-form-item label="截止日期" style="flex: 1;">
            <el-date-picker v-model="triageForm.dueDate" type="date" value-format="YYYY-MM-DD" style="width: 100%" />
          </el-form-item>
        </div>
        
        <el-form-item label="上下文">
          <el-select v-model="triageForm.context" placeholder="选择上下文" clearable style="width: 100%">
            <el-option v-for="opt in contextOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
          </el-select>
        </el-form-item>
      </el-form>
      
      <template #footer>
        <el-button @click="showTriageDialog = false">取消</el-button>
        <el-button type="primary" @click="submitTriage">确认</el-button>
      </template>
    </el-dialog>

    <!-- 任务编辑抽屉（增强版） -->
    <el-drawer v-model="showDrawer" title="编辑任务" direction="rtl" size="480px">
      <el-form label-width="80px" size="small">
        <el-form-item label="任务名称" required>
          <el-input v-model="editForm.taskName" />
        </el-form-item>
        <el-form-item label="描述">
          <el-input v-model="editForm.description" type="textarea" :rows="3" />
        </el-form-item>
        
        <div style="display: flex; gap: 12px;">
          <el-form-item label="任务类型" style="flex: 1;">
            <el-select v-model="editForm.taskType" style="width: 100%">
              <el-option v-for="opt in taskTypeOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
            </el-select>
          </el-form-item>
          <el-form-item label="优先级" style="flex: 1;">
            <el-select v-model="editForm.priority" style="width: 100%">
              <el-option v-for="opt in priorityOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
            </el-select>
          </el-form-item>
        </div>
        
        <div style="display: flex; gap: 12px;">
          <el-form-item label="开始日期" style="flex: 1;">
            <el-date-picker v-model="editForm.startDate" type="date" value-format="YYYY-MM-DD" style="width: 100%" />
          </el-form-item>
          <el-form-item label="截止日期" style="flex: 1;">
            <el-date-picker v-model="editForm.dueDate" type="date" value-format="YYYY-MM-DD" style="width: 100%" />
          </el-form-item>
        </div>
        
        <div v-if="editForm.taskType === 'waiting'" style="display: flex; gap: 12px;">
          <el-form-item label="等待谁" style="flex: 1;">
            <el-input v-model="editForm.waitingFor" placeholder="法院/对方/客户" />
          </el-form-item>
          <el-form-item label="跟进日期" style="flex: 1;">
            <el-date-picker v-model="editForm.followUpDate" type="date" value-format="YYYY-MM-DD" style="width: 100%" />
          </el-form-item>
        </div>
        
        <div style="display: flex; gap: 12px;">
          <el-form-item label="关联案件" style="flex: 1;">
            <el-select v-model="editForm.caseId" placeholder="选择案件" clearable filterable style="width: 100%">
              <el-option v-for="c in cases" :key="c.id" :label="c.caseName" :value="c.id" />
            </el-select>
          </el-form-item>
          <el-form-item label="领域" style="flex: 1;">
            <el-select v-model="editForm.areaId" placeholder="选择领域" clearable style="width: 100%">
              <el-option v-for="a in areas" :key="a.id" :label="a.name" :value="a.id" />
            </el-select>
          </el-form-item>
        </div>
        
        <div style="display: flex; gap: 12px;">
          <el-form-item label="上下文" style="flex: 1;">
            <el-select v-model="editForm.context" placeholder="选择上下文" clearable style="width: 100%">
              <el-option v-for="opt in contextOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
            </el-select>
          </el-form-item>
          <el-form-item label="预估时间" style="flex: 1;">
            <el-input-number v-model="editForm.estimatedMinutes" :min="0" :max="480" placeholder="分钟" style="width: 100%" />
          </el-form-item>
        </div>
        
        <el-form-item label="时间桶">
          <el-select v-model="editForm.startBucket" style="width: 100%">
            <el-option label="收件箱" value="inbox" />
            <el-option label="随时" value="anytime" />
            <el-option label="今日" value="today" />
            <el-option label="某天" value="someday" />
          </el-select>
        </el-form-item>
        
        <el-form-item>
          <el-checkbox v-model="editForm.flagged">标记为重要</el-checkbox>
        </el-form-item>
      </el-form>
      
      <template #footer>
        <el-button @click="showDrawer = false">取消</el-button>
        <el-button type="primary" :loading="savingTask" @click="saveTask">保存</el-button>
      </template>
    </el-drawer>
  </div>
</template>

<style scoped>
.tasks-page {
  padding: 16px;
}

.toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
}

.toolbar-left {
  display: flex;
  align-items: center;
  gap: 12px;
}

.toolbar-left h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
}

.shortcut-hint {
  font-size: 11px;
  color: #A1A1AA;
  background: #F4F4F5;
  padding: 2px 6px;
  border-radius: 4px;
}

.toolbar-right {
  display: flex;
  align-items: center;
  gap: 12px;
}

/* 已保存筛选下拉项 */
.saved-filter-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  min-width: 140px;
}

/* GTD 透视标签 */
.perspective-tabs {
  display: flex;
  gap: 6px;
  margin-bottom: 16px;
  padding: 6px;
  background: #FFFFFF;
  border-radius: 8px;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
}

.tab-item {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 6px 12px;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s;
  color: #52525B;
  font-size: 13px;
}

.tab-item:hover {
  background: #F4F4F5;
}

.tab-item.active {
  background: #EFF6FF;
  color: #2563EB;
}

.tab-label {
  font-weight: 500;
}

.tab-count {
  font-size: 11px;
  color: #FFFFFF;
  padding: 1px 5px;
  border-radius: 8px;
  min-width: 16px;
  text-align: center;
}

/* 骨架 */
.skeleton-wrapper {
  margin-top: 16px;
}

.skeleton-rows {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.skeleton-task-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}

/* 快速捕获条（Things 3 式） */
.capture-bar {
  margin-bottom: 12px;
}

.capture-bar .el-input__wrapper {
  border-radius: 8px;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05), 0 0 0 1px #E4E7ED inset;
}

.capture-bar .el-input__wrapper.is-focus {
  box-shadow: 0 0 0 1px #3E5C9A inset, 0 1px 2px rgba(0, 0, 0, 0.05);
}

.capture-hint {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 6px;
  font-size: 11px;
  color: #9BA2AF;
  padding-left: 4px;
}

.capture-hint-divider {
  color: #D4D4D8;
}

/* GTD 分组小节（Headings） */
.task-group {
  margin-bottom: 16px;
}

.group-header {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 8px;
  margin-bottom: 8px;
  font-size: 12px;
  font-weight: 600;
  color: #3E5C9A;
  border-bottom: 1px solid #EEF0F4;
}

.group-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.group-count {
  font-size: 11px;
  font-weight: 500;
  color: #9BA2AF;
  background: #F4F4F5;
  padding: 1px 6px;
  border-radius: 8px;
}

.task-card {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px;
  border-radius: 4px;
  margin-bottom: 6px;
  background: #fff;
  transition: background 0.15s;
}

.task-card:hover {
  background: #f5f7fa;
}

.task-card.flagged {
  background: #FFFBEB;
}

/* GTD 视图 */
.gtd-view {
  min-height: 400px;
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 300px;
  color: #A1A1AA;
}

.empty-state p {
  margin: 12px 0;
  font-size: 14px;
}

.task-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.task-list .task-card {
  background: #FFFFFF;
  border: 1px solid #E4E7ED;
  border-left: 3px solid #E5E7EB;
  border-radius: 8px;
  padding: 12px 16px;
}

.task-list .task-card:hover {
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
}

.task-list .task-card.overdue {
  border-left: 3px solid #EF4444;
  background: #FEF2F2;
}

.task-list .task-card.due-soon {
  border-left: 3px solid #F59E0B;
}

.task-list .task-card.flagged {
  background: #FFFBEB;
}

.task-list .task-card.blocked {
  border-left: 3px solid #9BA2AF;
  opacity: 0.85;
}

.task-check {
  width: 18px;
  height: 18px;
  border-radius: 50%;
  border: 2px solid #C0C4CC;
  background: transparent;
  cursor: pointer;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  box-sizing: border-box;
  transition: all 0.15s;
}

.task-check .el-icon {
  font-size: 11px;
  color: #FFFFFF;
}

.task-check:hover {
  border-color: #4C8067;
}

.task-check.done {
  background: #4C8067;
  border-color: #4C8067;
}

.task-content {
  flex: 1;
  min-width: 0;
  cursor: pointer;
}

.task-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 14px;
  font-weight: 500;
  color: #18181B;
  margin-bottom: 4px;
}

.task-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  font-size: 12px;
  color: #A1A1AA;
}

.meta-item {
  display: flex;
  align-items: center;
  gap: 3px;
}

.meta-item.case { color: #409EFF; }
.meta-item.area { color: #67C23A; }
.meta-item.deadline.overdue { color: #F56C6C; }
.meta-item.waiting { color: #E6A23C; }
.meta-item.flagged { color: #F59E0B; }
.meta-item.estimated { color: #6B7280; }
.meta-item.blocked { color: #9BA2AF; }

.waiting-days {
  color: #F56C6C;
}

.waiting-warning {
  color: #F59E0B;
  font-weight: 500;
}

.follow-up-btn {
  margin-left: 8px;
  font-size: 11px;
  padding: 2px 6px;
}

.meta-item.context {
  color: #909399;
  background: #F4F4F5;
  padding: 1px 5px;
  border-radius: 3px;
}

.task-actions {
  flex-shrink: 0;
}

/* Review 回顾闭环 */
.review-info {
  font-size: 11px;
  color: #9BA2AF;
  margin-right: 8px;
  white-space: nowrap;
}

.review-btn {
  margin-right: 8px;
  font-size: 12px;
  padding: 4px 10px;
  --el-button-border-color: #6C6A9C;
  --el-button-text-color: #6C6A9C;
  --el-button-hover-border-color: #6C6A9C;
  --el-button-hover-text-color: #FFFFFF;
  --el-button-hover-bg-color: #6C6A9C;
}
/* 厘清预览 */
.triage-preview {
  padding: 16px;
  background: #F4F4F5;
  border-radius: 8px;
}

.triage-preview-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}

.triage-preview-header strong {
  flex: 1;
  font-size: 15px;
  color: #18181B;
}

.triage-flagged {
  color: #F59E0B;
}

.triage-description {
  margin: 0 0 8px;
  font-size: 13px;
  color: #52525B;
}

.triage-meta {
  display: flex;
  gap: 12px;
  font-size: 12px;
  color: #A1A1AA;
}

.triage-deadline,
.triage-estimated {
  display: flex;
  align-items: center;
  gap: 4px;
}

/* 厘清类型卡片 */
.triage-type-cards {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 12px;
}

.triage-type-card {
  padding: 12px;
  border: 2px solid #E4E7ED;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s;
  text-align: center;
}

.triage-type-card:hover {
  border-color: #409EFF;
  background: #F0F7FF;
}

.triage-type-card.active {
  border-color: #409EFF;
  background: #EFF6FF;
}

.type-icon {
  margin-bottom: 8px;
  color: #409EFF;
}

.type-label {
  font-size: 14px;
  font-weight: 500;
  color: #18181B;
  margin-bottom: 4px;
}

.type-desc {
  font-size: 12px;
  color: #A1A1AA;
}
</style>
