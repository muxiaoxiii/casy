<script setup>
import { ref, computed, onMounted } from 'vue'
import { tauriCallSafe } from '../../../core/tauriBridge.js'
import { ElMessage } from 'element-plus'

const tasks = ref([])
const loading = ref(false)
const showCreateDialog = ref(false)
const newTask = ref({
  taskName: '',
  description: '',
  deadline: '',
  priority: 'normal',
  caseId: '',
})

// 任务编辑抽屉
const showDrawer = ref(false)
const editingTask = ref(null)
const editForm = ref({
  taskName: '',
  description: '',
  deadline: '',
  priority: 'normal',
  caseId: '',
})
const savingTask = ref(false)
const caseSearchQuery = ref('')
const caseSearchResults = ref([])
const searchingCases = ref(false)

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

onMounted(() => {
  loadTasks()
})

async function loadTasks() {
  loading.value = true
  const result = await tauriCallSafe('list_tasks', { filter: { completed: false } })
  if (result.ok) {
    tasks.value = result.data || []
  }
  loading.value = false
}

const quadrants = computed(() => ({
  urgentImportant: tasks.value.filter((t) => t.priority === 'urgent_important' && !t.completed),
  important: tasks.value.filter((t) => t.priority === 'important' && !t.completed),
  urgent: tasks.value.filter((t) => t.priority === 'urgent' && !t.completed),
  normal: tasks.value.filter((t) => t.priority === 'normal' && !t.completed),
}))

async function toggleComplete(task) {
  const result = await tauriCallSafe('toggle_task', { id: task.id })
  if (result.ok) {
    task.completed = task.completed ? 0 : 1
    ElMessage.success(task.completed ? '已完成' : '已恢复')
  }
}

async function deleteTask(task) {
  const result = await tauriCallSafe('delete_task', { id: task.id })
  if (result.ok) {
    tasks.value = tasks.value.filter((t) => t.id !== task.id)
    ElMessage.success('已删除')
  }
}

