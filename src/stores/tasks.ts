import { defineStore } from 'pinia'
import { tauriCallSafe } from '../core/tauriBridge'
import type { Task, TaskPriority, TaskType, Context, StartBucket } from '../types'

// ============================================================
// GTD 类型（字段定义见 types/index.ts 的 Task 接口）
// ============================================================
export type { TaskType, Context, StartBucket }
export type GTDTask = Task

export interface TasksState {
  tasks: GTDTask[]
  loading: boolean
  activePerspective: string
  filter: {
    completed: boolean
    caseId: string | null
    areaId: string | null
    taskType: TaskType | null
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
      state.tasks.filter(t => 
        !t.completed && 
        t.taskType === 'action' && 
        (t.blocked === 0 || !t.caseId)
      ),

    /**
     * 等待：task_type='waiting' 的未完成任务
     */
    waitingTasks: (state): GTDTask[] =>
      state.tasks.filter(t => !t.completed && t.taskType === 'waiting'),

    /**
     * 今日：startDate <= 今天 或 startBucket='today'
     */
    todayTasks: (state): GTDTask[] => {
      const today = new Date().toISOString().split('T')[0]
      return state.tasks
        .filter(t => 
          !t.completed && 
          (t.startBucket === 'today' || 
           (t.startDate && t.startDate <= today))
        )
        .sort((a, b) => (a.todayIndex || 0) - (b.todayIndex || 0))
    },

    /**
     * 回顾：nextReviewDate <= 今天
     */
    reviewTasks: (state): GTDTask[] => {
      const today = new Date().toISOString().split('T')[0]
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
      const today = new Date().toISOString().split('T')[0]
      return {
        inbox: state.tasks.filter(t => t.startBucket === 'inbox' && !t.completed).length,
        next: state.tasks.filter(t => !t.completed && t.taskType === 'action' && (t.blocked === 0 || !t.caseId)).length,
        waiting: state.tasks.filter(t => !t.completed && t.taskType === 'waiting').length,
        today: state.tasks.filter(t => !t.completed && (t.startBucket === 'today' || (t.startDate && t.startDate <= today))).length,
        review: state.tasks.filter(t => !t.completed && t.nextReviewDate && t.nextReviewDate <= today).length,
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
      const result = await tauriCallSafe<GTDTask[]>('list_tasks', {
        filter: {
          completed: this.filter.completed || null,
          caseId: this.filter.caseId || null,
          areaId: this.filter.areaId || null,
          taskType: this.filter.taskType || null,
        },
      })
      if (result.ok && result.data) {
        this.tasks = result.data
      }
      this.loading = false
    },

    // ============================================================
    // CRUD 操作
    // ============================================================
    
    async createTask(data: Partial<GTDTask>): Promise<ReturnType<typeof tauriCallSafe<GTDTask>>> {
      const result = await tauriCallSafe<GTDTask>('create_task', { data })
      if (result.ok) {
        await this.loadTasks()
      }
      return result
    },

    async updateTask(data: Partial<GTDTask> & { id: string }): Promise<ReturnType<typeof tauriCallSafe<void>>> {
      const result = await tauriCallSafe<void>('update_task', { data })
      if (result.ok) {
        await this.loadTasks()
      }
      return result
    },

    async toggleTask(id: string): Promise<ReturnType<typeof tauriCallSafe<void>>> {
      const result = await tauriCallSafe<void>('toggle_task', { id })
      if (result.ok) {
        const task = this.tasks.find(t => t.id === id)
        if (task) task.completed = task.completed ? 0 : 1
      }
      return result
    },

    async deleteTask(id: string): Promise<ReturnType<typeof tauriCallSafe<void>>> {
      const result = await tauriCallSafe<void>('delete_task', { id })
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
    }): Promise<ReturnType<typeof tauriCallSafe<void>>> {
      const updateData: any = {
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
    async moveToToday(taskId: string): Promise<ReturnType<typeof tauriCallSafe<void>>> {
      return await this.updateTask({
        id: taskId,
        startBucket: 'today',
        todayIndex: this.taskStats.today,
      })
    },

    /**
     * 标记为等待
     */
    async markAsWaiting(taskId: string, waitingFor?: string): Promise<ReturnType<typeof tauriCallSafe<void>>> {
      return await this.updateTask({
        id: taskId,
        taskType: 'waiting',
        waitingFor: waitingFor || null,
      })
    },

    /**
     * 标记旗标
     */
    async toggleFlag(taskId: string): Promise<ReturnType<typeof tauriCallSafe<void>>> {
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
      return task.dueDate < new Date().toISOString().split('T')[0]
    },

    /**
     * 检查任务是否即将到期（3天内）
     */
    isTaskDueSoon(task: GTDTask): boolean {
      if (!task.dueDate) return false
      const today = new Date()
      const dueDate = new Date(task.dueDate)
      const diffDays = Math.ceil((dueDate.getTime() - today.getTime()) / (1000 * 60 * 60 * 24))
      return diffDays >= 0 && diffDays <= 3
    },

    /**
     * 计算等待天数
     */
    getWaitingDays(task: GTDTask): number {
      if (!task.followUpDate) return 0
      const today = new Date()
      const followUp = new Date(task.followUpDate)
      return Math.ceil((today.getTime() - followUp.getTime()) / (1000 * 60 * 60 * 24))
    },
  },
})
