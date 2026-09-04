/**
 * 卷宗文件纯工具（从 CaseListView.vue 拆分 · 低风险重构）
 *
 * 只包含无副作用的纯函数与常量，供 useCaseFiles composable 复用，
 * 并可直接在 node 环境做单测。不触碰 IPC 类型 / CommandMap。
 */

/** 卷宗文件登记项（后端 list_case_files 数据；字段按组件实际消费面声明） */
export interface CaseFile {
  id?: string
  fileName?: string | null
  filePath?: string | null
  category?: string | null
  fileSize?: number | null
  createdAt?: string | null
  updatedAt?: string | null
  [key: string]: unknown
}

/** 卷宗目录树节点（后端 list_case_dirs 数据） */
export interface CaseDir {
  relPath?: string | null
  name?: string
  fileCount?: number
  [key: string]: unknown
}

export type FileSortOrder = 'added' | 'recent' | 'name'

/** 文件筛选/排序选项 */
export interface FileFilterOptions {
  selectedDirRel?: string
  activeCategory?: string
  searchQuery?: string
  sortOrder?: FileSortOrder
}

/** 文件分类（模板与筛选共用） */
export const FILE_CATEGORIES: Array<{ key: string; label: string }> = [
  { key: 'all', label: '全部分类' },
  { key: 'summons', label: '传票 / 通知书' },
  { key: 'evidence', label: '证据材料' },
  { key: 'submitted', label: '提交文件' },
  { key: 'received', label: '接收文件' },
  { key: 'internal', label: '内部文件' },
  { key: 'correspondence', label: '往来函件' },
  { key: 'other', label: '其他' },
]

/** 字节格式化（保留原实现语义：空/0 → '0 B'） */
export function formatFileSize(bytes?: number | null): string {
  if (!bytes || bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i]
}

/** 提取文件扩展名（无扩展名/空文件名为 'FILE'） */
export function getFileExt(fileName?: string | null): string {
  if (!fileName) return 'FILE'
  const ext = fileName.split('.').pop()
  return ext ? ext.toUpperCase() : 'FILE'
}

/**
 * 卷宗文件列表的穿透/子目录过滤、分类过滤、关键字搜索与排序。
 * 纯函数：入参为快照，不持有响应式状态。
 */
export function filterAndSortFiles(files: CaseFile[], opts: FileFilterOptions = {}): CaseFile[] {
  let list = [...files]

  // 1. 目录树穿透/子目录过滤
  const rel = opts.selectedDirRel
  if (rel) {
    list = list.filter((f) => {
      const p = f.filePath || ''
      return p.includes(`/${rel}/`) || p.includes(`\\${rel}\\`) || f.category === rel
    })
  }

  // 2. 分类筛选
  if (opts.activeCategory && opts.activeCategory !== 'all') {
    list = list.filter((f) => f.category === opts.activeCategory)
  }

  // 3. 关键字搜索
  const q = (opts.searchQuery || '').trim().toLowerCase()
  if (q) {
    list = list.filter((f) => (f.fileName || '').toLowerCase().includes(q))
  }

  // 4. 排序方式
  const sortOrder = opts.sortOrder || 'added'
  if (sortOrder === 'recent') {
    list.sort((a, b) =>
      (b.updatedAt || b.createdAt || '').localeCompare(a.updatedAt || a.createdAt || ''),
    )
  } else if (sortOrder === 'name') {
    list.sort((a, b) => (a.fileName || '').localeCompare(b.fileName || '', 'zh'))
  } else {
    list.sort((a, b) => (b.createdAt || '').localeCompare(a.createdAt || ''))
  }

  return list
}
