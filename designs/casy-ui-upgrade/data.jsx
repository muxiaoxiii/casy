// ═══════════════════════════════════════════════════════════
// Casy UI Upgrade — Mock Data
// ═══════════════════════════════════════════════════════════

window.CasyData = {
  // ── 案件列表 ──────────────────────────────────────────
  cases: [
    { id: 'C001', name: '隆基244号无效', client: '隆基绿能', opponent: '晶科能源', track: 'patent_invalidation', status: '进行中', caseNo: '(2024)国知局第244号', court: '国知局', priority: 'high', dueDate: '2025-01-15', hearingDate: '2025-01-20', attorneys: ['张律师', '李律师'], progress: 65 },
    { id: 'C002', name: '华为商标侵权案', client: '华为技术', opponent: '某科技公司', track: 'civil_tort', status: '进行中', caseNo: '(2024)粤01民初1234号', court: '广州知识产权法院', priority: 'urgent', dueDate: '2025-01-10', hearingDate: '2025-01-18', attorneys: ['王律师'], progress: 40 },
    { id: 'C003', name: '宁德时代专利无效', client: '宁德时代', opponent: '比亚迪', track: 'patent_invalidation', status: '进行中', caseNo: '(2024)国知局第567号', court: '国知局', priority: 'normal', dueDate: '2025-02-01', hearingDate: '2025-02-15', attorneys: ['张律师', '赵律师'], progress: 30 },
    { id: 'C004', name: '腾讯行政诉讼', client: '腾讯科技', opponent: '国知局', track: 'admin_litigation', status: '等待中', caseNo: '(2024)京73行初89号', court: '北京知识产权法院', priority: 'normal', dueDate: '2025-03-01', hearingDate: null, attorneys: ['李律师'], progress: 15 },
    { id: 'C005', name: '小米外观设计无效', client: '小米科技', opponent: 'OPPO', track: 'patent_invalidation', status: '已结案', caseNo: '(2023)国知局第890号', court: '国知局', priority: 'low', dueDate: null, hearingDate: null, attorneys: ['王律师', '赵律师'], progress: 100 },
    { id: 'C006', name: '百度专利侵权', client: '百度在线', opponent: '字节跳动', track: 'civil_tort', status: '进行中', caseNo: '(2024)京01民初567号', court: '北京互联网法院', priority: 'high', dueDate: '2025-01-25', hearingDate: '2025-02-05', attorneys: ['张律师'], progress: 55 },
    { id: 'C007', name: '大疆专利许可纠纷', client: '大疆创新', opponent: '某无人机公司', track: 'civil_tort', status: '进行中', caseNo: '(2024)粤03民初234号', court: '深圳中级法院', priority: 'normal', dueDate: '2025-02-10', hearingDate: '2025-02-20', attorneys: ['李律师', '赵律师'], progress: 25 },
    { id: 'C008', name: '中芯国际商业秘密', client: '中芯国际', opponent: '台积电', track: 'civil_tort', status: '进行中', caseNo: '(2024)沪73民初45号', court: '上海知识产权法院', priority: 'urgent', dueDate: '2025-01-08', hearingDate: '2025-01-12', attorneys: ['王律师', '张律师'], progress: 80 },
  ],

  // ── 案件关系 ──────────────────────────────────────────
  relations: [
    { source: 'C001', target: 'C003', type: 'same_patent', label: '同领域专利' },
    { source: 'C001', target: 'C005', type: 'same_party', label: '同行业对手' },
    { source: 'C002', target: 'C006', type: 'same_party', label: '互联网客户群' },
    { source: 'C003', target: 'C007', type: 'cross_reference', label: '技术交叉' },
    { source: 'C004', target: 'C008', type: 'appeal_of', label: '审级关联' },
    { source: 'C006', target: 'C008', type: 'same_party', label: '同法院管辖' },
  ],

  // ── 任务 ──────────────────────────────────────────────
  tasks: [
    { id: 'T001', name: '准备隆基无效口审意见陈述', caseId: 'C001', priority: 'urgent_important', dueDate: '2025-01-18', status: 'next', waitingFor: null },
    { id: 'T002', name: '提交华为侵权案补充证据', caseId: 'C002', priority: 'urgent', dueDate: '2025-01-12', status: 'next', waitingFor: null },
    { id: 'T003', name: '跟进宁德时代检索报告', caseId: 'C003', priority: 'normal', dueDate: '2025-01-20', status: 'waiting', waitingFor: '专利代理师' },
    { id: 'T004', name: '审核腾讯行政诉讼答辩状', caseId: 'C004', priority: 'important', dueDate: '2025-01-25', status: 'next', waitingFor: null },
    { id: 'T005', name: '大疆案技术对比分析', caseId: 'C007', priority: 'normal', dueDate: '2025-02-05', status: 'next', waitingFor: null },
    { id: 'T006', name: '中芯国际案庭前调解方案', caseId: 'C008', priority: 'urgent', dueDate: '2025-01-10', status: 'next', waitingFor: null },
    { id: 'T007', name: '整理百度案技术领域文献', caseId: 'C006', priority: 'normal', dueDate: '2025-01-30', status: 'waiting', waitingFor: '检索机构' },
    { id: 'T008', name: '更新案件进度周报', caseId: null, priority: 'normal', dueDate: '2025-01-11', status: 'today', waitingFor: null },
    { id: 'T009', name: '回顾所有进行中案件进展', caseId: null, priority: 'low', dueDate: '2025-01-14', status: 'review', waitingFor: null },
    { id: 'T010', name: '准备下周客户汇报材料', caseId: null, priority: 'important', dueDate: '2025-01-13', status: 'today', waitingFor: null },
  ],

  // ── 日历事件 ──────────────────────────────────────────
  events: [
    { id: 'E001', title: '隆基无效口审', date: '2025-01-20', type: 'hearing', caseId: 'C001', time: '09:30' },
    { id: 'E002', title: '华为侵权案开庭', date: '2025-01-18', type: 'hearing', caseId: 'C002', time: '14:00' },
    { id: 'E003', title: '中芯国际调解', date: '2025-01-12', type: 'hearing', caseId: 'C008', time: '10:00' },
    { id: 'E004', title: '腾讯答辩状截止', date: '2025-01-25', type: 'deadline', caseId: 'C004', time: '23:59' },
    { id: 'E005', title: '客户周会', date: '2025-01-13', type: 'meeting', caseId: null, time: '15:00' },
    { id: 'E006', title: '内部案件讨论', date: '2025-01-15', type: 'meeting', caseId: null, time: '10:00' },
    { id: 'E007', title: '百度案证据提交', date: '2025-01-25', type: 'deadline', caseId: 'C006', time: '17:00' },
    { id: 'E008', title: '大疆案检索报告', date: '2025-02-05', type: 'deadline', caseId: 'C007', time: '18:00' },
  ],

  // ── 最近活动 ──────────────────────────────────────────
  activities: [
    { id: 'A001', type: 'log', summary: '隆基案提交了第三次意见陈述', caseId: 'C001', time: '2小时前' },
    { id: 'A002', type: 'task', summary: '华为案证据整理完成', caseId: 'C002', time: '4小时前' },
    { id: 'A003', type: 'hearing', summary: '中芯国际调解日期确认', caseId: 'C008', time: '昨天' },
    { id: 'A004', type: 'log', summary: '宁德时代案检索报告已收', caseId: 'C003', time: '昨天' },
    { id: 'A005', type: 'task', summary: '腾讯案答辩状初稿完成', caseId: 'C004', time: '2天前' },
  ],

  // ── 跟踪配置 ──────────────────────────────────────────
  trackMap: {
    patent_invalidation: { label: '专利无效', color: 'blue' },
    admin_litigation: { label: '行政诉讼', color: 'amber' },
    civil_tort: { label: '民事侵权', color: 'purple' },
    other: { label: '其他', color: 'gray' },
  },

  priorityMap: {
    urgent_important: { label: '紧急重要', tagClass: 'tag-red' },
    urgent: { label: '紧急', tagClass: 'tag-red' },
    important: { label: '重要', tagClass: 'tag-amber' },
    high: { label: '高', tagClass: 'tag-amber' },
    normal: { label: '普通', tagClass: 'tag-gray' },
    low: { label: '低', tagClass: 'tag-gray' },
  },

  statusMap: {
    '进行中': { color: 'blue' },
    '等待中': { color: 'amber' },
    '已结案': { color: 'green' },
  },
};
