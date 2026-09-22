<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { Clock, Delete, Edit, Plus, RefreshRight } from '../../../shared/icons'
import { tauriCall, tauriCallSafe } from '../../../core/tauriBridge'
import DeadlineRuleAuditDrawer from './DeadlineRuleAuditDrawer.vue'
import {
  CALC_METHOD_LABELS,
  CALC_METHOD_OPTIONS,
  DEADLINE_SOURCE_LABELS,
  DEADLINE_SOURCE_OPTIONS,
  OFFSET_UNIT_OPTIONS,
  TRACK_LABELS,
  TRACK_TAG_TYPES,
  TRIGGER_FIELD_OPTIONS,
  formatOffset,
  formatProcedureTypes,
  trackLabel,
  triggerFieldLabel,
  type DeadlineRuleDto,
} from './deadlineRuleMeta'

// ── 列表数据 ─────────────────────────────────────────────────
const rules = ref<DeadlineRuleDto[]>([])
const loading = ref(false)

const KNOWN_TRACK_ORDER = ['patent_invalidation', 'admin_litigation', 'civil_tort', 'other']

interface TrackGroup {
  track: string
  label: string
  tagType: 'primary' | 'warning' | 'success' | 'info'
  rules: DeadlineRuleDto[]
}

const trackGroups = computed<TrackGroup[]>(() => {
  const map = new Map<string, DeadlineRuleDto[]>()
  for (const r of rules.value) {
    const list = map.get(r.track)
    if (list) list.push(r)
    else map.set(r.track, [r])
  }
  const keys = [...map.keys()].sort((a, b) => {
    const ia = KNOWN_TRACK_ORDER.indexOf(a)
    const ib = KNOWN_TRACK_ORDER.indexOf(b)
    const oa = ia === -1 ? KNOWN_TRACK_ORDER.length : ia
    const ob = ib === -1 ? KNOWN_TRACK_ORDER.length : ib
    return oa - ob || a.localeCompare(b)
  })
  return keys.map((track) => ({
    track,
    label: trackLabel(track),
    tagType: TRACK_TAG_TYPES[track] || 'info',
    rules: map.get(track) || [],
  }))
})

/** 表单轨道下拉：现有规则去重 + 已知轨道，可手动输入新轨道 */
const trackOptions = computed(() => {
  const seen = new Set<string>()
  const options: Array<{ value: string; label: string }> = []
  for (const t of [...KNOWN_TRACK_ORDER, ...rules.value.map((r) => r.track)]) {
    if (seen.has(t)) continue
    seen.add(t)
    options.push({ value: t, label: TRACK_LABELS[t] ? `${TRACK_LABELS[t]}（${t}）` : t })
  }
  return options
})

async function loadRules() {
  loading.value = true
  try {
    const data = await tauriCall('list_deadline_rules', {}, { silent: true })
    rules.value = data || []
  } finally {
    loading.value = false
  }
}

// ── 重算（规则变更后触发；命令未注册时静默跳过计数提示）─────
async function recalcTrack(track: string, baseMsg: string) {
  const n = await tauriCall('recalculate_deadlines_for_track', { track }, { silent: true })
  if (typeof n === 'number') {
    ElMessage.success(`${baseMsg}，已重算 ${n} 个案件期限`)
  } else {
    ElMessage.success(baseMsg)
  }
}

// ── 新建 / 编辑 ──────────────────────────────────────────────
interface RuleForm {
  track: string
  ruleName: string
  legalBasis: string
  triggerField: string
  offsetValue: number
  offsetUnit: string
  calcMethod: string
  procedureTypes: string
  deadlineSource: string
  priority: number
}

const dialogVisible = ref(false)
const saving = ref(false)
const editingRule = ref<DeadlineRuleDto | null>(null)
const form = reactive<RuleForm>({
  track: 'patent_invalidation',
  ruleName: '',
  legalBasis: '',
  triggerField: 'filing_date',
  offsetValue: 15,
  offsetUnit: 'day',
  calcMethod: 'civil',
  procedureTypes: '',
  deadlineSource: 'statutory',
  priority: 0,
})

