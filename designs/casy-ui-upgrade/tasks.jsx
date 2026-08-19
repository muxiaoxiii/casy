// ═══════════════════════════════════════════════════════════
// Casy UI Upgrade — 任务工作台（OmniFocus GTD + Things 3 风格）
// 特性：7 透视、时间双轨、上下文标签、顺序项目、回顾机制
// ═══════════════════════════════════════════════════════════

const TasksPage = () => {
  const [activePerspective, setActivePerspective] = React.useState('inbox');
  const [selectedTask, setSelectedTask] = React.useState(null);
  const { tasks, priorityMap } = CasyData;

  const perspectives = [
    { key: 'inbox', label: '收件箱', icon: '📥', desc: '先捕获，稍后厘清归属', count: 3 },
    { key: 'today', label: '今天', icon: '📅', desc: '今日聚焦 · 拖拽排优先级', count: 2 },
    { key: 'upcoming', label: '计划中', icon: '📆', desc: '有 When 日期的任务', count: 4 },
    { key: 'anytime', label: '随时', icon: '⚡', desc: '按上下文分组 · 无截止日', count: 3 },
    { key: 'waiting', label: '等待', icon: '⏳', desc: '委派出去 · 追踪跟进', count: 2 },
    { key: 'review', label: '回顾', icon: '🔄', desc: 'GTD Reflect · 定期审视', count: 1 },
    { key: 'someday', label: '某天', icon: '🌙', desc: '不承诺时间 · 灵感池', count: 1 },
  ];

  const currentPersp = perspectives.find(p => p.key === activePerspective);

  // 上下文标签
  const contextMap = {
    office: { label: '@办公室', color: 'var(--primary)', bg: 'var(--blue-soft)' },
    phone: { label: '@电话', color: 'var(--green)', bg: 'var(--green-soft)' },
    court: { label: '@法院', color: 'var(--purple)', bg: 'var(--purple-soft)' },
  };

  // 过滤任务
  const filteredTasks = tasks.filter(t => t.status === activePerspective);

  // 按上下文分组（anytime 透视）
  const groupedByContext = {};
  filteredTasks.forEach(t => {
    const ctx = t.context || 'other';
    if (!groupedByContext[ctx]) groupedByContext[ctx] = [];
    groupedByContext[ctx].push(t);
  });

  // 按日期分组（upcoming 透视）
  const groupedByDate = {};
  tasks.filter(t => t.status === 'next' && t.dueDate).forEach(t => {
    if (!groupedByDate[t.dueDate]) groupedByDate[t.dueDate] = [];
    groupedByDate[t.dueDate].push(t);
  });

  return (
    <div className="fade-in">
      {/* 透视切换（Things 3 风格：药丸标签 + 描述） */}
      <div style={{ marginBottom: 12 }}>
        <div className="chips" style={{ marginBottom: 6 }}>
          {perspectives.map(p => (
            <span key={p.key}
              className={`chip ${activePerspective === p.key ? 'active' : ''}`}
              onClick={() => { setActivePerspective(p.key); setSelectedTask(null); }}
            >
              {p.label} · {p.count}
            </span>
          ))}
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 12, color: 'var(--text-3)', padding: '0 4px' }}>
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round"><path d="M13 2 3 14h9l-1 8 10-12h-9l1-8z"/></svg>
          {currentPersp.desc}
        </div>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 340px', gap: 14, alignItems: 'start' }}>
        {/* ── 左栏：任务列表 ─────────────────────────── */}
        <div className="vstack">
          {/* 收件箱透视 */}
          {activePerspective === 'inbox' && (
            <div className="card">
              <div className="card-header">
                <span>待厘清</span>
                <span className="sub">{filteredTasks.length} 项</span>
                <button className="btn sm ghost" style={{ marginLeft: 'auto' }}>全部厘清</button>
              </div>
              {/* 高亮第一条 */}
              {filteredTasks.length > 0 && (
                <div style={{
                  border: '1px solid var(--primary-border)', background: 'var(--primary-soft)',
                  borderRadius: 'var(--radius-sm)', padding: 10,
                  display: 'flex', gap: 10, alignItems: 'flex-start', marginBottom: 8,
                }}>
                  <span className="check" />
                  <div style={{ flex: 1 }}>
                    <span style={{ fontWeight: 500 }}>{filteredTasks[0].name}</span>
                    <div style={{ display: 'flex', gap: 6, alignItems: 'center', fontSize: 11, color: 'var(--text-3)', marginTop: 3 }}>
                      <span className="tag gray">待判定</span>
                      <span>今天 09:12</span>
                      <span className="tag outline">电话</span>
                    </div>
                  </div>
                </div>
              )}
              {filteredTasks.slice(1).map(task => (
                <div key={task.id} className="task-card" onClick={() => setSelectedTask(task)}>
                  <span className="check" />
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div className="title">{task.name}</div>
                    <div className="meta">
                      <span className="tag gray">待判定</span>
                      <span>{task.dueDate}</span>
                      {task.caseId && <span className="tag outline">{task.caseId}</span>}
                    </div>
                  </div>
                </div>
              ))}
              <div style={{
                display: 'flex', alignItems: 'center', gap: 8,
                border: '1px dashed var(--border-strong)', borderRadius: 'var(--radius-md)',
                padding: '8px 12px', marginTop: 12, color: 'var(--text-3)', fontSize: 13,
                cursor: 'text',
              }}>
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M12 20h9M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4z"/></svg>
                快速捕获：先记下来，稍后厘清（⌘I）
              </div>
            </div>
          )}

          {/* 今天透视（Things 3 风格：编号 + 拖拽） */}
          {activePerspective === 'today' && (
            <div className="card">
              <div className="card-header">
                <span>1月14日 · 今天</span>
                <span className="sub">{filteredTasks.length} 项 · 拖拽重排</span>
              </div>
              {filteredTasks.map((task, i) => {
                const isOverdue = task.dueDate && task.dueDate < '2025-01-11';
                return (
                  <div key={task.id} className="task-card" onClick={() => setSelectedTask(task)}
                    style={{ borderLeft: isOverdue ? '3px solid var(--red)' : undefined }}
                  >
                    <span style={{
                      width: 18, height: 18, borderRadius: 5,
                      background: 'var(--surface-hover)', color: 'var(--text-3)',
                      display: 'grid', placeItems: 'center', fontSize: 10, flexShrink: 0,
                      fontFamily: 'var(--mono)',
                    }}>{i + 1}</span>
                    <span className="check" />
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <div className={`title ${isOverdue ? 'overdue' : ''}`}>
                        {task.name}
                        {task.priority === 'urgent_important' && <span style={{ color: 'var(--amber)', marginLeft: 4 }}>⚑</span>}
                      </div>
                      <div className="meta">
                        {task.context && (
                          <span style={{
                            display: 'inline-flex', alignItems: 'center', gap: 4,
                            fontSize: 11, padding: '2px 8px', borderRadius: 999,
                            background: contextMap[task.context]?.bg || 'var(--surface-hover)',
                            color: contextMap[task.context]?.color || 'var(--text-2)',
                          }}>{contextMap[task.context]?.label || task.context}</span>
                        )}
                        <span>{task.dueDate}</span>
                        {task.caseId && <span>{task.caseId}</span>}
                      </div>
                    </div>
                    <svg style={{ color: 'var(--text-3)', cursor: 'grab', opacity: .55, marginLeft: 'auto' }} width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M9 5h.01M9 12h.01M9 19h.01M15 5h.01M15 12h.01M15 19h.01"/></svg>
                  </div>
                );
              })}
              <div style={{
                display: 'flex', alignItems: 'center', gap: 8,
                border: '1px dashed var(--border-strong)', borderRadius: 'var(--radius-md)',
                padding: '8px 12px', marginTop: 12, color: 'var(--text-3)', fontSize: 13, cursor: 'pointer',
              }}>
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><path d="M12 5v14M5 12h14"/></svg>
                加入今天（⌘T）
              </div>
            </div>
          )}

          {/* 计划中透视（OmniFocus 风格：按日期分组） */}
          {activePerspective === 'upcoming' && (
            <div>
              {Object.entries(groupedByDate).sort().map(([date, dateTasks]) => (
                <div key={date} style={{ marginBottom: 14 }}>
                  <div style={{
                    display: 'flex', alignItems: 'center', gap: 8,
                    fontSize: 12, fontWeight: 700, color: 'var(--text-1)',
                    padding: '10px 4px 5px',
                  }}>
                    {date}
                    <span style={{ color: 'var(--text-3)', fontWeight: 400, fontSize: 11 }}>{dateTasks.length}</span>
                  </div>
                  {dateTasks.map(task => (
                    <div key={task.id} className="task-card" onClick={() => setSelectedTask(task)}>
                      <span className="check" />
                      <div style={{ flex: 1, minWidth: 0 }}>
                        <div className="title">{task.name}</div>
                        <div className="meta">
                          {task.context && <span style={{
                            fontSize: 11, padding: '2px 8px', borderRadius: 999,
                            background: contextMap[task.context]?.bg || 'var(--surface-hover)',
                            color: contextMap[task.context]?.color || 'var(--text-2)',
                          }}>{contextMap[task.context]?.label}</span>}
                          <span>{task.caseId}</span>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              ))}
            </div>
          )}

          {/* 等待透视（追踪委派） */}
          {activePerspective === 'waiting' && (
            <div className="card">
              <div className="card-header">
                <span>等待中</span>
                <span className="sub">{filteredTasks.length} 条 · 等 = 律师的日常</span>
              </div>
              {filteredTasks.map(task => (
                <div key={task.id} style={{
                  display: 'flex', alignItems: 'center', gap: 10,
                  padding: '10px 12px', borderRadius: 'var(--radius-md)',
                  border: '1px solid var(--border)', marginBottom: 8,
                  background: 'var(--surface)',
                }}>
                  <span className="circle amber" />
                  <div style={{ flex: 1 }}>
                    <div style={{ fontSize: 13 }}>等 {task.waitingFor} · {task.name}</div>
                    <div style={{ display: 'flex', gap: 6, fontSize: 11, color: 'var(--text-3)', marginTop: 3 }}>
                      <span style={{ color: 'var(--amber)', display: 'flex', alignItems: 'center', gap: 4 }}>
                        <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 3"/></svg>
                        已等 6 天
                      </span>
                      <span className="tag outline">跟进 {task.dueDate}</span>
                    </div>
                  </div>
                  <button className="btn sm">催办</button>
                </div>
              ))}
            </div>
          )}

          {/* 回顾透视（GTD Reflect） */}
          {activePerspective === 'review' && (
            <div className="card">
              <div className="card-header">
                <span>需要回顾（GTD Reflect）</span>
                <span className="sub">{filteredTasks.length} 案</span>
                <button className="btn sm ghost" style={{ marginLeft: 'auto' }}>标记已回顾</button>
              </div>
              {filteredTasks.map(task => (
                <div key={task.id} style={{
                  border: '1px solid var(--border)', borderRadius: 'var(--radius-md)',
                  padding: '12px 14px', marginBottom: 10, cursor: 'pointer',
                  transition: 'border-color var(--transition)',
                }}
                  onMouseEnter={e => e.currentTarget.style.borderColor = 'var(--border-strong)'}
                  onMouseLeave={e => e.currentTarget.style.borderColor = 'var(--border)'}
                >
                  <div style={{ fontSize: 13, fontWeight: 600, display: 'flex', alignItems: 'center', gap: 8 }}>
                    <span className="circle purple" />
                    {task.name}
                    <span className="tag amber">待回顾</span>
                  </div>
                  <div style={{ fontSize: 12, color: 'var(--text-2)', marginTop: 4 }}>{task.caseId}</div>
                  <div style={{ fontSize: 11.5, color: 'var(--text-3)', marginTop: 3 }}>距上次更新较久，建议审视进展</div>
                </div>
              ))}
            </div>
          )}

          {/* 某天透视 */}
          {activePerspective === 'someday' && (
            <div className="card">
              <div className="card-header">
                <span>某天（Someday）</span>
                <span className="sub">{filteredTasks.length} 项 · 不承诺时间</span>
              </div>
              {filteredTasks.map(task => (
                <div key={task.id} className="task-card" onClick={() => setSelectedTask(task)}>
                  <span className="check" style={{ background: 'var(--surface-hover)', borderColor: 'var(--border-strong)' }} />
                  <div style={{ flex: 1 }}>
                    <div className="title" style={{ color: 'var(--text-3)' }}>{task.name}</div>
                    <div className="meta"><span className="tag gray">未来</span></div>
                  </div>
                </div>
              ))}
            </div>
          )}

          {/* 随时透视（按上下文分组） */}
          {activePerspective === 'anytime' && (
            <div>
              {Object.entries(groupedByContext).map(([ctx, ctxTasks]) => (
                <div key={ctx} style={{ marginBottom: 14 }}>
                  <div style={{
                    display: 'flex', alignItems: 'center', gap: 8,
                    fontSize: 12, fontWeight: 700, color: 'var(--text-1)',
                    padding: '10px 4px 5px',
                  }}>
                    {contextMap[ctx]?.label || ctx}
                    <span style={{ color: 'var(--text-3)', fontWeight: 400, fontSize: 11 }}>{ctxTasks.length}</span>
                  </div>
                  {ctxTasks.map(task => (
                    <div key={task.id} className="task-card" onClick={() => setSelectedTask(task)}>
                      <span className="check" />
                      <div style={{ flex: 1 }}>
                        <div className="title">{task.name}</div>
                        <div className="meta"><span>{task.caseId}</span></div>
                      </div>
                    </div>
                  ))}
                </div>
              ))}
            </div>
          )}
        </div>

        {/* ── 右栏：任务详情面板（OmniFocus 风格）────── */}
        <div className="card" style={{ position: 'sticky', top: 0 }}>
          {selectedTask ? (
            <div>
              <div style={{ display: 'flex', gap: 10, alignItems: 'flex-start', padding: '2px 0 6px' }}>
                <span className="check" style={{ marginTop: 3 }} />
                <span style={{ fontSize: 15, fontWeight: 700, lineHeight: 1.45 }}>{selectedTask.name}</span>
              </div>
              {selectedTask.caseId && <span className="tag purple">{selectedTask.caseId}</span>}

              {/* 时间双轨（When / Deadline） */}
              <div style={{ fontSize: 10.5, fontWeight: 600, color: 'var(--text-3)', padding: '12px 0 6px', letterSpacing: '.4px' }}>
                时间双轨（When / Deadline · 原则二）
              </div>
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10, margin: '12px 0' }}>
                <div style={{ border: '1px solid var(--border)', borderRadius: 'var(--radius-md)', padding: '8px 12px' }}>
                  <div style={{ fontSize: 10, color: 'var(--text-3)', display: 'flex', alignItems: 'center', gap: 4 }}>
                    <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 3"/></svg>
                    When · 开始
                  </div>
                  <div style={{ fontSize: 13, fontWeight: 500, marginTop: 2 }}>今天</div>
                </div>
                <div style={{
                  border: '1px solid var(--border)', borderRadius: 'var(--radius-md)', padding: '8px 12px',
                  background: selectedTask.dueDate && selectedTask.dueDate < '2025-01-11' ? 'var(--red-soft)' : undefined,
                }}>
                  <div style={{ fontSize: 10, color: 'var(--text-3)', display: 'flex', alignItems: 'center', gap: 4 }}>
                    <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 3"/></svg>
                    Deadline · 截止
                  </div>
                  <div style={{
                    fontSize: 13, fontWeight: 500, marginTop: 2,
                    color: selectedTask.dueDate && selectedTask.dueDate < '2025-01-11' ? 'var(--red)' : 'var(--text-1)',
                  }}>{selectedTask.dueDate || '—'}</div>
                </div>
              </div>

              {/* 上下文 */}
              <div style={{ fontSize: 10.5, fontWeight: 600, color: 'var(--text-3)', padding: '8px 0 4px', letterSpacing: '.4px' }}>
                执行上下文
              </div>
              <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
                {['@办公室', '@电话', '@法院'].map(ctx => (
                  <span key={ctx} style={{
                    display: 'inline-flex', alignItems: 'center', gap: 4,
                    fontSize: 11.5, padding: '2px 8px', borderRadius: 999,
                    background: ctx === '@办公室' ? 'var(--blue-soft)' : ctx === '@电话' ? 'var(--green-soft)' : 'var(--purple-soft)',
                    color: ctx === '@办公室' ? 'var(--primary)' : ctx === '@电话' ? 'var(--green)' : 'var(--purple)',
                    cursor: 'pointer',
                  }}>{ctx}</span>
                ))}
              </div>

              {/* 任务信息 */}
              <div style={{ fontSize: 10.5, fontWeight: 600, color: 'var(--text-3)', padding: '12px 0 4px', letterSpacing: '.4px' }}>
                任务信息
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '6px 0', fontSize: 13 }}>
                <span style={{ width: 72, color: 'var(--text-3)', fontSize: 12, flexShrink: 0 }}>预计耗时</span>
                <span>45 分钟</span>
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '6px 0', fontSize: 13 }}>
                <span style={{ width: 72, color: 'var(--text-3)', fontSize: 12, flexShrink: 0 }}>关联案件</span>
                <span>{selectedTask.caseId || '—'}</span>
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '6px 0', fontSize: 13 }}>
                <span style={{ width: 72, color: 'var(--text-3)', fontSize: 12, flexShrink: 0 }}>优先级</span>
                <span className={`tag ${priorityMap[selectedTask.priority]?.tagClass || 'tag.gray'}`}>
                  {priorityMap[selectedTask.priority]?.label || '普通'}
                </span>
              </div>

              {/* 顺序项目（案件线性流程） */}
              <div style={{ fontSize: 10.5, fontWeight: 600, color: 'var(--text-3)', padding: '12px 0 4px', letterSpacing: '.4px' }}>
                顺序项目（blocked）
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8, height: 28 }}>
                <span className="circle green" />
                <span style={{ fontWeight: 500 }}>{selectedTask.name}</span>
                <span className="tag solid-green" style={{ marginLeft: 'auto' }}>当前</span>
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8, height: 28, opacity: .5 }}>
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="var(--text-3)" strokeWidth="2" strokeLinecap="round"><rect x="4" y="11" width="16" height="9" rx="2"/><path d="M8 11V7a4 4 0 0 1 8 0v4"/></svg>
                <span style={{ color: 'var(--text-3)' }}>下一步（锁定）</span>
              </div>

              {/* 备注 */}
              <div style={{ fontSize: 10.5, fontWeight: 600, color: 'var(--text-3)', padding: '12px 0 4px', letterSpacing: '.4px' }}>
                备注
              </div>
              <div style={{ fontSize: 12.5, color: 'var(--text-2)', lineHeight: 1.7 }}>
                {selectedTask.waitingFor ? `等待 ${selectedTask.waitingFor} 的回复。` : '无备注'}
              </div>

              {/* 操作按钮 */}
              <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end', paddingTop: 16 }}>
                <button className="btn">改期</button>
                <button className="btn primary">开始任务</button>
              </div>
            </div>
          ) : (
            <div className="empty" style={{ padding: 40 }}>
              <svg width="34" height="34" viewBox="0 0 24 24" fill="none" stroke="var(--text-3)" strokeWidth="1.5" strokeLinecap="round" opacity=".5"><path d="M9 6h11M9 12h11M9 18h11"/><path d="m3.5 6 1 1 2-2M3.5 12l1 1 2-2M3.5 18l1 1 2-2"/></svg>
              <div style={{ fontSize: 12 }}>选择一个任务查看详情</div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

window.TasksPage = TasksPage;
