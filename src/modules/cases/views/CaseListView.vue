<script setup>
import { taskPlanLabel } from '../../../shared/utils/taskSchedule'
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useCasesStore } from '../../../stores/cases'
import { useTasksStore } from '../../../stores/tasks'
import { casyContext } from '../../../core/plugin/context'
import { FILE_CATEGORIES, filterAndSortFiles, formatFileSize, getFileExt } from '../lib/fileUtils'
import { buildMemoRecord, parseMemosFromNotes } from '../lib/memoUtils'
import { ElMessage, ElMessageBox } from 'element-plus'
import {
  Search,
  Filter,
  Plus,
  ArrowRight,
  FolderOpened,
  Folder,
  Document,
  Clock,
  Briefcase,
  Check,
  More,
  Delete,
  View,
  TopRight,
  Share,
  Files,
  Timer,
  Calendar,
  Rank,
  TrendCharts,
  User,
  Location,
  Edit,
  OfficeBuilding,
  ScaleToOriginal,
  DocumentCopy,
  CopyDocument,
  FolderAdd,
  ArrowRightBold,
  ArrowLeft,
  Notebook,
  List,
  FullScreen,
  Position,
  Warning,
  Tickets,
  CollectionTag,
  MagicStick,
  Opportunity,
  ChatDotRound,
  Bell,
  Reading,
  Close,
  Upload,
} from '../../../shared/icons'
import CaseFilterBar from '../components/CaseFilterBar.vue'
import CaseWizard from '../components/CaseWizard.vue'
import CaseAttributes from '../components/CaseAttributes.vue'
import ProcedureBoard from '../components/ProcedureBoard.vue'
import WhiteboardEntry from '../../whiteboard/components/WhiteboardEntry.vue'
import CaseImportDialog from '../components/CaseImportDialog.vue'
import StateFeedback from '../../../shared/components/StateFeedback.vue'
import {
  CIVIL_STATUS_LABELS,
  INVALIDATION_STATUS_LABELS,
  ADMIN_STATUS_LABELS,
} from '../../../types'
const router = useRouter()
const route = useRoute()
const casesStore = useCasesStore()
const tasksStore = useTasksStore()
const groupBy = ref('none')
const showCaseWizard = ref(false)
const showExcelImportDialog = ref(false)
const showEditOverviewDialog = ref(false)
const showCaseFilters = ref(false)
const selectedCaseId = ref('')
// 完整工作区模式（隐藏左侧案件列表，展开丰富标签页与全量编辑）
const isFullWorkspace = ref(false)
// 当前激活的标签页
const selectedTab = ref('overview') // 'overview' | 'timeline' | 'record' | 'tracks' | 'files'
// 卷宗文件与目录树状态
const caseFiles = ref([])
const caseDirs = ref([])
const filesLoading = ref(false)
const selectedDirRel = ref('') // '' 代表根目录（穿透全部），非空代表具体子文件夹（不穿透）
const activeCategory = ref('all')
const fileSortOrder = ref('added') // 'added' | 'recent' | 'name'
const fileSearchQuery = ref('')
// 时间轴状态与过滤器
const domainTimeline = ref([])
const timelineFilter = ref('all') // 'all' | 'task' | 'event' | 'deadline' | 'doc' | 'memo'
const calendarEvents = ref([])
// 办案笔记/备忘列表
const caseMemos = ref([])
// ── 记录工作台输入表单 ──
const recordType = ref('memo') // 'memo' | 'task' | 'event'
const memoForm = ref({
  title: '',
  content: '',
  tags: ['办案记录'],
})
const taskForm = ref({
  taskName: '',
  dueDate: '',
  priority: 'medium',
  estimatedMinutes: 30,
  context: '办案推进',
})
const eventForm = ref({
  title: '',
  eventDate: '',
  eventType: 'court',
  location: '',
  reminderDate: '',
  notes: '',
})
// ── 任务编辑抽屉 ──
const showTaskDrawer = ref(false)
const editingTask = ref(null)
// 右键菜单状态
const contextMenu = ref({
  visible: false,
  x: 0,
  y: 0,
  file: null,
})
const fileCategories = FILE_CATEGORIES
// 案件要素编辑表单
const trackOptions = [
  { value: 'patent_invalidation', label: '专利无效宣告程序' },
  { value: 'civil_tort', label: '民事诉讼程序 (侵权/合同)' },
  { value: 'admin_litigation', label: '行政诉讼程序' },
  { value: 'arbitration', label: '商事仲裁程序' },
  { value: 'other', label: '非诉业务 / 常年顾问 / 其他' },
]
// 标准诉讼程序推荐节点库
const STANDARD_PROCEEDING_NODES = {
  civil_tort: [
    { key: 'filing', label: '起诉与立案' },
    { key: 'service', label: '送达与管辖' },
    { key: 'evidence', label: '证据交换与举证' },
    { key: 'trial', label: '法庭庭审质证' },
    { key: 'judgment', label: '判决与上诉/执行' },
  ],
  patent_invalidation: [
    { key: 'petition', label: '无效宣告请求' },
    { key: 'acceptance', label: '国知局受理立案' },
    { key: 'response', label: '答辩与意见陈述' },
    { key: 'oral_hearing', label: '口头审理辩论' },
    { key: 'decision', label: '审查决定与司法救济' },
  ],
  admin_litigation: [
    { key: 'filing', label: '行政起诉立案' },
    { key: 'defense', label: '行政行为举证答辩' },
    { key: 'cross_exam', label: '法庭质证与辩论' },
    { key: 'verdict', label: '裁判送达与履行' },
  ],
  arbitration: [
    { key: 'request', label: '仲裁申请与立案' },
    { key: 'tribunal', label: '组庭与仲裁员选定' },
    { key: 'hearing', label: '开庭审理与辩论' },
    { key: 'award', label: '裁决生效与执行' },
  ],
}
const selectedCase = computed(() => {
  return casesStore.cases.find((item) => item.id === selectedCaseId.value)
    || null
})
const selectedThirdParties = computed(() => {
  try { return JSON.parse(selectedCase.value?.thirdParties || '[]') } catch { return [] }
})
// 当前案件关联的任务列表 (活跃 + 已完成)
const caseAllTasks = computed(() => {
  return selectedTasks.value
})
const selectedTasks = ref([])
let selectedTaskVersion = 0
watch([() => selectedCase.value?.id, () => tasksStore.tasks], async ([caseId]) => {
  const version = ++selectedTaskVersion
  selectedTasks.value = []
  if (!caseId) return
  const result = await casyContext.tasks.list({caseId})
  if (version === selectedTaskVersion && result.ok) selectedTasks.value = result.data || []
}, {immediate:true,deep:true})
// 当前案件的最近待办列表 (最近 4 项)
const caseUpcomingTasks = computed(() => {
  return caseAllTasks.value.filter((t) => !t.completed).slice(0, 4)
})
// 通用并行程序数据计算
const proceedingsList = computed(() => {
  const item = selectedCase.value
  if (!item) return []
  const track = item.track || 'patent_invalidation'
  const isLitigation = ['civil_tort', 'patent_invalidation', 'admin_litigation', 'arbitration'].includes(track)
  if (isLitigation) {
    const nodes = STANDARD_PROCEEDING_NODES[track] || []
    return [
      {
        id: 'proc-main',
        name: trackLabel(track),
        isStandard: true,
        progress: null,
        statusLabel: item.caseStatus || '未填写',
        colorVar: 'var(--c-primary)',
        nodes: nodes.map((node) => ({
          ...node,
          state: 'upcoming',
        })),
      },
    ]
  }
  // 非诉/通用事项: 不强制推荐诉讼节点
  return [
    {
      id: 'proc-general',
      name: '业务推进计划 (通用流程)',
      isStandard: false,
      progress: null,
      statusLabel: item.caseStatus || '未填写',
      colorVar: 'var(--c-primary)',
      nodes: [],
    },
  ]
})
const overallProgress = computed(() => {
  const list = caseAllTasks.value
  if (!list.length) return 0
  return Math.round(list.filter(task => task.completed).length / list.length * 100)
})
/**
 * 聚合全景时间轴流 (本案全部事件、任务、发文、绝限、收文、备忘笔记)
 */
const fullTimelineItems = computed(() => {
  const c = selectedCase.value
  if (!c) return []
  const items = []
  // 1. 办案备忘笔记
  caseMemos.value.forEach((m) => {
    items.push({
      id: `memo-${m.id}`,
      type: 'memo',
      title: m.title || '办案纪要与策略备忘',
      date: m.date || m.createdAt || '',
      time: m.time || '',
      content: m.content,
      tags: m.tags || ['办案笔记'],
      raw: m,
    })
  })
  // 2. 案件任务
  caseAllTasks.value.forEach((t) => {
    items.push({
      id: `task-${t.id}`,
      type: 'task',
      title: t.taskName,
      date: t.dueDate || t.deadline || '待排期',
      completed: Boolean(t.completed),
      priority: t.priority || 'medium',
      content: t.description || (t.estimatedMinutes ? `预计工时: ${t.estimatedMinutes}分钟` : '未填写预估时长'),
      raw: t,
    })
  })
  // 3. 客观法庭事件与日历排期
  domainTimeline.value.filter(ev=>ev.sourceTable!=='tasks').forEach((ev) => {
    items.push({id:`domain-${ev.id}`,type:'event',title:ev.title,date:ev.eventDate,content:ev.detail,sourceTable:ev.sourceTable})
  })
  calendarEvents.value.filter(ev=>ev.type==='event').forEach((ev) => {
    items.push({
      id: `event-${ev.id}`,
      type: 'event',
      title: ev.title,
      date: ev.eventDate || ev.date,
      time: ev.startTime || '全天',
      content: ev.location ? `地点: ${ev.location}` : ev.notes || '法庭审理与日程',
      raw: ev,
    })
  })
  // 4. 案件法定里程碑 (立案、开庭、绝限)
  if (c.filingDate) {
    items.push({
      id: 'stat-filing',
      type: 'event',
      title: '案件正式受理立案',
      date: c.filingDate,
      content: `案号: ${c.caseNo || '未填写'} · 受诉机构: ${c.court || '未填写'}`,
    })
  }
  if (c.reliefDeadline) {
    items.push({
      id: 'stat-relief',
      type: 'deadline',
      title: '旧字段救济期限 · 待核对',
      date: c.reliefDeadline,
      content: '原始字段保留供核对；具体责任方、起算与期限依据请查看程序事项',
    })
  }
  if (c.trialDate && !domainTimeline.value.some(e=>e.sourceTable==='hearings')) {
    items.push({
      id: 'stat-trial',
      type: 'event',
      title: '旧字段开庭 / 口审 · 待核对',
      date: c.trialDate,
      content: `合议组: ${c.judgePanel || '未填写'} · 书记员: ${c.clerk || '未填写'}`,
    })
  }
  // 5. 卷宗收发文
  caseFiles.value.forEach((f) => {
    const isSummons = f.category === 'summons' || f.category === 'received'
    const isSubmitted = f.category === 'submitted'
    items.push({
      id: `file-${f.id}`,
      type: 'doc',
      title: isSummons ? `【归卷·收文】${f.fileName}` : isSubmitted ? `【归卷·发文】${f.fileName}` : `【存卷】${f.fileName}`,
      date: f.createdAt ? f.createdAt.slice(0, 10) : '',
      content: `入库日期不代表实际收文或送达日。大小: ${formatFileSize(f.fileSize)} · 分类: ${f.category || '卷宗材料'}`,
      raw: f,
    })
  })
  // 按日期倒序排列
  items.sort((a, b) => (b.date || '').localeCompare(a.date || ''))
  return items
})
// 筛选后的时间轴列表
const filteredTimelineItems = computed(() => {
  if (timelineFilter.value === 'all') return fullTimelineItems.value
  return fullTimelineItems.value.filter((item) => item.type === timelineFilter.value)
})
/**
 * 卷宗文件列表穿透与非穿透逻辑
 */