async function createTask() {
  if (!newTask.value.taskName.trim()) {
    ElMessage.warning('请输入任务名称')
    return
  }
  const result = await tauriCallSafe('create_task', { data: newTask.value })
  if (result.ok) {
    ElMessage.success('任务已创建')
    showCreateDialog.value = false
    newTask.value = { taskName: '', description: '', deadline: '', priority: 'normal', caseId: '' }
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
  const result = await tauriCallSafe('update_task', {
    id: editingTask.value.id,
    data: editForm.value,
  })
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

function deadlineText(deadline) {
  const days = daysUntil(deadline)
  if (days === null) return ''
  if (days < 0) return `已逾期${Math.abs(days)}天`
  if (days === 0) return '今天到期'
  return `${days}天`
}

function isOverdue(deadline) {
  const days = daysUntil(deadline)
  return days !== null && days < 0
}
</script>

<template>
  <div class="tasks-page">
    <div class="toolbar">
      <h3>📌 任务管理</h3>
      <el-button type="primary" size="small" @click="showCreateDialog = true">➕ 新建任务</el-button>
    </div>

    <div v-if="loading" class="skeleton-wrapper">
      <el-skeleton :rows="8" animated>
        <template #template>
          <div class="skeleton-quadrants">
            <div v-for="i in 4" :key="i" class="skeleton-quadrant">
              <el-skeleton-item variant="rect" style="width: 100%; height: 32px; border-radius: 4px 4px 0 0;" />
              <div style="padding: 8px;">
                <div v-for="j in 3" :key="j" class="skeleton-task-row">
                  <el-skeleton-item variant="circle" style="width: 16px; height: 16px;" />
                  <el-skeleton-item variant="text" style="width: 60%; height: 16px;" />
                  <el-skeleton-item variant="text" style="width: 20%; height: 16px;" />
                </div>
              </div>
            </div>
          </div>
        </template>
      </el-skeleton>
    </div>

    <div v-else class="quadrants-grid">
      <!-- 重要紧急 -->
      <div class="quadrant">
        <div class="quadrant-header urgent-important">重要紧急 ({{ quadrants.urgentImportant.length }})</div>
        <div class="quadrant-body">
          <div v-for="task in quadrants.urgentImportant" :key="task.id" class="task-card" :class="{ 'overdue-card': isOverdue(task.deadline) }">
            <div class="task-main">
              <el-checkbox :model-value="!!task.completed" @change="toggleComplete(task)" />
              <span class="task-name clickable" :class="{ done: task.completed }" @click="openDrawer(task)">{{ task.taskName }}</span>
            </div>
            <div v-if="task.deadline" class="task-deadline" :class="{ overdue: isOverdue(task.deadline) }">
              {{ deadlineText(task.deadline) }}
            </div>
            <el-button size="small" text type="danger" @click="deleteTask(task)">×</el-button>
          </div>
          <div v-if="!quadrants.urgentImportant.length" class="empty-quadrant">无</div>
        </div>
      </div>

      <!-- 重要不紧急 -->
      <div class="quadrant">
        <div class="quadrant-header important">重要不紧急 ({{ quadrants.important.length }})</div>
        <div class="quadrant-body">
          <div v-for="task in quadrants.important" :key="task.id" class="task-card" :class="{ 'overdue-card': isOverdue(task.deadline) }">
            <div class="task-main">
              <el-checkbox :model-value="!!task.completed" @change="toggleComplete(task)" />
              <span class="task-name clickable" :class="{ done: task.completed }" @click="openDrawer(task)">{{ task.taskName }}</span>
            </div>
            <div v-if="task.deadline" class="task-deadline" :class="{ overdue: isOverdue(task.deadline) }">
              {{ deadlineText(task.deadline) }}
            </div>
            <el-button size="small" text type="danger" @click="deleteTask(task)">×</el-button>
          </div>
          <div v-if="!quadrants.important.length" class="empty-quadrant">无</div>
        </div>
      </div>

      <!-- 紧急不重要 -->
      <div class="quadrant">
        <div class="quadrant-header urgent">紧急不重要 ({{ quadrants.urgent.length }})</div>
        <div class="quadrant-body">
          <div v-for="task in quadrants.urgent" :key="task.id" class="task-card" :class="{ 'overdue-card': isOverdue(task.deadline) }">
            <div class="task-main">
              <el-checkbox :model-value="!!task.completed" @change="toggleComplete(task)" />
              <span class="task-name clickable" :class="{ done: task.completed }" @click="openDrawer(task)">{{ task.taskName }}</span>
            </div>
            <div v-if="task.deadline" class="task-deadline" :class="{ overdue: isOverdue(task.deadline) }">
              {{ deadlineText(task.deadline) }}
            </div>
            <el-button size="small" text type="danger" @click="deleteTask(task)">×</el-button>
          </div>
          <div v-if="!quadrants.urgent.length" class="empty-quadrant">无</div>
        </div>
      </div>

      <!-- 普通 -->
      <div class="quadrant">
        <div class="quadrant-header normal">普通 ({{ quadrants.normal.length }})</div>
        <div class="quadrant-body">
          <div v-for="task in quadrants.normal" :key="task.id" class="task-card" :class="{ 'overdue-card': isOverdue(task.deadline) }">
            <div class="task-main">
              <el-checkbox :model-value="!!task.completed" @change="toggleComplete(task)" />
              <span class="task-name clickable" :class="{ done: task.completed }" @click="openDrawer(task)">{{ task.taskName }}</span>
            </div>
            <div v-if="task.deadline" class="task-deadline" :class="{ overdue: isOverdue(task.deadline) }">
              {{ deadlineText(task.deadline) }}
            </div>
            <el-button size="small" text type="danger" @click="deleteTask(task)">×</el-button>
          </div>
          <div v-if="!quadrants.normal.length" class="empty-quadrant">无</div>
        </div>
      </div>
    </div>

    <!-- 新建任务弹窗 -->
    <el-dialog v-model="showCreateDialog" title="新建任务" width="400">
      <el-form label-width="80px" size="small">
        <el-form-item label="任务名称" required>
          <el-input v-model="newTask.taskName" />
        </el-form-item>
        <el-form-item label="描述">
          <el-input v-model="newTask.description" type="textarea" :rows="2" />
        </el-form-item>
        <el-form-item label="截止日期">
          <el-date-picker v-model="newTask.deadline" type="date" value-format="YYYY-MM-DD" style="width: 100%" />
        </el-form-item>
        <el-form-item label="优先级">
          <el-select v-model="newTask.priority" style="width: 100%">
            <el-option v-for="opt in priorityOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
          </el-select>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showCreateDialog = false">取消</el-button>
        <el-button type="primary" @click="createTask">创建</el-button>
      </template>
    </el-dialog>

    <!-- 任务编辑抽屉 -->
    <el-drawer v-model="showDrawer" title="编辑任务" direction="rtl" size="400px">
      <template v-if="editingTask">
        <el-form label-width="80px" size="default">
          <el-form-item label="任务名称" required>
            <el-input v-model="editForm.taskName" />
          </el-form-item>
          <el-form-item label="描述">
            <el-input v-model="editForm.description" type="textarea" :rows="4" />
          </el-form-item>
          <el-form-item label="截止日期">
            <el-date-picker v-model="editForm.deadline" type="date" value-format="YYYY-MM-DD" style="width: 100%" />
          </el-form-item>
          <el-form-item label="优先级">
            <el-select v-model="editForm.priority" style="width: 100%">
              <el-option v-for="opt in priorityOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
            </el-select>
          </el-form-item>
          <el-form-item label="关联案件">
            <div class="case-search-wrapper">
              <el-input
                v-model="caseSearchQuery"
                placeholder="搜索案件名称..."
                clearable
                @input="searchCases"
                @clear="clearCaseSelection"
              />
              <div v-if="caseSearchResults.length" class="case-dropdown">
                <div
                  v-for="c in caseSearchResults"
                  :key="c.id"
                  class="case-dropdown-item"
                  @click="selectCase(c)"
                >
                  {{ c.caseName || c.case_name }}
                </div>
              </div>
            </div>
            <div v-if="editForm.caseId" class="selected-case">
              已关联：{{ caseSearchQuery || editForm.caseId }}
              <el-button size="small" text type="danger" @click="clearCaseSelection">取消关联</el-button>
            </div>
          </el-form-item>
        </el-form>
        <div class="drawer-footer">
          <el-button @click="showDrawer = false">取消</el-button>
          <el-button type="primary" :loading="savingTask" @click="saveTask">保存</el-button>
        </div>
      </template>
    </el-drawer>
  </div>
</template>

<style scoped>
.tasks-page {
  max-width: 1200px;
  margin: 0 auto;
}

.toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
}

.toolbar h3 {
  margin: 0;
}

.quadrants-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
}

