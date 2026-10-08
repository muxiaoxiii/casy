<script setup lang="ts">
import { ref, watch } from 'vue'
import { tauriCall } from '../../../core/tauriBridge'
import {
  AUDIT_ACTION_META,
  AUDIT_FIELD_META,
  safeParseJson,
  type DeadlineRuleAuditDto,
  type DeadlineRuleDto,
} from './deadlineRuleMeta'
import EmptyState from '../../../shared/components/EmptyState.vue'

const props = defineProps<{
  visible: boolean
  rule: DeadlineRuleDto | null
}>()

const emit = defineEmits<{
  (e: 'update:visible', value: boolean): void
}>()

const loading = ref(false)
const audits = ref<DeadlineRuleAuditDto[]>([])

interface AuditChange {
  key: string
  label: string
  before: string
  after: string
}

interface AuditSnapshot {
  label: string
  value: string
}

function formatField(key: string, format: ((v: unknown) => string) | undefined, value: unknown): string {
  if (value === null || value === undefined || value === '') return '（空）'
  return format ? format(value) : String(value)
}

/** before→after 字段级对比（仅列出有变化的字段） */
function diffOf(audit: DeadlineRuleAuditDto): AuditChange[] {
  const before = safeParseJson(audit.beforeJson)
  const after = safeParseJson(audit.afterJson)
  if (!before || !after) return []
  const changes: AuditChange[] = []
  for (const meta of AUDIT_FIELD_META) {
    const b = before[meta.key]
    const a = after[meta.key]
    if (JSON.stringify(b ?? null) !== JSON.stringify(a ?? null)) {
      changes.push({
        key: meta.key,
        label: meta.label,
        before: formatField(meta.key, meta.format, b),
        after: formatField(meta.key, meta.format, a),
      })
    }
  }
  return changes
}

/** create/delete 时展示快照关键字段 */
function snapshotOf(audit: DeadlineRuleAuditDto): AuditSnapshot[] {
  const raw = audit.action === 'create' ? audit.afterJson : audit.beforeJson
  const obj = safeParseJson(raw)
  if (!obj) return []
  return AUDIT_FIELD_META.filter((m) => obj[m.key] !== undefined && obj[m.key] !== null).map((m) => ({
    label: m.label,
    value: formatField(m.key, m.format, obj[m.key]),
  }))
}

function actionMeta(action: string) {
  return AUDIT_ACTION_META[action] || { label: action, type: 'info' as const }
}

async function loadAudits() {
  if (!props.rule) return
  loading.value = true
  try {
    const data = await tauriCall(
      'list_deadline_rule_audit',
      { ruleId: props.rule.id },
      { errorMessage: '加载变更留痕失败' },
    )
    audits.value = data || []
  } finally {
    loading.value = false
  }
}

watch(
  () => props.visible,
  (v) => {
    if (v) loadAudits()
  },
)

function close() {
  emit('update:visible', false)
}
</script>

<template>
  <el-drawer
    :model-value="visible"
    :title="rule ? `变更留痕 · ${rule.ruleName}` : '变更留痕'"
    size="440px"
    @update:model-value="emit('update:visible', $event)"
    @close="close"
  >
    <div v-loading="loading" class="audit-body">
      <EmptyState v-if="!loading && audits.length === 0" type="custom" compact hide-action title="暂无留痕记录" />

      <el-timeline v-else class="audit-timeline">
        <el-timeline-item
          v-for="item in audits"
          :key="item.id"
          :timestamp="item.createdAt || ''"
          placement="top"
        >
          <div class="audit-card">
            <div class="audit-head ui-row ui-row--between">
              <el-tag :type="actionMeta(item.action).type" size="small" effect="light">
                {{ actionMeta(item.action).label }}
              </el-tag>
              <span class="audit-actor">操作人：{{ item.actor || 'user' }}</span>
            </div>

            <!-- update / toggle：字段级 before → after -->
            <template v-if="item.action === 'update' || item.action === 'toggle'">
              <div v-if="diffOf(item).length" class="audit-diff ui-col ui-col--tight">
                <div v-for="c in diffOf(item)" :key="c.key" class="diff-row">
                  <span class="diff-label">{{ c.label }}</span>
                  <span class="diff-values">
                    <span class="diff-before">{{ c.before }}</span>
                    <span class="diff-arrow">→</span>
                    <span class="diff-after">{{ c.after }}</span>
                  </span>
                </div>
              </div>
              <p v-else class="audit-empty-line">无字段变化</p>
            </template>

            <!-- create / delete：快照 -->
            <div v-else class="audit-diff ui-col ui-col--tight">
              <div v-for="s in snapshotOf(item)" :key="s.label" class="diff-row">
                <span class="diff-label">{{ s.label }}</span>
                <span class="diff-values">
                  <span :class="item.action === 'create' ? 'diff-after' : 'diff-before'">{{ s.value }}</span>
                </span>
              </div>
              <p v-if="snapshotOf(item).length === 0" class="audit-empty-line">无快照内容</p>
            </div>
          </div>
        </el-timeline-item>
      </el-timeline>
    </div>
  </el-drawer>
</template>

<style scoped>
.audit-body {
  min-height: 200px;
}
.audit-timeline {
  padding-left: 4px;
}
.audit-card {
  background: var(--gray-50);
  border: 1px solid var(--c-border-light);
  border-radius: var(--c-radius-lg);
  padding: 10px 12px;
}
.audit-head {
  margin-bottom: 8px;
}
.audit-actor {
  font-size: 12px;
  color: var(--c-text-secondary);
}
.audit-diff {
  gap: 4px;
}
.diff-row {
  display: flex;
  align-items: baseline;
  gap: 8px;
  font-size: 12px;
  line-height: 1.6;
}
.diff-label {
  flex: 0 0 64px;
  color: var(--c-text-secondary);
}
.diff-values {
  display: flex;
  align-items: baseline;
  gap: 6px;
  flex-wrap: wrap;
  word-break: break-all;
}
.diff-before {
  color: var(--c-danger);
  text-decoration: line-through;
  text-decoration-color: var(--c-danger);
  text-decoration-thickness: 1px;
}
.diff-arrow {
  color: var(--gray-400);
}
.diff-after {
  color: var(--c-success);
  font-weight: 500;
}
.audit-empty-line {
  margin: 0;
  font-size: 12px;
  color: var(--c-text-placeholder);
}
</style>
