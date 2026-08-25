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
import {
  Check, Folder, Collection, Star, Calendar, Timer, Clock,
  Lock, More, Edit, ArrowRight, Delete,
} from '@element-plus/icons-vue'
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
}>()

const done = computed(() => props.task.completed === 1)
const dueText = computed(() => props.task.dueDate || props.task.deadline || '')
const overdue = computed(() => !done.value && isOverdue(dueText.value))
const waitingDays = computed(() =>
  props.task.taskType === 'waiting' ? getWaitingDays(props.task) : 0
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

    <!-- 完成圆圈：Things3 式填充动画 -->
    <div class="task-check" :class="{ done }" @click="emit('toggle', task)">
      <el-icon v-if="done"><Check /></el-icon>
    </div>

    <!-- 任务内容 -->
    <div class="task-content" @click="emit('open', task)">
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
      <el-dropdown trigger="click">
        <el-button :icon="More" circle size="small" />
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
  color: #9BA2AF;
  cursor: pointer;
  transition: transform var(--motion-fast) var(--ease-out);
  user-select: none;
}
.twistie.open { transform: rotate(90deg); }
.twistie:hover { color: #18181B; }
.twistie-spacer { width: 14px; flex-shrink: 0; }

/* ── 行容器 ── */
.task-card {
  display: flex;
  align-items: center;
  gap: 8px;
  background: #ffffff;
  border: 1px solid var(--c-border-light, #E4E7ED);
  border-left: 3px solid #E5E7EB;
  border-radius: 8px;
  padding: 12px 16px;
  transition:
    box-shadow var(--motion-fast) var(--ease-out),
    border-color var(--motion-fast) var(--ease-out),
    opacity var(--motion-base) var(--ease-out);
}
.task-card:hover { box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08); }
.task-card.overdue { border-left-color: #EF4444; background: #FEF2F2; }
.task-card.due-soon { border-left-color: #F59E0B; }
.task-card.flagged { background: #FFFBEB; }
.task-card.blocked { border-left-color: #9BA2AF; opacity: 0.85; }

/* ── 完成圆圈：Things3 式填充动画 ── */
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
  position: relative;
  overflow: hidden;
  transition:
    border-color var(--motion-fast) var(--ease-out),
    transform var(--motion-fast) var(--ease-out);
}
.task-check::before {
  content: '';
  position: absolute;
  inset: 1px;
  border-radius: 50%;
  background: var(--c-success, #4C8067);
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
.task-check:hover { border-color: #4C8067; transform: scale(1.08); }
.task-check:active { transform: scale(0.92); }
.task-check.done { border-color: #4C8067; }
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
  color: #18181B;
  margin-bottom: 4px;
}

/* 划线过渡：background-size 0→100%（可动画的删除线） */
.task-name-text {
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
  color: var(--c-text-secondary, #9BA2AF);
}

.task-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  font-size: 12px;
  color: #A1A1AA;
}
.meta-item { display: flex; align-items: center; gap: 3px; }
.meta-item.case { color: var(--c-primary, #409EFF); }
.meta-item.area { color: var(--c-success, #67C23A); }
.meta-item.deadline.overdue { color: var(--c-danger, #F56C6C); }
.meta-item.waiting { color: var(--c-warning, #E6A23C); }
.meta-item.flagged { color: #F59E0B; }
.meta-item.estimated { color: #6B7280; }
.meta-item.blocked { color: #9BA2AF; }

.waiting-days { color: var(--c-danger, #F56C6C); }
.waiting-warning { color: #F59E0B; font-weight: 500; }
.follow-up-btn { margin-left: 8px; font-size: 11px; padding: 2px 6px; }
.meta-item.context {
  color: #909399;
  background: #F4F4F5;
  padding: 1px 5px;
  border-radius: 3px;
}

/* ── 操作区 ── */
.task-actions { flex-shrink: 0; }
.review-info {
  font-size: 11px;
  color: var(--c-text-secondary, #9BA2AF);
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
  --el-button-hover-text-color: #ffffff;
  --el-button-hover-bg-color: #6C6A9C;
}
</style>
