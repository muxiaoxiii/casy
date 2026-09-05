import type { SearchResult } from './bindings'

export interface KnowledgeIndexJob {
  id: string
  itemId: string
  title: string
  model: string
  status: 'queued' | 'running' | 'completed' | 'failed' | 'cancelled' | 'stale'
  completedChunks: number
  totalChunks: number
  error: string | null
}
export interface KnowledgeIndexStatus {
  configured: boolean
  total: number
  indexed: number
  queued: number
  running: number
  failed: number
  jobs: KnowledgeIndexJob[]
}
export interface KnowledgeSearchResponse {
  results: SearchResult[]
  semanticStatus: string
  warning: string | null
}