function openCreate() {
  editingRule.value = null
  Object.assign(form, {
    track: 'patent_invalidation',
    ruleName: '',
    legalBasis: '',
    triggerField: 'filing_date',
    offsetValue: 15,
    offsetUnit: 'day',
    calcMethod: 'civil',
    procedureTypes: '',
    deadlineSource: 'statutory',
    priority: 0,
  })
  dialogVisible.value = true
}

function openEdit(rule: DeadlineRuleDto) {
  editingRule.value = rule
  Object.assign(form, {
    track: rule.track,
    ruleName: rule.ruleName,
    legalBasis: rule.legalBasis,
    triggerField: rule.triggerField,
    offsetValue: rule.offsetValue,
    offsetUnit: rule.offsetUnit,
    calcMethod: rule.calcMethod,
    procedureTypes: rule.procedureTypes || '',
    deadlineSource: rule.deadlineSource,
    priority: rule.priority,
  })
  dialogVisible.value = true
}

/**
 * 适用程序归一化：引擎期望 JSON 数组字符串（如 ["简易"]）。
 * 用户输入纯文本时包装为单元素数组；输入 JSON 时校验合法性。
 */
function normalizeProcedureTypes(raw: string): string | null {
  const text = raw.trim()
  if (!text) return null
  if (text.startsWith('[') || text.startsWith('{')) {
    JSON.parse(text) // 不合法则抛错，由调用方捕获
    return text
  }
  return JSON.stringify([text])
}

async function saveRule() {
  if (!form.track.trim()) return ElMessage.warning('请选择或输入所属轨道')
  if (!form.ruleName.trim()) return ElMessage.warning('请填写规则名称')
  if (!form.legalBasis.trim()) return ElMessage.warning('请填写法条依据')
  if (!form.triggerField) return ElMessage.warning('请选择触发字段')

  let procedureTypes: string | null
  try {
    procedureTypes = normalizeProcedureTypes(form.procedureTypes)
  } catch {
    return ElMessage.warning('适用程序格式不正确：请输入纯文本（如「简易」）或合法 JSON')
  }

  saving.value = true
  try {
    const id = await tauriCall('upsert_deadline_rule', {
      id: editingRule.value ? editingRule.value.id : null,
      track: form.track.trim(),
      ruleName: form.ruleName.trim(),
      legalBasis: form.legalBasis.trim(),
      triggerField: form.triggerField,
      offsetValue: form.offsetValue,
      offsetUnit: form.offsetUnit,
      calcMethod: form.calcMethod,
      procedureTypes,
      deadlineSource: form.deadlineSource,
      priority: form.priority,
    })
    if (!id) return
    dialogVisible.value = false
    await loadRules()
    await recalcTrack(form.track.trim(), editingRule.value ? '规则已更新' : '规则已创建')
  } finally {
    saving.value = false
  }
}

// ── 启停开关 ─────────────────────────────────────────────────
async function onToggle(row: DeadlineRuleDto, enabled: string | number | boolean) {
  const target = Boolean(enabled)
  const res = await tauriCallSafe('toggle_deadline_rule', { id: row.id, enabled: target })
  if (!res.ok) {
    ElMessage.error(`切换失败: ${res.error || '未知错误'}`)
    return
  }
  row.autoCalculate = target
  await recalcTrack(row.track, target ? '规则已启用' : '规则已停用')
}

// ── 删除（二次确认）──────────────────────────────────────────
async function onDelete(row: DeadlineRuleDto) {
  try {
    await ElMessageBox.confirm(
      `确定删除规则「${row.ruleName}」？删除后该轨道案件将不再自动计算此期限，操作会记录留痕且不可恢复。`,
      '删除确认',
      {
        type: 'warning',
        confirmButtonText: '删除',
        cancelButtonText: '取消',
        confirmButtonClass: 'el-button--danger',
      },
    )
  } catch {
    return // 用户取消
  }
  const res = await tauriCallSafe('delete_deadline_rule', { id: row.id })
  if (!res.ok) {
    ElMessage.error(`删除失败: ${res.error || '未知错误'}`)
    return
  }
  rules.value = rules.value.filter((r) => r.id !== row.id)
  await recalcTrack(row.track, '规则已删除')
}

