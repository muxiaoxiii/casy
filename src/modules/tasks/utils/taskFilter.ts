/**
 * 任务过滤与统计（M-UI-0 · 低风险拆分自 TasksView.vue）
 *
 * 纯函数模块——把视图里的 gtdStats / gtdTasks / caseGroupSections / matrixQuadrants /
 * childrenMap 五个 computed 的判定逻辑下沉为可单测函数。视图只保留「取值 + 触发」。
 *
 * 行为与原视图完全一致：
 * - 透视切换（all/inbox/today/upcoming/multiday/next/waiting/matrix/bycase/completed/deferred/自定义）
 * - 卡片过滤三连（关键字 / 上下文 / 案件）
 * - 统计计数（gtdStats）、案件分组、四象限、子任务映射
 *
 * 一处健壮性增强：关键字匹配对 taskName/description/caseName 统一 String(x ?? '') 兜底，
 * 避免某字段为 null 时 `?.toLowerCase().includes()` 抛 TypeError（原实现仅靠类型保证非空）。
 * 在非空分支下结果与原实现逐字节一致。
 */

import type { Task } from '../../../types'

// ============================================================
// 统计计数（gtdStats）
// ============================================================

export interface GtdStats {
  all: number
  inbox: number
  today: number
  upcoming: number
  multiday: number
  next: number
  waiting: number
  deferred: number
  matrix: number
  bycase: number
  completed: number
  review: number
}

/** 顶部透视标签徽标计数：未完成/已完成域的各口径数量 */
export function buildGtdStats(tasks: Task[], todayStr: string): GtdStats {
  const keys = ['all','inbox','today','upcoming','multiday','next','waiting','deferred','matrix','bycase','completed','review'] as const
  return Object.fromEntries(keys.map(key => [key,tasksForPerspective(tasks,key,{todayStr}).length])) as unknown as GtdStats
}

// ============================================================
// 透视任务流过滤（gtdTasks 的透视分支）
// ============================================================

/**
 * 按透视返回原始任务流（未做搜索/上下文/案件三连过滤——那是 applyTaskCardFilters 的职责）。
 * 自定义透视（default 分支）通过 getCustomTasks 回调交由 store 解析。
 */
export function tasksForPerspective(
  tasks: Task[],
  perspective: string,
  opts: {
    todayStr: string
    getCustomTasks?: (perspectiveId: string) => Task[]
  }
): Task[] {
  const today = opts.todayStr
  let list: Task[]

  switch (perspective) {
    case 'all':
      list = tasks.filter(t => !t.completed)
      break

    case 'inbox':
      list = tasks.filter(t => !t.completed && t.startBucket === 'inbox')
      break

    case 'today':
      // W2：今日专注隐藏未到期推迟任务（deferUntil > 今天才藏，到期当天自动回归）
      list = tasks.filter(t => !t.completed && (
        ((t.dueDate || t.deadline) && (t.dueDate || t.deadline)! <= today)
        || ((!t.deferUntil || t.deferUntil <= today) && t.startBucket !== 'someday'
          && (t.startBucket === 'today' || (t.startDate && t.startDate <= today)))))
      break

    case 'deferred':
      // W2 已推迟透视：deferUntil 未到期（未来日期）的未完成任务，按回归日升序
      list = tasks.filter(t => !t.completed && t.deferUntil && t.deferUntil > today)
      list.sort((a, b) => String(a.deferUntil).localeCompare(String(b.deferUntil)))
      break

    case 'upcoming':
      list = tasks.filter(t => !t.completed && (t.dueDate || t.startDate || t.deadline))
      list.sort((a, b) => (a.dueDate || a.startDate || '9999').localeCompare(b.dueDate || b.startDate || '9999'))
      break

    case 'multiday':
      list = tasks.filter(t => !t.completed && t.startDate && t.dueDate && t.startDate !== t.dueDate)
      break

    case 'next':
      list = tasks.filter(t => !t.completed && !t.blocked && !t.waitingFor
        && (t.taskType === 'action' || !t.taskType)
        && !['inbox','someday'].includes(t.startBucket)
        && (!t.startDate || t.startDate <= today) && (!t.deferUntil || t.deferUntil <= today))
      break

    case 'waiting':
      list = tasks.filter(t => !t.completed && (t.taskType === 'waiting' || !!t.waitingFor))
      break

    case 'completed':
      list = tasks.filter(t => !!t.completed)
      break

    case 'review':
      list = tasks.filter(t => !t.completed && t.nextReviewDate && t.nextReviewDate <= today)
      break

    case 'matrix':
    case 'bycase':
      list = tasks.filter(t => !t.completed)
      break

    default:
      // 自定义透视交由 store 解析
      list = opts.getCustomTasks ? opts.getCustomTasks(perspective) : []
      break
  }

  return list
}

/**
 * 顶级透视只渲染顶级任务：父任务已在列表中时，子任务仅在展开区出现；
 * 父任务不在当前透视时，子任务提升为独立可行动项。
 */
export function topLevelPerspectiveTasks(tasks: Task[]): Task[] {
  const visible = new Map(tasks.map(t => [t.id, t]))
  return tasks.filter(t => {
    const parentId = (t as Task & { parentTaskId?: string | null }).parentTaskId ?? t.parentId
    return !parentId || !visible.has(parentId)
  })
}

// ============================================================
// 卡片过滤三连（搜索 / 上下文 / 案件）
// ============================================================

