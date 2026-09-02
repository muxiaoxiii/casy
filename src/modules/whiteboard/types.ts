/**
 * 事实白板（W7 · LiquidText 式事实节点网络）本地类型定义
 * 与 src-tauri/src/commands/whiteboard.rs 的 DTO camelCase 字段对齐
 */

export interface WhiteboardDto {
  id: string
  caseId: string
  name: string
  nodeCount: number
  createdAt: string | null
  updatedAt: string | null
}

export interface FactNodeDto {
  id: string
  whiteboardId: string
  fileId: string | null
  fileName: string | null
  page: number | null
  excerpt: string
  note: string | null
  x: number
  y: number
  createdAt: string | null
  updatedAt: string | null
}

export interface WhiteboardEdgeDto {
  id: string
  whiteboardId: string
  sourceNodeId: string
  targetNodeId: string
  createdAt: string | null
}


/** list_case_files 返回的最小字段集（与 files.rs 的 CaseFile 对齐） */
export interface CaseFileLite {
  id: string
  caseId: string
  fileName: string
  filePath: string
  fileSize: number | null
  fileType: string | null
  category: string
  subCategory: string | null
  createdAt: string | null
}
