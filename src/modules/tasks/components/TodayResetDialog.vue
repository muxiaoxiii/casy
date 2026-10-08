<script setup lang="ts">
/**
 * TodayResetDialog —— 「整理今天」仪式（序时参考 · 律师版转译）
 *
 * 序时的三桶分流在律师场景的再学习：
 * - 「今天」里有不可谈判的硬锚点（庭审/法定期限）→ 不假装自动判定硬度，
 *   按**临近截止度**分车道呈现（urgent≤1天 / soon≤3天 / free），由人定夺
 * - 容量焦虑只对自主安排类计数：硬性项不由你选，不制造压力
 * - 移出临近截止项前二次确认（EULA §5.2：辅助手段，专业判断独立负责）
 * - 分流去向仅用现有桶：someday=计划 / anytime=回炉，永不物理删除
 */
import { ref, computed } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { casyContext } from '../../../core/plugin/context'
import EmptyState from '../../../shared/components/EmptyState.vue'

interface ResetTask {
  id: string
  taskName: string
  isFocus?: number
  dueDate?: string | null
  deadline?: string | null
  [key: string]: unknown
}

const props = defineProps<{
  modelValue: boolean
  tasks: ResetTask[]
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', v: boolean): void
  (e: 'changed'): void
}>()

const actions: Array<{ key: string; label: string; type: string; bucket: string | null }> = [
  { key: 'stay', label: '留在今天', type: 'danger', bucket: null },
  { key: 'later', label: '改天再说', type: '', bucket: 'someday' },
  { key: 'anytime', label: '移回随时', type: '', bucket: 'anytime' },
]

const working = ref<ResetTask[]>([])
const busyId = ref<string | null>(null)

/** 距到期天数 */
function daysToDue(t: ResetTask): number | null {
  const due = t.dueDate || t.deadline
  if (!due) return null
  const diff = Math.ceil((new Date(due).getTime() - Date.now()) / 86400000)
  return Number.isNaN(diff) ? null : diff
}

/** 车道：urgent(≤1天) / soon(2-3天) / free(无近期期限) */
function laneOf(t: ResetTask): 'urgent' | 'soon' | 'free' {
  const d = daysToDue(t)
  if (d !== null && d <= 1) return 'urgent'
  if (d !== null && d <= 3) return 'soon'
  return 'free'
}

const sortedWorking = computed(() => {
  const rank = { urgent: 0, soon: 1, free: 2 }
  return [...working.value].sort(
    (a, b) =>
      rank[laneOf(a)] - rank[laneOf(b)] ||
      (b.isFocus || 0) - (a.isFocus || 0) ||
      String(a.dueDate || '').localeCompare(String(b.dueDate || '')),
  )
})

/** 容量只计自主安排类；urgent 是硬锚点，不计入焦虑 */
const softCount = computed(() => working.value.filter(t => laneOf(t) !== 'urgent').length)
const overCapacity = computed(() => softCount.value > 5)
const urgentCount = computed(() => working.value.filter(t => laneOf(t) === 'urgent').length)

function syncFromProps() {
  working.value = props.tasks.map(t => ({ ...t }))
}

async function move(task: ResetTask, bucket: string | null, label: string) {
  const d = daysToDue(task)
  if (d !== null && d <= 3) {
    try {
      await ElMessageBox.confirm(
        `「${task.taskName}」${d === 0 ? '今天' : d === 1 ? '明天' : `${d} 天后`}到期。确定${label}？`,
        '临近截止',
        { confirmButtonText: '仍要移出', cancelButtonText: '留在今天', type: 'warning' },
      )
    } catch {
      return
    }
  }
  busyId.value = task.id
  const idx = working.value.findIndex(t => t.id === task.id)
  const backup = working.value[idx]
  working.value.splice(idx, 1)
  const result = await casyContext.tasks.update({
    id: task.id,
    startBucket: bucket,
    todayIndex: 0,
    isFocus: 0,
  })
  busyId.value = null
  if (result.ok) {
    ElMessage.success(`「${task.taskName}」${label}`)
    emit('changed')
  } else {
    working.value.splice(idx, 0, backup)
    ElMessage.error(result.error || '操作失败')
  }
}

function onAction(action: { bucket: string | null; label: string }, task: ResetTask) {
  if (action.bucket === null) return
  const label =
    action.bucket === 'someday'
      ? '已移入计划（某天）'
      : '已移回随时——需要时再排期'
  void move(task, action.bucket, label)
}
</script>

