// ═══════════════════════════════════════════════════════════
// Casy UI Upgrade — Main App (融合已有设计系统)
// ═══════════════════════════════════════════════════════════

const App = () => {
  const [currentPage, setCurrentPage] = React.useState('home');

  const pageTitles = {
    home: '今日', cases: '案件工作台', tasks: '任务工作台',
    calendar: '日历', dashboard: '数据看板', inbox: '收件箱',
    docs: '文书工坊', knowledge: '知识库', ai: 'AI 智伴', settings: '设置',
  };

  return (
    <div className="app-layout">
      <Sidebar currentPage={currentPage} onNavigate={setCurrentPage} />
      <div className="main-area">
        {/* Topbar */}
        <header className="topbar">
          <span className="topbar-title">{pageTitles[currentPage] || '今日'}</span>
          <div className="topbar-search">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3"/></svg>
            <input placeholder="搜索案件 / 任务 / 法条 / 客户…" />
            <span className="kbd">⌘K</span>
          </div>
          <div className="topbar-actions">
            <button className="btn" onClick={() => setCurrentPage('dashboard')}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round"><rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/></svg>
              数据看板
            </button>
            <button className="btn primary">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M12 5v14M5 12h14"/></svg>
              捕获
            </button>
          </div>
        </header>

        {/* Page Content */}
        <div className="page-scroll">
          {currentPage === 'home' && <HomePage />}
          {currentPage === 'dashboard' && <DashboardPage />}
          {currentPage === 'cases' && <CasesPage />}
          {currentPage === 'tasks' && <TasksPage />}
          {currentPage === 'calendar' && <CalendarPage />}
          {currentPage === 'ai' && <AICompanionPage />}
          {!['home','dashboard','cases','tasks','calendar','ai'].includes(currentPage) && (
            <div className="empty fade-in">
              <div style={{ fontSize: 32, opacity: .5 }}>📋</div>
              <div style={{ fontWeight: 600 }}>{pageTitles[currentPage]}</div>
              <div style={{ fontSize: 12 }}>此页面为设计预留</div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

// ── Home Page (今日焦点 · 融合已有设计) ─────────────────
const HomePage = () => {
  const { cases, tasks, events, activities } = CasyData;

  return (
    <div className="fade-in">
      {/* AI Banner */}
      <div style={{
        display: 'flex', alignItems: 'center', gap: 8, marginBottom: 14,
        background: 'var(--surface)', border: '1px solid var(--border)',
        borderRadius: 'var(--radius-lg)', padding: '9px 14px',
        fontSize: 12.5, color: 'var(--text-2)',
      }}>
        <span style={{ width: 7, height: 7, borderRadius: '50%', background: 'var(--green)' }} />
        <span><strong>智能推荐已就绪</strong> · 基于 {tasks.length} 条任务 + {cases.length} 个案件，给出今日建议</span>
      </div>

      <div className="focus-grid">
        {/* 左栏：今日秩序 */}
        <div className="vstack">
          {/* 硬性日程 */}
          <div className="card">
            <div className="card-header">
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round"><rect x="3" y="5" width="18" height="16" rx="2"/><path d="M8 3v4M16 3v4M3 10h18"/></svg>
              硬性日程 · 近期
            </div>
            {events.filter(e => e.type === 'hearing').map(ev => (
              <div key={ev.id} style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '8px 4px', borderBottom: '1px solid var(--divider)' }}>
                <span style={{ fontFamily: 'var(--mono)', fontSize: 12, color: 'var(--text-1)', width: 46, flexShrink: 0 }}>{ev.time}</span>
                <span className="circle red" />
                <div style={{ flex: 1 }}>
                  <div style={{ fontSize: 13 }}><strong>{ev.type === 'hearing' ? '口审/开庭' : '期限'}</strong> · {ev.title}</div>
                </div>
                <span className="tag purple">{ev.caseId}</span>
              </div>
            ))}
          </div>

          {/* 今日到期 */}
          <div className="card">
            <div className="card-header">
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round"><path d="M9 6h11M9 12h11M9 18h11"/><path d="m3.5 6 1 1 2-2M3.5 12l1 1 2-2M3.5 18l1 1 2-2"/></svg>
              今日到期 · {tasks.filter(t => t.status === 'today').length}
            </div>
            {tasks.filter(t => t.status === 'today').map(task => (
              <div key={task.id} className="row">
                <span className="check" />
                <span style={{ flex: 1 }}>{task.name}</span>
                <span style={{ fontSize: 11, color: 'var(--text-3)' }}>{task.caseId}</span>
              </div>
            ))}
          </div>

          {/* 等待跟进 */}
          <div className="card">
            <div className="card-header">
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round"><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 3"/></svg>
              等待跟进 · {tasks.filter(t => t.status === 'waiting').length} 条
            </div>
            {tasks.filter(t => t.status === 'waiting').map(task => (
              <div key={task.id} className="row">
                <span className="circle amber" />
                <span style={{ flex: 1 }}>等 {task.waitingFor} · {task.name}</span>
                <span className="tag amber">已等 6 天</span>
              </div>
            ))}
          </div>
        </div>

        {/* 右栏：推荐 + 统计 */}
        <div className="vstack">
          {/* 智能推荐 */}
          <div className="card">
            <div className="card-header">
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round"><path d="M12 3l2.4 5.2 5.6.8-4 4 1 5.8-5-2.7-5 2.7 1-5.8-4-4 5.6-.8z"/></svg>
              智能推荐 · 今日 3 件事
            </div>
            {tasks.filter(t => t.status === 'next').slice(0, 3).map((task, i) => (
              <div key={task.id} style={{
                display: 'flex', gap: 10, alignItems: 'flex-start',
                border: '1px solid var(--border)', borderRadius: 'var(--radius-sm)',
                padding: 10, marginBottom: 8,
              }}>
                <span style={{
                  width: 20, height: 20, borderRadius: 'var(--radius-sm)',
                  background: 'var(--primary-soft)', color: 'var(--primary)',
                  display: 'grid', placeItems: 'center',
                  fontSize: 11, fontWeight: 700, flexShrink: 0,
                }}>{i + 1}</span>
                <div style={{ flex: 1 }}>
                  <div style={{ fontSize: 12.5, fontWeight: 500 }}>{task.name}</div>
                  <div style={{ fontSize: 11, color: 'var(--text-3)' }}>{task.caseId} · {task.dueDate}</div>
                </div>
                <button className="btn sm primary">确认</button>
              </div>
            ))}
          </div>

          {/* 案件分布 */}
          <div className="card">
            <div className="card-header">
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round"><rect x="3" y="4" width="18" height="16" rx="2"/><path d="M8 9h8M8 13h5"/></svg>
              案件分布 · 按轨道
              <span className="sub">共 {cases.length} 个</span>
            </div>
            {[
              { label: '专利无效', color: 'var(--purple)', count: cases.filter(c => c.track === 'patent_invalidation').length },
              { label: '民事侵权', color: 'var(--primary)', count: cases.filter(c => c.track === 'civil_tort').length },
              { label: '行政诉讼', color: 'var(--amber)', count: cases.filter(c => c.track === 'admin_litigation').length },
            ].map(item => (
              <div key={item.label} style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '7px 0', fontSize: 13 }}>
                <span style={{ width: 84, color: 'var(--text-2)', display: 'flex', alignItems: 'center', gap: 6 }}>
                  <span className="track-dot" style={{ background: item.color }} />{item.label}
                </span>
                <span style={{ flex: 1, height: 7, borderRadius: 4, background: 'var(--surface-hover)', overflow: 'hidden' }}>
                  <span style={{ display: 'block', height: '100%', borderRadius: 4, background: item.color, width: `${(item.count / cases.length) * 100}%` }} />
                </span>
                <span style={{ width: 40, textAlign: 'right', fontFamily: 'var(--mono)', color: 'var(--text-2)', fontSize: 12 }}>{item.count}</span>
              </div>
            ))}
          </div>

          {/* 需回顾 */}
          <div className="card">
            <div className="card-header">
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round"><path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5"/></svg>
              需要回顾 · {tasks.filter(t => t.status === 'review').length}
            </div>
            {tasks.filter(t => t.status === 'review').map(task => (
              <div key={task.id} className="row" style={{ cursor: 'pointer' }}>
                <span className="circle purple" />
                <span style={{ flex: 1 }}>{task.name}</span>
                <span className="tag amber">待回顾</span>
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* 底部统计 */}
      <div className="statline" style={{ marginTop: 14 }}>
        <div className="st"><span className="v">{cases.length}</span><span className="k">活跃案件</span></div>
        <div className="st"><span className="v">{tasks.filter(t => t.status === 'waiting').length}</span><span className="k">等待中</span></div>
        <div className="st"><span className="v">{cases.filter(c => c.status === '已结案').length}</span><span className="k">已结案</span></div>
        <div className="st"><span className="v" style={{ color: 'var(--red)' }}>{tasks.filter(t => t.dueDate && t.dueDate < '2025-01-11').length}</span><span className="k">逾期</span></div>
      </div>
    </div>
  );
};

// ── Mount ───────────────────────────────────────────────
const root = ReactDOM.createRoot(document.getElementById('root'));
root.render(<App />);
