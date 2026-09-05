export interface FeishuSnapshot {
  assets?: Array<{fileToken: string; name: string; mimeType: string; size: number; contentBase64?: string; error?: string}>
  version: number
  appToken: string
  fetchedAt: string
  tables: Array<{
    table_id: string
    name: string
    fields: Array<{ field_id?: string; field_name: string; type: number; property?: unknown }>
    records: Array<{ record_id: string; fields: Record<string, unknown> }>
  }>
}
export interface SnapshotReport {
  assets: number
  files: number
  cases: number
  logs: number
  hearings: number
  tasks: number
  officials: number
  relations: number
  sourceRecords: number
  sourceLinks: number
  skipped: number
  warnings: string[]
}
export interface SourceRecord {
  source: string
  tableName: string
  recordId: string
  fields: Record<string, string>
  raw: Record<string, unknown>
  schema: Array<{ field_name: string; type: number; property?: unknown }>
}
