<script setup>
import { ref, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useCasesStore } from '../../../stores/cases.js'
import { tauriCallSafe } from '../../../core/tauriBridge.js'
import { ElMessage, ElMessageBox } from 'element-plus'
import CaseInfoPanel from '../components/CaseInfoPanel.vue'
import CaseTimelinePanel from '../components/CaseTimelinePanel.vue'
import ReferenceSelect from '../../../shared/components/ReferenceSelect.vue'

const route = useRoute()
const router = useRouter()
const casesStore = useCasesStore()
const form = ref({})
const timeline = ref([])
const timelineLoading = ref(false)
const showAddLogDialog = ref(false)
const newLog = ref({
  eventSummary: '',
  eventType: 'record',
  eventDate: new Date().toISOString().split('T')[0],
  content: '',
})
let saveTimer = null

// 快捷操作相关
const showAddHearingDialog = ref(false)
const showAddTaskDialog = ref(false)
const newHearing = ref({
  hearingName: '',
  hearingDate: new Date().toISOString().split('T')[0],
  hearingType: 'oral',
  court: '',
  linkedCaseId: null,
})
const newTask = ref({
  taskName: '',
  deadline: '',
  priority: 'normal',
  description: '',
})

// 关系相关
const relations = ref([])
const relationsLoading = ref(false)
const showAddRelationDialog = ref(false)
const selectedRelationType = ref('cross_reference')
const relationLabel = ref('')
const selectedRelationTarget = ref(null)

const relationTypeOptions = [
  { value: 'same_patent', label: '同专利号' },
  { value: 'same_party', label: '同客户' },
  { value: 'appeal_of', label: '审级关联' },
  { value: 'cross_reference', label: '交叉引用' },
]

const relationTypeMap = {
  same_patent: { label: '同专利号', color: '#409eff' },
  same_party: { label: '同客户', color: '#67c23a' },
  appeal_of: { label: '审级关联', color: '#e6a23c' },
  cross_reference: { label: '交叉引用', color: '#909399' },
}

const logTypeOptions = [
  { value: 'record', label: '记录' },
  { value: 'submitted', label: '交文' },
  { value: 'received', label: '收文' },
  { value: 'task', label: '任务' },
]

onMounted(async () => {
  const id = route.params.id
  if (id) {
    await casesStore.loadCase(id)
    if (casesStore.currentCase) {
      form.value = { ...casesStore.currentCase }
    }
    await loadTimeline()
    await loadRelations()
  }
})

// 自动保存（2秒防抖）
function scheduleSave() {
  if (saveTimer) clearTimeout(saveTimer)
  saveTimer = setTimeout(async () => {
    if (!casesStore.currentCase?.id) return
    const result = await casesStore.updateCase(casesStore.currentCase.id, form.value)
    if (result.ok) {
      ElMessage.success('已自动保存')
    }
  }, 2000)
}

function goBack() {
  router.push({ name: 'cases' })
}

async function loadTimeline() {
  if (!casesStore.currentCase?.id) return
  timelineLoading.value = true
  const result = await tauriCallSafe('get_case_timeline', { caseId: casesStore.currentCase.id })
  if (result.ok) {
    timeline.value = result.data || []
  }
  timelineLoading.value = false
}

async function addLog() {
  if (!newLog.value.eventSummary.trim()) {
    ElMessage.warning('请输入事件概述')
    return
  }
  const result = await tauriCallSafe('add_case_log', {
    caseId: casesStore.currentCase.id,
    eventSummary: newLog.value.eventSummary,
    eventType: newLog.value.eventType,
    eventDate: newLog.value.eventDate,
    content: newLog.value.content || null,
  })
  if (result.ok) {
    ElMessage.success('已添加')
    showAddLogDialog.value = false
    newLog.value = { eventSummary: '', eventType: 'record', eventDate: new Date().toISOString().split('T')[0], content: '' }
    await loadTimeline()
  }
}

async function deleteLog(id) {
  try {
    await ElMessageBox.confirm('确定删除此事件？', '确认', { type: 'warning' })
    const result = await tauriCallSafe('delete_case_log', { id })
    if (result.ok) {
      await loadTimeline()
    }
  } catch {}
}

// ===== 关系管理 =====
async function loadRelations() {
  if (!casesStore.currentCase?.id) return
  relationsLoading.value = true
  const result = await tauriCallSafe('get_relations', { caseId: casesStore.currentCase.id })
  if (result.ok) {
    relations.value = result.data || []
  }
  relationsLoading.value = false
}