const sortedCaseFiles = computed(() => filterAndSortFiles(caseFiles.value, {
  selectedDirRel: selectedDirRel.value,
  activeCategory: activeCategory.value,
  searchQuery: fileSearchQuery.value,
  sortOrder: fileSortOrder.value,
}))
function trackLabel(track) {
  return trackOptions.find((option) => option.value === track)?.label || '其他事项'
}
function statusBadgeClass(status) {
  if (!status) return 'badge-active'
  if (status.includes('庭') || status.includes('Trial')) return 'badge-risk'
  if (status.includes('审') || status.includes('Pre-trial')) return 'badge-discovery'
  if (status.includes('诉') || status.includes('Appeal')) return 'badge-warning'
  if (status.includes('完结') || status.includes('Closed')) return 'badge-muted'
  return 'badge-active'
}
function selectCase(item) {
  selectedCaseId.value = item.id
  router.replace({ query: { ...route.query, caseId: item.id } })
  selectedDirRel.value = ''
  loadCaseMemos(item)
  loadCaseFiles()
  loadCaseEvents()
}
function enterFullWorkspace() {
  isFullWorkspace.value = true
}
function exitFullWorkspace() {
  isFullWorkspace.value = false
}
// 加载案件备忘
function loadCaseMemos(item) {
  caseMemos.value = parseMemosFromNotes(item?.notes)
}
// 加载案件文件与目录
let filesRequest = 0
async function loadCaseFiles() {
  const request = ++filesRequest
  const id = selectedCaseId.value
  caseFiles.value = []; caseDirs.value = []
  if (!selectedCase.value) {
    caseFiles.value = []
    caseDirs.value = []
    return
  }
  filesLoading.value = true
  const [filesRes, dirsRes] = await Promise.all([
    casyContext.files.list(selectedCase.value.id),
    casyContext.files.listCaseDirs(selectedCase.value.id),
  ])
  if (request !== filesRequest || id !== selectedCaseId.value) return
  if (filesRes.ok && Array.isArray(filesRes.data)) {
    caseFiles.value = filesRes.data
  } else {
    caseFiles.value = []
  }
  if (dirsRes.ok && Array.isArray(dirsRes.data)) {
    caseDirs.value = dirsRes.data
  } else {
    caseDirs.value = []
  }
  filesLoading.value = false
}
// 加载案件日历事件
let eventsRequest = 0
async function loadCaseEvents() {
  const request = ++eventsRequest; const id = selectedCaseId.value
  calendarEvents.value = [];domainTimeline.value=[]
  if (!selectedCase.value) return
  const now = new Date()
  const [res,domain] = await Promise.all([casyContext.calendar.events(now.getFullYear(), now.getMonth() + 1),casyContext.cases.timeline(id)])
  if (request !== eventsRequest || id !== selectedCaseId.value) return
  if(domain.ok)domainTimeline.value=domain.data || []
  if (res.ok && Array.isArray(res.data)) {
    calendarEvents.value = res.data.filter((ev) => ev.caseId === id)
  }
}
// 快速完成/取消待办
async function toggleCaseTask(task) {
  const newDone = !task.completed
  const res = await casyContext.tasks.update({ id: task.id, completed: newDone ? 1 : 0 })
  // 后端失败：报错并退出，不弹成功、不刷新列表
  if (!res.ok) return ElMessage.error(res.error || '操作失败')
  ElMessage.success(newDone ? '任务已完成' : '已恢复待办')
  await tasksStore.loadTasks()
}
// 打开任务编辑抽屉
function openTaskDrawer(task) {
  editingTask.value = {
    ...task,
    subtasks: task.subtasks || [],
  }
  showTaskDrawer.value = true
}
// 保存任务抽屉编辑
async function saveEditingTask() {
  if (!editingTask.value) return
  const res = await casyContext.tasks.update({
    id: editingTask.value.id,
    taskName: editingTask.value.taskName,
    dueDate: editingTask.value.dueDate,
    dueTime: editingTask.value.dueTime,
    priority: editingTask.value.priority,
    estimatedMinutes: Number(editingTask.value.estimatedMinutes) || 30,
    context: editingTask.value.context,
    description: editingTask.value.description,
  })
  if (res.ok) {
    ElMessage.success('待办详情已保存')
    showTaskDrawer.value = false
    await tasksStore.loadTasks()
  } else {
    ElMessage.error(res.error || '保存失败')
  }
}
// 删除任务
async function deleteTaskFromDrawer() {
  if (!editingTask.value) return
  let confirmed = false
  try {
    await ElMessageBox.confirm('确定删除此任务吗？', '删除确认', { type: 'warning' })
    confirmed = true
  } catch { /* 用户取消：属预期 */ }
  if (!confirmed) return
  const res = await casyContext.tasks.remove(editingTask.value.id)
  if (!res.ok) return ElMessage.error(res.error || '删除任务失败')
  ElMessage.success('任务已删除')
  showTaskDrawer.value = false
  await tasksStore.loadTasks()
}
// ── 记录工作台核心操作 ──
// 1. 保存备忘
async function submitMemo() {
  if (!memoForm.value.content.trim()) {
    ElMessage.warning('请输入备忘内容')
    return
  }
  const newMemo = buildMemoRecord({
    title: memoForm.value.title,
    content: memoForm.value.content,
    tags: memoForm.value.tags,
    caseName: selectedCase.value?.caseName,
  })
  // 先持久化（同步入库案件 notes 字段），成功后才更新本地状态：
  // 后端失败时不写入 caseMemos、不弹成功，直接报错，避免本地污染/虚假成功。
  const nextMemos = [newMemo, ...caseMemos.value]
  const res = await casyContext.cases.update(selectedCase.value.id, {
    notes: JSON.stringify(nextMemos),
  })
  if (!res.ok) return ElMessage.error(res.error || '备忘保存失败，未写入本地')
  caseMemos.value = nextMemos
  ElMessage.success('办案备忘已记录并收录进本案时间轴')
  memoForm.value.title = ''
  memoForm.value.content = ''
  selectedTab.value = 'timeline'
}
// 2. 提交任务
async function submitTask() {
  if (!taskForm.value.taskName.trim()) {
    ElMessage.warning('请输入任务名称')
    return
  }
  const res = await casyContext.tasks.create({
    taskName: taskForm.value.taskName,
    caseId: selectedCase.value.id,
    dueDate: taskForm.value.dueDate || null,
    priority: taskForm.value.priority || 'medium',
    estimatedMinutes: Number(taskForm.value.estimatedMinutes) || 30,
    context: taskForm.value.context || '办案推进',
    taskType: 'action',
  })
  if (res.ok) {
    ElMessage.success('待办已创建，已同步至全局任务与日历')
    taskForm.value.taskName = ''
    taskForm.value.dueDate = ''
    await tasksStore.loadTasks()
    selectedTab.value = 'timeline'
  } else {
    ElMessage.error(res.error || '创建待办失败')
  }
}
// 3. 提交客观事件
async function submitEvent() {
  if (!eventForm.value.title.trim()) {
    ElMessage.warning('请输入事件名称')
    return
  }
  if (!eventForm.value.eventDate) {
    ElMessage.warning('请选择事件日期')
    return
  }
  const res = await casyContext.calendar.createEvent({
    caseId: selectedCase.value.id,
    title: eventForm.value.title,
    eventDate: eventForm.value.eventDate,
    eventType: eventForm.value.eventType,
    location: eventForm.value.location || '',
    reminderDate: eventForm.value.reminderDate || null,
    notes: eventForm.value.notes || '',
  })
  if (res.ok) {
    ElMessage.success('客观事件已记入本案时间轴与日历')
    eventForm.value.title = ''
    eventForm.value.eventDate = ''
    await loadCaseEvents()
    selectedTab.value = 'timeline'
  } else {
    ElMessage.error(res.error || '记录事件失败')
  }
}
// 4. 从备忘一键智能提炼并生成事件/待办
function extractFromMemo() {
  const text = memoForm.value.content
  if (!text) {
    ElMessage.warning('请先输入备忘记录文本')
    return
  }
  // 简易 NLP 抽取日期与关键词
  const dateMatch = text.match(/\d{4}[-/年]\d{1,2}[-/月]\d{1,2}/) || text.match(/\d{1,2}月\d{1,2}日/)
  if (dateMatch) {
    eventForm.value.eventDate = dateMatch[0].replace('年', '-').replace('月', '-').replace('日', '')
  }
  if (text.includes('开庭') || text.includes('口审')) {
    eventForm.value.title = '法庭开庭审理'
    eventForm.value.eventType = 'court'
    recordType.value = 'event'
    ElMessage.success('已自动提炼提取为「法庭开庭事件」，请确认并保存')
  } else if (text.includes('截止') || text.includes('到期') || text.includes('提交')) {
    taskForm.value.taskName = memoForm.value.title || text.slice(0, 30)
    recordType.value = 'task'
    ElMessage.success('已自动提炼提取为「行动待办」，请确认并保存')
  } else {
    ElMessage.info('未检测到明显开庭或绝限关键词，可手动切换上方选项')
  }
}
// 双击用默认方式打开文件
async function openFile(file) {
  if (!file?.filePath) {
    ElMessage.warning('文件路径不存在')
    return
  }
  const result = await casyContext.files.open(file.filePath)
  if (result.ok) {
    ElMessage.success(`已使用系统默认应用打开：${file.fileName}`)
  } else {
    ElMessage.error(result.error || '打开文件失败')
  }
}
// 在访达中显示文件
async function revealFile(file) {
  if (!file?.filePath) return
  await casyContext.files.reveal(file.filePath)
  ElMessage.success('已在访达/资源管理器中定位')
}
// 复制文件路径
async function copyFilePath(file) {
  if (!file?.filePath) return
  try {
    await navigator.clipboard.writeText(file.filePath)
    ElMessage.success('文件路径已复制到剪贴板')
  } catch {
    ElMessage.info(file.filePath)
  }
}
function onFileContextMenu(e, file) {
  contextMenu.value = {
    visible: true,
    x: Math.min(e.clientX, window.innerWidth - 200),
    y: Math.min(e.clientY, window.innerHeight - 220),
    file,
  }
}
function closeContextMenu() {
  contextMenu.value.visible = false
}
async function createSubdir() {
  if (!selectedCase.value) return
  let dirName = ''
  try {
    const { value } = await ElMessageBox.prompt(
      '请输入新建文件夹名称',
      '新建卷宗子文件夹',
      { inputPlaceholder: '如：06_补充反诉证据', inputValue: '' }
    )
    dirName = value?.trim() || ''
  } catch { /* 用户取消：属预期 */ }
  if (!dirName) return
  const res = await casyContext.files.createSubdir(selectedCase.value.id, null, dirName)
  if (res.ok) {
    ElMessage.success('子文件夹已创建')
    await loadCaseFiles()
    selectedDirRel.value = dirName
  } else {
    ElMessage.error(res.error || '创建失败')
  }
}
async function deleteFile(file) {
  let confirmed = false
  try {
    await ElMessageBox.confirm(`确定从案件卷宗中移除「${file.fileName}」吗？`, '删除确认', {
      confirmButtonText: '移除',
      cancelButtonText: '取消',
      type: 'warning',
    })
    confirmed = true
  } catch { /* 用户取消：属预期 */ }
  if (!confirmed) return
  const res = await casyContext.files.remove(file.id)
  if (!res.ok) return ElMessage.error(res.error || '移除文件登记失败')
  ElMessage.success('已移除文件登记')
  await loadCaseFiles()
}
// 唯一的案件二级菜单：进入完整卷宗管理工作台
function openSelectedFiles() {
  if (!selectedCase.value) return
  router.push({ name: 'files', params: { caseId: selectedCase.value.id } })
}
function openEditOverviewModal() {
  if (selectedCase.value) showEditOverviewDialog.value = true
}
onUnmounted(casyContext.on('inbox:confirmed', () => {
  casesStore.loadCases()
  tasksStore.loadTasks()
  if (selectedCaseId.value) { loadCaseFiles(); loadCaseEvents() }
}))
onMounted(async () => {
  if (trackOptions.some(option => option.value === route.query.track)) {
    casesStore.filter.track = route.query.track
    casesStore.page = 1
    showCaseFilters.value = true
  }
  await Promise.all([
    casesStore.loadCases(),
    tasksStore.loadTasks(),
  ])
  const wanted = typeof route.query.caseId === 'string' ? route.query.caseId : ''
  if (wanted && !casesStore.cases.some(c => c.id === wanted)) {
    await router.replace({ name: 'case-detail', params: { id: wanted } }); return
  }
  selectedCaseId.value = wanted || casesStore.cases[0]?.id || ''
  if (selectedCase.value) {
    loadCaseMemos(selectedCase.value)
    await loadCaseFiles()
    await loadCaseEvents()
  }
  window.addEventListener('click', closeContextMenu)
})
onUnmounted(() => {
  window.removeEventListener('click', closeContextMenu)
})
watch(
  () => casesStore.cases,
  (items) => {
    if (!items.length) {
      selectedCaseId.value = ''
      caseFiles.value = []
      return
    }
    if (route.query.caseId && route.query.caseId !== selectedCaseId.value) return
    if (!items.some((item) => item.id === selectedCaseId.value)) {
      selectCase(items[0])
    }
  }
)
watch(() => route.query.caseId, id => {
  if (typeof id !== 'string' || id === selectedCaseId.value) return
  const found = casesStore.cases.find(c => c.id === id)
  if (found) selectCase(found)
  else router.replace({name:'case-detail', params:{id}})
})
watch(selectedCaseId, () => {
  showEditOverviewDialog.value = false
})
function onSearch() {
  casesStore.page = 1
  casesStore.loadCases()
}
// 分页（审查 P1-3）：页变更时写入 store.page 并重载，使超过 perPage 的案件可访问。
async function onPageChange(page) {
  casesStore.page = page
  await casesStore.loadCases()
}
// 处理向导提交
async function handleCreateCase(formData) {
  const result = await casesStore.createCase(formData)
  if (result.ok) {
    ElMessage.success('案件已创建')
    selectedCaseId.value = result.data.id
    loadCaseFiles()
  } else {
    ElMessage.error(result.error || '创建失败')
  }
  return result
}
</script>
<template>
  <div class="stitch-cases-view">
    <!-- ═══ 顶部 Action Bar (完整工作区模式下切换返回按钮) ═══ -->
    <div class="cases-topbar">
      <div class="topbar-heading">
        <template v-if="isFullWorkspace">
          <button class="btn-back-cases-list" @click="exitFullWorkspace">
            <el-icon><ArrowLeft /></el-icon>
            <span>返回案件列表</span>
          </button>
          <div class="topbar-ws-title">
            <span class="eyebrow-kicker">Full Matter Workspace</span>
            <h1 class="page-main-title">{{ selectedCase?.caseName || '案件工作区' }}</h1>
          </div>
        </template>
        <template v-else>
          <h1 class="page-main-title">案件</h1>
        </template>
      </div>
      <div class="topbar-actions">
        <template v-if="!isFullWorkspace">
          <button
            class="btn-action-filter"
            :class="{ active: showCaseFilters }"
            @click="showCaseFilters = !showCaseFilters"
          >
            <el-icon :size="16"><Filter /></el-icon>
            <span>{{ showCaseFilters ? '收起筛选' : '筛选' }}</span>
          </button>
          <button class="btn-action-import" @click="showExcelImportDialog = true" title="从 Excel / 飞书多维表格批量导入案件">
            <el-icon :size="15"><Upload /></el-icon>
            <span>批量导入</span>
          </button>
          <button class="btn-action-primary" @click="showCaseWizard = true">
            <el-icon :size="16"><Plus /></el-icon>
            <span>新建案件</span>
          </button>
        </template>
        <template v-else>
          <button class="btn-edit-all-facts" @click="openEditOverviewModal">
            <el-icon><Edit /></el-icon>
            <span>编辑全案要素</span>
          </button>
        </template>
      </div>
    </div>
    <!-- 筛选面板（抽屉式） -->
    <transition name="vslide">
      <section v-if="showCaseFilters && !isFullWorkspace" class="filter-drawer-well">
        <CaseFilterBar
          :filter="casesStore.filter"
          :group-by="groupBy"
          :total="casesStore.total"
          @update:filter="(v) => Object.assign(casesStore.filter, v)"
          @update:groupBy="(v) => groupBy = v"
          @search="onSearch"
          @create="showCaseWizard = true"
        />
      </section>
    </transition>
    <!-- ═══ 工作台主布局 (Master-Detail / Full Workspace) ═══ -->
    <div class="cases-master-detail" :class="{ 'full-workspace-mode': isFullWorkspace }">
      <!-- ── 左侧 320px 案件列表 (进入完整工作区时隐藏) ── -->
      <aside v-if="!isFullWorkspace" class="cases-sidebar-index">
        <div class="search-input-box">
          <el-icon class="search-ico" :size="15"><Search /></el-icon>
          <input
            :value="casesStore.filter.search"
            class="input-clean"
            placeholder="搜索案件"
            aria-label="搜索案件"
            @input="(e) => { casesStore.filter.search = e.target.value; onSearch() }"
          />
        </div>
        <!-- 仅在无数据时展示加载/空态；翻页期间保留当前列表，避免整列闪烁 -->
        <StateFeedback
          v-if="!casesStore.cases.length"
          :state="casesStore.loading ? 'loading' : 'empty'"
          empty-text="暂无匹配案件"
          style="padding-top: 20px"
        />
        <template v-else>
          <div class="cases-scroll-list" role="listbox">
            <button
              v-for="item in casesStore.cases"
              :key="item.id"
              type="button"
              class="case-index-card"
              :class="{ active: selectedCase?.id === item.id }"
              @click="selectCase(item)"
              @dblclick="enterFullWorkspace"
              title="单击查看概览 · 双击进入案件完整工作区"
            >
              <div class="card-meta-line">
                <span v-if="item.internalNo || item.caseNo" class="mono-case-code">{{ item.internalNo || item.caseNo }}</span>
                <span :class="['case-status-badge', statusBadgeClass(item.caseStatus)]">
                  {{ item.caseStatus || '未填写' }}
                </span>
              </div>
              <strong class="case-card-title">{{ item.caseName }}</strong>
              <span class="case-card-client" v-if="item.clientName">{{ item.clientName }}</span>
            </button>
          </div>
          <!-- 分页（审查 P1-3）：超过 perPage 的案件可通过翻页访问 -->
          <div v-if="casesStore.total > casesStore.perPage" class="case-pagination">
            <el-pagination
              layout="prev, pager, next"
              :current-page="casesStore.page"
              :page-size="casesStore.perPage"
              :total="casesStore.total"
              @current-change="onPageChange"
            />
          </div>
        </template>
      </aside>
      <!-- ── 右侧工作台区域 (在 Full Workspace 模式下全宽展开) ── -->
      <main v-if="selectedCase" class="cases-detail-area">
        <!-- 1. Case Summary Header 卡片 -->
        <div class="matter-summary-header-card">
          <div class="summary-left-group">
            <!-- 环形进度 -->
            <div class="summary-ring-wrapper">
              <svg class="summary-ring-svg" viewBox="0 0 36 36">
                <path
                  class="ring-bg"
                  d="M18 2.0845 a 15.9155 15.9155 0 0 1 0 31.831 a 15.9155 15.9155 0 0 1 0 -31.831"
                />
                <path
                  class="ring-progress"
                  :stroke-dasharray="`${overallProgress}, 100`"
                  d="M18 2.0845 a 15.9155 15.9155 0 0 1 0 31.831 a 15.9155 15.9155 0 0 1 0 -31.831"
                />
              </svg>
              <div class="ring-center-content">
                <span class="ring-stage-num">{{ caseAllTasks.length ? `${overallProgress}%` : '-' }}</span>
                <span class="ring-stage-label">任务完成</span>
              </div>
            </div>
            <!-- 案件名称与标识 -->
            <div class="summary-matter-identity">
              <div class="identity-badge-row">
                <span v-if="selectedCase.internalNo" class="badge-matter-pill">MATTER {{ selectedCase.internalNo }}</span>
                <span class="badge-pat-code" v-if="selectedCase.caseNo">{{ selectedCase.caseNo }}</span>
              </div>
              <h2 class="matter-main-name">{{ selectedCase.caseName }}</h2>
              <p class="matter-meta-lead">
                <span v-if="selectedCase.clientName">客户：{{ selectedCase.clientName }} · </span>{{ trackLabel(selectedCase.track) }}
              </p>
            </div>
          </div>
          <!-- 右侧 Next Actions 待办列表卡片 -->
          <div class="summary-next-actions-card">
            <div class="action-card-top">
              <span class="label-next-act">近期待办</span>
              <span class="mono-due-act">{{ caseUpcomingTasks.length }} 项待处理</span>
            </div>
            <div class="upcoming-tasks-stream">
              <div
                v-for="task in caseUpcomingTasks"
                :key="task.id"
                class="upcoming-task-row"
                @click="openTaskDrawer(task)"
              >
                <button
                  class="task-row-check"
                  :class="{ checked: task.completed }"
                  @click.stop="toggleCaseTask(task)"
                  title="标记完成"
                >
                  <el-icon v-if="task.completed" :size="11"><Check /></el-icon>
                </button>
                <span class="task-row-name" :class="{ struck: task.completed }">{{ task.taskName }}</span>
                <span v-if="task.planDefined" class="task-row-plan" :title="taskPlanLabel(task)">{{ taskPlanLabel(task) }}</span>
                <span v-if="task.dueDate" class="task-row-due">截止 {{ task.dueDate.slice(5) }}</span>
              </div>
              <div v-if="!caseUpcomingTasks.length" class="empty-upcoming-box">
                <span>暂无待处理事项 · 状态良好</span>
              </div>
            </div>
          </div>
        </div>
        <!-- 2. 详情 Tabs 栏 (最左侧放置进入/退出完整工作区纯文字链接，随后是各标签) -->
        <div class="matter-tools" aria-label="案件工作区快捷操作">
            <el-button text @click="router.push(`/whiteboard/${selectedCase.id}`)">事实白板</el-button>
            <el-button text @click="router.push({name: 'case-detail', params: {id: selectedCase.id}, query: {tab: 'hearings'}})">历次开庭 / 口审</el-button>
            <!-- 进入/退出完整案件工作区纯文字链接 (无框、无分割线) -->
            <button type="button"
              v-if="!isFullWorkspace"
              class="link-jump-workspace-head"
              @click="enterFullWorkspace"
              title="隐藏左侧列表，全宽沉浸式处理案件所有信息"
            >
              <span>进入案件完整工作区</span>
              <el-icon :size="13"><FullScreen /></el-icon>
            </button>
            <button type="button"
              v-else
              class="link-jump-workspace-head"
              @click="exitFullWorkspace"
            >
              <span>返回列表模式</span>
              <el-icon :size="13"><ArrowLeft /></el-icon>
            </button>
        </div>
        <div class="matter-detail-tabs-bar">
          <div class="tabs-group-left">
            <button
              class="tab-btn"
              :class="{ active: selectedTab === 'overview' }"
              @click="selectedTab = 'overview'"
            >
              案件要素
            </button>
            <!-- 🌟 本案全景时间轴 Tab -->
            <button
              class="tab-btn"
              :class="{ active: selectedTab === 'timeline' }"
              @click="selectedTab = 'timeline'"
            >
              本案时间轴
            </button>
            <!-- 🌟 写记录与备忘输入口 Tab -->
            <button
              class="tab-btn"
              :class="{ active: selectedTab === 'record' }"
              @click="selectedTab = 'record'"
            >
              记录与备忘
            </button>
            <button
              class="tab-btn"
              :class="{ active: selectedTab === 'tracks' }"
              @click="selectedTab = 'tracks'"
            >
              程序期限与统筹
            </button>
            <button
              class="tab-btn"
              :class="{ active: selectedTab === 'files' }"
              @click="selectedTab = 'files'; loadCaseFiles()"
            >
              卷宗文件 · {{ caseFiles.length }}
            </button>
          </div>
        </div>
        <!-- 3. Tab 内容区 -->
        <!-- A. 案件要素全景 (Overview) -->
        <div v-if="selectedTab === 'overview'" class="tab-pane-card">
          <WhiteboardEntry :key="selectedCase.id" :case-id="selectedCase.id" />
          <div class="overview-pane-header">
            <div>
              <h3 class="pane-title">案件核心事实与要素全景</h3>
              <p class="pane-sub">涵盖程序审级、当事人身份、受诉法庭、标的权属及法定上诉救济期限事实。</p>
            </div>
            <button class="btn-edit-facts" @click="openEditOverviewModal">
              <el-icon><Edit /></el-icon>
              <span>编辑要素</span>
            </button>
          </div>
          <div class="facts-sections-container">
            <!-- 1. 审级与基本要素 -->
            <div class="fact-section-block">
              <div class="sec-headline">
                <el-icon class="sec-ico"><ScaleToOriginal /></el-icon>
                <span>基本程序与审级事实</span>
              </div>
              <div class="facts-grid-three">
                <div class="fact-item-card">
                  <span class="fact-lbl">案由 (Cause of Action)</span>
                  <strong class="fact-val">{{ selectedCase.causeAction || '未填写' }}</strong>
                </div>
                <div class="fact-item-card">
                  <span class="fact-lbl">程序分类 (Category)</span>
                  <strong class="fact-val">{{ trackLabel(selectedCase.track) }}</strong>
                </div>
                <div class="fact-item-card">
                  <span class="fact-lbl">审级 / 阶段 (Level)</span>
                  <strong class="fact-val">{{ selectedCase.caseLevel || '未填写' }}</strong>
                </div>
                <div class="fact-item-card">
                  <span class="fact-lbl">适用程序 (Procedure)</span>
                  <strong class="fact-val">{{ selectedCase.procedureType || '未填写' }}</strong>
                </div>
                <div class="fact-item-card">
                  <span class="fact-lbl">案号 / 官方公文字号</span>
                  <strong class="fact-val mono">{{ selectedCase.caseNo || '未登记官方案号' }}</strong>
                </div>
                <div class="fact-item-card">
                  <span class="fact-lbl">我方代理律师 (Attorneys)</span>
                  <strong class="fact-val">{{ Array.isArray(selectedCase.attorneys) && selectedCase.attorneys.length ? selectedCase.attorneys.join('、') : '未填写' }}</strong>
                </div>
              </div>
            </div>
            <!-- 2. 当事人与代理关系 -->
            <div class="fact-section-block">
              <div class="sec-headline">
                <el-icon class="sec-ico"><User /></el-icon>
                <span>当事人与代理关系 (Parties)</span>
              </div>
              <div class="facts-grid-two">
                <div class="fact-item-card highlight-client">
                  <span class="fact-lbl">我方客户 (委托人)</span>
                  <strong class="fact-val text-primary">{{ selectedCase.clientName || '未填写' }}</strong>
                  <small class="fact-sub-txt">诉讼地位: {{ selectedCase.ourRole || '未填写' }}</small>
                </div>
                <div class="fact-item-card highlight-opponent">
                  <span class="fact-lbl">对方当事人 (相对人)</span>
                  <strong class="fact-val">{{ selectedCase.opponentName || '未登记对方当事人' }}</strong>
                  <small class="fact-sub-txt">
                    地位: {{ selectedCase.opponentRole || '未填写' }}
                    <span v-if="selectedCase.opponentFirm"> · 代理律所: {{ selectedCase.opponentFirm }}</span>
                  </small>
                </div>
                <div v-for="(party,index) in selectedThirdParties" :key="index" class="fact-item-card">
                  <span class="fact-lbl">第三人 {{ index + 1 }}</span>
                  <strong class="fact-val">{{ party.name }}</strong>
                  <small class="fact-sub-txt">{{ [party.role,party.agent,party.firm,party.contact].filter(Boolean).join(' · ') }}</small>
                </div>
              </div>
            </div>
            <!-- 3. 受诉机构与合议庭 -->
            <div class="fact-section-block">
              <div class="sec-headline">
                <el-icon class="sec-ico"><OfficeBuilding /></el-icon>
                <span>受诉机构与审理合议庭 (Tribunal)</span>
              </div>
              <div class="facts-grid-three">
                <div class="fact-item-card">
                  <span class="fact-lbl">受理机构 / 法院</span>
                  <strong class="fact-val">{{ selectedCase.court || '未填写' }}</strong>
                </div>
                <div class="fact-item-card">
                  <span class="fact-lbl">合议组 / 审判长</span>
                  <strong class="fact-val">{{ selectedCase.judgePanel || '未填写' }}</strong>
                </div>
                <div class="fact-item-card">
                  <span class="fact-lbl">法官助理 / 书记员联系</span>
                  <strong class="fact-val">{{ selectedCase.clerk || '未填写' }}</strong>
                </div>
              </div>
            </div>
            <!-- 4. 标的物与知识产权要素 -->
            <div class="fact-section-block" v-if="selectedCase.patentName || selectedCase.patentAppNo || selectedCase.track === 'patent_invalidation'">
              <div class="sec-headline">
                <el-icon class="sec-ico"><DocumentCopy /></el-icon>
                <span>标的权属与知识产权要素 (IP Specifics)</span>
              </div>
              <div class="facts-grid-two">
                <div class="fact-item-card">
                  <span class="fact-lbl">涉案标的 / 专利名称</span>
                  <strong class="fact-val">{{ selectedCase.patentName || '未填写' }}</strong>
                </div>
                <div class="fact-item-card">
                  <span class="fact-lbl">专利号 / 申请号</span>
                  <strong class="fact-val mono">{{ selectedCase.patentAppNo || '未填写' }}</strong>
                </div>
              </div>
            </div>
            <!-- 5. 重要法定节点与期限 -->
            <div class="fact-section-block">
              <div class="sec-headline">
                <el-icon class="sec-ico"><Clock /></el-icon>
                <span>重要法定时间节点 (Statutory Milestones)</span>
              </div>
              <div class="facts-grid-four">
                <div class="fact-item-card">
                  <span class="fact-lbl">立案 / 受理日期</span>
                  <strong class="fact-val mono">{{ selectedCase.filingDate || '未填写' }}</strong>
                </div>
                <div class="fact-item-card">
                  <span class="fact-lbl">举证 / 答辩期限</span>
                  <strong class="fact-val mono" :class="{ 'text-risk': selectedCase.reliefDeadline }">{{ selectedCase.reliefDeadline || '未填写' }}</strong>
                </div>
                <div class="fact-item-card">
                  <span class="fact-lbl">开庭 / 口审日期</span>
                  <strong class="fact-val mono">{{ selectedCase.trialDate || '未填写' }}</strong>
                </div>
                <div class="fact-item-card">
                  <span class="fact-lbl">裁判作出 / 结案日</span>
                  <strong class="fact-val mono">{{ selectedCase.verdictDate || '未填写' }}</strong>
                </div>
              </div>
            </div>
          </div>
        </div>
        <!-- B. 🌟 本案全景时间轴 (Timeline · 任务/事件/发文/绝限/收文/备忘笔记按时间全景串联) -->
        <div v-else-if="selectedTab === 'timeline'" class="tab-pane-card timeline-pane-card">
          <div class="timeline-toolbar-flex">
            <div>
              <h3 class="pane-title">本案全景时序脉络 (Timeline)</h3>
              <p class="pane-sub">按时间顺序全量沉淀本案的客观事件、行动待办、收发文存卷、法定绝限与办案笔记。</p>
            </div>
            <!-- 分类过滤器 -->
            <div class="timeline-filter-pills">
              <button class="t-filter-btn" :class="{ active: timelineFilter === 'all' }" @click="timelineFilter = 'all'">全部 ({{ fullTimelineItems.length }})</button>
              <button class="t-filter-btn" :class="{ active: timelineFilter === 'task' }" @click="timelineFilter = 'task'">待办任务</button>
              <button class="t-filter-btn" :class="{ active: timelineFilter === 'event' }" @click="timelineFilter = 'event'">诉讼事件</button>
              <button class="t-filter-btn" :class="{ active: timelineFilter === 'deadline' }" @click="timelineFilter = 'deadline'">期限记录</button>
              <button class="t-filter-btn" :class="{ active: timelineFilter === 'doc' }" @click="timelineFilter = 'doc'">收发文书</button>
              <button class="t-filter-btn" :class="{ active: timelineFilter === 'memo' }" @click="timelineFilter = 'memo'">备忘随笔</button>
            </div>
          </div>
          <!-- 纵向时间轴流 -->
          <div class="chronological-stream">
            <div
              v-for="item in filteredTimelineItems"
              :key="item.id"
              class="timeline-event-card"
              :class="item.type"
            >
              <!-- 时间与标记列 -->
              <div class="timeline-left-col">
                <span class="time-stamp-date">{{ item.date }}</span>
                <span v-if="item.time" class="time-stamp-hour">{{ item.time }}</span>
                <div class="timeline-node-bullet">
                  <el-icon v-if="item.type === 'task'"><Check /></el-icon>
                  <el-icon v-else-if="item.type === 'event'"><OfficeBuilding /></el-icon>
                  <el-icon v-else-if="item.type === 'deadline'"><Warning /></el-icon>
                  <el-icon v-else-if="item.type === 'doc'"><DocumentCopy /></el-icon>
                  <el-icon v-else><ChatDotRound /></el-icon>
                </div>
                <div class="timeline-vertical-line" />
              </div>
              <!-- 事件内容卡片 -->
              <div class="timeline-card-body">
                <div class="t-card-header">
                  <span class="t-type-tag" :class="item.type">
                    {{ item.type === 'task' ? '行动待办' : item.type === 'event' ? '诉讼事件' : item.type === 'deadline' ? '期限记录' : item.type === 'doc' ? '收发文书' : '办案备忘' }}
                  </span>
                  <strong class="t-card-title">{{ item.title }}</strong>
                  <el-button v-if="item.sourceTable==='hearings'" text @click="router.push({name:'case-detail',params:{id:selectedCase.id},query:{tab:'hearings'}})">维护排期</el-button><el-button v-else-if="item.sourceTable==='procedure_events'" text @click="selectedTab='tracks'">核对原事件</el-button>
                  <!-- 任务专有打勾操作与编辑 -->
                  <div v-if="item.type === 'task'" class="t-task-ops">
                    <button class="btn-check-sm" :class="{ checked: item.completed }" @click.stop="toggleCaseTask(item.raw)">
                      {{ item.completed ? '已完成' : '打勾完成' }}
                    </button>
                    <button class="btn-edit-task-sm" @click.stop="openTaskDrawer(item.raw)">
                      <el-icon><Edit /></el-icon>
                      <span>详情</span>
                    </button>
                  </div>
                  <!-- 文书专有打开操作 -->
                  <div v-if="item.type === 'doc'" class="t-task-ops">
                    <button class="btn-open-doc-sm" @click.stop="openFile(item.raw)">打开文书</button>
                  </div>
                </div>
                <p v-if="item.content" class="t-card-desc">{{ item.content }}</p>
                <!-- 标签展示 -->
                <div v-if="item.tags && item.tags.length" class="t-card-tags">
                  <span v-for="tag in item.tags" :key="tag" class="t-tag-pill">{{ tag }}</span>
                </div>
              </div>
            </div>
            <div v-if="!filteredTimelineItems.length" class="empty-timeline-box">
              <el-icon :size="40" color="var(--slate-gray-light)"><Clock /></el-icon>
              <p>暂无该分类时序记录，可点击上方「写记录与备忘」快速录入</p>
            </div>
          </div>
        </div>
        <!-- C. 🌟 写记录与备忘输入口 (Record · 备忘/任务/事件三合一智能录入) -->
        <div v-else-if="selectedTab === 'record'" class="tab-pane-card record-workbench-card">
          <div class="record-pane-header">
            <div>
              <h3 class="pane-title">案件信息补充与日志记录</h3>
              <p class="pane-sub">一站式录入办案备忘纪要、派发待办任务或登记法庭客观事件，支持 NLP 智能识别。</p>
            </div>
            <!-- 录入类型切换 -->
            <div class="record-type-selector">
              <button class="rec-type-btn" :class="{ active: recordType === 'memo' }" @click="recordType = 'memo'">
                <el-icon><ChatDotRound /></el-icon>
                <span>记录备忘 / 纪要</span>
              </button>
              <button class="rec-type-btn" :class="{ active: recordType === 'task' }" @click="recordType = 'task'">
                <el-icon><Check /></el-icon>
                <span>指派 / 记录待办</span>
              </button>
              <button class="rec-type-btn" :class="{ active: recordType === 'event' }" @click="recordType = 'event'">
                <el-icon><OfficeBuilding /></el-icon>
                <span>登记客观法庭事件</span>
              </button>
            </div>
          </div>
          <!-- 1. 记录备忘表单 -->
          <div v-if="recordType === 'memo'" class="record-form-block">
            <div class="form-field-group">
              <label>备忘标题 (选填)</label>
              <input v-model="memoForm.title" placeholder="如：电话沟通纪要 / 争议焦点思路速记" class="rec-native-input" />
            </div>
            <div class="form-field-group">
              <label>备忘正文 (支持随手输入任何案情，可一键提取为待办或事件)</label>
              <textarea
                v-model="memoForm.content"
                placeholder="记录与法官、客户的谈话要点，或办案思路、突发证据线索...