export interface TaskCardFilterOpts {
  metric?: string
  todayStr?: string
  searchQuery?: string
  /** 'all' 或具体上下文值 */
  contextFilter?: string
  /** 'all' 或具体 caseId */
  caseFilter?: string
  /** 案件名解析回调（默认空串，避免依赖视图的 cases 列表） */
  resolveCaseName?: (caseId: string | null) => string
}

/** 在透视结果之上叠加关键字/上下文/案件过滤（顺序与原 gtdTasks 一致） */
export function applyTaskCardFilters(list: Task[], opts: TaskCardFilterOpts = {}): Task[] {
  const q = (opts.searchQuery ?? '').trim().toLowerCase()
  let result = list
  if (opts.metric === 'dueOrOverdue') {
    result = result.filter(t => !t.completed && t.dueDate && opts.todayStr && t.dueDate <= opts.todayStr)
  } else if (opts.metric === 'dueToday') {
    result = result.filter(t => !t.completed && (t.dueDate === opts.todayStr || t.deadline === opts.todayStr))
  } else if (opts.metric === 'waitingOverdue') {
    result = result.filter(t => !t.completed && (t.taskType === 'waiting' || !!t.waitingFor) && t.followUpDate && t.followUpDate < (opts.todayStr || ''))
  }

  // 关键字搜索：名称/备注/案件名任一命中
  if (q) {
    const resolveCase = opts.resolveCaseName ?? (() => '')
    result = result.filter(t => {
      const name = String(t.taskName ?? '').toLowerCase()
      const desc = String(t.description ?? '').toLowerCase()
      const cn = String(resolveCase(t.caseId) ?? '').toLowerCase()
      return name.includes(q) || desc.includes(q) || cn.includes(q)
    })
  }

  // 上下文过滤
  const ctx = opts.contextFilter
  if (ctx && ctx !== 'all') {
    result = result.filter(t => t.context === ctx)
  }

  // 案件过滤
  const caseF = opts.caseFilter
  if (caseF && caseF !== 'all') {
    result = result.filter(t => t.caseId === caseF)
  }

  return result
}

// ============================================================
// 案件分组视图（caseGroupSections）
// ============================================================

export interface CaseGroupSection {
  caseId: string
  caseName: string
  tasks: Task[]
}

/** 把（已过滤的）任务按案件分组；无案件归入「律所通用 / 未指定案件」 */
export function buildCaseGroupSections(
  tasks: Task[],
  resolveCaseName: (caseId: string | null) => string
): CaseGroupSection[] {
  const map = new Map<string, CaseGroupSection>()
  const unassigned: Task[] = []

  for (const t of tasks) {
    if (t.caseId) {
      if (!map.has(t.caseId)) {
        map.set(t.caseId, {
          caseId: t.caseId,
          caseName: resolveCaseName(t.caseId) || '未知案件',
          tasks: [],
        })
      }
      map.get(t.caseId)!.tasks.push(t)
    } else {
      unassigned.push(t)
    }
  }

  const list = Array.from(map.values())
  if (unassigned.length) {
    list.push({
      caseId: '__unassigned__',
      caseName: '律所通用 / 未指定案件',
      tasks: unassigned,
    })
  }
  return list
}

// ============================================================
// 四象限看板（matrixQuadrants）
// ============================================================

export interface MatrixQuadrant {
  key: string
  title: string
  desc: string
  color: string
  tasks: Task[]
}

export interface MatrixQuadrants {
  q1: MatrixQuadrant
  q2: MatrixQuadrant
  q3: MatrixQuadrant
  q4: MatrixQuadrant
}

/** 未完成任务按优先级/类型落进四象限 */
export function buildMatrixQuadrants(tasks: Task[]): MatrixQuadrants {
  const uncompleted = tasks.filter(t => !t.completed)
  return {
    q1: {
      key: 'urgent_important',
      title: '重要且紧急 (Do First)',
      desc: '诉讼举证截止、明日开庭准备、紧急保全',
      color: '#f56c6c',
      tasks: uncompleted.filter(t => t.priority === 'urgent_important'),
    },
    q2: {
      key: 'important',
      title: '重要不紧急 (Schedule)',
      desc: '起草长篇辩护词、战略推演、客户深度维系',
      color: '#e6a23c',
      tasks: uncompleted.filter(t => t.priority === 'important' || (!t.priority && (t.startDate && t.dueDate && t.startDate !== t.dueDate))),
    },
    q3: {
      key: 'urgent',
      title: '紧急不重要 (Delegate)',
      desc: '调取常规档案、法庭文书盖章送达、助理跟进',
      color: '#409eff',
      tasks: uncompleted.filter(t => t.priority === 'urgent' || t.taskType === 'waiting'),
    },
    q4: {
      key: 'normal',
      title: '普通 / 不紧急 (Someday)',
      desc: '模板整理、行业合规资讯查阅、备忘归档',
      color: '#909399',
      tasks: uncompleted.filter(t => t.priority === 'normal' || !t.priority),
    },
  }
}

// ============================================================
// 子任务映射（childrenMap）
// ============================================================

/** parentId → 子任务列表 */
export function buildChildrenMap(tasks: Task[]): Map<string, Task[]> {
  const m = new Map<string, Task[]>()
  for (const t of tasks) {
    if (!t.parentId) continue
    if (!m.has(t.parentId)) m.set(t.parentId, [])
    m.get(t.parentId)!.push(t)
  }
  return m
}