.quadrant {
  border: 1px solid #e0e0e0;
  border-radius: 8px;
  overflow: hidden;
}

.quadrant-header {
  padding: 8px 12px;
  font-weight: 500;
  font-size: 14px;
}

.quadrant-header.urgent-important { background: #fef0f0; color: #f56c6c; }
.quadrant-header.important { background: #fdf6ec; color: #e6a23c; }
.quadrant-header.urgent { background: #ecf5ff; color: #409eff; }
.quadrant-header.normal { background: #f5f5f5; color: #909399; }

.quadrant-body {
  padding: 8px;
  min-height: 100px;
}

.task-card {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border-radius: 4px;
  margin-bottom: 4px;
}

.task-card:hover {
  background: #f5f7fa;
}

.task-card.overdue-card {
  border-left: 3px solid #f56c6c;
  background: #fef0f0;
}

.task-card.overdue-card:hover {
  background: #fde2e2;
}

.task-main {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 8px;
}

.task-name.done {
  text-decoration: line-through;
  color: #999;
}

.task-deadline {
  font-size: 12px;
  color: #67c23a;
  white-space: nowrap;
}

.task-deadline.overdue {
  color: #f56c6c;
}

.empty-quadrant {
  text-align: center;
  color: #ccc;
  padding: 20px;
  font-size: 13px;
}

.skeleton-wrapper {
  padding: 16px 0;
}

.skeleton-quadrants {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
}

.skeleton-quadrant {
  border: 1px solid #e0e0e0;
  border-radius: 8px;
  overflow: hidden;
}

.skeleton-task-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 0;
}

.task-name.clickable {
  cursor: pointer;
  user-select: none;
}

.task-name.clickable:hover {
  color: #409eff;
  text-decoration: underline;
}

.case-search-wrapper {
  position: relative;
  width: 100%;
}

.case-dropdown {
  position: absolute;
  top: 100%;
  left: 0;
  right: 0;
  z-index: 100;
  background: white;
  border: 1px solid #e0e0e0;
  border-radius: 4px;
  box-shadow: 0 2px 12px rgba(0, 0, 0, 0.1);
  max-height: 200px;
  overflow-y: auto;
}

.case-dropdown-item {
  padding: 8px 12px;
  cursor: pointer;
  font-size: 13px;
}

.case-dropdown-item:hover {
  background: #f5f7fa;
}

.selected-case {
  margin-top: 4px;
  font-size: 13px;
  color: #67c23a;
  display: flex;
  align-items: center;
  gap: 8px;
}

.drawer-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding-top: 16px;
  border-top: 1px solid #e0e0e0;
  margin-top: 16px;
}
</style>
