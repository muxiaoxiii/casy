<script setup lang="ts">
/**
 * TaskRow —— 任务行核心表面（M-UI-0 · U-3 自绘组件起步）
 *
 * 自绘含义：行的结构、完成交互、动效全部由本组件负责（不再依赖
 * el-table/el-card 类封装；el-tag/el-icon/el-dropdown 仅作叶子控件）。
 *
 * 职责边界：
 * - 本组件只做"展示 + 语义事件"，不含任何 IPC / 业务规则（双路径铁律）
 * - 完成动画（Things3 式填充→对勾→划线）随组件迁移，样式自包含
 * - 案件/领域名称解析由父级注入（视图持有主数据）
 */
import { computed } from 'vue'
import { useLocalDay } from '../../../shared/useLocalDay'
import {
  Check, Folder, Collection, Star, Calendar, Timer, Clock,
  Lock, More, Edit, ArrowRight, Delete, AlarmClock, RefreshLeft,
} from '../../../shared/icons'
import {
  isOverdue, formatDate, getWaitingDays,
  getTaskTypeLabel, getTaskTypeColor,
} from '../utils/taskDisplay'

export interface RowTask {
  id: string
  taskName: string
  completed: number
  taskType?: string | null
  caseId?: string | null
  areaId?: string | null
  flagged?: number | null
  blocked?: number | null
  dueDate?: string | null
  deadline?: string | null
  dueTime?: string | null
  estimatedMinutes?: number | null
  waitingFor?: string | null
  followUpDate?: string | null
  context?: string | null
  nextReviewDate?: string | null
  /** W2 推迟日（YYYY-MM-DD），未到期时今日/焦点透视隐藏 */
  deferUntil?: string | null
  [key: string]: unknown
}

const props = defineProps<{
  task: RowTask
  /** 当前透视 key：review 显示回顾操作，inbox 显示厘清入口 */
  perspective?: string
  snoozeOptions?: Array<{ value: string; label: string }>
  resolveCaseName?: (id: string) => string
  resolveAreaName?: (id: string) => string
  /** A1-4 子任务 */
  hasChildren?: boolean
  expanded?: boolean
  /** 序时参考：今日重点星标（仅 today 透视启用） */
  focusable?: boolean
}>()

const emit = defineEmits<{
  (e: 'toggle', task: RowTask): void
  (e: 'open', task: RowTask): void
  (e: 'triage', task: RowTask): void
  (e: 'move-today', task: RowTask): void
  (e: 'mark-waiting', task: RowTask): void
  (e: 'snooze', task: RowTask, option: string): void
  (e: 'delete', task: RowTask): void
  (e: 'reviewed', task: RowTask): void
  (e: 'follow-up', task: RowTask): void
  (e: 'toggle-expand', task: RowTask): void
  (e: 'toggle-focus', task: RowTask): void
  /** W2：推迟到某日（父级弹出日期选择） */
  (e: 'defer', task: RowTask): void
  /** W2：提前结束推迟 */
  (e: 'undefer', task: RowTask): void
}>()

const done = computed(() => props.task.completed === 1)
const dueText = computed(() => props.task.dueDate || props.task.deadline || '')
const overdue = computed(() => { todayStr.value; return !done.value && isOverdue(dueText.value) })
const waitingDays = computed(() =>
  (todayStr.value, props.task.taskType === 'waiting' ? getWaitingDays(props.task) : 0)
)

const focused = computed(() => props.task.isFocus === 1)

/** W2：推迟中（deferUntil 晚于今天，本地日期字符串比较即可） */
const todayStr = useLocalDay()
const deferActive = computed(
  () => !!props.task.deferUntil && props.task.deferUntil > todayStr.value
)

function caseName(id: string | null | undefined): string {
  return id && props.resolveCaseName ? props.resolveCaseName(id) : ''
}
function areaName(id: string | null | undefined): string {
  return id && props.resolveAreaName ? props.resolveAreaName(id) : ''
}
</script>

