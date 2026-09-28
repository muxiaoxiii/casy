<script setup>
import { ref, onMounted } from 'vue'
import { casyContext } from '../../../core/plugin/context'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Plus, Delete, Edit, VideoPlay, RefreshRight, Bell } from '../../../shared/icons'

const saving = ref(false)
const loadError = ref('')
const rules = ref([])
const logs = ref([])
const engineRunning = ref(false)
const dialogVisible = ref(false)
const editingRule = ref(null)
const form = ref({ name: '', triggerType: 'deadline_before', triggerValue: 7, channels: ['local'], enabled: true })

const triggerTypeLabels = {
  deadline_before: '期限前 N 天',
  deadline_on: '期限当天',
  deadline_after: '期限逾期 N 天',
  hearing_before: '开庭前 N 天',
  task_due: '任务到期',
  task_overdue: '任务逾期',
}
const channelLabels = { local: '本地弹窗', system: '系统通知', feishu_message: '飞书消息', feishu_task: '飞书任务' }

async function loadRules() {
  const res = await casyContext.reminder.rules()
  if (res.ok && res.data) { rules.value = res.data; loadError.value = '' }
  else loadError.value = res.error || '提醒规则加载失败'
}
async function loadLogs() {
  const res = await casyContext.reminder.log(20)
  if (res.ok && res.data) logs.value = res.data
  else ElMessage.error(res.error || '提醒记录加载失败')
}
async function checkEngine() {
  // 引擎是否在运行：通过启动命令幂等探测（已在运行则直接返回）
  const res = await casyContext.reminder.startEngine(300)
  engineRunning.value = res.ok
}
function openCreate() {
  editingRule.value = null
  form.value = { name: '', triggerType: 'deadline_before', triggerValue: 7, channels: ['local'], enabled: true }
  dialogVisible.value = true
}
function openEdit(rule) {
  editingRule.value = rule
  form.value = {
    name: rule.name,
    triggerType: rule.triggerType,
    triggerValue: rule.triggerValue ?? 7,
    channels: JSON.parse(rule.channels || '["local"]'),
    enabled: rule.enabled,
  }
  dialogVisible.value = true
}
async function saveRule() {
  if (saving.value) return
  if (!form.value.name.trim()) return ElMessage.warning('请填写规则名称')
  if (!form.value.channels.length) return ElMessage.warning('请至少选择一个提醒通道')
  saving.value = true
  try {
    const payload = { ...form.value, name: form.value.name.trim(), channels: JSON.stringify(form.value.channels) }
    const res = editingRule.value
      ? await casyContext.reminder.updateRule(editingRule.value.id, payload)
      : await casyContext.reminder.createRule(payload)
    if (!res.ok) return ElMessage.error(res.error || '保存失败，输入已保留')
    ElMessage.success(editingRule.value ? '规则已更新' : '规则已创建')
    dialogVisible.value = false
    await loadRules()
  } finally { saving.value = false }
}

async function removeRule(id) {
  try { await ElMessageBox.confirm('删除后将不再按此规则触发提醒，已有记录会保留。', '删除提醒规则', { type: 'warning', confirmButtonText: '删除规则', cancelButtonText: '取消' }) } catch { return }
  const res = await casyContext.reminder.removeRule(id)
  if (res.ok) {
    ElMessage.success('已删除')
    loadRules()
  } else ElMessage.error(res.error || '删除失败')
}
async function testRule(rule) {
  const res = await casyContext.reminder.test({
    ruleId: rule.id,
    channel: 'local',
    message: `测试提醒：${rule.name}`,
  })
  if (res.ok) ElMessage.success('测试提醒已发送（本地弹窗）')
  else ElMessage.error(res.error || '测试提醒发送失败')
}
async function startEngine() {
  const res = await casyContext.reminder.startEngine(300)
  if (res.ok) {
    engineRunning.value = true
    ElMessage.success('提醒引擎已启动（每 5 分钟检查）')
  }
}

onMounted(() => {
  loadRules()
  loadLogs()
  checkEngine()
})
</script>