例如：“今天法官通知对方补充了2份对比文件，要求我们在9月15日前提交质证意见，暂定9月28日开庭。”"
                class="rec-native-textarea"
              ></textarea>
            </div>
            <div class="record-form-footer">
              <button class="btn-ai-extract" @click="extractFromMemo" title="自动分析文本中的开庭日期或截止期限">
                <el-icon><MagicStick /></el-icon>
                <span>智能提炼为事件 / 待办</span>
              </button>
              <button class="btn-submit-rec" @click="submitMemo">
                <el-icon><DocumentCopy /></el-icon>
                <span>存入本案备忘库</span>
              </button>
            </div>
          </div>
          <!-- 2. 记录任务表单 -->
          <div v-else-if="recordType === 'task'" class="record-form-block">
            <div class="form-field-group">
              <label>待办任务名称</label>
              <input v-model="taskForm.taskName" placeholder="如：撰写专利无效宣告答辩意见第二部分" class="rec-native-input" />
            </div>
            <div class="rec-grid-two">
              <div class="form-field-group">
                <label>截止日期 (Due Date)</label>
                <input v-model="taskForm.dueDate" type="date" class="rec-native-input" />
              </div>
              <div class="form-field-group">
                <label>预估工时 (分钟)</label>
                <input v-model="taskForm.estimatedMinutes" type="number" step="15" class="rec-native-input" />
              </div>
            </div>
            <div class="rec-grid-two">
              <div class="form-field-group">
                <label>优先级</label>
                <el-select v-model="taskForm.priority" style="width: 100%">
                  <el-option label="普通 (Medium)" value="medium" />
                  <el-option label="重要 (High)" value="high" />
                  <el-option label="重要且紧急 (Urgent)" value="urgent" />
                </el-select>
              </div>
              <div class="form-field-group">
                <label>场景标签 (Context)</label>
                <input v-model="taskForm.context" placeholder="如：@起草 / @开庭准备 / @取证" class="rec-native-input" />
              </div>
            </div>
            <div class="record-form-footer">
              <button class="btn-submit-rec" @click="submitTask">
                <el-icon><Plus /></el-icon>
                <span>创建并排期待办</span>
              </button>
            </div>
          </div>
          <!-- 3. 登记客观事件表单 -->
          <div v-else-if="recordType === 'event'" class="record-form-block">
            <div class="form-field-group">
              <label>事件 / 节点名称</label>
              <input v-model="eventForm.title" placeholder="如：一审第二次开庭审理 / 国知局口头审理辩论" class="rec-native-input" />
            </div>
            <div class="rec-grid-two">
              <div class="form-field-group">
                <label>事件发生 / 开庭日期</label>
                <input v-model="eventForm.eventDate" type="date" class="rec-native-input" />
              </div>
              <div class="form-field-group">
                <label>事件类型</label>
                <el-select v-model="eventForm.eventType" style="width: 100%">
                  <el-option label="法庭开庭 / 口审 (Court/Hearing)" value="court" />
                  <el-option label="法定绝限 / 举证期 (Deadline)" value="deadline" />
                  <el-option label="文书送达 / 收发 (Summons/Doc)" value="summons" />
                  <el-option label="客户会议 / 专家论证 (Meeting)" value="meeting" />
                  <el-option label="其他重要客观节点 (Custom)" value="custom" />
                </el-select>
              </div>
            </div>
            <div class="rec-grid-two">
              <div class="form-field-group">
                <label>审判庭 / 开庭地点</label>
                <input v-model="eventForm.location" placeholder="如：北京知识产权法院第三法庭" class="rec-native-input" />
              </div>
              <div class="form-field-group">
                <label>提前提醒日期 (可选)</label>
                <input v-model="eventForm.reminderDate" type="date" class="rec-native-input" />
              </div>
            </div>
            <div class="record-form-footer">
              <button class="btn-submit-rec" @click="submitEvent">
                <el-icon><OfficeBuilding /></el-icon>
                <span>记入本案时间轴与日历</span>
              </button>
            </div>
          </div>
        </div>
        <!-- D. 通用并行程序 (Parallel Proceedings) -->
        <div v-else-if="selectedTab === 'tracks'" class="tab-pane-card">
          <WhiteboardEntry :key="selectedCase.id" :case-id="selectedCase.id" />
          <ProcedureBoard :key="selectedCase.id" :case-id="selectedCase.id" @changed="loadCaseEvents" @open-case="router.push({name: 'case-detail', params: {id: $event}, query: {tab: 'tracks'}})" />
        </div>
        <!-- E. 卷宗文件 (Files Tab · 唯一的二级入口指向卷宗工作台) -->
        <div v-else-if="selectedTab === 'files'" class="tab-pane-card files-workbench-card">
          <div class="files-workbench-layout">
            <aside class="files-tree-sidebar">
              <div class="tree-sidebar-header">
                <span class="tree-title">卷宗目录树</span>
                <button class="btn-new-subdir" @click="createSubdir" title="新建子文件夹">
                  <el-icon><FolderAdd /></el-icon>
                </button>
              </div>
              <div
                class="dir-tree-node root-node"
                :class="{ active: selectedDirRel === '' }"
                @click="selectedDirRel = ''"
              >
                <el-icon class="dir-ico"><FolderOpened /></el-icon>
                <span class="dir-name-text">{{ selectedCase.caseName || '全部卷宗根目录' }}</span>
                <span class="dir-count-pill">{{ caseFiles.length }}</span>
              </div>
              <div class="dirs-sub-stack">
                <div
                  v-for="d in caseDirs"
                  :key="d.relPath || d.name"
                  class="dir-tree-node sub-node"
                  :class="{ active: selectedDirRel === (d.relPath || d.name) }"
                  @click="selectedDirRel = (d.relPath || d.name)"
                >
                  <el-icon class="dir-ico"><Folder /></el-icon>
                  <span class="dir-name-text">{{ d.name }}</span>
                  <span class="dir-count-pill" v-if="d.fileCount !== undefined">{{ d.fileCount }}</span>
                </div>
              </div>
              <div class="category-filter-section">
                <span class="cat-filter-title">类型筛选</span>
                <div class="cat-pills-stack">
                  <button
                    v-for="cat in fileCategories"
                    :key="cat.key"
                    class="cat-filter-pill"
                    :class="{ active: activeCategory === cat.key }"
                    @click="activeCategory = cat.key"
                  >
                    {{ cat.label }}
                  </button>
                </div>
              </div>
            </aside>
            <section class="files-content-main">
              <div class="files-tab-toolbar">
                <div class="files-toolbar-left">
                  <div class="files-path-breadcrumb">
                    <span class="crumb-link" @click="selectedDirRel = ''">案件卷宗</span>
                    <el-icon class="crumb-sep" :size="10"><ArrowRightBold /></el-icon>
                    <strong class="crumb-current">{{ selectedDirRel ? selectedDirRel : '全部穿透模式' }}</strong>
                    <span class="crumb-mode-tag">({{ selectedDirRel ? '仅当前目录' : '穿透全部子目录' }})</span>
                  </div>
                  <div class="file-sort-pills">
                    <button class="sort-pill" :class="{ active: fileSortOrder === 'added' }" @click="fileSortOrder = 'added'">加入时间</button>
                    <button class="sort-pill" :class="{ active: fileSortOrder === 'recent' }" @click="fileSortOrder = 'recent'">最近使用</button>
                    <button class="sort-pill" :class="{ active: fileSortOrder === 'name' }" @click="fileSortOrder = 'name'">文件名</button>
                  </div>
                </div>
                <div class="files-toolbar-right">
                  <div class="file-search-mini">
                    <el-icon :size="13"><Search /></el-icon>
                    <input v-model="fileSearchQuery" placeholder="搜索卷宗文件..." class="file-search-input" />
                  </div>
                  <button class="btn-jump-files-ws" @click="openSelectedFiles">
                    <el-icon><FolderOpened /></el-icon>
                    <span>进入卷宗管理工作台 →</span>
                  </button>
                </div>
              </div>
              <div class="files-grid-container" v-loading="filesLoading">
                <div
                  v-for="file in sortedCaseFiles"
                  :key="file.id"
                  class="case-file-item-card"
                  @dblclick="openFile(file)"
                  @contextmenu.prevent="onFileContextMenu($event, file)"
                  title="双击直接打开 · 右键呼出系统级操作菜单"
                >
                  <div class="file-card-main">
                    <div class="file-icon-box">
                      <span class="file-ext-tag">{{ getFileExt(file.fileName) }}</span>
                    </div>
                    <div class="file-info-col">
                      <strong class="file-name-txt">{{ file.fileName }}</strong>
                      <div class="file-meta-sub">
                        <span>{{ formatFileSize(file.fileSize) }}</span>
                        <span v-if="file.createdAt">· 加入: {{ file.createdAt.slice(0, 10) }}</span>
                      </div>
                    </div>
                  </div>
                  <div class="file-card-ops">
                    <el-dropdown trigger="click" @click.stop>
                      <button class="btn-file-more" title="操作菜单">
                        <el-icon :size="15"><More /></el-icon>
                      </button>
                      <template #dropdown>
                        <el-dropdown-menu>
                          <el-dropdown-item @click="openFile(file)">
                            <el-icon><View /></el-icon>
                            <span>系统默认应用打开</span>
                          </el-dropdown-item>
                          <el-dropdown-item @click="revealFile(file)">
                            <el-icon><FolderOpened /></el-icon>
                            <span>在访达/资源管理器中显示</span>
                          </el-dropdown-item>
                          <el-dropdown-item @click="copyFilePath(file)">
                            <el-icon><CopyDocument /></el-icon>
                            <span>复制文件路径</span>
                          </el-dropdown-item>
                          <el-dropdown-item divided @click="deleteFile(file)">
                            <el-icon><Delete /></el-icon>
                            <span class="text-risk">从卷宗移除</span>
                          </el-dropdown-item>
                        </el-dropdown-menu>
                      </template>
                    </el-dropdown>
                  </div>
                </div>
                <div v-if="!sortedCaseFiles.length && !filesLoading" class="empty-files-hint">
                  <el-icon :size="38" color="var(--slate-gray-light)"><Document /></el-icon>
                  <p>当前目录/分类下暂无文件 · 可在卷宗管理工作台中导入本地文件</p>
                </div>
              </div>
            </section>
          </div>
        </div>
      </main>
      <div v-else class="empty-selection-view">
        <el-icon :size="48" color="var(--slate-gray-light)"><Briefcase /></el-icon>
        <p>请在左侧选择一个案件</p>
      </div>
    </div>
    <!-- ═══ 任务详情编辑抽屉 (直连 SQLite 与任务系统) ═══ -->
    <el-drawer v-model="showTaskDrawer" title="待办详情编辑" size="420px">
      <div v-if="editingTask" class="task-drawer-content">
        <div class="form-field-group">
          <label>任务名称</label>
          <input v-model="editingTask.taskName" class="rec-native-input" />
        </div>
        <div class="rec-grid-two">
          <div class="form-field-group">
            <label>截止日期</label>
            <input v-model="editingTask.dueDate" type="date" class="rec-native-input" />
          </div>
          <div class="form-field-group">
            <label>截止时刻</label>
            <input v-model="editingTask.dueTime" type="time" class="rec-native-input" />
          </div>
        </div>
        <div class="rec-grid-two">
          <div class="form-field-group">
            <label>优先级</label>
            <el-select v-model="editingTask.priority" style="width: 100%">
              <el-option label="普通 (Medium)" value="medium" />
              <el-option label="重要 (High)" value="high" />
              <el-option label="重要且紧急 (Urgent)" value="urgent" />
            </el-select>
          </div>
          <div class="form-field-group">
            <label>预估工时 (分钟)</label>
            <input v-model="editingTask.estimatedMinutes" type="number" step="15" class="rec-native-input" />
          </div>
        </div>
        <div class="form-field-group">
          <label>场景标签 (Context)</label>
          <input v-model="editingTask.context" placeholder="如：@起草 / @开庭准备" class="rec-native-input" />
        </div>
        <div class="form-field-group">
          <label>任务详细备忘与交接说明</label>
          <textarea v-model="editingTask.description" rows="4" class="rec-native-textarea"></textarea>
        </div>
        <div class="drawer-action-footer">
          <button class="btn-drawer-delete" @click="deleteTaskFromDrawer">删除任务</button>
          <button class="btn-drawer-save" @click="saveEditingTask">保存修改</button>
        </div>
      </div>
    </el-drawer>
    <!-- ═══ 右键上下文菜单 ═══ -->
    <transition name="el-zoom-in-top">
      <div
        v-if="contextMenu.visible && contextMenu.file"
        class="custom-context-menu"
        :style="{ top: `${contextMenu.y}px`, left: `${contextMenu.x}px` }"
        @click.stop
      >
        <div class="menu-item-header">
          <span class="menu-file-name">{{ contextMenu.file.fileName }}</span>
        </div>
        <div class="menu-item" @click="openFile(contextMenu.file); closeContextMenu()">
          <el-icon><View /></el-icon>
          <span>系统默认方式打开</span>
        </div>
        <div class="menu-item" @click="revealFile(contextMenu.file); closeContextMenu()">
          <el-icon><FolderOpened /></el-icon>
          <span>在访达/资源管理器中显示</span>
        </div>
        <div class="menu-item" @click="copyFilePath(contextMenu.file); closeContextMenu()">
          <el-icon><CopyDocument /></el-icon>
          <span>复制文件完整路径</span>
        </div>
        <div class="menu-divider" />
        <div class="menu-item text-risk" @click="deleteFile(contextMenu.file); closeContextMenu()">
          <el-icon><Delete /></el-icon>
          <span>从卷宗中移除</span>
        </div>
      </div>
    </transition>
    <!-- ═══ 编辑案件要素弹窗 ═══ -->
    <el-dialog v-model="showEditOverviewDialog" title="编辑案件要素" width="min(820px, calc(100vw - 24px))" destroy-on-close :close-on-click-modal="false">
      <CaseAttributes v-if="selectedCase" :case-data="selectedCase" initially-editing @saved="showEditOverviewDialog = false; casesStore.loadCases()" />
    </el-dialog>
    <!-- ═══ 新建案件向导 (Sprint 2) ═══ -->
    <CaseWizard v-model="showCaseWizard" :submit="handleCreateCase" />
    <!-- ═══ Excel 案件批量导入向导 ═══ -->
    <CaseImportDialog v-model="showExcelImportDialog" @imported="casesStore.loadCases" />
  </div>