<template>
  <div
    class="task-card"
    :class="{
      overdue,
      'due-soon': task.dueSoon === 1,
      flagged: task.flagged === 1,
      blocked: task.blocked === 1,
    }"
  >
    <!-- 子任务展开钮（Things3 惯例：有子项时显示） -->
    <span
      v-if="hasChildren"
      class="twistie"
      :class="{ open: expanded }"
      @click.stop="emit('toggle-expand', task)"
    >▸</span>
    <span v-else class="twistie-spacer" />

    <!-- 今日重点星标 -->
    <span
      v-if="focusable"
      class="focus-star"
      :class="{ on: focused }"
      :title="focused ? '取消重点' : '设为今日重点'"
      @click.stop="emit('toggle-focus', task)"
    >★</span>

    <!-- 完成圆圈：Things3 式填充动画 -->
    <button type="button" role="checkbox" :aria-checked="done" :aria-label="task.taskName" class="task-check" :class="{ done }" @click="emit('toggle', task)">
      <el-icon v-if="done"><Check /></el-icon>
    </button>

    <!-- 任务内容 -->
    <div class="task-content" role="button" tabindex="0" :aria-label="'编辑任务：' + task.taskName" @keydown.enter.prevent="emit('open', task)" @keydown.space.prevent="emit('open', task)" @click="emit('open', task)">
      <div class="task-title">
        <span class="task-name-text" :class="{ struck: done }">{{ task.taskName }}</span>
        <el-tag
          v-if="task.taskType && task.taskType !== 'action'"
          :color="getTaskTypeColor(task.taskType)"
          size="small"
          effect="dark"
        >
          {{ getTaskTypeLabel(task.taskType) }}
        </el-tag>
      </div>

      <div class="task-meta">
        <span v-if="task.caseId" class="meta-item case">
          <el-icon><Folder /></el-icon>
          {{ caseName(task.caseId) }}
        </span>

        <span v-if="task.areaId" class="meta-item area">
          <el-icon><Collection /></el-icon>
          {{ areaName(task.areaId) }}
        </span>

        <span v-if="task.flagged === 1" class="meta-item flagged">
          <el-icon color="#F59E0B"><Star /></el-icon>
        </span>

        <span
          v-if="dueText"
          class="meta-item deadline"
          :class="{ overdue }"
        >
          <el-icon><Calendar /></el-icon>
          {{ formatDate(dueText) }}{{ task.dueTime ? ' ' + task.dueTime : '' }}
        </span>

        <span v-if="task.estimatedMinutes" class="meta-item estimated">
          <el-icon><Timer /></el-icon>
          {{ task.estimatedMinutes }}分钟
        </span>

        <span
          v-if="task.taskType === 'waiting' && task.waitingFor"
          class="meta-item waiting"
          :class="{ 'waiting-warning': waitingDays > 3 }"
        >
          <el-icon><Clock /></el-icon>
          等 {{ task.waitingFor }}
          <span v-if="waitingDays > 0" class="waiting-days">({{ waitingDays }}天)</span>
          <el-button
            v-if="waitingDays > 3"
            size="small"
            type="warning"
            plain
            class="follow-up-btn"
            @click.stop="emit('follow-up', task)"
          >
            催办
          </el-button>
        </span>

        <span v-if="task.context" class="meta-item context">@{{ task.context }}</span>

        <span v-if="task.blocked === 1" class="meta-item blocked">
          <el-icon><Lock /></el-icon>
          已锁定
        </span>

        <span v-if="task.deferUntil" class="meta-item deferred" :class="{ active: deferActive }">
          <el-icon><AlarmClock /></el-icon>
          {{ deferActive ? `推迟至 ${formatDate(task.deferUntil)}` : `曾推迟至 ${formatDate(task.deferUntil)}` }}
        </span>
      </div>
    </div>

    <!-- 操作区：Review 透视显示回顾闭环，其余为语义菜单 -->
    <div class="task-actions">
      <template v-if="perspective === 'review'">
        <span v-if="task.nextReviewDate" class="review-info">下次回顾 {{ task.nextReviewDate }}</span>
        <el-button size="small" type="primary" plain class="review-btn" @click="emit('reviewed', task)">
          已回顾
        </el-button>
      </template>
      <!-- W2：已推迟透视提供提前结束入口 -->
      <el-button
        v-if="perspective === 'deferred' && deferActive"
        size="small"
        plain
        class="undefer-btn"
        @click.stop="emit('undefer', task)"
      >
        <el-icon><RefreshLeft /></el-icon>
        提前结束推迟
      </el-button>
      <el-dropdown trigger="click">
        <el-button :icon="More" circle size="small" aria-label="任务操作" title="任务操作" />
        <template #dropdown>
          <el-dropdown-menu>
            <el-dropdown-item :icon="Edit" @click="emit('open', task)">编辑</el-dropdown-item>
            <el-dropdown-item
              v-if="perspective === 'inbox'"
              :icon="ArrowRight"
              @click="emit('triage', task)"
            >
              厘清
            </el-dropdown-item>
            <el-dropdown-item
              v-if="perspective !== 'today'"
              :icon="Calendar"
              @click="emit('move-today', task)"
            >
              移至今日
            </el-dropdown-item>
            <el-dropdown-item
              v-if="task.taskType !== 'waiting'"
              :icon="Clock"
              @click="emit('mark-waiting', task)"
            >
              标记等待
            </el-dropdown-item>
            <el-dropdown-item
              v-for="s in snoozeOptions ?? []"
              :key="s.value"
              :icon="Clock"
              divided
              @click="emit('snooze', task, s.value)"
            >
              稍后：{{ s.label }}
            </el-dropdown-item>
            <el-dropdown-item :icon="Calendar" divided @click="emit('defer', task)">
              推迟到…
            </el-dropdown-item>
            <el-dropdown-item
              v-if="task.deferUntil"
              :icon="RefreshLeft"
              @click="emit('undefer', task)"
            >
              提前结束推迟
            </el-dropdown-item>
            <el-dropdown-item :icon="Delete" divided @click="emit('delete', task)">删除</el-dropdown-item>
          </el-dropdown-menu>
        </template>
      </el-dropdown>
    </div>
  </div>