<template>
  <div class="reminder-settings">
    <el-alert v-if="loadError" :title="loadError" type="error" :closable="false"><el-button text @click="loadRules">重试</el-button></el-alert>
    <el-alert title="休息日前工作提醒" type="info" :closable="false" description="法定节假日、周末及自己添加的休息日，如果当天仍有未完成任务、日程或期限，或休息前一天仍有工作，通知中心会特别提醒。同一事项每天最多一条；完成或改期后自动撤下，个人上班日不额外提醒。" style="margin-bottom: 20px" />
    <div class="section-head">
      <div>
        <h4>提醒规则</h4>
        <p class="desc">按期限/开庭/任务自动触发，支持多通道分发</p>
      </div>
      <div class="head-actions">
        <span class="engine-badge" :class="{ running: engineRunning }">
          <el-icon><Bell /></el-icon>
          {{ engineRunning ? '引擎运行中' : '引擎未启动' }}
        </span>
        <el-button size="small" @click="startEngine">启动引擎</el-button>
        <el-button size="small" type="primary" @click="openCreate">
          <el-icon><Plus /></el-icon> 新建规则
        </el-button>
      </div>
    </div>

    <el-table :data="rules" size="small" style="width: 100%">
      <el-table-column prop="name" label="规则名称" min-width="140" />
      <el-table-column label="触发条件" min-width="140">
        <template #default="{ row }">
          {{ triggerTypeLabels[row.triggerType] || row.triggerType }}
          {{ row.triggerValue !== null ? `(${row.triggerValue})` : '' }}
        </template>
      </el-table-column>
      <el-table-column label="通道" min-width="180">
        <template #default="{ row }">
          <el-tag v-for="c in JSON.parse(row.channels || '[]')" :key="c" size="small" class="chan-tag">
            {{ channelLabels[c] || c }}
          </el-tag>
        </template>
      </el-table-column>
      <el-table-column label="状态" width="80">
        <template #default="{ row }">
          <el-tag :type="row.enabled ? 'success' : 'info'" size="small">{{ row.enabled ? '启用' : '停用' }}</el-tag>
        </template>
      </el-table-column>
      <el-table-column label="操作" width="170">
        <template #default="{ row }">
          <el-button size="small" text type="primary" @click="testRule(row)">测试</el-button>
          <el-button size="small" text @click="openEdit(row)">编辑</el-button>
          <el-button size="small" text type="danger" @click="removeRule(row.id)">删除</el-button>
        </template>
      </el-table-column>
    </el-table>

    <div class="section-head" style="margin-top: 24px">
      <div>
        <h4>最近触发记录</h4>
        <p class="desc">最近 20 条提醒日志</p>
      </div>
      <el-button size="small" @click="loadLogs"><el-icon><RefreshRight /></el-icon> 刷新</el-button>
    </div>
    <el-table :data="logs" size="small" style="width: 100%">
      <el-table-column prop="sentAt" label="时间" width="160" />
      <el-table-column label="通道" width="100">
        <template #default="{ row }">{{ channelLabels[row.channel] || row.channel }}</template>
      </el-table-column>
      <el-table-column prop="message" label="内容" min-width="240" show-overflow-tooltip />
      <el-table-column label="状态" width="80">
        <template #default="{ row }">
          <el-tag :type="row.status === 'sent' ? 'success' : 'danger'" size="small">{{ row.status }}</el-tag>
        </template>
      </el-table-column>
    </el-table>

    <el-dialog v-model="dialogVisible" :title="editingRule ? '编辑提醒规则' : '新建提醒规则'" width="460px" :close-on-click-modal="false" :close-on-press-escape="!saving" :show-close="!saving">
      <el-form label-width="80px" :disabled="saving">
        <el-form-item label="规则名称">
          <el-input v-model="form.name" placeholder="如：期限前 7 天提醒" />
        </el-form-item>
        <el-form-item label="触发类型">
          <el-select v-model="form.triggerType" style="width: 100%">
            <el-option v-for="(label, key) in triggerTypeLabels" :key="key" :label="label" :value="key" />
          </el-select>
        </el-form-item>
        <el-form-item label="触发值" v-if="form.triggerType.includes('before') || form.triggerType.includes('after')">
          <el-input-number v-model="form.triggerValue" :min="0" :max="60" />
        </el-form-item>
        <el-form-item label="分发通道">
          <el-checkbox-group v-model="form.channels">
            <el-checkbox value="local">本地弹窗</el-checkbox>
            <el-checkbox value="system">系统通知</el-checkbox>
            <el-checkbox value="feishu_message">飞书消息</el-checkbox>
            <el-checkbox value="feishu_task">飞书任务</el-checkbox>
          </el-checkbox-group>
        </el-form-item>
        <el-form-item label="启用">
          <el-switch v-model="form.enabled" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button :disabled="saving" @click="dialogVisible = false">取消</el-button>
        <el-button type="primary" :loading="saving" @click="saveRule">保存</el-button>
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.section-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  margin-bottom: 12px;
}
.section-head h4 { margin: 0 0 4px; }
.desc { font-size: 12px; color: var(--gray-400); margin: 0; }
.head-actions { display: flex; align-items: center; gap: 8px; }
.engine-badge {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 12px;
  padding: 3px 10px;
  border-radius: 999px;
  background: var(--gray-50);
  color: var(--gray-400);
}
.engine-badge.running { background: #f0f9eb; color: #67c23a; }
.chan-tag { margin-right: 4px; }
</style>