async function handleDetectRelations() {
  if (!casesStore.currentCase?.id) return
  relationsLoading.value = true
  const result = await tauriCallSafe('detect_relations', { caseId: casesStore.currentCase.id })
  if (result.ok) {
    const detected = result.data || []
    if (detected.length > 0) {
      ElMessage.success(`检测到 ${detected.length} 个新关系`)
    } else {
      ElMessage.info('未检测到新关系')
    }
    await loadRelations()
  }
  relationsLoading.value = false
}

function openAddRelationDialog() {
  selectedRelationTarget.value = null
  selectedRelationType.value = 'cross_reference'
  relationLabel.value = ''
  showAddRelationDialog.value = true
}

function onRelationCaseSelect(caseObj) {
  selectedRelationTarget.value = caseObj
}

async function addRelation() {
  const caseId = casesStore.currentCase?.id
  const targetId = selectedRelationTarget.value?.id
  if (!caseId || !targetId) {
    ElMessage.warning('请选择要关联的案件')
    return
  }
  const result = await tauriCallSafe('add_relation', {
    caseId,
    relatedId: targetId,
    relationType: selectedRelationType.value,
    label: relationLabel.value || null,
  })
  if (result.ok) {
    ElMessage.success('关联已添加')
    showAddRelationDialog.value = false
    await loadRelations()
  }
}

async function removeRelation(relationId) {
  try {
    await ElMessageBox.confirm('确定删除此关联？', '确认', { type: 'warning' })
    const result = await tauriCallSafe('remove_relation', { id: relationId })
    if (result.ok) {
      await loadRelations()
    }
  } catch {}
}

function goToRelatedCase(caseId) {
  router.push({ name: 'case-detail', params: { id: caseId } })
}

// ===== 快捷操作 =====
async function addHearing() {
  if (!newHearing.value.hearingName.trim()) {
    ElMessage.warning('请输入庭审名称')
    return
  }
  const result = await tauriCallSafe('add_case_log', {
    caseId: casesStore.currentCase.id,
    eventSummary: `庭审: ${newHearing.value.hearingName}`,
    eventType: 'hearing',
    eventDate: newHearing.value.hearingDate,
    content: `法院: ${newHearing.value.court || form.value.court || '—'}\n类型: ${newHearing.value.hearingType}${newHearing.value.linkedCaseId ? `\n关联案件: ${newHearing.value.linkedCaseId}` : ''}`,
  })
  if (result.ok) {
    ElMessage.success('庭审已添加')
    showAddHearingDialog.value = false
    newHearing.value = { hearingName: '', hearingDate: new Date().toISOString().split('T')[0], hearingType: 'oral', court: '', linkedCaseId: null }
    await loadTimeline()
  }
}

async function addQuickTask() {
  if (!newTask.value.taskName.trim()) {
    ElMessage.warning('请输入任务名称')
    return
  }
  const result = await tauriCallSafe('create_task', {
    data: {
      taskName: newTask.value.taskName,
      description: newTask.value.description,
      deadline: newTask.value.deadline || null,
      priority: newTask.value.priority,
      caseId: casesStore.currentCase.id,
    },
  })
  if (result.ok) {
    ElMessage.success('任务已创建')
    showAddTaskDialog.value = false
    newTask.value = { taskName: '', deadline: '', priority: 'normal', description: '' }
  }
}

function goToDocWorkshop() {
  router.push({ name: 'docs', query: { caseId: casesStore.currentCase?.id } })
}

async function openCaseFolder() {
  const caseId = casesStore.currentCase?.id
  if (!caseId) return
  try {
    const { openPath } = await import('../../../core/tauriBridge.js')
    const result = await tauriCallSafe('get_case', { id: caseId })
    if (result.ok) {
      const folderPath = result.data.folderPath
      if (folderPath) {
        await openPath(folderPath)
      } else {
        ElMessage.info('该案件暂无关联文件夹')
      }
    }
  } catch (err) {
    ElMessage.error('无法打开文件夹: ' + err.message)
  }
}
</script>

