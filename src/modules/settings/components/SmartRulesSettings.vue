<script setup lang="ts">
import { useFormBaseline } from '../../../composables/useFormBaseline'
// W5 · DEVONthink 式 Smart Rules 设置面板（自包含，由主代理挂载进设置页）
// 匹配语义：对匹配字段做大小写不敏感的子串匹配；OCR 文本类规则在 OCR 完成后自动生效。
import { ref, onMounted } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Plus, Refresh, VideoPlay } from '../../../shared/icons'
import { tauriCall, tauriCallSafe } from '../../../core/tauriBridge'

interface SmartRule {
  id: string
  name: string
  enabled: boolean
  matchField: 'filename' | 'ocr_text'
  matchPattern: string
  actionType: 'set_category' | 'mark_urgent' | 'add_keyword'
  actionPayload: string
  createdAt?: string | null
  updatedAt?: string | null
}

interface RuleForm {
  name: string
  enabled: boolean
  matchField: 'filename' | 'ocr_text'
  matchPattern: string
  actionType: 'set_category' | 'mark_urgent' | 'add_keyword'
  actionPayload: string
}

const rules = ref<SmartRule[]>([])
const loading = ref(false)
const saving = ref(false)
const runningAll = ref(false)
const dialogVisible = ref(false)
const editingRule = ref<SmartRule | null>(null)
const form = ref<RuleForm>(emptyForm())

const matchFieldLabels: Record<string, string> = {
  filename: '文件名',
  ocr_text: 'OCR 文本',
}
const actionTypeLabels: Record<string, string> = {
  set_category: '设置分类',
  mark_urgent: '标记紧急',
  add_keyword: '加关键词',
}
const categoryOptions = [
  { value: 'summons', label: '传票 / 通知书' },
  { value: 'evidence', label: '证据材料' },
  { value: 'submitted', label: '提交文件' },
  { value: 'received', label: '接收文件' },
  { value: 'internal', label: '内部文件' },
  { value: 'correspondence', label: '往来函件' },
  { value: 'other', label: '其他' },
]

function emptyForm(): RuleForm {
  return {
    name: '',
    enabled: true,
    matchField: 'filename',
    matchPattern: '',
    actionType: 'set_category',
    actionPayload: 'evidence',
  }
}

function categoryLabel(value: string): string {
  return categoryOptions.find(c => c.value === value)?.label || value
}

function actionSummary(rule: SmartRule): string {
  if (rule.actionType === 'set_category') return `设置分类 → ${categoryLabel(rule.actionPayload)}`
  if (rule.actionType === 'add_keyword') return `加关键词 → ${rule.actionPayload}`
  return '标记紧急（写通知）'
}

async function loadRules() {
  loading.value = true
  const data = await tauriCall('list_smart_rules', {}, { silent: true })
  if (data) rules.value = data
  loading.value = false
}

const formDraft = useFormBaseline('智能规则', () => dialogVisible.value ? form.value : null, () => saving.value)
async function closeDialog(done?: () => void) {
  if (!(await formDraft.canLeave())) return
  dialogVisible.value = false
  formDraft.markSaved()
  done?.()
}
function openCreate() {
  editingRule.value = null
  form.value = emptyForm()
  dialogVisible.value = true
  formDraft.markSaved()
}

function openEdit(rule: SmartRule) {
  editingRule.value = rule
  form.value = {
    name: rule.name,
    enabled: rule.enabled,
    matchField: rule.matchField,
    matchPattern: rule.matchPattern,
    actionType: rule.actionType,
    actionPayload: rule.actionType === 'mark_urgent' ? '' : rule.actionPayload,
  }
  dialogVisible.value = true
  formDraft.markSaved()
}

async function saveRule() {
  if (saving.value) return
  const f = form.value
  if (!f.name.trim()) {
    ElMessage.warning('请填写规则名称')
    return
  }
  if (!f.matchPattern.trim()) {
    ElMessage.warning('请填写匹配内容')
    return
  }
  if (f.actionType !== 'mark_urgent' && !f.actionPayload.trim()) {
    ElMessage.warning('请填写动作内容')
    return
  }
  saving.value = true
  try {
  const payload = {
    id: editingRule.value?.id ?? null,
    name: f.name.trim(),
    enabled: f.enabled,
    matchField: f.matchField,
    matchPattern: f.matchPattern.trim(),
    actionType: f.actionType,
    actionPayload: f.actionType === 'mark_urgent' ? '' : f.actionPayload.trim(),
  }
  const id = await tauriCall('upsert_smart_rule', payload, {
    errorMessage: '保存规则失败',
  })
  if (id) {
    ElMessage.success(editingRule.value ? '规则已更新' : '规则已创建')
    dialogVisible.value = false
    formDraft.markSaved()
    await loadRules()
  }
  } catch (cause) { ElMessage.error(String(cause)) }
  finally { saving.value = false }
}

async function toggleRule(rule: SmartRule, enabled: boolean) {
  const id = await tauriCall('upsert_smart_rule', {
    id: rule.id,
    name: rule.name,
    enabled,
    matchField: rule.matchField,
    matchPattern: rule.matchPattern,
    actionType: rule.actionType,
    actionPayload: rule.actionPayload,
  })
  if (id) {
    rule.enabled = enabled
    ElMessage.success(enabled ? `规则「${rule.name}」已启用` : `规则「${rule.name}」已停用`)
  }
}