</template>
<style scoped>
/* ═══════════════════════════════════════════════════════════
   Stitch Cases & Matters Workspace Layout
   ═══════════════════════════════════════════════════════════ */
.stitch-cases-view {
  max-width: 1440px;
  margin: 0 auto;
  padding: 16px 24px 32px;
  display: flex;
  flex-direction: column;
  gap: 16px;
  color: var(--c-text);
  font-family: var(--font-family);
  min-height: calc(100vh - 80px);
}
/* ── 顶部 Action Bar ─────────────────────────────────────── */
.cases-topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
}
.topbar-heading {
  display: flex;
  align-items: center;
  gap: 16px;
}
.btn-back-cases-list {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 7px 12px;
  border-radius: var(--c-radius-lg);
  border: 1px solid var(--c-border);
  background: var(--c-bg-card);
  color: var(--c-text);
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  box-shadow: var(--shadow-sm);
  transition: all var(--motion-fast);
}
.btn-back-cases-list:hover {
  background: var(--c-bg-hover);
  border-color: var(--c-primary);
  color: var(--c-primary);
}
.topbar-ws-title {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.eyebrow-kicker {
  font-family: var(--font-mono);
  font-size: 10.5px;
  font-weight: 700;
  text-transform: uppercase;
  color: var(--slate-gray-light);
  letter-spacing: 0.8px;
}
.page-main-title {
  font-size: 24px;
  font-weight: 700;
  color: var(--c-text-heading);
  letter-spacing: -0.3px;
  margin: 0;
}
.topbar-actions {
  display: flex;
  align-items: center;
  gap: 12px;
}
.btn-action-filter {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 14px;
  border-radius: var(--c-radius-lg);
  border: 1px solid var(--c-border);
  background: var(--c-bg-card);
  color: var(--c-text);
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  box-shadow: var(--shadow-sm);
  transition: all var(--motion-fast);
}
.btn-action-filter:hover {
  background: var(--c-bg-hover);
  border-color: var(--c-border-strong);
}
.btn-action-filter.active {
  background: var(--c-primary-light);
  border-color: var(--c-primary);
  color: var(--c-primary);
}
.btn-action-import {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 14px;
  border-radius: var(--c-radius-lg);
  border: 1px solid var(--c-border);
  background: var(--c-bg-card);
  color: var(--c-text-main);
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  box-shadow: var(--shadow-xs);
  transition: all var(--motion-fast);
}
.btn-action-import:hover {
  background: var(--c-bg-hover);
  border-color: var(--c-border-strong);
  color: var(--c-primary);
}
.btn-action-primary {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 16px;
  border-radius: var(--c-radius-lg);
  border: none;
  background: var(--c-primary);
  color: var(--c-primary-contrast);
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  box-shadow: var(--shadow-sm);
  transition: all var(--motion-fast);
}
.btn-action-primary:hover {
  filter: brightness(1.08);
}
.btn-edit-all-facts {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 16px;
  border-radius: var(--c-radius-lg);
  border: 1px solid var(--c-border);
  background: var(--c-bg-card);
  color: var(--c-primary);
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  box-shadow: var(--shadow-sm);
  transition: all var(--motion-fast);
}
.btn-edit-all-facts:hover {
  background: var(--c-primary-light);
  border-color: var(--c-primary);
}
/* ── 筛选面板 ─────────────────────────────────────────────── */
.filter-drawer-well {
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-xl);
  padding: 16px;
  box-shadow: var(--shadow-sm);
}
/* ═══════════════════════════════════════════════════════════
   主从分栏 (Master-Detail) 与 完整工作区模式
   ═══════════════════════════════════════════════════════════ */