// ── 留痕抽屉 ─────────────────────────────────────────────────
const auditVisible = ref(false)
const auditRule = ref<DeadlineRuleDto | null>(null)

function openAudit(row: DeadlineRuleDto) {
  auditRule.value = row
  auditVisible.value = true
}

onMounted(loadRules)
</script>

<template>
  <div class="deadline-rules-settings">
    <div class="section-head">
      <div>
        <h4>法定期限规则</h4>
        <p class="desc">按案件轨道自定义法定期限的触发字段与偏移算法，变更自动留痕并重算受影响案件</p>
      </div>
      <div class="head-actions">
        <el-button size="small" @click="loadRules">
          <el-icon><RefreshRight /></el-icon> 刷新
        </el-button>
        <el-button size="small" type="primary" @click="openCreate">
          <el-icon><Plus /></el-icon> 新建规则
        </el-button>
      </div>
    </div>

    <div v-loading="loading">
      <el-empty v-if="!loading && rules.length === 0" description="暂无期限规则" :image-size="80" />

      <div v-for="group in trackGroups" :key="group.track" class="track-group">
        <div class="track-head">
          <el-tag :type="group.tagType" size="small" effect="light">{{ group.label }}</el-tag>
          <span class="track-code">{{ group.track }}</span>
          <span class="track-count">{{ group.rules.length }} 条规则</span>
        </div>

        <el-table :data="group.rules" size="small" style="width: 100%">
          <el-table-column label="规则" min-width="200">
            <template #default="{ row }">
              <div class="rule-name">{{ row.ruleName }}</div>
              <div class="rule-basis">{{ row.legalBasis }}</div>
            </template>
          </el-table-column>
          <el-table-column label="触发字段" min-width="130">
            <template #default="{ row }">{{ triggerFieldLabel(row.triggerField) }}</template>
          </el-table-column>
          <el-table-column label="偏移" width="100">
            <template #default="{ row }">
              <span class="offset-text">{{ formatOffset(row) }}</span>
            </template>
          </el-table-column>
          <el-table-column label="算法" width="110">
            <template #default="{ row }">{{ CALC_METHOD_LABELS[row.calcMethod] || row.calcMethod }}</template>
          </el-table-column>
          <el-table-column label="适用程序" min-width="110">
            <template #default="{ row }">{{ formatProcedureTypes(row.procedureTypes) }}</template>
          </el-table-column>
          <el-table-column label="来源" width="76">
            <template #default="{ row }">
              <el-tag :type="row.deadlineSource === 'statutory' ? 'danger' : 'info'" size="small" effect="plain">
                {{ DEADLINE_SOURCE_LABELS[row.deadlineSource] || row.deadlineSource }}
              </el-tag>
            </template>
          </el-table-column>
          <el-table-column label="优先级" width="64" align="center">
            <template #default="{ row }">{{ row.priority }}</template>
          </el-table-column>
          <el-table-column label="启用" width="64" align="center">
            <template #default="{ row }">
              <el-switch
                :model-value="row.autoCalculate"
                size="small"
                @change="(v: string | number | boolean) => onToggle(row, v)"
              />
            </template>
          </el-table-column>
          <el-table-column label="操作" width="170" fixed="right">
            <template #default="{ row }">
              <el-button size="small" text type="primary" @click="openAudit(row)">
                <el-icon><Clock /></el-icon> 留痕
              </el-button>
              <el-button size="small" text @click="openEdit(row)">
                <el-icon><Edit /></el-icon> 编辑
              </el-button>
              <el-button size="small" text type="danger" @click="onDelete(row)">
                <el-icon><Delete /></el-icon> 删除
              </el-button>
            </template>
          </el-table-column>
        </el-table>
      </div>
    </div>

    <!-- 新建 / 编辑对话框 -->
    <el-dialog
      v-model="dialogVisible"
      :title="editingRule ? '编辑期限规则' : '新建期限规则'"
      width="560px"
      :close-on-click-modal="false"
    >
      <el-form label-width="92px" label-position="right">
        <el-form-item label="所属轨道" required>
          <el-select
            v-model="form.track"
            filterable
            allow-create
            default-first-option
            placeholder="选择或输入轨道标识"
            style="width: 100%"
          >
            <el-option v-for="o in trackOptions" :key="o.value" :label="o.label" :value="o.value" />
          </el-select>
        </el-form-item>
        <el-form-item label="规则名称" required>
          <el-input v-model="form.ruleName" placeholder="如：提交答辩状期间" maxlength="60" />
        </el-form-item>
        <el-form-item label="法条依据" required>
          <el-input v-model="form.legalBasis" placeholder="如：民事诉讼法第128条" maxlength="120" />
        </el-form-item>
        <el-form-item label="触发字段" required>
          <el-select v-model="form.triggerField" style="width: 100%">
            <el-option v-for="o in TRIGGER_FIELD_OPTIONS" :key="o.value" :label="o.label" :value="o.value" />
          </el-select>
        </el-form-item>
        <el-form-item label="偏移设置" required>
          <div class="offset-row">
            <el-input-number v-model="form.offsetValue" :min="-365" :max="3650" :step="1" />
            <el-select v-model="form.offsetUnit" style="width: 130px">
              <el-option v-for="o in OFFSET_UNIT_OPTIONS" :key="o.value" :label="o.label" :value="o.value" />
            </el-select>
          </div>
          <p class="field-hint">正数表示触发日之后，负数表示之前（如 -1 自然日）</p>
        </el-form-item>
        <el-form-item label="计算算法">
          <el-select v-model="form.calcMethod" style="width: 100%">
            <el-option v-for="o in CALC_METHOD_OPTIONS" :key="o.value" :label="o.label" :value="o.value" />
          </el-select>
        </el-form-item>
        <el-form-item label="适用程序">
          <el-input v-model="form.procedureTypes" placeholder='留空全部适用；或填「简易」/ JSON 数组 ["简易"]' />
        </el-form-item>
        <el-form-item label="期限来源">
          <el-radio-group v-model="form.deadlineSource">
            <el-radio v-for="o in DEADLINE_SOURCE_OPTIONS" :key="o.value" :value="o.value">
              {{ o.label }}
            </el-radio>
          </el-radio-group>
        </el-form-item>
        <el-form-item label="优先级">
          <el-input-number v-model="form.priority" :min="0" :max="100" />
          <span class="field-hint" style="margin-left: 8px">数值越大越优先展示</span>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="dialogVisible = false">取消</el-button>
        <el-button type="primary" :loading="saving" @click="saveRule">保存</el-button>
      </template>
    </el-dialog>

    <!-- 变更留痕抽屉 -->
    <DeadlineRuleAuditDrawer v-model:visible="auditVisible" :rule="auditRule" />
  </div>
</template>

<style scoped>
.section-head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  margin-bottom: 12px;
}
.section-head h4 {
  margin: 0 0 4px;
}
.desc {
  font-size: 12px;
  color: var(--gray-400);
  margin: 0;
}
.head-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}
.track-group {
  margin-bottom: 20px;
}
.track-head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}
.track-code {
  font-size: 12px;
  color: var(--c-text-placeholder);
  font-family: monospace;
}
.track-count {
  font-size: 12px;
  color: var(--c-text-secondary);
  margin-left: auto;
}
.rule-name {
  font-size: 13px;
  color: var(--c-text);
  line-height: 1.5;
}
.rule-basis {
  font-size: 12px;
  color: var(--c-text-secondary);
  line-height: 1.4;
}
.offset-text {
  font-family: monospace;
  font-size: 12px;
  color: var(--c-primary);
}
.offset-row {
  display: flex;
  gap: 8px;
  width: 100%;
}
.field-hint {
  margin: 4px 0 0;
  font-size: 12px;
  color: var(--c-text-placeholder);
  line-height: 1.4;
}
</style>