async function removeRule(rule: SmartRule) {
  try {
    await ElMessageBox.confirm(
      `确定删除规则「${rule.name}」？该操作不可撤销。`,
      '删除规则',
      { type: 'warning', confirmButtonText: '删除', cancelButtonText: '取消' }
    )
  } catch {
    return
  }
  const res = await tauriCallSafe('delete_smart_rule', { id: rule.id })
  if (res.ok) {
    ElMessage.success('规则已删除')
    await loadRules()
  } else {
    ElMessage.error(res.error || '删除失败')
  }
}

async function runForAll() {
  try {
    await ElMessageBox.confirm(
      '将对全部已登记文件执行所有启用的规则，可能修改文件分类、关键词并产生紧急通知。是否继续？',
      '对全部文件立即执行',
      { type: 'warning', confirmButtonText: '立即执行', cancelButtonText: '取消' }
    )
  } catch {
    return
  }
  runningAll.value = true
  const affected = await tauriCall('run_smart_rules_for_all', {})
  runningAll.value = false
  if (affected !== null) {
    ElMessage.success(`执行完成，${affected} 个文件命中了规则`)
  }
}

onMounted(loadRules)
</script>

<template>
  <div class="smart-rules-settings">
    <div class="section-head">
      <div>
        <h4>Smart Rules 自动化</h4>
        <p class="desc">
          文件名类规则在文件登记时立即生效；OCR 文本类规则在 OCR 完成后自动生效（大小写不敏感的包含匹配）
        </p>
      </div>
      <div class="head-actions">
        <el-button size="small" :loading="runningAll" @click="runForAll">
          <el-icon><VideoPlay /></el-icon> 对全部文件立即执行
        </el-button>
        <el-button size="small" @click="loadRules"><el-icon><Refresh /></el-icon></el-button>
        <el-button size="small" type="primary" @click="openCreate">
          <el-icon><Plus /></el-icon> 新建规则
        </el-button>
      </div>
    </div>

    <el-table v-loading="loading" :data="rules" size="small" style="width: 100%">
      <el-table-column prop="name" label="规则名称" min-width="140" show-overflow-tooltip />
      <el-table-column label="匹配字段" width="100">
        <template #default="{ row }">{{ matchFieldLabels[row.matchField] || row.matchField }}</template>
      </el-table-column>
      <el-table-column prop="matchPattern" label="匹配内容" min-width="140" show-overflow-tooltip />
      <el-table-column label="动作" min-width="170">
        <template #default="{ row }">{{ actionSummary(row) }}</template>
      </el-table-column>
      <el-table-column label="启用" width="70">
        <template #default="{ row }">
          <el-switch
            :model-value="row.enabled"
            size="small"
            @change="(v: boolean) => toggleRule(row, v)"
          />
        </template>
      </el-table-column>
      <el-table-column label="操作" width="120">
        <template #default="{ row }">
          <el-button size="small" text @click="openEdit(row)">编辑</el-button>
          <el-button size="small" text type="danger" @click="removeRule(row)">删除</el-button>
        </template>
      </el-table-column>
      <template #empty>
        <span class="empty-tip">还没有规则。新建一条，例如：文件名包含「传票」→ 设置分类为传票/通知书。</span>
      </template>
    </el-table>

    <el-dialog
      v-model="dialogVisible"
      :before-close="closeDialog"
      :close-on-click-modal="false"
      :close-on-press-escape="!saving"
      :title="editingRule ? '编辑规则' : '新建规则'"
      width="480px"
    >
      <el-form :disabled="saving" label-width="86px">
        <el-form-item label="规则名称">
          <el-input v-model="form.name" placeholder="如：传票自动归类" />
        </el-form-item>
        <el-form-item label="匹配字段">
          <el-select v-model="form.matchField" style="width: 100%">
            <el-option label="文件名" value="filename" />
            <el-option label="OCR 文本" value="ocr_text" />
          </el-select>
        </el-form-item>
        <el-form-item label="匹配内容">
          <el-input
            v-model="form.matchPattern"
            placeholder="包含这段文字即命中（不区分大小写）"
          />
        </el-form-item>
        <el-form-item label="动作">
          <el-select v-model="form.actionType" style="width: 100%">
            <el-option label="设置分类" value="set_category" />
            <el-option label="标记紧急" value="mark_urgent" />
            <el-option label="加关键词" value="add_keyword" />
          </el-select>
        </el-form-item>
        <el-form-item v-if="form.actionType === 'set_category'" label="目标分类">
          <el-select v-model="form.actionPayload" style="width: 100%">
            <el-option
              v-for="c in categoryOptions"
              :key="c.value"
              :label="c.label"
              :value="c.value"
            />
          </el-select>
        </el-form-item>
        <el-form-item v-else-if="form.actionType === 'add_keyword'" label="关键词">
          <el-input v-model="form.actionPayload" placeholder="追加到文件的知识关键词" />
        </el-form-item>
        <el-form-item v-else label="说明">
          <span class="inline-note">命中后向通知中心写入一条紧急通知</span>
        </el-form-item>
        <el-form-item label="启用">
          <el-switch v-model="form.enabled" />
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="closeDialog()" :disabled="saving">取消</el-button>
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
.desc { font-size: 12px; color: var(--gray-400); margin: 0; max-width: 520px; line-height: 1.6; }
.head-actions { display: flex; align-items: center; gap: 8px; flex-shrink: 0; }
.empty-tip { font-size: 12px; color: var(--gray-400); }
.inline-note { font-size: 12px; color: var(--gray-400); }
</style>
