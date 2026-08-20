/**
 * Saved Filters Store（设计哲学 §9）
 * 筛选/排序/分组规则可保存复用
 */
import { defineStore } from 'pinia'
import { ref } from 'vue'
import { casyContext } from '../core/plugin/context'

export interface SavedFilter {
  id: string
  name: string
  module: string  // 'cases' | 'tasks' | 'knowledge'
  filter: Record<string, any>
  sortBy?: string
  groupBy?: string
  createdAt: string
}

export const useFiltersStore = defineStore('filters', () => {
  const filters = ref<SavedFilter[]>([])
  const loading = ref(false)

  async function loadFilters(module: string) {
    loading.value = true
    const result = await casyContext.settings.savedFilters(module)
    if (result.ok && result.data) {
      filters.value = result.data as SavedFilter[]
    }
    loading.value = false
  }

  async function saveFilter(filter: Omit<SavedFilter, 'id' | 'createdAt'>) {
    const result = await casyContext.settings.saveFilter({ ...filter })
    if (result.ok) {
      await loadFilters(filter.module)
    }
    return result
  }

  async function deleteFilter(id: string) {
    const result = await casyContext.settings.deleteFilter(id)
    if (result.ok) {
      filters.value = filters.value.filter(f => f.id !== id)
    }
    return result
  }

  async function applyFilter(id: string) {
    const filter = filters.value.find(f => f.id === id)
    if (filter) {
      return filter
    }
    return null
  }

  return {
    filters,
    loading,
    loadFilters,
    saveFilter,
    deleteFilter,
    applyFilter,
  }
})
