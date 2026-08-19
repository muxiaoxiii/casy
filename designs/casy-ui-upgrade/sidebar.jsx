// ═══════════════════════════════════════════════════════════
// Casy UI Upgrade — 浅色侧栏（融合已有设计 + 可视化入口）
// ═══════════════════════════════════════════════════════════

const Sidebar = ({ currentPage, onNavigate }) => {
  const navGroups = [
    { label: '工作台', items: [
      { key: 'home', label: '今日', icon: 'home' },
    ]},
    { label: '核心', items: [
      { key: 'tasks', label: '任务工作台', icon: 'tasks', badge: 7, badgeColor: 'amber' },
      { key: 'cases', label: '案件工作台', icon: 'cases' },
      { key: 'calendar', label: '日历', icon: 'calendar' },
      { key: 'dashboard', label: '数据看板', icon: 'chart', badge: '新', badgeColor: 'blue' },
    ]},
    { label: '知识', items: [
      { key: 'inbox', label: '收件箱', icon: 'inbox', badge: 4, badgeColor: 'red' },
      { key: 'docs', label: '文书工坊', icon: 'docs' },
      { key: 'knowledge', label: '知识库', icon: 'knowledge' },
      { key: 'ai', label: 'AI 智伴', icon: 'ai' },
    ]},
  ];

  const iconMap = {
    home: <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4"><circle cx="8" cy="8" r="6.2"/><path d="M8 4.8v3.4l2.2 1.4"/></svg>,
    tasks: <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4"><rect x="2.5" y="3.5" width="11" height="9" rx="2"/><path d="M5.5 6.5h5M5.5 9h3.5"/></svg>,
    cases: <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4"><path d="M2.5 4.5a2 2 0 0 1 2-2h2.6a1 1 0 0 1 .8.4l.9 1.2h4.7a2 2 0 0 1 2 2v6a2 2 0 0 1-2 2h-9a2 2 0 0 1-2-2v-8z"/></svg>,
    calendar: <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4"><rect x="2.5" y="3" width="11" height="10" rx="1.5"/><path d="M2.5 6.5h11M5.5 2v2.5M10.5 2v2.5"/></svg>,
    chart: <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4"><rect x="2" y="9" width="3" height="5" rx=".5"/><rect x="6.5" y="5" width="3" height="9" rx=".5"/><rect x="11" y="2" width="3" height="12" rx=".5"/></svg>,
    inbox: <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4"><path d="M2.5 4a2 2 0 0 1 2-2h9v11h-9a2 2 0 0 0-2 2V4z"/><path d="M13.5 13v1.5h-8A1.5 1.5 0 0 1 6.5 13"/></svg>,
    docs: <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4"><path d="M4 1.5h5.5L13 5v9.5H4z"/><path d="M9.5 1.5V5H13"/></svg>,
    knowledge: <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4"><path d="M3 2.5h6a2 2 0 0 1 2 2V13a1.5 1.5 0 0 0-1.5-1.5H3V2.5z"/><path d="M11 4.5h2a1 1 0 0 1 1 1V13a1.5 1.5 0 0 0-1.5-1.5H11"/></svg>,
    ai: <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4"><path d="M8 2l1.2 2.6 2.8.4-2 2 .5 2.8L8 8.6 5.5 9.8l.5-2.8-2-2 2.8-.4z"/></svg>,
  };

  return (
    <aside style={{
      width: 'var(--sidebar-w)',
      background: 'var(--surface)',
      borderRight: '1px solid var(--border)',
      display: 'flex',
      flexDirection: 'column',
      flexShrink: 0,
      padding: '12px 10px',
      gap: 4,
    }}>
      {/* Logo */}
      <div style={{
        display: 'flex', alignItems: 'center', gap: 9,
        padding: '8px 10px 16px',
        fontWeight: 700, fontSize: 15, letterSpacing: '.2px',
      }}>
        <span style={{
          width: 26, height: 26, borderRadius: 7,
          background: 'var(--primary)',
          display: 'grid', placeItems: 'center', color: '#fff',
          fontSize: 14, fontWeight: 700, flexShrink: 0,
        }}>C</span>
        Casy
      </div>

      {/* Nav Groups */}
      <nav style={{ flex: 1, overflowY: 'auto' }}>
        {navGroups.map(group => (
          <React.Fragment key={group.label}>
            <div style={{
              fontSize: 11, color: 'var(--text-3)', textTransform: 'uppercase',
              letterSpacing: '.8px', padding: '12px 10px 5px', fontWeight: 600,
            }}>{group.label}</div>
            {group.items.map(item => {
              const isActive = currentPage === item.key;
              return (
                <button
                  key={item.key}
                  onClick={() => onNavigate(item.key)}
                  style={{
                    display: 'flex', alignItems: 'center', gap: 10,
                    padding: '7px 10px', borderRadius: 'var(--radius-md)',
                    color: isActive ? 'var(--primary)' : 'var(--text-2)',
                    fontSize: 13, lineHeight: 1.4,
                    position: 'relative', width: '100%', textAlign: 'left',
                    background: isActive ? 'var(--primary-soft)' : 'transparent',
                    fontWeight: isActive ? 600 : 400,
                    transition: 'all var(--transition)',
                    border: isActive ? '1px solid var(--primary-border)' : '1px solid transparent',
                  }}
                  onMouseEnter={e => { if (!isActive) e.currentTarget.style.background = 'var(--surface-hover)'; }}
                  onMouseLeave={e => { if (!isActive) e.currentTarget.style.background = 'transparent'; }}
                >
                  {isActive && (
                    <div style={{
                      position: 'absolute', left: -10, top: 6, bottom: 6,
                      width: 3, borderRadius: '0 3px 3px 0',
                      background: 'var(--primary)',
                    }} />
                  )}
                  <span style={{ width: 15, height: 15, flexShrink: 0, display: 'flex', alignItems: 'center', justifyContent: 'center', opacity: .85 }}>
                    {iconMap[item.icon]}
                  </span>
                  {item.label}
                  {item.badge && (
                    <span style={{
                      marginLeft: 'auto',
                      background: item.badgeColor === 'red' ? 'var(--red)' : item.badgeColor === 'amber' ? 'var(--amber)' : 'var(--primary)',
                      color: '#fff', fontSize: typeof item.badge === 'number' ? 11 : 9,
                      minWidth: 18, height: 18, borderRadius: 9,
                      display: 'grid', placeItems: 'center',
                      padding: '0 5px', fontWeight: 600,
                    }}>{item.badge}</span>
                  )}
                </button>
              );
            })}
          </React.Fragment>
        ))}
      </nav>

      {/* AI Status */}
      <div style={{
        display: 'flex', alignItems: 'center', gap: 8,
        padding: '8px 10px', borderRadius: 'var(--radius-md)',
        background: 'var(--bg)', fontSize: 12, color: 'var(--text-2)',
        marginTop: 6,
      }}>
        <span style={{ width: 7, height: 7, borderRadius: '50%', background: 'var(--green)' }} />
        AI 智伴 · 本地模型可用
      </div>

      {/* User */}
      <div style={{
        display: 'flex', alignItems: 'center', gap: 9,
        padding: '8px 10px', borderTop: '1px solid var(--border)', marginTop: 4,
      }}>
        <span style={{
          width: 26, height: 26, borderRadius: '50%',
          background: 'var(--primary-soft)', color: 'var(--primary)',
          display: 'grid', placeItems: 'center', fontSize: 12, fontWeight: 700,
        }}>张</span>
        <div style={{ minWidth: 0 }}>
          <div style={{ fontSize: 12.5, fontWeight: 600 }}>张律师</div>
          <div style={{ fontSize: 11, color: 'var(--text-3)' }}>专利诉讼 · 无效程序</div>
        </div>
      </div>
    </aside>
  );
};

window.Sidebar = Sidebar;
