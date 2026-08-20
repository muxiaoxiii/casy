/**
 * 浏览器开发模式 Mock 数据层
 *
 * 在没有 Tauri 后端的浏览器环境（`npm run dev` 纯前端预览）中，
 * 为常用命令提供模拟数据，让 UI 可以正常展示而不全空白。
 *
 * 判断依据：window.__TAURI_INTERNALS__ 是否存在。
 */

export function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && !!(window as any).__TAURI_INTERNALS__
}

// ── Mock 数据 ────────────────────────────────────────────
const mockCases = [
  { id: 'c1', caseName: '隆基244号无效案', caseNo: '(2024)国知局第244号', clientName: '隆基绿能', opponentName: '晶科能源', track: 'patent_invalidation', caseStatus: '进行中', caseRoute: '专利无效', court: '国知局', attorneys: ['张律师', '李律师'], caseGoal: '使权利要求1-4全部无效', caseType: 'computational', progress: 65, dueDate: '2026-08-25' },
  { id: 'c2', caseName: '华为商标侵权案', caseNo: '(2024)粤01民初1234号', clientName: '华为技术', opponentName: '某科技公司', track: 'civil_tort', caseStatus: '进行中', caseRoute: '民事诉讼', court: '广州知识产权法院', attorneys: ['王律师'], caseGoal: '获得损害赔偿', caseType: 'exploratory', progress: 40, dueDate: '2026-08-20' },
  { id: 'c3', caseName: '宁德时代专利无效', caseNo: '(2024)国知局第567号', clientName: '宁德时代', opponentName: '比亚迪', track: 'patent_invalidation', caseStatus: '等待中', caseRoute: '专利无效', court: '国知局', attorneys: ['张律师', '赵律师'], caseGoal: '扫清客户产品侵权风险', caseType: 'computational', progress: 30, dueDate: '2026-09-01' },
  { id: 'c4', caseName: '腾讯行政诉讼', caseNo: '(2024)京73行初89号', clientName: '腾讯科技', opponentName: '国知局', track: 'admin_litigation', caseStatus: '等待中', caseRoute: '行政诉讼', court: '北京知识产权法院', attorneys: ['李律师'], caseGoal: '撤销无效决定', caseType: 'growth', progress: 15 },
  { id: 'c5', caseName: '小米外观设计无效', caseNo: '(2023)国知局第890号', clientName: '小米科技', opponentName: 'OPPO', track: 'patent_invalidation', caseStatus: '已结案', caseRoute: '专利无效', court: '国知局', attorneys: ['王律师'], caseGoal: '维持无效决定', caseType: 'computational', progress: 100 },
  { id: 'c6', caseName: '百度专利侵权', caseNo: '(2024)京01民初567号', clientName: '百度在线', opponentName: '字节跳动', track: 'civil_tort', caseStatus: '进行中', caseRoute: '民事诉讼', court: '北京互联网法院', attorneys: ['张律师'], caseGoal: '停止侵权并获得赔偿', caseType: 'exploratory', progress: 55, dueDate: '2026-08-22' },
]

const mockTasks = [
  { id: 't1', taskName: '核对隆基口审证据清单', caseId: 'c1', priority: 'urgent_important', dueDate: '2026-08-25', taskType: 'action', startBucket: 'today', blocked: 0, sequenceOrder: 1, context: 'office', estimatedMinutes: 45, todayIndex: 0, flagged: true },
  { id: 't2', taskName: '起草华为案补充证据说明', caseId: 'c2', priority: 'urgent', dueDate: '2026-08-20', taskType: 'action', startBucket: 'today', blocked: 0, context: 'office', estimatedMinutes: 60, todayIndex: 1 },
  { id: 't3', taskName: '跟进宁德时代检索报告', caseId: 'c3', priority: 'normal', dueDate: '2026-08-28', taskType: 'waiting', waitingFor: '专利代理师', followUpDate: '2026-08-20', startBucket: 'anytime' },
  { id: 't4', taskName: '审核腾讯行政诉讼答辩状', caseId: 'c4', priority: 'important', dueDate: '2026-08-30', taskType: 'action', startBucket: 'upcoming', blocked: 0, context: 'court' },
  { id: 't5', taskName: '整理百度案技术文献', caseId: 'c6', priority: 'normal', taskType: 'action', startBucket: 'anytime', blocked: 0 },
  { id: 't6', taskName: '大疆案庭前调解方案', caseId: null, priority: 'normal', taskType: 'action', startBucket: 'someday' },
  { id: 't7', taskName: '更新案件进度周报', caseId: null, priority: 'normal', dueDate: '2026-08-21', taskType: 'action', startBucket: 'today', blocked: 0 },
]