.cases-master-detail {
  display: grid;
  grid-template-columns: 290px minmax(0, 1fr);
  gap: 20px;
  align-items: stretch;
  flex: 1;
  transition: all var(--motion-base);
}
.cases-master-detail.full-workspace-mode {
  grid-template-columns: 1fr;
}
/* ── 左侧 320px 案件列表 ─────────────────────────────────── */
.cases-sidebar-index {
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-2xl);
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: calc(100dvh - 166px);
  box-shadow: var(--shadow-sm);
}
.search-input-box {
  position: relative;
  display: flex;
  align-items: center;
}
.search-ico {
  position: absolute;
  left: 10px;
  color: var(--slate-gray-light);
  pointer-events: none;
}
.input-clean {
  width: 100%;
  padding: 7px 10px 7px 32px;
  border-radius: var(--c-radius-lg);
  border: 1px solid var(--c-border);
  background: var(--c-bg-subtle);
  color: var(--c-text);
  font-size: 12.5px;
  outline: none;
  transition: border-color var(--motion-fast);
}
.input-clean:focus {
  border-color: var(--c-primary);
  background: var(--c-bg-card);
}
.cases-scroll-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  overflow-y: auto;
  flex: 1;
  padding-right: 2px;
}
.case-pagination {
  display: flex;
  justify-content: center;
  padding: 8px 0 4px;
}
.case-index-card {
  text-align: left;
  border: 1px solid transparent;
  background: transparent;
  padding: 10px 12px;
  border-radius: var(--c-radius-lg);
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: 4px;
  transition: all var(--motion-fast);
  width: 100%;
}
.case-index-card:hover {
  background: var(--c-bg-hover);
}
.case-index-card.active {
  background: var(--c-primary-light);
  border-color: color-mix(in srgb, var(--c-primary) 30%, transparent);
}
.card-meta-line {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.mono-case-code {
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 700;
  color: var(--slate-gray-light);
}
.case-status-badge {
  font-size: 10px;
  font-weight: 600;
  padding: 1px 6px;
  border-radius: 4px;
}
.badge-active { background: var(--c-bg-subtle); color: var(--c-text-regular); }
.badge-risk { background: var(--bg-risk-weak); color: var(--status-risk); font-weight: 700; }
.badge-discovery { background: color-mix(in srgb, var(--status-discovery) 15%, transparent); color: var(--status-discovery); font-weight: 700; }
.badge-warning { background: var(--bg-warning-weak); color: var(--status-warning); font-weight: 700; }
.badge-muted { background: var(--c-bg-subtle); color: var(--slate-gray-light); }
.case-card-title {
  font-size: 13px;
  font-weight: 700;
  color: var(--c-text-heading);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.case-card-client {
  font-size: 11px;
  color: var(--slate-gray-light);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.empty-case-hint {
  padding: 30px 0;
  text-align: center;
  color: var(--slate-gray-light);
  font-size: 12px;
}
/* ── 右侧上下文工作台 ─────────────────────────────────────── */
.cases-detail-area {
  display: flex;
  flex-direction: column;
  gap: 16px;
  overflow-y: auto;
}
/* 1. Summary Header 卡片 */
.matter-summary-header-card {
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-2xl);
  padding: 22px 24px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 24px;
  box-shadow: var(--shadow-sm);
}
.summary-left-group {
  display: flex;
  align-items: center;
  gap: 20px;
  flex: 1;
  min-width: 0;
}
.summary-ring-wrapper {
  position: relative;
  width: 76px;
  height: 76px;
  flex-shrink: 0;
}
.summary-ring-svg {
  width: 100%;
  height: 100%;
  transform: rotate(-90deg);
}
.ring-bg {
  fill: none;
  stroke: var(--c-bg-subtle);
  stroke-width: 3.5;
}
.ring-progress {
  fill: none;
  stroke: var(--c-primary);
  stroke-width: 3.5;
  stroke-linecap: round;
  transition: stroke-dasharray 0.8s var(--ease-out);
}
.ring-center-content {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
}
.ring-stage-num {
  font-family: var(--font-mono);
  font-size: 14px;
  font-weight: 700;
  color: var(--c-text-heading);
  line-height: 1;
}
.ring-stage-label {
  font-size: 9.5px;
  color: var(--slate-gray-light);
  margin-top: 2px;
}
.summary-matter-identity {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}
.identity-badge-row {
  display: flex;
  align-items: center;
  gap: 8px;
}
.badge-matter-pill {
  padding: 2px 7px;
  background: var(--c-primary-light);
  color: var(--c-primary);
  font-family: var(--font-mono);
  font-size: 10.5px;
  font-weight: 700;
  border-radius: 4px;
}
.badge-pat-code {
  padding: 2px 7px;
  border: 1px solid var(--c-border);
  font-family: var(--font-mono);
  font-size: 10.5px;
  color: var(--slate-gray-light);
  border-radius: 4px;
}
.matter-main-name {
  font-size: 22px;
  font-weight: 700;
  color: var(--c-text-heading);
  letter-spacing: -0.3px;
  margin: 2px 0 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.matter-meta-lead {
  font-size: 12px;
  color: var(--slate-gray-light);
  margin: 0;
  word-break: keep-all;
  overflow-wrap: break-word;
}
/* 右侧 Next Actions 待办列表卡片 */
.summary-next-actions-card {
  width: 320px;
  background: var(--c-bg-subtle);
  border: 1px solid color-mix(in srgb, var(--status-risk) 30%, var(--c-border));
  border-left: 4.5px solid var(--status-risk);
  border-radius: var(--c-radius-xl);
  padding: 14px 16px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  box-shadow: var(--shadow-sm);
}
.action-card-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--c-border);
  padding-bottom: 6px;
}
.label-next-act {
  font-size: 11px;
  font-weight: 700;
  color: var(--status-risk);
  text-transform: uppercase;
}
.mono-due-act {
  font-family: var(--font-mono);
  font-size: 10.5px;
  color: var(--slate-gray-light);
}
.upcoming-tasks-stream {
  display: flex;
  flex-direction: column;
  gap: 5px;
  max-height: 96px;
  overflow-y: auto;
}
.upcoming-task-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 3px 6px;
  border-radius: 4px;
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  cursor: pointer;
  transition: all var(--motion-fast);
}
.upcoming-task-row:hover {
  background: var(--c-bg-hover);
  border-color: var(--c-primary);
}
.task-row-check {
  width: 14px;
  height: 14px;
  border-radius: 3px;
  border: 1.5px solid var(--c-border-strong);
  background: transparent;
  cursor: pointer;
  display: grid;
  place-items: center;
  color: #fff;
  flex-shrink: 0;
}
.task-row-check.checked {
  background: var(--c-primary);
  border-color: var(--c-primary);
}
.task-row-name {
  font-size: 11.5px;
  color: var(--c-text-heading);
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.task-row-name.struck {
  text-decoration: line-through;
  color: var(--slate-gray-light);
}
.task-row-plan { font-size:11px; color:var(--c-text-secondary); max-width:140px; min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.task-row-due {
  font-family: var(--font-mono);
  font-size: 10px;
  color: var(--status-risk);
  flex-shrink: 0;
}
.empty-upcoming-box {
  font-size: 11px;
  color: var(--slate-gray-light);
  text-align: center;
  padding: 10px 0;
}
/* 2. Tabs 栏 */
.matter-detail-tabs-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid var(--c-border);
  padding-bottom: 2px;
}
.tabs-group-left {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  overflow-x: auto;
  padding-bottom: 4px;
}
.tab-btn {
  flex-shrink: 0;
  white-space: nowrap;
  border: none;
  background: transparent;
  padding: 8px 12px;
  font-size: 13px;
  font-weight: 600;
  color: var(--slate-gray-light);
  cursor: pointer;
  position: relative;
  transition: color var(--motion-fast);
}
.tab-btn:hover { color: var(--c-text); }
.tab-btn.active {
  color: var(--c-primary);
  font-weight: 700;
}
.tab-btn.active::after {
  content: '';
  position: absolute;
  bottom: -3px;
  left: 12px;
  right: 12px;
  height: 2.5px;
  background: var(--c-primary);
  border-radius: 2px;
}
/* 纯文字轻量链接 (放置在最左侧，无框无分割线) */
.link-jump-workspace-head {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 8px 10px;
  border: none;
  background: transparent;
  color: var(--c-primary);
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  text-decoration: none;
  transition: all var(--motion-fast);
}
.link-jump-workspace-head:hover {
  color: var(--c-primary);
  text-decoration: underline;
  gap: 6px;
}
/* 3. Tab 内容卡片 */
.tab-pane-card {
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-2xl);
  padding: 20px 24px;
  box-shadow: var(--shadow-sm);
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.overview-pane-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--c-border);
}
.btn-edit-facts {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 5px 12px;
  border-radius: var(--c-radius-lg);
  border: 1px solid var(--c-border);
  background: var(--c-bg-subtle);
  color: var(--c-text);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: all var(--motion-fast);
}
.btn-edit-facts:hover {
  background: var(--c-primary-light);
  border-color: var(--c-primary);
  color: var(--c-primary);
}
.pane-title {
  font-size: 16px;
  font-weight: 700;
  color: var(--c-text-heading);
  margin: 0;
}
.pane-sub {
  font-size: 12px;
  color: var(--slate-gray-light);
  margin: 4px 0 0;
}
/* A. 案件要素全景 (Overview) */
.facts-sections-container {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.fact-section-block {
  background: var(--c-bg-page);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-xl);
  padding: 14px 16px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.sec-headline {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12.5px;
  font-weight: 700;
  color: var(--c-text-heading);
}
.sec-ico { color: var(--c-primary); }
.facts-grid-two {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 10px;
}
.facts-grid-three {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 10px;
}
.facts-grid-four {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 10px;
}
.fact-item-card {
  padding: 10px 12px;
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  display: flex;
  flex-direction: column;
  gap: 3px;
}
.fact-item-card.highlight-client {
  border-left: 3.5px solid var(--c-primary);
}
.fact-item-card.highlight-opponent {
  border-left: 3.5px solid var(--status-risk);
}
.fact-lbl {
  font-size: 11px;
  color: var(--slate-gray-light);
}
.fact-val {
  font-size: 13px;
  font-weight: 600;
  color: var(--c-text-heading);
}
.fact-val.mono { font-family: var(--font-mono); font-size: 12px; }
.fact-val.text-primary { color: var(--c-primary); }
.fact-val.text-risk { color: var(--status-risk); }
.fact-sub-txt {
  font-size: 11px;
  color: var(--slate-gray-light);
  margin-top: 2px;
}
/* B. 🌟 本案全景时间轴 (Timeline) 样式 */
.timeline-pane-card {
  padding: 20px 24px;
}
.timeline-toolbar-flex {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  padding-bottom: 14px;
  border-bottom: 1px solid var(--c-border);
  flex-wrap: wrap;
}
.timeline-filter-pills {
  display: flex;
  background: var(--c-bg-subtle);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  padding: 3px;
  gap: 2px;
  flex-wrap: wrap;
}
.t-filter-btn {
  padding: 4px 10px;
  border-radius: 4px;
  border: none;
  background: transparent;
  color: var(--c-text-secondary);
  font-size: 11.5px;
  font-weight: 600;
  cursor: pointer;
  transition: all var(--motion-fast);
}
.t-filter-btn.active {
  background: var(--c-bg-card);
  color: var(--c-primary);
  font-weight: 700;
  box-shadow: var(--shadow-sm);
}
.chronological-stream {
  display: flex;
  flex-direction: column;
  gap: 0;
  padding: 10px 0;
}
.timeline-event-card {
  display: flex;
  gap: 16px;
  position: relative;
}
.timeline-left-col {
  width: 90px;
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  position: relative;
  flex-shrink: 0;
  padding-top: 2px;
}
.time-stamp-date {
  font-family: var(--font-mono);
  font-size: 11.5px;
  font-weight: 700;
  color: var(--c-text-heading);
}
.time-stamp-hour {
  font-family: var(--font-mono);
  font-size: 10px;
  color: var(--slate-gray-light);
}
.timeline-node-bullet {
  position: absolute;
  right: -23px;
  top: 2px;
  width: 22px;
  height: 22px;
  border-radius: 50%;
  background: var(--c-bg-card);
  border: 2px solid var(--c-border);
  display: grid;
  place-items: center;
  font-size: 11px;
  color: var(--slate-gray-light);
  z-index: 2;
}
.timeline-event-card.task .timeline-node-bullet { border-color: var(--c-primary); color: var(--c-primary); }
.timeline-event-card.event .timeline-node-bullet { border-color: #3b82f6; background: #eff6ff; color: #3b82f6; }
.timeline-event-card.deadline .timeline-node-bullet { border-color: var(--status-risk); background: var(--bg-risk-weak); color: var(--status-risk); }
.timeline-event-card.doc .timeline-node-bullet { border-color: var(--status-discovery); color: var(--status-discovery); }
.timeline-event-card.memo .timeline-node-bullet { border-color: var(--status-warning); background: var(--bg-warning-weak); color: var(--status-warning); }
.timeline-vertical-line {
  position: absolute;
  right: -13px;
  top: 24px;
  bottom: -16px;
  width: 2px;
  background: var(--c-border);
}
.timeline-card-body {
  flex: 1;
  background: var(--c-bg-page);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  padding: 12px 16px;
  margin-bottom: 16px;
  margin-left: 20px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  transition: all var(--motion-fast);
}
.timeline-card-body:hover {
  background: var(--c-bg-hover);
  border-color: var(--c-border-strong);
}
.t-card-header {
  display: flex;
  align-items: center;
  gap: 8px;
}
.t-type-tag {
  font-size: 10px;
  font-weight: 700;
  padding: 2px 6px;
  border-radius: 4px;
  text-transform: uppercase;
}
.t-type-tag.task { background: var(--c-primary-light); color: var(--c-primary); }
.t-type-tag.event { background: #dbeafe; color: #1d4ed8; }
.t-type-tag.deadline { background: var(--bg-risk-weak); color: var(--status-risk); }
.t-type-tag.doc { background: color-mix(in srgb, var(--status-discovery) 15%, transparent); color: var(--status-discovery); }
.t-type-tag.memo { background: var(--bg-warning-weak); color: #d97706; }
.t-card-title {
  font-size: 13.5px;
  font-weight: 600;
  color: var(--c-text-heading);
  flex: 1;
}
.t-card-desc {
  font-size: 12px;
  color: var(--c-text-secondary);
  margin: 0;
  line-height: 1.45;
  white-space: pre-wrap;
}
.t-card-tags {
  display: flex;
  gap: 6px;
  margin-top: 4px;
}
.t-tag-pill {
  font-size: 10.5px;
  padding: 1px 6px;
  border-radius: 4px;
  background: var(--c-bg-subtle);
  color: var(--slate-gray-light);
}
.t-task-ops {
  display: flex;
  align-items: center;
  gap: 6px;
}
.btn-check-sm {
  padding: 3px 8px;
  font-size: 11px;
  border-radius: 4px;
  border: 1px solid var(--c-border);
  background: var(--c-bg-card);
  color: var(--c-text);
  cursor: pointer;
}
.btn-check-sm.checked {
  background: var(--c-primary-light);
  color: var(--c-primary);
  border-color: var(--c-primary);
}
.btn-edit-task-sm, .btn-open-doc-sm {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 3px 8px;
  font-size: 11px;
  border-radius: 4px;
  border: 1px solid var(--c-border);
  background: var(--c-bg-card);
  color: var(--c-text);
  cursor: pointer;
}
.empty-timeline-box {
  padding: 40px 0;
  text-align: center;
  color: var(--slate-gray-light);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  font-size: 13px;
}
/* C. 🌟 写记录与备忘 (Record) 样式 */
.record-workbench-card {
  padding: 24px;
}
.record-pane-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  padding-bottom: 16px;
  border-bottom: 1px solid var(--c-border);
  flex-wrap: wrap;
}
.record-type-selector {
  display: flex;
  background: var(--c-bg-subtle);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  padding: 3px;
  gap: 3px;
}
.rec-type-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 14px;
  border-radius: 6px;
  border: none;
  background: transparent;
  color: var(--c-text-secondary);
  font-size: 12.5px;
  font-weight: 600;
  cursor: pointer;
  transition: all var(--motion-fast);
}
.rec-type-btn.active {
  background: var(--c-bg-card);
  color: var(--c-primary);
  font-weight: 700;
  box-shadow: var(--shadow-sm);
}
.record-form-block {
  display: flex;
  flex-direction: column;
  gap: 16px;
  margin-top: 10px;
}
.form-field-group {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.form-field-group label {
  font-size: 12px;
  font-weight: 600;
  color: var(--c-text-heading);
}
.rec-native-input {
  width: 100%;
  padding: 8px 12px;
  border-radius: var(--c-radius-lg);
  border: 1px solid var(--c-border);
  background: var(--c-bg-page);
  color: var(--c-text);
  font-size: 13px;
  outline: none;
}
.rec-native-input:focus {
  border-color: var(--c-primary);
  background: var(--c-bg-card);
}
.rec-native-textarea {
  width: 100%;
  min-height: 160px;
  padding: 12px 14px;
  border-radius: var(--c-radius-lg);
  border: 1px solid var(--c-border);
  background: var(--c-bg-page);
  color: var(--c-text);
  font-family: var(--font-family);
  font-size: 13px;
  line-height: 1.6;
  outline: none;
  resize: vertical;
}
.rec-native-textarea:focus {
  border-color: var(--c-primary);
  background: var(--c-bg-card);
}
.rec-grid-two {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
}
.record-form-footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 12px;
  padding-top: 10px;
  border-top: 1px solid var(--c-border);
}
.btn-ai-extract {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 14px;
  border-radius: var(--c-radius-lg);
  border: 1px solid var(--c-primary);
  background: var(--c-primary-light);
  color: var(--c-primary);
  font-size: 12.5px;
  font-weight: 600;
  cursor: pointer;
  transition: all var(--motion-fast);
}
.btn-ai-extract:hover {
  background: var(--c-primary);
  color: var(--c-primary-contrast);
}
.btn-submit-rec {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 20px;
  border-radius: var(--c-radius-lg);
  border: none;
  background: var(--c-primary);
  color: var(--c-primary-contrast);
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  box-shadow: var(--shadow-sm);
  transition: all var(--motion-fast);
}
.btn-submit-rec:hover {
  filter: brightness(1.08);
}
/* D. 通用并行程序 */
.proceedings-list-stack {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.proceeding-track-box {
  background: var(--c-bg-page);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-xl);
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.proc-header-line {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.proc-name-badge {
  display: flex;
  align-items: center;
  gap: 8px;
}
.proc-title {
  font-size: 14px;
  color: var(--c-text-heading);
}
.proc-status-tag {
  font-size: 11px;
  padding: 2px 7px;
  border-radius: 4px;
  background: var(--c-primary-light);
  color: var(--c-primary);
  font-weight: 600;
}
.proc-pct-num {
  font-family: var(--font-mono);
  font-size: 12px;
  font-weight: 700;
  color: var(--c-primary);
}
.proc-bar-bg {
  width: 100%;
  height: 6px;
  background: var(--c-bg-subtle);
  border-radius: var(--c-radius-full);
  overflow: hidden;
}
.proc-bar-fill {
  height: 100%;
  border-radius: var(--c-radius-full);
  transition: width 0.8s var(--ease-out);
}
.standard-nodes-flow {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 8px;
  position: relative;
  padding: 0 8px;
}
.proc-step-node {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  position: relative;
  flex: 1;
  z-index: 1;
}
.step-bullet-circle {
  width: 22px;
  height: 22px;
  border-radius: 50%;
  background: var(--c-bg-card);
  border: 2px solid var(--c-border);
  display: grid;
  place-items: center;
  font-size: 10px;
  font-weight: 700;
  color: var(--slate-gray-light);
}
.proc-step-node.completed .step-bullet-circle {
  background: var(--c-primary);
  border-color: var(--c-primary);
  color: var(--c-primary-contrast);
}
.proc-step-node.current .step-bullet-circle {
  border-color: var(--c-primary);
  color: var(--c-primary);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--c-primary) 20%, transparent);
}
.step-node-label {
  font-size: 11px;
  font-weight: 600;
  color: var(--slate-gray-light);
  text-align: center;
}
.proc-step-node.completed .step-node-label { color: var(--c-text-heading); }
.proc-step-node.current .step-node-label { color: var(--c-primary); font-weight: 700; }
.step-node-connector {
  position: absolute;
  top: 11px;
  left: 50%;
  right: -50%;
  height: 2px;
  background: var(--c-border);
  z-index: -1;
}
.proc-step-node.completed .step-node-connector {
  background: var(--c-primary);
}
/* E. 卷宗工作台 (左目录树 + 右文件列表) */
.files-workbench-card {
  padding: 0;
  overflow: hidden;
}
.files-workbench-layout {
  display: grid;
  grid-template-columns: 240px 1fr;
  min-height: 480px;
}
.files-tree-sidebar {
  background: var(--c-bg-subtle);
  border-right: 1px solid var(--c-border);
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.tree-sidebar-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-bottom: 4px;
}
.tree-title {
  font-size: 12px;
  font-weight: 700;
  color: var(--slate-gray-light);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}
.btn-new-subdir {
  border: none;
  background: transparent;
  color: var(--slate-gray-light);
  cursor: pointer;
  padding: 2px 4px;
  border-radius: 4px;
}
.btn-new-subdir:hover {
  background: var(--c-bg-card);
  color: var(--c-primary);
}
.dir-tree-node {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  border-radius: var(--c-radius-md);
  cursor: pointer;
  transition: all var(--motion-fast);
  color: var(--c-text);
  font-size: 12.5px;
}
.dir-tree-node:hover {
  background: var(--c-bg-hover);
}
.dir-tree-node.active {
  background: var(--c-primary-light);
  color: var(--c-primary);
  font-weight: 600;
}
.dir-tree-node.root-node {
  font-weight: 600;
  border: 1px solid var(--c-border);
  background: var(--c-bg-card);
}
.dir-tree-node.root-node.active {
  background: var(--c-primary-light);
  border-color: var(--c-primary);
}
.dir-ico { font-size: 15px; flex-shrink: 0; }
.dir-name-text { flex: 1; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.dir-count-pill { font-family: var(--font-mono); font-size: 10px; background: var(--c-bg-page); padding: 1px 6px; border-radius: 10px; color: var(--slate-gray-light); }
.dirs-sub-stack {
  display: flex;
  flex-direction: column;
  gap: 3px;
  padding-left: 8px;
  border-left: 2px solid var(--c-border);
  margin-left: 6px;
}
.category-filter-section {
  margin-top: auto;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding-top: 12px;
  border-top: 1px solid var(--c-border);
}
.cat-filter-title {
  font-size: 11px;
  font-weight: 600;
  color: var(--slate-gray-light);
}
.cat-pills-stack {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.cat-filter-pill {
  border: 1px solid var(--c-border);
  background: var(--c-bg-card);
  padding: 3px 7px;
  border-radius: 4px;
  font-size: 10.5px;
  color: var(--c-text-secondary);
  cursor: pointer;
  transition: all var(--motion-fast);
}
.cat-filter-pill:hover {
  color: var(--c-text);
  border-color: var(--c-border-strong);
}
.cat-filter-pill.active {
  background: var(--c-primary);
  border-color: var(--c-primary);
  color: #ffffff;
  font-weight: 600;
}
.files-content-main {
  padding: 16px 20px;
  display: flex;
  flex-direction: column;
  gap: 14px;
  overflow-y: auto;
}
.files-tab-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  flex-wrap: wrap;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--c-border);
}
.files-toolbar-left {
  display: flex;
  align-items: center;
  gap: 16px;
  flex-wrap: wrap;
}
.files-path-breadcrumb {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12.5px;
}
.crumb-link { color: var(--c-primary); cursor: pointer; }
.crumb-link:hover { text-decoration: underline; }
.crumb-sep { color: var(--slate-gray-light); }
.crumb-current { color: var(--c-text-heading); }
.crumb-mode-tag { font-size: 11px; color: var(--slate-gray-light); }
.files-toolbar-right {
  display: flex;
  align-items: center;
  gap: 10px;
}
.file-sort-pills {
  display: flex;
  background: var(--c-bg-subtle);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  padding: 2px;
  gap: 2px;
}
.sort-pill {
  padding: 4px 10px;
  border-radius: 4px;
  border: none;
  background: transparent;
  color: var(--c-text-secondary);
  font-size: 11.5px;
  font-weight: 600;
  cursor: pointer;
}
.sort-pill.active {
  background: var(--c-bg-card);
  color: var(--c-text);
  box-shadow: var(--shadow-sm);
}
.file-search-mini {
  position: relative;
  display: flex;
  align-items: center;
}
.file-search-mini .el-icon {
  position: absolute;
  left: 8px;
  color: var(--slate-gray-light);
}
.file-search-input {
  padding: 4px 8px 4px 26px;
  background: var(--c-bg-page);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius);
  font-size: 12px;
  outline: none;
  color: var(--c-text);
}
.btn-jump-files-ws {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  border-radius: var(--c-radius-lg);
  border: none;
  background: var(--c-primary);
  color: var(--c-primary-contrast);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  box-shadow: var(--shadow-sm);
}
.files-grid-container {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: 10px;
  min-height: 240px;
}
.case-file-item-card {
  padding: 10px 12px;
  border-radius: var(--c-radius-lg);
  border: 1px solid var(--c-border);
  background: var(--c-bg-page);
  display: flex;
  align-items: center;
  justify-content: space-between;
  cursor: pointer;
  transition: all var(--motion-fast);
}
.case-file-item-card:hover {
  background: var(--c-bg-hover);
  border-color: var(--c-primary);
  transform: translateY(-1px);
  box-shadow: var(--shadow-sm);
}
.file-card-main {
  display: flex;
  align-items: center;
  gap: 10px;
  flex: 1;
  min-width: 0;
}
.file-icon-box {
  width: 36px;
  height: 36px;
  border-radius: var(--c-radius);
  background: var(--c-primary-light);
  color: var(--c-primary);
  display: grid;
  place-items: center;
  flex-shrink: 0;
}
.file-ext-tag {
  font-family: var(--font-mono);
  font-size: 9.5px;
  font-weight: 700;
}
.file-info-col {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}
.file-name-txt {
  font-size: 12px;
  color: var(--c-text-heading);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.file-meta-sub {
  font-family: var(--font-mono);
  font-size: 10.5px;
  color: var(--slate-gray-light);
}
.btn-file-more {
  border: none;
  background: transparent;
  color: var(--slate-gray-light);
  cursor: pointer;
  padding: 4px;
  border-radius: 4px;
}
.btn-file-more:hover {
  background: var(--c-bg-subtle);
  color: var(--c-text);
}
.empty-files-hint {
  grid-column: 1 / -1;
  padding: 40px 0;
  text-align: center;
  color: var(--slate-gray-light);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  font-size: 12.5px;
}
/* 抽屉样式 */
.task-drawer-content {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 10px 0;
}
.drawer-action-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 20px;
  padding-top: 14px;
  border-top: 1px solid var(--c-border);
}
.btn-drawer-delete {
  padding: 7px 14px;
  border-radius: var(--c-radius-lg);
  border: 1px solid var(--status-risk);
  background: var(--bg-risk-weak);
  color: var(--status-risk);
  font-size: 12.5px;
  font-weight: 600;
  cursor: pointer;
}
.btn-drawer-save {
  padding: 7px 20px;
  border-radius: var(--c-radius-lg);
  border: none;
  background: var(--c-primary);
  color: var(--c-primary-contrast);
  font-size: 12.5px;
  font-weight: 600;
  cursor: pointer;
}
/* 右键上下文菜单 */
.custom-context-menu {
  position: fixed;
  z-index: 3000;
  width: 200px;
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-radius: var(--c-radius-lg);
  box-shadow: var(--shadow-lg, 0 10px 25px rgba(0, 0, 0, 0.15));
  padding: 6px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.menu-item-header {
  padding: 4px 8px;
  font-size: 11px;
  color: var(--slate-gray-light);
  border-bottom: 1px solid var(--c-border);
  margin-bottom: 2px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.menu-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 10px;
  border-radius: 4px;
  font-size: 12px;
  color: var(--c-text);
  cursor: pointer;
  transition: all var(--motion-fast);
}
.menu-item:hover {
  background: var(--c-primary-light);
  color: var(--c-primary);
}
.menu-item.text-risk:hover {
  background: var(--bg-risk-weak);
  color: var(--status-risk);
}
.menu-divider {
  height: 1px;
  background: var(--c-border);
  margin: 4px 0;
}
.empty-selection-view {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 400px;
  color: var(--slate-gray-light);
  gap: 12px;
}
/* 模态框 */
.edit-facts-dialog-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.form-field-item {
  display: flex;
  flex-direction: column;
  gap: 5px;
  flex: 1;
}
.form-field-item label {
  font-size: 11.5px;
  font-weight: 600;
  color: var(--c-text-heading);
}
.dialog-native-input {
  width: 100%;
  padding: 7px 10px;
  border-radius: var(--c-radius);
  border: 1px solid var(--c-border);
  background: var(--c-bg-page);
  color: var(--c-text);
  font-size: 12.5px;
  outline: none;
}
.dialog-native-input:focus { border-color: var(--c-primary); }
.modal-header-copy h2 { font-size: 18px; font-weight: 700; margin: 0; }
.modal-header-copy p { font-size: 12px; color: var(--slate-gray-light); margin: 4px 0 0; }
.form-row-two { display: grid; grid-template-columns: 1fr 1fr; gap: 14px; }
.modal-footer-actions { display: flex; justify-content: flex-end; gap: 10px; }
.btn-cancel { padding: 6px 14px; border-radius: var(--c-radius-lg); border: 1px solid var(--c-border); background: var(--c-bg-subtle); cursor: pointer; }
.btn-submit-primary { padding: 6px 16px; border-radius: var(--c-radius-lg); border: none; background: var(--c-primary); color: var(--c-primary-contrast); font-weight: 600; cursor: pointer; }
/* 中等宽度下，摘要头部改为纵向堆叠，避免身份列被挤压成逐字竖排 */
@media (max-width: 1360px) {
  .matter-summary-header-card { flex-direction: column; align-items: flex-start; }
  .summary-next-actions-card { width: 100%; }
}
@media (max-width: 1024px) {
  .cases-master-detail { grid-template-columns: 1fr; }
  .matter-summary-header-card { flex-direction: column; align-items: flex-start; }
  .summary-next-actions-card { width: 100%; }
  .files-workbench-layout { grid-template-columns: 1fr; }
  .facts-grid-three, .facts-grid-four { grid-template-columns: 1fr; }
  .rec-grid-two { grid-template-columns: 1fr; }
}
.matter-tools { display: flex; align-items: center; flex-wrap: wrap; gap: 4px 8px; }
.matter-tools .el-button { margin-left: 0; }
.matter-tools .link-jump-workspace-head { margin-left: auto; }
.overview-pane-header { gap: 16px; flex-wrap: wrap; }
@container (max-width: 800px) {
  .cases-topbar { flex-wrap: wrap; }
  .topbar-actions { flex-wrap: wrap; gap: 8px; }
  .cases-master-detail { grid-template-columns: minmax(0, 1fr); }
  .cases-sidebar-index { height: auto; max-height: 350px; }
  .tab-pane-card { padding: 18px 16px; }
  .matter-tools .link-jump-workspace-head { margin-left: 0; }
}
</style>
