import { describe, expect, it } from 'vitest'
import { reactive } from 'vue'
import {
  useBriefingModel,
  EMPTY_FOCUS,
  SAMPLE_FOCUS,
  SAMPLE_REDLINES,
  SAMPLE_HEARINGS,
  type BriefingModelProps,
} from '../../src/shared/components/briefing/useBriefingModel'

function baseProps(overrides: Partial<BriefingModelProps> = {}): BriefingModelProps {
  return {
    type: 'daily',
    styleVariant: '',
    title: '',
    dateText: '',
    content: '',
    dateRange: '',
    preview: false,
    nextAction: null,
    redlines: [],
    hearings: [],
    metrics: { committedHours: '0.0', freeSpaceHours: '8.5h', waitingCount: 0, completedCount: 0, totalCases: 0 },
    ...overrides,
  }
}

describe('useBriefingModel · 类型/标题/编号派生', () => {
  it('styleVariant 为空时按类型回退到默认风格', () => {
    expect(useBriefingModel(baseProps({ type: 'daily' })).activeStyle.value).toBe('gazette')
    expect(useBriefingModel(baseProps({ type: 'weekly' })).activeStyle.value).toBe('dossier')
  })

  it('显式 styleVariant 优先于类型回退', () => {
    const model = useBriefingModel(baseProps({ type: 'weekly', styleVariant: 'tarot' }))
    expect(model.activeStyle.value).toBe('tarot')
    expect(model.activeMeta.value.id).toBe('tarot')
  })

  it('未知风格回退到 gazette 元数据', () => {
    const model = useBriefingModel(baseProps({ styleVariant: 'not-a-style' }))
    expect(model.activeMeta.value.id).toBe('gazette')
  })

  it('文案标签按每日/每周区分', () => {
    const daily = useBriefingModel(baseProps({ type: 'daily' }))
    expect(daily.reportTypeLabel.value).toBe('DAILY BRIEF')
    expect(daily.reportTypeCn.value).toBe('每日早报')
    const weekly = useBriefingModel(baseProps({ type: 'weekly' }))
    expect(weekly.reportTypeLabel.value).toBe('WEEKLY REVIEW')
    expect(weekly.reportTypeCn.value).toBe('每周复盘')
  })

  it('标题回退：优先外部标题，其次类型默认', () => {
    expect(useBriefingModel(baseProps({ type: 'daily' })).effectiveTitle.value).toBe('今日办案简报')
    expect(useBriefingModel(baseProps({ type: 'weekly' })).effectiveTitle.value).toBe('本周工作复盘')
    expect(useBriefingModel(baseProps({ title: '自定义标题' })).effectiveTitle.value).toBe('自定义标题')
  })

  it('日期：周报优先日期区间，其次 dateText，最后回退到当天', () => {
    expect(useBriefingModel(baseProps({ type: 'weekly', dateRange: '2024-01-01 ~ 2024-01-07' })).effectiveDate.value).toBe('2024-01-01 ~ 2024-01-07')
    expect(useBriefingModel(baseProps({ dateText: '2024年6月1日' })).effectiveDate.value).toBe('2024年6月1日')
    // 区间对日报不生效，仍走 dateText
    expect(useBriefingModel(baseProps({ type: 'daily', dateRange: 'RANGE', dateText: '2024年6月1日' })).effectiveDate.value).toBe('2024年6月1日')
    const today = useBriefingModel(baseProps()).effectiveDate.value
    expect(today).toMatch(/^\d{4}年\d{1,2}月\d{1,2}日$/)
  })

  it('报告编号按每日/每周前缀 + 紧凑日期生成', () => {
    expect(useBriefingModel(baseProps({ dateText: '2024年1月5日' })).reportCode.value).toBe('CASY-D-202415')
    expect(useBriefingModel(baseProps({ type: 'weekly', dateRange: '2024-01-01 ~ 2024-01-07' })).reportCode.value).toBe('CASY-W-2024010120240107')
  })

  it('无数字的日期回退到 CURRENT', () => {
    expect(useBriefingModel(baseProps({ dateText: '未定' })).reportCode.value).toBe('CASY-D-CURRENT')
  })
})