const mockEvents = [
  { id: 'e1', title: '隆基无效口审', date: '2026-08-25', type: 'hearing', caseId: 'c1', time: '09:30' },
  { id: 'e2', title: '华为侵权案开庭', date: '2026-08-20', type: 'court', caseId: 'c2', time: '14:00' },
  { id: 'e3', title: '腾讯答辩状截止', date: '2026-08-30', type: 'deadline', caseId: 'c4', time: '23:59' },
  { id: 'e4', title: '客户周会', date: '2026-08-21', type: 'meeting', caseId: null, time: '15:00' },
  { id: 'e5', title: '百度案证据提交', date: '2026-08-22', type: 'deadline', caseId: 'c6', time: '17:00' },
]

const mockKnowledge = [
  { id: 'k1', title: '专利无效程序时间节点汇总', category: 'method', content: '提无效请求 → 受理 → 答复 → 口审 → 决定', lawName: '专利法' },
  { id: 'k2', title: '最高法知识产权案件裁判要旨', category: 'reference', content: '关于权利要求解释的裁判规则…', lawName: '司法解释' },
  { id: 'k3', title: '口审答辩策略思考', category: 'inspiration', content: '考虑从技术特征对比入手…' },
  { id: 'k4', title: '无效决定救济途径', category: 'question', content: '对无效决定不服可以提起行政诉讼…' },
  { id: 'k5', title: '某案办理经验复盘', category: 'experience', content: '证据链构建需要注意…' },
]

const mockStats = {
  hardSchedule: 2,
  dueToday: 3,
  waitingOverdue: 1,
  needReview: 2,
  activeCases: 4,
  waiting: 1,
  closed: 1,
  overdue: 1,
}

