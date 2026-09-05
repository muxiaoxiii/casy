import { defineStore } from 'pinia'
import { casyContext } from '../core/plugin/context'
import type { Task, TaskPriority, TaskType, Context, StartBucket } from '../types'
import type { CreateTaskPayload, UpdateTaskPayload } from '../types/ipc'
import { parseLocalDate, todayLocalISO, addDaysLocalISO, toLocalISODate } from '../shared/utils/date'
import { tasksForPerspective, buildGtdStats } from '../modules/tasks/utils/taskFilter'

// ============================================================
// GTD 类型（字段定义见 types/index.ts 的 Task 接口）
// ============================================================
export type { TaskType, Context, StartBucket }
export type GTDTask = Task

/**
 * 自定义透视配置（设计哲学 §5.2 透视保存）
 */
export interface CustomPerspective {
  id: string
  name: string
  icon?: string
  color?: string
  filters: {
    taskType?: TaskType | null
    priority?: TaskPriority | null
    context?: Context | null
    caseId?: string | null
    areaId?: string | null
    flagged?: boolean | null
    completed?: boolean
    dateRange?: 'overdue' | 'today' | 'week' | 'month' | null
  }
  sortBy?: 'dueDate' | 'priority' | 'createdAt' | 'todayIndex'
  sortOrder?: 'asc' | 'desc'
  createdAt: string
}

export interface TasksState {
  tasks: GTDTask[]
  loading: boolean
  activePerspective: string
  customPerspectives: CustomPerspective[]
  filter: {
    completed: boolean
    caseId: string | null
    areaId: string | null
    taskType: TaskType | null
  }
}

function taskSortValue(task: GTDTask, sortBy: NonNullable<CustomPerspective['sortBy']>): string | number | null {
  switch (sortBy) {
    case 'dueDate':
      return task.dueDate
    case 'priority':
      return task.priority
    case 'createdAt':
      return task.createdDate
    case 'todayIndex':
      return task.todayIndex
  }
}