describe('useBriefingModel · 展示态与预览降级', () => {
  it('焦点任务：优先传入，其次预览示例，最后诚实空态', () => {
    const focus = { taskName: '真实任务' }
    expect(useBriefingModel(baseProps({ nextAction: focus })).displayNextAction.value).toEqual(focus)
    expect(useBriefingModel(baseProps({ preview: true })).displayNextAction.value).toBe(SAMPLE_FOCUS)
    expect(useBriefingModel(baseProps()).displayNextAction.value).toBe(EMPTY_FOCUS)
  })

  it('期限/排期：预览且为空时使用示例，否则沿用传入列表', () => {
    const redline = { id: 'r1', title: '真实期限' }
    expect(useBriefingModel(baseProps({ preview: true })).displayRedlines.value).toBe(SAMPLE_REDLINES)
    expect(useBriefingModel(baseProps({ preview: true, redlines: [redline] })).displayRedlines.value).toEqual([redline])
    expect(useBriefingModel(baseProps({ redlines: [redline] })).displayRedlines.value).toEqual([redline])

    const hearing = { id: 'h1', time: '10:00' }
    expect(useBriefingModel(baseProps({ preview: true })).displayHearings.value).toBe(SAMPLE_HEARINGS)
    expect(useBriefingModel(baseProps({ preview: true, hearings: [hearing] })).displayHearings.value).toEqual([hearing])
  })

  it('正文内容：优先传入内容，预览为空给示例文案，非预览为空则不渲染', () => {
    expect(useBriefingModel(baseProps({ content: '真实正文' })).displayContent.value).toBe('真实正文')
    expect(useBriefingModel(baseProps({ preview: true })).displayContent.value).toContain('示例内容')
    expect(useBriefingModel(baseProps()).displayContent.value).toBe('')
  })

  it('指标：非预览原样返回；预览对空值做示例覆盖', () => {
    const metrics = { committedHours: '3.0', freeSpaceHours: '6.0h', waitingCount: 2, completedCount: 5, totalCases: 3 }
    expect(useBriefingModel(baseProps({ metrics })).displayMetrics.value).toEqual(metrics)

    const previewMetrics = useBriefingModel(baseProps({ preview: true })).displayMetrics.value
    expect(previewMetrics).toEqual({ committedHours: '6.5', freeSpaceHours: '2.0h', waitingCount: 3, completedCount: 12, totalCases: 4 })
  })
})

describe('useBriefingModel · 指标卡片', () => {
  it('按当前展示的期限/排期与负荷生成四张卡片（补零）', () => {
    const model = useBriefingModel(baseProps({
      redlines: [{ id: 'r1' }, { id: 'r2' }, { id: 'r3' }],
      hearings: [{ id: 'h1' }],
      metrics: { committedHours: '4.5', freeSpaceHours: '3.0h', waitingCount: 1, completedCount: 7, totalCases: 2 },
    }))
    expect(model.metricCards.value).toEqual([
      { label: '期限事项', value: '03', note: '以当前列表为准' },
      { label: '排期事项', value: '01', note: '庭审与硬日程' },
      { label: '承诺负荷', value: '4.5h', note: '余量 3.0h' },
      { label: '已完成', value: '07', note: '等待 1' },
    ])
  })
})

describe('useBriefingModel · 响应式', () => {
  it('传入 reactive props 时，类型切换会联动更新派生值', () => {
    const props = reactive(baseProps({ styleVariant: '' }))
    const model = useBriefingModel(props)
    expect(model.activeStyle.value).toBe('gazette')
    expect(model.reportTypeLabel.value).toBe('DAILY BRIEF')
    props.type = 'weekly'
    expect(model.activeStyle.value).toBe('dossier')
    expect(model.reportTypeLabel.value).toBe('WEEKLY REVIEW')
    props.styleVariant = 'ledger'
    expect(model.activeStyle.value).toBe('ledger')
  })
})