// ── Mock 命令处理 ────────────────────────────────────────
function handleMockCommand(command: string, args: Record<string, unknown>): unknown {
  switch (command) {
    case 'get_today_stats':
      return { ...mockStats }
    case 'list_cases':
    case 'get_dashboard_stats': {
      if (command === 'get_dashboard_stats') {
        return {
          stats: mockStats,
          cases: mockCases,
          tasks: mockTasks,
          events: mockEvents,
        }
      }
      const items = mockCases
      return { items, total: items.length }
    }
    case 'get_case':
      return mockCases.find(c => c.id === args.id) || null
    case 'list_tasks': {
      const filter = (args.filter as any) || {}
      let items = mockTasks.filter(t => !t.completed)
      if (filter.perspective === 'inbox') items = mockTasks.filter(t => t.startBucket === 'inbox')
      if (filter.perspective === 'today') items = mockTasks.filter(t => t.startBucket === 'today')
      if (filter.perspective === 'waiting') items = mockTasks.filter(t => t.taskType === 'waiting')
      if (filter.perspective === 'someday') items = mockTasks.filter(t => t.startBucket === 'someday')
      if (filter.perspective === 'next') items = mockTasks.filter(t => t.taskType === 'action' && (t.blocked === 0 || !t.caseId))
      if (filter.caseId) items = items.filter(t => t.caseId === filter.caseId)
      // 多个消费者期望数组（tasks store / TasksView / DashboardView）
      return items
    }
    case 'list_areas':
      return [
        { id: 'a1', name: '专利诉讼', description: '民事诉讼代理' },
        { id: 'a2', name: '专利无效', description: '无效宣告程序' },
        { id: 'a3', name: '行政诉讼', description: '对无效决定起诉' },
      ]
    case 'case_stats':
      return {
        total: mockCases.length,
        active: mockCases.filter(c => c.caseStatus !== '已结案').length,
        closed: mockCases.filter(c => c.caseStatus === '已结案').length,
        byTrack: [],
        byClient: [],
      }
    case 'list_knowledge':
      return { items: mockKnowledge, total: mockKnowledge.length }
    case 'get_calendar_events': {
      const year = (args.year as number) || new Date().getFullYear()
      const month = (args.month as number) || new Date().getMonth() + 1
      return mockEvents.filter(e => {
        const [y, m] = e.date.split('-').map(Number)
        return y === year && m === month
      })
    }
    case 'get_deadline_warnings_with_levels':
    case 'get_deadline_warnings':
      return [
        { deadlineId: 'd1', caseId: 'c2', caseName: '华为商标侵权案', deadlineName: '补充证据提交', dueDate: '2026-08-20', daysLeft: 1, level: 'R2', levelLabel: '明确', levelColor: '#B0823A', message: '明天到期：补充证据提交（华为商标侵权案）' },
        { deadlineId: 'd2', caseId: 'c6', caseName: '百度专利侵权', deadlineName: '证据提交', dueDate: '2026-08-22', daysLeft: 3, level: 'R1', levelLabel: '温和', levelColor: '#9BA2AF', message: '3 天后到期：证据提交' },
      ]
    case 'get_today_recommendations':
      return {
        recommendations: [
          { taskId: 't1', taskName: '核对隆基口审证据清单', caseId: 'c1', caseName: '隆基244号无效案', reason: '已标记重要 · 今天到期', score: 90, priority: 'urgent_important' },
          { taskId: 't2', taskName: '起草华为案补充证据说明', caseId: 'c2', caseName: '华为商标侵权案', reason: '明天到期', score: 80, priority: 'urgent' },
          { taskId: 't4', taskName: '审核腾讯行政诉讼答辩状', caseId: 'c4', caseName: '腾讯行政诉讼', reason: '重要任务 · 4 天后到期', score: 60, priority: 'important' },
        ],
        followupSuggestions: [
          { taskId: 't3', taskName: '跟进宁德时代检索报告', waitingFor: '专利代理师', waitingDays: 6, reason: '已等 6 天', action: '建议催办' },
        ],
        source: 'rule_engine',
        generatedAt: new Date().toISOString(),
      }
    case 'generate_daily_brief_cmd':
    case 'get_today_brief':
      return {
        content: `早安，张律师 · 每日早报\n\n【昨日回顾】完成 6/9，比上周五高 12%\n【今日要点】09:30 隆基口审 · 华为证据截止\n【等待跟进】1 个等待超 3 天\n【智能建议】今日有多个庭审，请提前准备材料`,
        date: new Date().toISOString().split('T')[0],
        createdAt: new Date().toISOString().split('T')[0],
      }
    case 'get_learning_analysis':
      return {
        durationStats: [
          { taskPattern: '起草文书', avgEstimated: 60, avgActual: 75, sampleCount: 8, accuracy: 0.8 },
          { taskPattern: '核对检查', avgEstimated: 45, avgActual: 38, sampleCount: 5, accuracy: 1.18 },
        ],
        activityPatterns: [
          { hour: 9, completions: 12, percentage: 24 },
          { hour: 14, completions: 10, percentage: 20 },
          { hour: 16, completions: 8, percentage: 16 },
        ],
        delayPatterns: [
          { caseType: 'patent_invalidation', avgDelayDays: 2.5, delayCount: 3, totalTasks: 10, delayRate: 0.3 },
        ],
        generatedAt: new Date().toISOString(),
      }
    case 'get_ai_config':
      return { mode: 'ollama', apiUrl: 'http://localhost:11434', model: 'qwen2.5:14b', dailyLimit: 50 }
    case 'get_ai_usage':
      return { todayCalls: 3 }
    case 'ai_chat': {
      const messages = (args.messages as Array<{ role: string; content: string }>) || []
      const lastUser = [...messages].reverse().find((m) => m.role === 'user')
      const text = lastUser?.content || ''
      // 浏览器预览模式：简单回应 + 展示一次工具调用流程（list_cases）
      if (/案件|案子/.test(text) && !/工具结果/.test(text)) {
        return JSON.stringify({
          tool: 'list_cases',
          params: { filter: { search: '' } },
        })
      }
      if (/工具结果/.test(text)) {
        return '（浏览器预览模式）以上是案件列表模拟数据。真实数据请在 Tauri 应用中使用。'
      }
      return '（浏览器预览模式）我是 Casy AI 助手。真实 AI 对话需要在 Tauri 应用中配置 Ollama 或 OpenAI 后端。'
    }
    case 'get_sync_status':
      return { webdav: { connected: false }, feishu: { connected: false } }
    default:
      return undefined
  }
}

/**
 * 浏览器模式下执行 mock 命令
 * 返回 undefined 表示没有 mock 处理，调用方正常走 invoke（会失败）
 */
export function tryMockCommand(command: string, args: Record<string, unknown>): unknown {
  return handleMockCommand(command, args)
}
