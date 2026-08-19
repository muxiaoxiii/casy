// ═══════════════════════════════════════════════════════════
// Casy UI Upgrade — 日历（融合优效日历 + OmniFocus Forecast）
// 特性：周视图时间块、月视图、Forecast 聚合、多日历叠加
// ═══════════════════════════════════════════════════════════

const CalendarPage = () => {
  const { events, tasks } = CasyData;
  const [viewMode, setViewMode] = React.useState('week');
  const [selectedDate, setSelectedDate] = React.useState(14); // 1月14日

  // 时间轴 7:00 - 22:00
  const hours = Array.from({ length: 16 }, (_, i) => i + 7);

  // 本周日期 (1月13日周一 ~ 1月19日周日)
  const weekDays = [
    { day: 13, label: '一', isToday: false },
    { day: 14, label: '二', isToday: true },
    { day: 15, label: '三', isToday: false },
    { day: 16, label: '四', isToday: false },
    { day: 17, label: '五', isToday: false },
    { day: 18, label: '六', isToday: false },
    { day: 19, label: '日', isToday: false },
  ];

  // 事件映射到日期
  function getEventsForDay(d) {
    const dateStr = `2025-01-${String(d).padStart(2, '0')}`;
    return events.filter(e => e.date === dateStr);
  }

  // 任务映射到日期（有 dueDate 的任务）
  function getTasksForDay(d) {
    const dateStr = `2025-01-${String(d).padStart(2, '0')}`;
    return tasks.filter(t => t.dueDate === dateStr);
  }

  const eventColorMap = {
    hearing: 'var(--red)',
    deadline: 'var(--amber)',
    meeting: 'var(--purple)',
    task: 'var(--primary)',
  };

  const typeLabelMap = {
    hearing: '开庭/口审',
    deadline: '期限',
    meeting: '会议',
    task: '任务',
  };

  // 月视图网格
  const year = 2025, month = 0;
  const firstDay = new Date(year, month, 1).getDay() || 7; // 周一=1
  const daysInMonth = 31;
  const calCells = [];
  for (let i = 1; i < firstDay; i++) calCells.push(null);
  for (let d = 1; d <= daysInMonth; d++) calCells.push(d);
  while (calCells.length % 7 !== 0) calCells.push(null);

  return (
    <div className="fade-in">
      {/* 顶栏：视图切换 + 日期导航 + 筛选 */}
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 14 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <button className="btn sm" style={{ width: 28, height: 28, padding: 0, display: 'grid', placeItems: 'center' }}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="m15 18-6-6 6-6"/></svg>
          </button>
          <span style={{ fontSize: 15, fontWeight: 700 }}>2025 年 1 月</span>
          <button className="btn sm" style={{ width: 28, height: 28, padding: 0, display: 'grid', placeItems: 'center' }}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="m9 18 6-6-6-6"/></svg>
          </button>
          <button className="btn sm" onClick={() => setSelectedDate(14)}>今天</button>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <div className="chips">
            {[
              { key: 'day', label: '日' },
              { key: 'week', label: '周' },
              { key: 'month', label: '月' },
              { key: 'forecast', label: 'Forecast' },
            ].map(v => (
              <span key={v.key} className={`chip ${viewMode === v.key ? 'active' : ''}`}
                onClick={() => setViewMode(v.key)}>{v.label}</span>
            ))}
          </div>
          <button className="btn sm" onClick={() => {}}>+ 新日程</button>
        </div>
      </div>

      {/* ── 周视图（优效日历风格：时间块 + 时间轴）────────── */}
      {viewMode === 'week' && (
        <div style={{ display: 'grid', gridTemplateColumns: '48px 1fr', gap: 0, background: 'var(--surface)', border: '1px solid var(--border)', borderRadius: 'var(--radius-lg)', overflow: 'hidden' }}>
          {/* 星期头 */}
          <div style={{ background: 'var(--bg)' }} />
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(7, 1fr)', borderBottom: '1px solid var(--border)' }}>
            {weekDays.map(d => (
              <div key={d.day} style={{
                padding: '8px 0', textAlign: 'center', fontSize: 12,
                color: d.isToday ? 'var(--primary)' : 'var(--text-3)',
                fontWeight: d.isToday ? 700 : 500,
                borderRight: '1px solid var(--divider)',
                background: d.isToday ? 'var(--primary-soft)' : 'transparent',
              }}>
                <div style={{ fontSize: 10, letterSpacing: '.5px' }}>周{d.label}</div>
                <div style={{
                  width: 24, height: 24, borderRadius: '50%', margin: '2px auto 0',
                  display: 'grid', placeItems: 'center', fontSize: 13, fontWeight: 700,
                  background: d.isToday ? 'var(--primary)' : 'transparent',
                  color: d.isToday ? '#fff' : 'var(--text-1)',
                }}>{d.day}</div>
              </div>
            ))}
          </div>

          {/* 时间轴 + 网格 */}
          <div style={{ display: 'flex', flexDirection: 'column' }}>
            {hours.map(h => (
              <div key={h} style={{ display: 'flex', minHeight: 52, borderBottom: '1px solid var(--divider)' }}>
                <div style={{
                  width: 48, flexShrink: 0, padding: '0 6px', fontSize: 10,
                  color: 'var(--text-3)', fontFamily: 'var(--mono)',
                  textAlign: 'right', paddingTop: 2,
                }}>{`${String(h).padStart(2, '0')}:00`}</div>
                <div style={{ flex: 1, display: 'grid', gridTemplateColumns: 'repeat(7, 1fr)' }}>
                  {weekDays.map(d => {
                    const dayEvents = getEventsForDay(d.day);
                    const hourEvents = dayEvents.filter(e => {
                      const eHour = parseInt(e.time?.split(':')[0] || '0');
                      return eHour === h;
                    });
                    return (
                      <div key={d.day} style={{
                        borderRight: '1px solid var(--divider)',
                        padding: '1px 2px',
                        position: 'relative',
                        background: d.isToday ? 'rgba(62,92,154,0.02)' : 'transparent',
                        cursor: 'pointer',
                      }}
                        onMouseEnter={e => e.currentTarget.style.background = 'var(--surface-hover)'}
                        onMouseLeave={e => e.currentTarget.style.background = d.isToday ? 'rgba(62,92,154,0.02)' : 'transparent'}
                      >
                        {hourEvents.map(ev => (
                          <div key={ev.id} style={{
                            fontSize: 10, padding: '2px 4px', borderRadius: 3,
                            background: eventColorMap[ev.type] + '18',
                            color: eventColorMap[ev.type],
                            fontWeight: 500, borderLeft: `2px solid ${eventColorMap[ev.type]}`,
                            whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis',
                            marginBottom: 1,
                          }}>
                            {ev.title}
                          </div>
                        ))}
                      </div>
                    );
                  })}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* ── 月视图 ──────────────────────────────────────── */}
      {viewMode === 'month' && (
        <div className="card">
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(7, 1fr)', gap: 3 }}>
            {['一', '二', '三', '四', '五', '六', '日'].map(d => (
              <div key={d} style={{ fontSize: 11, color: 'var(--text-3)', textAlign: 'center', padding: '4px 0', fontWeight: 600 }}>{d}</div>
            ))}
            {calCells.map((d, i) => {
              if (!d) return <div key={i} />;
              const dayEvents = getEventsForDay(d);
              const dayTasks = getTasksForDay(d);
              const isToday = d === 14;
              const isSelected = d === selectedDate;
              const hasRed = dayEvents.some(e => e.type === 'hearing');
              return (
                <div key={i}
                  className={`cal-cell ${isToday ? 'today' : ''} ${isSelected ? 'selected' : ''} ${hasRed ? 'danger' : ''}`}
                  onClick={() => setSelectedDate(d)}
                >
                  <span className={`cal-num ${isToday ? 'today-num' : ''}`}>{d}</span>
                  <div style={{ display: 'flex', gap: 2, flexWrap: 'wrap', marginTop: 2 }}>
                    {dayEvents.slice(0, 3).map(ev => (
                      <span key={ev.id} style={{ width: 6, height: 6, borderRadius: '50%', background: eventColorMap[ev.type] }} />
                    ))}
                    {dayTasks.slice(0, 2).map(t => (
                      <span key={t.id} style={{ width: 6, height: 6, borderRadius: '50%', background: 'var(--primary)' }} />
                    ))}
                  </div>
                </div>
              );
            })}
          </div>
          <div style={{ display: 'flex', gap: 14, marginTop: 10, fontSize: 11, color: 'var(--text-2)', alignItems: 'center' }}>
            <span style={{ display: 'flex', alignItems: 'center', gap: 5 }}><span style={{ width: 8, height: 8, borderRadius: '50%', background: 'var(--red)' }} />开庭/口审</span>
            <span style={{ display: 'flex', alignItems: 'center', gap: 5 }}><span style={{ width: 8, height: 8, borderRadius: '50%', background: 'var(--amber)' }} />期限</span>
            <span style={{ display: 'flex', alignItems: 'center', gap: 5 }}><span style={{ width: 8, height: 8, borderRadius: '50%', background: 'var(--purple)' }} />会议</span>
            <span style={{ display: 'flex', alignItems: 'center', gap: 5 }}><span style={{ width: 8, height: 8, borderRadius: '50%', background: 'var(--primary)' }} />任务到期</span>
          </div>
        </div>
      )}

      {/* ── Forecast 视图（OmniFocus 风格：日历+任务聚合）── */}
      {viewMode === 'forecast' && (
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 320px', gap: 14, alignItems: 'start' }}>
          <div className="vstack">
            {/* 未来 7 天 Forecast */}
            {[14, 15, 16, 17, 18, 19, 20].map(d => {
              const dayEvents = getEventsForDay(d);
              const dayTasks = getTasksForDay(d);
              const isToday = d === 14;
              const date = new Date(2025, 0, d);
              const dayNames = ['周日', '周一', '周二', '周三', '周四', '周五', '周六'];
              return (
                <div key={d} className="card" style={{ borderLeft: isToday ? '3px solid var(--primary)' : undefined }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: dayEvents.length + dayTasks.length > 0 ? 10 : 0 }}>
                    <span style={{
                      fontSize: 13, fontWeight: isToday ? 700 : 500,
                      color: isToday ? 'var(--primary)' : 'var(--text-1)',
                    }}>
                      {isToday ? '今天' : `${d}日`} · {dayNames[date.getDay()]}
                    </span>
                    {dayEvents.length > 0 && <span className="tag red">{dayEvents.length} 场</span>}
                    {dayTasks.length > 0 && <span className="tag blue">{dayTasks.length} 项到期</span>}
                  </div>
                  {dayEvents.map(ev => (
                    <div key={ev.id} style={{
                      display: 'flex', alignItems: 'center', gap: 10,
                      padding: '6px 0', borderBottom: '1px solid var(--divider)',
                    }}>
                      <span style={{
                        width: 4, height: 28, borderRadius: 2,
                        background: eventColorMap[ev.type], flexShrink: 0,
                      }} />
                      <span style={{ fontFamily: 'var(--mono)', fontSize: 12, color: 'var(--text-2)', width: 40 }}>{ev.time}</span>
                      <span style={{ flex: 1, fontSize: 13 }}>{ev.title}</span>
                      <span className="tag" style={{
                        background: eventColorMap[ev.type] + '15',
                        color: eventColorMap[ev.type],
                      }}>{typeLabelMap[ev.type]}</span>
                    </div>
                  ))}
                  {dayTasks.map(task => (
                    <div key={task.id} style={{
                      display: 'flex', alignItems: 'center', gap: 10,
                      padding: '6px 0',
                    }}>
                      <span className="check" />
                      <span style={{ flex: 1, fontSize: 13 }}>{task.name}</span>
                      <span className="tag amber">到期</span>
                    </div>
                  ))}
                  {dayEvents.length + dayTasks.length === 0 && (
                    <div style={{ fontSize: 12, color: 'var(--text-3)', padding: '4px 0' }}>暂无安排</div>
                  )}
                </div>
              );
            })}
          </div>

          {/* 右侧：月历小日历 + 图例 */}
          <div className="vstack">
            <div className="card">
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 8 }}>
                <span style={{ fontSize: 12, fontWeight: 700 }}>1 月</span>
              </div>
              <div style={{ display: 'grid', gridTemplateColumns: 'repeat(7, 1fr)', gap: 2 }}>
                {['一','二','三','四','五','六','日'].map(d => (
                  <div key={d} style={{ fontSize: 9, color: 'var(--text-3)', textAlign: 'center', padding: '2px 0' }}>{d}</div>
                ))}
                {calCells.slice(0, 35).map((d, i) => (
                  <div key={i} style={{
                    width: 28, height: 24, borderRadius: 4, display: 'grid', placeItems: 'center',
                    fontSize: 10, cursor: d ? 'pointer' : 'default',
                    background: d === 14 ? 'var(--primary)' : 'transparent',
                    color: d === 14 ? '#fff' : d ? 'var(--text-2)' : 'transparent',
                    fontWeight: d === 14 ? 700 : 400,
                  }} onClick={() => d && setSelectedDate(d)}>{d}</div>
                ))}
              </div>
            </div>

            <div className="card">
              <div className="card-header">日历图层</div>
              {[
                { label: '庭审日历', color: 'var(--red)', on: true },
                { label: '期限日历', color: 'var(--amber)', on: true },
                { label: '内部会议', color: 'var(--purple)', on: true },
                { label: '任务到期', color: 'var(--primary)', on: true },
                { label: '个人日历', color: 'var(--green)', on: false },
              ].map(cal => (
                <div key={cal.label} style={{
                  display: 'flex', alignItems: 'center', gap: 8,
                  padding: '5px 0', fontSize: 12, color: 'var(--text-2)',
                }}>
                  <span style={{
                    width: 12, height: 12, borderRadius: 3,
                    background: cal.on ? cal.color : 'var(--border)',
                    opacity: cal.on ? 1 : 0.4,
                  }} />
                  <span style={{ flex: 1, opacity: cal.on ? 1 : 0.5 }}>{cal.label}</span>
                  <span style={{
                    width: 28, height: 16, borderRadius: 8, position: 'relative',
                    background: cal.on ? cal.color : 'var(--border-strong)',
                    transition: 'background var(--transition)',
                    cursor: 'pointer',
                  }}>
                    <span style={{
                      position: 'absolute', top: 2,
                      left: cal.on ? 14 : 2,
                      width: 12, height: 12, borderRadius: '50%', background: '#fff',
                      transition: 'left var(--transition)',
                      boxShadow: '0 1px 2px rgba(0,0,0,.15)',
                    }} />
                  </span>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {/* ── 日视图（选中日期的详细时间块）────────────────── */}
      {viewMode === 'day' && (
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 300px', gap: 14, alignItems: 'start' }}>
          <div className="card">
            <div className="card-header">
              <span>1 月 {selectedDate} 日 · 周{['日','一','二','三','四','五','六'][new Date(2025,0,selectedDate).getDay()]}</span>
              <span className="sub">时间块 · 拖拽排程</span>
            </div>
            {hours.map(h => {
              const hourEvents = getEventsForDay(selectedDate).filter(e => parseInt(e.time?.split(':')[0]) === h);
              const hourTasks = getTasksForDay(selectedDate);
              return (
                <div key={h} style={{
                  display: 'flex', minHeight: 48, borderBottom: '1px solid var(--divider)',
                  alignItems: 'flex-start',
                }}>
                  <span style={{
                    width: 44, flexShrink: 0, fontSize: 11,
                    color: 'var(--text-3)', fontFamily: 'var(--mono)',
                    padding: '4px 8px 0 0', textAlign: 'right',
                  }}>{`${String(h).padStart(2,'0')}:00`}</span>
                  <div style={{ flex: 1, minHeight: 48, padding: '2px 4px' }}>
                    {hourEvents.map(ev => (
                      <div key={ev.id} style={{
                        padding: '6px 10px', borderRadius: 'var(--radius-sm)',
                        borderLeft: `3px solid ${eventColorMap[ev.type]}`,
                        background: eventColorMap[ev.type] + '10',
                        marginBottom: 4, fontSize: 13, cursor: 'pointer',
                      }}>
                        <div style={{ fontWeight: 500 }}>{ev.title}</div>
                        <div style={{ fontSize: 11, color: 'var(--text-3)' }}>{ev.time} · {ev.caseId || '—'}</div>
                      </div>
                    ))}
                  </div>
                </div>
              );
            })}
          </div>

          {/* 右侧：当日议程 */}
          <div className="vstack">
            <div className="card">
              <div className="card-header">当日议程</div>
              {getEventsForDay(selectedDate).length > 0 ? getEventsForDay(selectedDate).map(ev => (
                <div key={ev.id} style={{
                  display: 'flex', alignItems: 'center', gap: 10,
                  padding: '8px 0', borderBottom: '1px solid var(--divider)',
                }}>
                  <span style={{
                    width: 4, height: 28, borderRadius: 2,
                    background: eventColorMap[ev.type],
                  }} />
                  <div>
                    <div style={{ fontSize: 13, fontWeight: 500 }}>{ev.title}</div>
                    <div style={{ fontSize: 11, color: 'var(--text-3)' }}>{ev.time} · {typeLabelMap[ev.type]}</div>
                  </div>
                </div>
              )) : (
                <div style={{ fontSize: 12, color: 'var(--text-3)', padding: '12px 0', textAlign: 'center' }}>暂无安排</div>
              )}
            </div>
            <div className="card">
              <div className="card-header">当日到期任务</div>
              {getTasksForDay(selectedDate).map(task => (
                <div key={task.id} className="row">
                  <span className="check" />
                  <span style={{ flex: 1 }}>{task.name}</span>
                </div>
              ))}
              {getTasksForDay(selectedDate).length === 0 && (
                <div style={{ fontSize: 12, color: 'var(--text-3)', padding: '12px 0', textAlign: 'center' }}>无到期任务</div>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

window.CalendarPage = CalendarPage;