</template>

<style scoped>
/* ── 子任务展开钮 ── */
.twistie {
  width: 14px;
  flex-shrink: 0;
  text-align: center;
  font-size: 11px;
  color: var(--c-text-secondary);
  cursor: pointer;
  transition: transform var(--motion-fast) var(--ease-out);
  user-select: none;
}
.twistie.open { transform: rotate(90deg); }
.twistie:hover { color: var(--c-text); }
.twistie-spacer { width: 14px; flex-shrink: 0; }

.focus-star {
  width: 16px;
  flex-shrink: 0;
  text-align: center;
  font-size: 13px;
  line-height: 1;
  color: var(--gray-300);
  cursor: pointer;
  transition: transform var(--motion-fast) var(--ease-out), color var(--motion-fast) var(--ease-out);
}
.focus-star:hover { transform: scale(1.25); }
.focus-star.on { color: var(--c-warning); }

/* ── 行容器 ── */
.task-card {
  display: flex;
  align-items: center;
  gap: 8px;
  background: var(--c-bg-card);
  border: 1px solid var(--c-border);
  border-left: 3px solid var(--c-border-strong);
  border-radius: var(--c-radius-lg);
  padding: 12px 16px;
  transition:
    box-shadow var(--motion-fast) var(--ease-out),
    border-color var(--motion-fast) var(--ease-out),
    opacity var(--motion-base) var(--ease-out);
}
.task-card:hover { box-shadow: var(--shadow-sm); border-color: var(--c-border-strong); }
.task-card.overdue { border-left-color: var(--status-risk); }
.task-card.due-soon { border-left-color: var(--status-warning); }
.task-card.flagged { background: var(--bg-warning-weak); }
.task-card.blocked { border-left-color: var(--c-text-secondary); opacity: 0.85; }

/* ── 完成圆圈：Things3 式填充动画 ── */
.task-check {
  padding: 0;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  border: 2px solid var(--c-border-strong);
  background: var(--c-bg-card);
  cursor: pointer;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  box-sizing: border-box;
  position: relative;
  overflow: hidden;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06);
  transition:
    border-color var(--motion-fast) var(--ease-out),
    background-color var(--motion-fast) var(--ease-out),
    transform var(--motion-fast) var(--ease-out);
}

.task-card.overdue .task-check {
  border-color: var(--status-risk);
  background: var(--c-bg-card);
}

.task-card.flagged .task-check {
  border-color: var(--status-warning);
  background: var(--c-bg-card);
}

.task-card.due-soon .task-check {
  border-color: var(--status-warning);
  background: var(--c-bg-card);
}