<template>
  <div class="case-detail-page">
    <!-- 顶部导航 -->
    <div class="detail-header">
      <el-button @click="goBack" text>← 返回列表</el-button>
      <span class="case-title">{{ form.caseName || '案件详情' }}</span>
      <el-tag v-if="form.caseStatus" size="small" :type="form.caseStatus === '已完结' ? 'info' : 'success'">
        {{ form.caseStatus }}
      </el-tag>
    </div>

    <div v-if="!casesStore.currentCase" class="loading-state">
      加载中...
    </div>

    <div v-else class="detail-grid">
      <!-- 左栏：案件信息面板 -->
      <div class="detail-left">
        <CaseInfoPanel :form="form" @update="scheduleSave" />
      </div>

      <!-- 中栏：时间线面板 -->
      <div class="detail-center">
        <CaseTimelinePanel
          :timeline="timeline"
          :loading="timelineLoading"
          @add-log="showAddLogDialog = true"
          @delete-log="deleteLog"
        />
      </div>

      <!-- 右栏：关联 + 快捷操作 -->
      <div class="detail-right">
        <el-card>
          <template #header><strong>日期里程碑</strong></template>
          <el-form label-width="70px" size="small">
            <el-form-item label="立案">
              <el-date-picker v-model="form.filingDate" type="date" value-format="YYYY-MM-DD" @change="scheduleSave" style="width: 100%" />
            </el-form-item>
            <el-form-item label="开庭">
              <el-date-picker v-model="form.trialDate" type="date" value-format="YYYY-MM-DD" @change="scheduleSave" style="width: 100%" />
            </el-form-item>
            <el-form-item label="判决">
              <el-date-picker v-model="form.verdictDate" type="date" value-format="YYYY-MM-DD" @change="scheduleSave" style="width: 100%" />
            </el-form-item>
          </el-form>
        </el-card>

        <el-card style="margin-top: 12px">
          <template #header>
            <div class="card-header-row">
              <strong>关联案件</strong>
              <div>
                <el-button size="small" text @click="handleDetectRelations" :loading="relationsLoading">🔍 检测</el-button>
                <el-button size="small" text type="primary" @click="openAddRelationDialog">+ 添加</el-button>
              </div>
            </div>
          </template>
          <div v-if="relationsLoading" class="relations-loading">加载中...</div>
          <div v-else-if="!relations.length" class="relations-empty">
            <el-empty description="暂无关联" :image-size="40">
              <el-button size="small" @click="handleDetectRelations">自动检测</el-button>
            </el-empty>
          </div>
          <div v-else class="relations-list">
            <div v-for="rel in relations" :key="rel.relationId" class="relation-item" @click="goToRelatedCase(rel.caseId)">
              <div class="relation-info">
                <div class="relation-name">{{ rel.caseName }}</div>
                <div class="relation-meta">
                  <el-tag :color="relationTypeMap[rel.relationType]?.color" size="small" effect="dark" style="border: none; color: #fff;">
                    {{ relationTypeMap[rel.relationType]?.label || rel.relationType }}
                  </el-tag>
                  <span v-if="rel.caseNo" class="relation-case-no">{{ rel.caseNo }}</span>
                </div>
                <div class="relation-sub">
                  <span>{{ rel.clientName }}</span>
                  <span v-if="rel.caseStatus"> · {{ rel.caseStatus }}</span>
                </div>
              </div>
              <el-button size="small" text type="danger" class="relation-remove" @click.stop="removeRelation(rel.relationId)">×</el-button>
            </div>
          </div>
        </el-card>

        <el-card style="margin-top: 12px">
          <template #header><strong>快捷操作</strong></template>
          <div class="quick-actions">
            <el-button size="small" block @click="showAddLogDialog = true">📝 添加日志</el-button>
            <el-button size="small" block @click="showAddHearingDialog = true">📅 添加庭审</el-button>
            <el-button size="small" block @click="showAddTaskDialog = true">📌 添加任务</el-button>
            <el-button size="small" block @click="goToDocWorkshop">📄 生成文书</el-button>
            <el-button size="small" block @click="router.push({ name: 'write', params: { caseId: casesStore.currentCase?.id } })">✍️ 撰写文书</el-button>
            <el-button size="small" block @click="router.push({ name: 'files', params: { caseId: casesStore.currentCase?.id } })">📂 案件文件</el-button>
            <el-button size="small" block @click="openCaseFolder">📁 打开文件夹</el-button>
          </div>
        </el-card>
      </div>
    </div>

    <!-- 添加日志弹窗 -->
    <el-dialog v-model="showAddLogDialog" title="添加事件" width="450">
      <el-form label-width="80px" size="small">
        <el-form-item label="事件概述" required>
          <el-input v-model="newLog.eventSummary" placeholder="如：提交无效宣告请求书" />
        </el-form-item>
        <el-form-item label="类型">
          <el-select v-model="newLog.eventType" style="width: 100%">
            <el-option v-for="opt in logTypeOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
          </el-select>
        </el-form-item>
        <el-form-item label="日期">
          <el-date-picker v-model="newLog.eventDate" type="date" value-format="YYYY-MM-DD" style="width: 100%" />
        </el-form-item>
        <el-form-item label="详细内容">
          <el-input v-model="newLog.content" type="textarea" :rows="3" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showAddLogDialog = false">取消</el-button>
        <el-button type="primary" @click="addLog">添加</el-button>
      </template>
    </el-dialog>

    <!-- 添加庭审弹窗 -->
    <el-dialog v-model="showAddHearingDialog" title="添加庭审" width="450">
      <el-form label-width="80px" size="small">
        <el-form-item label="庭审名称" required>
          <el-input v-model="newHearing.hearingName" placeholder="如：口头审理" />
        </el-form-item>
        <el-form-item label="日期">
          <el-date-picker v-model="newHearing.hearingDate" type="date" value-format="YYYY-MM-DD" style="width: 100%" />
        </el-form-item>
        <el-form-item label="类型">
          <el-select v-model="newHearing.hearingType" style="width: 100%">
            <el-option label="口头审理" value="oral" />
            <el-option label="开庭" value="trial" />
            <el-option label="调解" value="mediation" />
          </el-select>
        </el-form-item>
        <el-form-item label="法院">
          <el-input v-model="newHearing.court" :placeholder="form.court || ''" />
        </el-form-item>
        <el-form-item label="关联案件">
          <ReferenceSelect
            type="case"
            placeholder="可选：关联其他案件"
            @select="(c) => newHearing.linkedCaseId = c?.id || null"
          />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showAddHearingDialog = false">取消</el-button>
        <el-button type="primary" @click="addHearing">添加</el-button>
      </template>
    </el-dialog>

    <!-- 添加任务弹窗 -->
    <el-dialog v-model="showAddTaskDialog" title="添加任务" width="450">
      <el-form label-width="80px" size="small">
        <el-form-item label="任务名称" required>
          <el-input v-model="newTask.taskName" placeholder="输入任务名称" />
        </el-form-item>
        <el-form-item label="描述">
          <el-input v-model="newTask.description" type="textarea" :rows="2" />
        </el-form-item>
        <el-form-item label="截止日期">
          <el-date-picker v-model="newTask.deadline" type="date" value-format="YYYY-MM-DD" style="width: 100%" />
        </el-form-item>
        <el-form-item label="优先级">
          <el-select v-model="newTask.priority" style="width: 100%">
            <el-option label="重要紧急" value="urgent_important" />
            <el-option label="重要不紧急" value="important" />
            <el-option label="紧急不重要" value="urgent" />
            <el-option label="普通" value="normal" />
          </el-select>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showAddTaskDialog = false">取消</el-button>
        <el-button type="primary" @click="addQuickTask">创建</el-button>
      </template>
    </el-dialog>

    <!-- 添加关联弹窗 -->
    <el-dialog v-model="showAddRelationDialog" title="添加关联案件" width="500">
      <el-form label-width="80px" size="small">
        <el-form-item label="关联类型">
          <el-select v-model="selectedRelationType" style="width: 100%">
            <el-option v-for="opt in relationTypeOptions" :key="opt.value" :label="opt.label" :value="opt.value" />
          </el-select>
        </el-form-item>
        <el-form-item label="备注">
          <el-input v-model="relationLabel" placeholder="可选备注" />
        </el-form-item>
        <el-form-item label="搜索案件">
          <ReferenceSelect
            type="case"
            placeholder="输入案件名称、案号或客户名"
            @select="onRelationCaseSelect"
          />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showAddRelationDialog = false">关闭</el-button>
        <el-button type="primary" :disabled="!selectedRelationTarget" @click="addRelation">添加关联</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.case-detail-page {
  max-width: 1400px;
  margin: 0 auto;
}