<template>
  <el-dialog
    :model-value="modelValue"
    title="整理今天"
    width="620px"
    @update:model-value="(v: boolean) => emit('update:modelValue', v)"
    @open="syncFromProps"
  >
    <div class="reset-head ui-row">
      <div class="reset-count" :class="{ over: overCapacity }">
        {{ softCount }}
      </div>
      <div class="reset-head-text ui-col">
        <b>{{ softCount }} 件自主安排<template v-if="urgentCount"> · {{ urgentCount }} 件临近截止</template></b>
        <span v-if="overCapacity">
          超出建议容量（≤5）。做完再加，不做堆积——临近截止的已置顶标红。
        </span>
        <span v-else-if="urgentCount">临近截止项已置顶并默认建议保留。</span>
        <span v-else>逐条确认：它今天必须推进吗？</span>
      </div>
    </div>

    <div class="reset-list">
      <div
        v-for="t in sortedWorking"
        :key="t.id"
        class="reset-row ui-row"
        :class="'lane-' + laneOf(t)"
      >
        <span class="rr-focus" :class="{ on: t.isFocus === 1 }">{{ t.isFocus === 1 ? '★' : '' }}</span>
        <span class="rr-name ui-truncate">
          {{ t.taskName }}
          <span v-if="laneOf(t) === 'urgent'" class="lane-badge urgent">
            {{ (daysToDue(t) ?? 0) <= 0 ? '今日到期' : '明天到期' }}
          </span>
          <span v-else-if="laneOf(t) === 'soon'" class="lane-badge soon">{{ daysToDue(t) }} 天后到期</span>
        </span>
        <div class="rr-actions ui-row">
          <el-button
            v-for="a in actions"
            :key="a.key"
            size="small"
            :type="busyId === t.id ? 'info' : a.bucket === null && laneOf(t) === 'urgent' ? 'danger' : a.type"
            :plain="!(a.bucket === null && laneOf(t) === 'urgent')"
            :disabled="busyId === t.id"
            @click="onAction(a, t)"
          >
            {{ a.label }}
          </el-button>
        </div>
      </div>
      <EmptyState
        v-if="working.length === 0"
        type="custom"
        title="今天清空了"
        description="三个结果，不是十五条待办。接下来做的每件事都是主动选择。"
        compact
      />
    </div>

    <template #footer>
      <span class="reset-foot-hint">
        客户来电等临时事：先记进收件箱不打断当下，稍后再判断是否挤占今天的重点。
      </span>
      <el-button @click="emit('update:modelValue', false)">完成</el-button>
    </template>
  </el-dialog>
</template>

<style scoped>
.reset-head {
  gap: 14px;
  padding: 4px 4px 14px;
}
.reset-count {
  width: 52px;
  height: 52px;
  border-radius: var(--c-radius-full);
  background: var(--c-success-light);
  color: var(--c-success);
  font-size: 22px;
  font-weight: 700;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}
.reset-count.over {
  background: var(--c-danger-light);
  color: var(--c-danger);
}
.reset-head-text {
  gap: 2px;
  font-size: 13px;
  color: var(--c-text-secondary);
}
.reset-head-text b { color: var(--c-text); }

.reset-list {
  max-height: 46vh;
  overflow-y: auto;
  border: 1px solid var(--c-border-light);
  border-radius: var(--c-radius-lg);
}
.reset-row {
  padding: 9px 12px;
  border-bottom: 1px solid var(--c-border-lighter);
}
.reset-row:last-child { border-bottom: none; }
.reset-row.lane-urgent .rr-name { font-weight: 600; }
.rr-focus { width: 14px; color: var(--c-warning); text-align: center; }
.rr-name {
  flex: 1;
  font-size: 13.5px;
  color: var(--c-text);
}
.lane-badge {
  display: inline-block;
  margin-left: 6px;
  font-size: 10.5px;
  padding: 0 5px;
  border-radius: 3px;
  vertical-align: 1px;
}
.lane-badge.urgent {
  background: var(--c-danger-light);
  color: var(--c-danger);
}
.lane-badge.soon {
  background: var(--c-warning-light);
  color: var(--c-warning);
}
.rr-actions { gap: 6px; flex-shrink: 0; }
.reset-foot-hint {
  margin-right: auto;
  font-size: 11.5px;
  color: var(--c-text-secondary);
}
</style>