.task-check::before {
  content: '';
  position: absolute;
  inset: 1px;
  border-radius: 50%;
  background: var(--c-success);
  transform: scale(0);
  transition: transform var(--motion-base) var(--ease-spring);
}
.task-check .el-icon {
  position: relative;
  z-index: 1;
  font-size: 11px;
  color: #ffffff;
  opacity: 0;
  transform: scale(0.4) rotate(-30deg);
  transition:
    opacity var(--motion-fast) ease-out,
    transform var(--motion-base) var(--ease-spring);
}
.task-check:hover {
  border-color: var(--c-primary);
  transform: scale(1.1);
}
.task-check:active { transform: scale(0.92); }
.task-check.done {
  border-color: var(--c-success) !important;
  background: var(--c-success) !important;
}
.task-check.done::before { transform: scale(1); }
.task-check.done .el-icon {
  opacity: 1;
  transform: none;
  transition-delay: 60ms;
}

/* ── 内容区 ── */
.task-content { flex: 1; min-width: 0; cursor: pointer; }

.task-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 14px;
  font-weight: 500;
  color: var(--c-text);
  margin-bottom: 4px;
}

/* 划线过渡：background-size 0→100%（可动画的删除线） */
.task-name-text {
  overflow-wrap: anywhere;
  background-image: linear-gradient(currentColor, currentColor);
  background-size: 0% 1px;
  background-repeat: no-repeat;
  background-position: 0 55%;
  transition:
    background-size var(--motion-base) var(--ease-out),
    color var(--motion-base) var(--ease-out);
}
.task-name-text.struck {
  background-size: 100% 1px;
  color: var(--c-text-secondary, var(--c-text-secondary));
}

.task-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  font-size: 12px;
  color: var(--c-text-secondary);
}
.meta-item { display: flex; align-items: center; gap: 3px; }
.meta-item.case { color: var(--c-primary, #409EFF); }
.meta-item.area { color: var(--c-success, #67C23A); }
.meta-item.deadline.overdue { color: var(--c-danger, #F56C6C); }
.meta-item.waiting { color: var(--c-warning, #E6A23C); }
.meta-item.flagged { color: #F59E0B; }
.meta-item.estimated { color: var(--gray-500); }
.meta-item.blocked { color: var(--c-text-secondary); }

.waiting-days { color: var(--c-danger, #F56C6C); white-space: nowrap; }
.waiting-warning { color: #F59E0B; font-weight: 500; }
.follow-up-btn { margin-left: 8px; font-size: 11px; padding: 2px 6px; }
.meta-item.context {
  color: var(--c-text-secondary);
  background: var(--c-bg-subtle);
  padding: 1px 5px;
  border-radius: 3px;
}
.meta-item.deferred { color: var(--gray-500); }
.meta-item.deferred.active {
  color: #5b7a9e;
  background: color-mix(in srgb, #5b7a9e 12%, transparent);
  padding: 1px 6px;
  border-radius: 4px;
}

.undefer-btn {
  margin-right: 8px;
  font-size: 12px;
  padding: 4px 10px;
  --el-button-border-color: #5b7a9e;
  --el-button-text-color: #5b7a9e;
  --el-button-hover-border-color: #5b7a9e;
  --el-button-hover-text-color: #ffffff;
  --el-button-hover-bg-color: #5b7a9e;
}

/* ── 操作区 ── */
.task-actions { flex-shrink: 0; }
@media (max-width: 600px) {
  .task-card { flex-wrap: wrap; padding: 12px 8px; }
  .task-content { min-width: 120px; }
  .task-title { flex-wrap: wrap; }
  .meta-item.waiting { flex-wrap: wrap; }
  .task-actions { margin-left: auto; max-width: 100%; }
  .review-info { white-space: normal; }
}
.review-info {
  font-size: 11px;
  color: var(--c-text-secondary, var(--c-text-secondary));
  margin-right: 8px;
  white-space: nowrap;
}
.review-btn {
  margin-right: 8px;
  font-size: 12px;
  padding: 4px 10px;
  --el-button-border-color: var(--c-info);
  --el-button-text-color: var(--c-info);
  --el-button-hover-border-color: var(--c-info);
  --el-button-hover-text-color: #ffffff;
  --el-button-hover-bg-color: var(--c-info);
}
</style>
