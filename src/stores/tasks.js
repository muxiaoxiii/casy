import { defineStore } from 'pinia'
import { tauriCallSafe } from '../core/tauriBridge.js'

export const useTasksStore = defineStore('tasks', {
  state: () => ({
    tasks: [],
    loading: false,
    filter: {
      completed: false,
      caseId: null,
    },
  }),

  getters: {
    pendingTasks: (state) => state.tasks.filter((t) => !t.completed),
    completedTasks: (state) => state.tasks.filter((t) => t.completed),

    urgentImportant: (state) =>
      state.tasks.filter((t) => t.priority === 'urgent_important' && !t.completed),
    important: (state) =>
      state.tasks.filter((t) => t.priority === 'important' && !t.completed),
    urgent: (state) =>
      state.tasks.filter((t) => t.priority === 'urgent' && !t.completed),
    normal: (state) =>
      state.tasks.filter((t) => t.priority === 'normal' && !t.completed),
  },

  actions: {
    async loadTasks() {
      this.loading = true
      const result = await tauriCallSafe('list_tasks', {
        filter: {
          completed: this.filter.completed || null,
          caseId: this.filter.caseId || null,
        },
      })
      if (result.ok) {
        this.tasks = result.data || []
      }
      this.loading = false
    },

    async createTask(data) {
      const result = await tauriCallSafe('create_task', { data })
      if (result.ok) {
        await this.loadTasks()
      }
      return result
    },

    async toggleTask(id) {
      const result = await tauriCallSafe('toggle_task', { id })
      if (result.ok) {
        const task = this.tasks.find((t) => t.id === id)
        if (task) task.completed = task.completed ? 0 : 1
      }
      return result
    },

    async deleteTask(id) {
      const result = await tauriCallSafe('delete_task', { id })
      if (result.ok) {
        this.tasks = this.tasks.filter((t) => t.id !== id)
      }
      return result
    },
  },
})
