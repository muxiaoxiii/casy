import { computed } from 'vue'
import { briefingStyleMeta, type BriefingStyleOption } from '../../briefingStyles'

/**
 * 简报呈现模型（BriefingModal 的展示派生逻辑）。
 *
 * 这一层从 BriefingModal 里剥离出来：它只依据 props 派生「展示态」——文案、标题、
 * 报告编号、日期、预览降级、指标卡片等——不含任何 DOM/浏览器 API 或静态资源。
 * 因此它可以在 Node 环境下被独立单测，也方便未来在导出/打印场景里复用而无需挂载整个弹窗。
 *
 * 边界约定：
 * - 输入：BriefingModelProps（与组件 props 在展示语义上对齐，天然可传组件的 props 对象）。
 * - 输出：一组 computed ref。组件只负责把这些值渲染进模板，并把资产/剪贴板/图片导出等
 *   依赖 DOM 或静态资源的行为留在组件内部。
 */

export type BriefingType = 'daily' | 'weekly'

export interface FocusItem {
  taskName?: string
  description?: string
  caseName?: string
  caseCode?: string
}

export interface RedlineItem {
  id?: string | number
  title?: string
  caseTitle?: string
  timeText?: string
}

export interface HearingItem {
  id?: string | number
  time?: string
  title?: string
  court?: string
  timeRemaining?: string
}

export interface ReportMetrics {
  committedHours?: string | number
  freeSpaceHours?: string | number
  waitingCount?: number
  completedCount?: number
  totalCases?: number
}

export interface BriefingModelProps {
  type: BriefingType
  styleVariant: string
  title: string
  dateText: string
  content: string
  dateRange: string
  preview: boolean
  nextAction: FocusItem | null
  redlines: RedlineItem[]
  hearings: HearingItem[]
  metrics: ReportMetrics
}

/** 预览态下占位的示例焦点任务（仅用于展示排版，不写入真实数据）。 */
export const SAMPLE_FOCUS: FocusItem = {
  taskName: '复核证据目录与庭审提纲',
  description: '确认引用材料与原件索引一致，并标记仍需补证的条目。',
  caseName: '示例案件',
  caseCode: 'SAMPLE-01',
}

/** 无焦点任务时的「诚实空态」，明确告知当前没有数据。 */
export const EMPTY_FOCUS: FocusItem = {
  taskName: '暂无重点行动',
  description: '当前没有可展示的焦点任务。请结合任务列表与案件进度自行确认下一步。',
  caseName: '未指定案件',
  caseCode: 'NO-DATA',
}

export const SAMPLE_REDLINES: RedlineItem[] = [
  { id: 'sample-r1', title: '提交证据交换清单', caseTitle: '示例案件', timeText: '今日 17:00' },
  { id: 'sample-r2', title: '确认客户授权范围', caseTitle: '示例咨询', timeText: '明日到期' },
]

export const SAMPLE_HEARINGS: HearingItem[] = [
  { id: 'sample-h1', time: '09:30', title: '庭前会议', court: '第二审判法庭', timeRemaining: '今日排期' },
]

/** 依据展示语义派生的一套状态。传入组件的 props 对象即可（会响应式追踪其字段）。 */
export function useBriefingModel(props: BriefingModelProps) {
  const activeStyle = computed(() => props.styleVariant || (props.type === 'daily' ? 'gazette' : 'dossier'))
  const activeMeta = computed<BriefingStyleOption>(() => briefingStyleMeta[activeStyle.value] || briefingStyleMeta.gazette)
  const reportTypeLabel = computed(() => (props.type === 'daily' ? 'DAILY BRIEF' : 'WEEKLY REVIEW'))
  const reportTypeCn = computed(() => (props.type === 'daily' ? '每日早报' : '每周复盘'))
  const effectiveTitle = computed(() => props.title || (props.type === 'daily' ? '今日办案简报' : '本周工作复盘'))

  const effectiveDate = computed(() => {
    if (props.type === 'weekly' && props.dateRange) return props.dateRange
    if (props.dateText) return props.dateText
    const now = new Date()
    return `${now.getFullYear()}年${now.getMonth() + 1}月${now.getDate()}日`
  })

  const reportCode = computed(() => {
    const compactDate = effectiveDate.value.replace(/\D/g, '').slice(0, 16) || 'CURRENT'
    return `CASY-${props.type === 'daily' ? 'D' : 'W'}-${compactDate}`
  })

  const displayNextAction = computed(() => props.nextAction || (props.preview ? SAMPLE_FOCUS : EMPTY_FOCUS))
  const displayRedlines = computed(() =>
    props.preview && props.redlines.length === 0 ? SAMPLE_REDLINES : props.redlines,
  )
  const displayHearings = computed(() =>
    props.preview && props.hearings.length === 0 ? SAMPLE_HEARINGS : props.hearings,
  )
  const displayContent = computed(() => {
    if (props.content) return props.content
    if (!props.preview) return ''
    return '本报告用于展示排版与导出效果。示例内容不会写入案件、任务或日历数据。'
  })

  const displayMetrics = computed(() => {
    if (!props.preview) return props.metrics
    return {
      committedHours: props.metrics.committedHours && Number(props.metrics.committedHours) > 0 ? props.metrics.committedHours : '6.5',
      freeSpaceHours: props.metrics.freeSpaceHours !== '8.5h' ? props.metrics.freeSpaceHours : '2.0h',
      waitingCount: props.metrics.waitingCount || 3,
      completedCount: props.metrics.completedCount || 12,
      totalCases: props.metrics.totalCases || 4,
    }
  })

  const metricCards = computed(() => [
    { label: '期限事项', value: String(displayRedlines.value.length).padStart(2, '0'), note: '以当前列表为准' },
    { label: '排期事项', value: String(displayHearings.value.length).padStart(2, '0'), note: '庭审与硬日程' },
    { label: '承诺负荷', value: `${displayMetrics.value.committedHours ?? '0.0'}h`, note: `余量 ${displayMetrics.value.freeSpaceHours ?? '0.0h'}` },
    { label: '已完成', value: String(displayMetrics.value.completedCount ?? 0).padStart(2, '0'), note: `等待 ${displayMetrics.value.waitingCount ?? 0}` },
  ])

  return {
    activeStyle,
    activeMeta,
    reportTypeLabel,
    reportTypeCn,
    effectiveTitle,
    effectiveDate,
    reportCode,
    displayNextAction,
    displayRedlines,
    displayHearings,
    displayContent,
    displayMetrics,
    metricCards,
  }
}

export type BriefingModel = ReturnType<typeof useBriefingModel>