.detail-header {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
}

.case-title {
  font-size: 18px;
  font-weight: 500;
}

.loading-state {
  text-align: center;
  padding: 60px;
  color: #666;
}

.detail-grid {
  display: grid;
  grid-template-columns: 320px 1fr 280px;
  gap: 16px;
}

.detail-left,
.detail-center,
.detail-right {
  min-width: 0;
}

.card-header-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.quick-actions {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

/* 关系列表 */
.relations-loading,
.relations-empty {
  text-align: center;
  padding: 12px 0;
  color: #999;
  font-size: 13px;
}

.relations-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.relation-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px;
  border-radius: 6px;
  cursor: pointer;
  transition: background 0.2s;
}

.relation-item:hover {
  background: #f5f7fa;
}

.relation-info {
  flex: 1;
  min-width: 0;
}

.relation-name {
  font-size: 13px;
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.relation-meta {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 4px;
}

.relation-case-no {
  font-size: 11px;
  color: #999;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.relation-sub {
  font-size: 11px;
  color: #999;
  margin-top: 2px;
}

.relation-remove {
  flex-shrink: 0;
  opacity: 0;
  transition: opacity 0.2s;
}

.relation-item:hover .relation-remove {
  opacity: 1;
}

.quick-actions .el-button {
  width: 100%;
}

@media (max-width: 1199px) {
  .detail-grid {
    grid-template-columns: 1fr;
  }
}
</style>