// ============================================================
// Store 定义
// ============================================================
export const useTasksStore = defineStore('tasks', {
  state: (): TasksState => ({
    tasks: [],
    loading: false,
    activePerspective: 'inbox',
    customPerspectives: [],
    filter: {
      completed: false,
      caseId: null,
      areaId: null,
      taskType: null,
    },
  }),

  getters: {
    // ============================================================
    // GTD 透视过滤器
    // ============================================================
    
    /**
     * 收件箱：start_bucket='inbox' 的未完成任务
     */
    inboxTasks: (state): GTDTask[] =>
      state.tasks.filter(t => t.startBucket === 'inbox' && !t.completed),

    /**
     * 下一步行动：blocked=0 的任务（顺序项目中）或无案件的 action 任务
     */
    nextActions: (state): GTDTask[] =>
      tasksForPerspective(state.tasks, 'next', { todayStr: todayLocalISO() }),

    /**
     * 等待：task_type='waiting' 的未完成任务
     */
    waitingTasks: (state): GTDTask[] =>
      tasksForPerspective(state.tasks, 'waiting', { todayStr: todayLocalISO() }),

    /**
     * 今日：startDate <= 今天 或 startBucket='today'
     */
    todayTasks: (state): GTDTask[] => {
      const today = todayLocalISO()
      return tasksForPerspective(state.tasks, 'today', { todayStr: today })
        .sort((a, b) => (a.todayIndex || 0) - (b.todayIndex || 0))
    },

    /**
     * 回顾：nextReviewDate <= 今天
     */
    reviewTasks: (state): GTDTask[] => {
      const today = todayLocalISO()
      return state.tasks.filter(t => 
        !t.completed && t.nextReviewDate && t.nextReviewDate <= today
      )
    },

    /**
     * 某天：start_bucket='someday'
     */
    somedayTasks: (state): GTDTask[] =>
      state.tasks.filter(t => !t.completed && t.startBucket === 'someday'),

    // ============================================================
    // 统计
    // ============================================================
    
    taskStats: (state) => {
      const today = todayLocalISO()
      const stats = buildGtdStats(state.tasks, today)
      return {
        inbox: stats.inbox, next: stats.next, waiting: stats.waiting, today: stats.today, review: stats.review,
        someday: state.tasks.filter(t => !t.completed && t.startBucket === 'someday').length,
        overdue: state.tasks.filter(t => !t.completed && t.isOverdue === 1).length,
      }
    },

    // ============================================================
    // 旧版兼容
    // ============================================================
    
    pendingTasks: (state): GTDTask[] =>
      state.tasks.filter(t => !t.completed),

    completedTasks: (state): GTDTask[] =>
      state.tasks.filter(t => t.completed),

    urgentImportant: (state): GTDTask[] =>
      state.tasks.filter(t => t.priority === 'urgent_important' && !t.completed),

    important: (state): GTDTask[] =>
      state.tasks.filter(t => t.priority === 'important' && !t.completed),

    urgent: (state): GTDTask[] =>
      state.tasks.filter(t => t.priority === 'urgent' && !t.completed),

    normal: (state): GTDTask[] =>
      state.tasks.filter(t => t.priority === 'normal' && !t.completed),
  },

  actions: {
    // ============================================================
    // 数据加载
    // ============================================================
    
    async loadTasks(): Promise<void> {
      this.loading = true
      const result = await casyContext.tasks.list({
        completed: this.filter.completed || null,
        caseId: this.filter.caseId || null,
        areaId: this.filter.areaId || null,
        taskType: this.filter.taskType || null,
      })
      if (result.ok && result.data) {
        this.tasks = result.data
      }
      this.loading = false
    },

    // ============================================================
    // CRUD 操作
    // ============================================================
    
    async createTask(data: CreateTaskPayload): Promise<{ ok: boolean; data?: { id: string }; error?: string }> {
      const result = await casyContext.tasks.create({ ...data })
      if (result.ok) {
        await this.loadTasks()
      }
      return result
    },

    async updateTask(data: UpdateTaskPayload): Promise<{ ok: boolean; error?: string }> {
      const result = await casyContext.tasks.update({ ...data })
      if (result.ok) {
        await this.loadTasks()
      }
      return result
    },

    async toggleTask(id: string, actualMinutes?: number | null): Promise<{ ok: boolean; error?: string }> {
      const result = await casyContext.tasks.toggle(id, actualMinutes)
      if (result.ok) {
        await this.loadTasks()
      }
      return result
    },

    async deleteTask(id: string): Promise<{ ok: boolean; error?: string }> {
      const result = await casyContext.tasks.remove(id)
      if (result.ok) {
        this.tasks = this.tasks.filter(t => t.id !== id)
      }
      return result
    },

    // ============================================================
    // GTD 操作
    // ============================================================
    
    /**
     * 厘清任务：从收件箱移动到指定透视
     */
    async triageTask(taskId: string, data: {
      taskType: TaskType
      caseId?: string
      areaId?: string
      startDate?: string
      dueDate?: string
      context?: Context
    }): Promise<{ ok: boolean; error?: string }> {
      const updateData: UpdateTaskPayload = {
        id: taskId,
        taskType: data.taskType,
        caseId: data.caseId || null,
        areaId: data.areaId || null,
        startDate: data.startDate || null,
        dueDate: data.dueDate || null,
        context: data.context || null,
        startBucket: data.taskType === 'someday' ? 'someday' : 'anytime',
      }
      
      return await this.updateTask(updateData)
    },

    /**
     * 移动到今日列表
     */
    async moveToToday(taskId: string): Promise<{ ok: boolean; error?: string }> {
      return await this.updateTask({
        id: taskId,
        startBucket: 'today',
        todayIndex: this.taskStats.today,
      })
    },

    /**
     * 标记为等待
     */
    async markAsWaiting(taskId: string, waitingFor?: string): Promise<{ ok: boolean; error?: string }> {
      return await this.updateTask({
        id: taskId,
        taskType: 'waiting',
        waitingFor: waitingFor || null,
      })
    },

    /**
     * 标记旗标
     */
    async toggleFlag(taskId: string): Promise<{ ok: boolean; error?: string }> {
      const task = this.tasks.find(t => t.id === taskId)
      if (!task) return { ok: false, error: 'Task not found' }
      
      return await this.updateTask({
        id: taskId,
        flagged: task.flagged ? 0 : 1,
      })
    },

    /**
     * 重排今日列表
     */
    async reorderToday(taskIds: string[]): Promise<void> {
      for (let i = 0; i < taskIds.length; i++) {
        await this.updateTask({
          id: taskIds[i],
          todayIndex: i,
        })
      }
    },

    // ============================================================
    // 辅助方法
    // ============================================================
    
    /**
     * 获取指定透视的任务
     */
    getTasksByPerspective(perspective: string): GTDTask[] {
      switch (perspective) {
        case 'inbox': return this.inboxTasks
        case 'next': return this.nextActions
        case 'waiting': return this.waitingTasks
        case 'today': return this.todayTasks
        case 'review': return this.reviewTasks
        case 'someday': return this.somedayTasks
        default: return this.pendingTasks
      }
    },

    /**
     * 检查任务是否逾期
     */
    isTaskOverdue(task: GTDTask): boolean {
      if (!task.dueDate) return false
      // 时区（审查 P1-2）：原本 `toISOString().split('T')[0]` 在东八区
      // 00:00–07:59 取到的是昨天，导致昨天到期的任务不被判为逾期。
      return task.dueDate < todayLocalISO()
    },

    /**
     * 检查任务是否即将到期（3天内）
     */
    isTaskDueSoon(task: GTDTask): boolean {
      if (!task.dueDate) return false
      // 时区（审查 P1-2）：原本 `new Date(dueDate)`（UTC 午夜）减 `new Date()`（本地当下）
      // 再 Math.ceil，差值含小时分量，"今天到期"会随一天内的时刻在 0/1 之间跳变。
      const due = parseLocalDate(task.dueDate)
      if (!due) return false
      const today = parseLocalDate(todayLocalISO())!
      const diffDays = Math.round((due.getTime() - today.getTime()) / 86400000)
      return diffDays >= 0 && diffDays <= 3
    },

    /**
     * 计算等待天数
     */
    getWaitingDays(task: GTDTask): number {
      if (!task.followUpDate) return 0
      // 时区（审查 P1-2）：两侧均按本地午夜，消除小时分量导致的跳变。
      const followUp = parseLocalDate(task.followUpDate)
      if (!followUp) return 0
      const today = parseLocalDate(todayLocalISO())!
      return Math.round((today.getTime() - followUp.getTime()) / 86400000)
    },

    // ============================================================
    // 自定义透视管理（设计哲学 §5.2）
    // ============================================================

    /**
     * 加载自定义透视（从 localStorage）
     */
    loadCustomPerspectives(): void {
      try {
        const stored = localStorage.getItem('casy_custom_perspectives')
        if (stored) {
          this.customPerspectives = JSON.parse(stored)
        }
      } catch (e) {
        console.error('Failed to load custom perspectives:', e)
      }
    },

    /**
     * 保存自定义透视（到 localStorage）
     */
    saveCustomPerspective(perspective: Omit<CustomPerspective, 'id' | 'createdAt'>): CustomPerspective {
      const newPerspective: CustomPerspective = {
        ...perspective,
        id: 'custom_' + Date.now() + '_' + Math.random().toString(36).substr(2, 9),
        createdAt: new Date().toISOString(),
      }
      this.customPerspectives.push(newPerspective)
      this._persistPerspectives()
      return newPerspective
    },

    /**
     * 更新自定义透视
     */
    updateCustomPerspective(id: string, updates: Partial<CustomPerspective>): void {
      const index = this.customPerspectives.findIndex(p => p.id === id)
      if (index >= 0) {
        this.customPerspectives[index] = { ...this.customPerspectives[index], ...updates }
        this._persistPerspectives()
      }
    },

    /**
     * 删除自定义透视
     */
    deleteCustomPerspective(id: string): void {
      this.customPerspectives = this.customPerspectives.filter(p => p.id !== id)
      this._persistPerspectives()
      // 如果删除的是当前激活的透视，切换到收件箱
      if (this.activePerspective === id) {
        this.activePerspective = 'inbox'
      }
    },

    /**
     * 获取自定义透视的任务
     */
    getTasksByCustomPerspective(perspectiveId: string): GTDTask[] {
      const perspective = this.customPerspectives.find(p => p.id === perspectiveId)
      if (!perspective) return []

      let tasks = [...this.tasks]

      // 应用过滤器
      const { filters } = perspective
      if (filters.completed === false || filters.completed === undefined) {
        tasks = tasks.filter(t => !t.completed)
      }
      if (filters.taskType) {
        tasks = tasks.filter(t => t.taskType === filters.taskType)
      }
      if (filters.priority) {
        tasks = tasks.filter(t => t.priority === filters.priority)
      }
      if (filters.context) {
        tasks = tasks.filter(t => t.context === filters.context)
      }
      if (filters.caseId) {
        tasks = tasks.filter(t => t.caseId === filters.caseId)
      }
      if (filters.areaId) {
        tasks = tasks.filter(t => t.areaId === filters.areaId)
      }
      if (filters.flagged === true) {
        tasks = tasks.filter(t => t.flagged)
      }

      // 日期范围过滤
      const today = todayLocalISO()
      if (filters.dateRange === 'overdue') {
        tasks = tasks.filter(t => t.dueDate && t.dueDate < today)
      } else if (filters.dateRange === 'today') {
        tasks = tasks.filter(t => t.dueDate === today || t.startBucket === 'today')
      } else if (filters.dateRange === 'week') {
        // 时区（审查 P1-2）：`toISOString()` 取 UTC 日期，东八区凌晨会偏早一天
        const weekEndStr = addDaysLocalISO(today, 7)
        tasks = tasks.filter(t => t.dueDate && t.dueDate >= today && t.dueDate <= weekEndStr)
      } else if (filters.dateRange === 'month') {
        const monthEnd = parseLocalDate(today)!
        monthEnd.setMonth(monthEnd.getMonth() + 1)
        const monthEndStr = toLocalISODate(monthEnd)
        tasks = tasks.filter(t => t.dueDate && t.dueDate >= today && t.dueDate <= monthEndStr)
      }

      // 排序
      const sortBy = perspective.sortBy
      if (sortBy) {
        const order = perspective.sortOrder === 'desc' ? -1 : 1
        tasks.sort((a, b) => {
          const aVal = String(taskSortValue(a, sortBy) || '')
          const bVal = String(taskSortValue(b, sortBy) || '')
          return aVal.localeCompare(bVal) * order
        })
      }

      return tasks
    },

    /**
     * 持久化透视到 localStorage
     */
    _persistPerspectives(): void {
      try {
        localStorage.setItem('casy_custom_perspectives', JSON.stringify(this.customPerspectives))
      } catch (e) {
        console.error('Failed to persist custom perspectives:', e)
      }
    },
  },
})
