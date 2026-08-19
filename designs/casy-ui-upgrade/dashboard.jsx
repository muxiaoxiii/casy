// ═══════════════════════════════════════════════════════════
// Casy UI Upgrade — Dashboard with Data Visualization
// ═══════════════════════════════════════════════════════════

// ── D3 Chart: 案件状态分布 (Donut) ──────────────────────
const CaseStatusChart = () => {
  const ref = React.useRef(null);

  React.useEffect(() => {
    if (!ref.current) return;
    const el = ref.current;
    el.innerHTML = '';

    const data = [
      { label: '进行中', value: 5, color: '#3b82f6' },
      { label: '等待中', value: 2, color: '#f59e0b' },
      { label: '已结案', value: 1, color: '#22c55e' },
    ];

    const width = 200, height = 200;
    const radius = Math.min(width, height) / 2;
    const innerRadius = radius * 0.6;

    const svg = d3.select(el).append('svg')
      .attr('width', width).attr('height', height)
      .append('g').attr('transform', `translate(${width/2},${height/2})`);

    const pie = d3.pie().value(d => d.value).sort(null).padAngle(0.03);
    const arc = d3.arc().innerRadius(innerRadius).outerRadius(radius).cornerRadius(4);

    svg.selectAll('path')
      .data(pie(data))
      .join('path')
      .attr('d', arc)
      .attr('fill', d => d.data.color)
      .attr('stroke', 'white')
      .attr('stroke-width', 2)
      .style('cursor', 'pointer')
      .on('mouseenter', function(e, d) {
        d3.select(this).transition().duration(200).attr('d', d3.arc().innerRadius(innerRadius).outerRadius(radius + 6).cornerRadius(4));
      })
      .on('mouseleave', function(e, d) {
        d3.select(this).transition().duration(200).attr('d', arc);
      });

    // Center label
    svg.append('text').attr('text-anchor', 'middle').attr('dy', '-0.2em')
      .style('font-size', '28px').style('font-weight', '700').style('fill', '#0f172a')
      .text('8');
    svg.append('text').attr('text-anchor', 'middle').attr('dy', '1.2em')
      .style('font-size', '12px').style('fill', '#94a3b8')
      .text('总案件');

  }, []);

  return <div ref={ref} style={{ display: 'flex', justifyContent: 'center' }} />;
};

// ── D3 Chart: 案件轨道分布 (Horizontal Bar) ──────────────
const TrackChart = () => {
  const ref = React.useRef(null);

  React.useEffect(() => {
    if (!ref.current) return;
    const el = ref.current;
    el.innerHTML = '';

    const data = [
      { label: '专利无效', value: 3, color: '#3b82f6' },
      { label: '民事侵权', value: 3, color: '#8b5cf6' },
      { label: '行政诉讼', value: 1, color: '#f59e0b' },
      { label: '其他', value: 1, color: '#94a3b8' },
    ].sort((a, b) => b.value - a.value);

    const margin = { top: 8, right: 40, bottom: 8, left: 80 };
    const width = 320 - margin.left - margin.right;
    const height = 140 - margin.top - margin.bottom;

    const svg = d3.select(el).append('svg')
      .attr('width', width + margin.left + margin.right)
      .attr('height', height + margin.top + margin.bottom)
      .append('g').attr('transform', `translate(${margin.left},${margin.top})`);

    const x = d3.scaleLinear().domain([0, d3.max(data, d => d.value)]).range([0, width]);
    const y = d3.scaleBand().domain(data.map(d => d.label)).range([0, height]).padding(0.35);

    // Bars
    svg.selectAll('rect').data(data).join('rect')
      .attr('x', 0).attr('y', d => y(d.label))
      .attr('width', 0).attr('height', y.bandwidth())
      .attr('fill', d => d.color).attr('rx', 4)
      .transition().duration(800).delay((d, i) => i * 100)
      .attr('width', d => x(d.value));

    // Labels
    svg.selectAll('.label').data(data).join('text')
      .attr('x', -8).attr('y', d => y(d.label) + y.bandwidth() / 2)
      .attr('text-anchor', 'end').attr('dominant-baseline', 'central')
      .style('font-size', '12px').style('fill', '#475569')
      .text(d => d.label);

    // Values
    svg.selectAll('.val').data(data).join('text')
      .attr('x', d => x(d.value) + 8).attr('y', d => y(d.label) + y.bandwidth() / 2)
      .attr('dominant-baseline', 'central')
      .style('font-size', '12px').style('font-weight', '600').style('fill', '#0f172a')
      .text(d => d.value);

  }, []);

  return <div ref={ref} style={{ display: 'flex', justifyContent: 'center' }} />;
};

// ── D3 Chart: 月度任务趋势 (Area + Line) ────────────────
const TaskTrendChart = () => {
  const ref = React.useRef(null);

  React.useEffect(() => {
    if (!ref.current) return;
    const el = ref.current;
    el.innerHTML = '';

    const data = [
      { month: '8月', created: 12, completed: 10 },
      { month: '9月', created: 18, completed: 15 },
      { month: '10月', created: 14, completed: 16 },
      { month: '11月', created: 22, completed: 19 },
      { month: '12月', created: 16, completed: 18 },
      { month: '1月', created: 20, completed: 14 },
    ];

    const margin = { top: 20, right: 20, bottom: 30, left: 40 };
    const width = 420 - margin.left - margin.right;
    const height = 180 - margin.top - margin.bottom;

    const svg = d3.select(el).append('svg')
      .attr('width', width + margin.left + margin.right)
      .attr('height', height + margin.top + margin.bottom)
      .append('g').attr('transform', `translate(${margin.left},${margin.top})`);

    const x = d3.scalePoint().domain(data.map(d => d.month)).range([0, width]);
    const yMax = d3.max(data, d => Math.max(d.created, d.completed));
    const y = d3.scaleLinear().domain([0, yMax + 4]).range([height, 0]);

    // Grid lines
    svg.selectAll('.grid').data(y.ticks(4)).join('line')
      .attr('x1', 0).attr('x2', width)
      .attr('y1', d => y(d)).attr('y2', d => y(d))
      .attr('stroke', '#f1f5f9').attr('stroke-width', 1);

    // X axis
    svg.selectAll('.xlab').data(data).join('text')
      .attr('x', d => x(d.month)).attr('y', height + 20)
      .attr('text-anchor', 'middle')
      .style('font-size', '11px').style('fill', '#94a3b8')
      .text(d => d.month);

    // Y axis
    svg.selectAll('.ylab').data(y.ticks(4)).join('text')
      .attr('x', -8).attr('y', d => y(d))
      .attr('text-anchor', 'end').attr('dominant-baseline', 'central')
      .style('font-size', '11px').style('fill', '#94a3b8')
      .text(d => d);

    // Area - created
    const areaCreated = d3.area().x(d => x(d.month)).y0(height).y1(d => y(d.created)).curve(d3.curveMonotoneX);
    svg.append('path').datum(data).attr('d', areaCreated)
      .attr('fill', '#dbeafe').attr('opacity', 0.6);

    // Area - completed
    const areaCompleted = d3.area().x(d => x(d.month)).y0(height).y1(d => y(d.completed)).curve(d3.curveMonotoneX);
    svg.append('path').datum(data).attr('d', areaCompleted)
      .attr('fill', '#dcfce7').attr('opacity', 0.6);

    // Lines
    const lineCreated = d3.line().x(d => x(d.month)).y(d => y(d.created)).curve(d3.curveMonotoneX);
    svg.append('path').datum(data).attr('d', lineCreated)
      .attr('fill', 'none').attr('stroke', '#3b82f6').attr('stroke-width', 2.5);

    const lineCompleted = d3.line().x(d => x(d.month)).y(d => y(d.completed)).curve(d3.curveMonotoneX);
    svg.append('path').datum(data).attr('d', lineCompleted)
      .attr('fill', 'none').attr('stroke', '#22c55e').attr('stroke-width', 2.5);

    // Dots
    svg.selectAll('.dot-c').data(data).join('circle')
      .attr('cx', d => x(d.month)).attr('cy', d => y(d.created))
      .attr('r', 3.5).attr('fill', '#3b82f6').attr('stroke', 'white').attr('stroke-width', 2);
    svg.selectAll('.dot-cm').data(data).join('circle')
      .attr('cx', d => x(d.month)).attr('cy', d => y(d.completed))
      .attr('r', 3.5).attr('fill', '#22c55e').attr('stroke', 'white').attr('stroke-width', 2);

    // Legend
    const legend = svg.append('g').attr('transform', `translate(${width - 160}, -12)`);
    legend.append('circle').attr('cx', 0).attr('cy', 0).attr('r', 4).attr('fill', '#3b82f6');
    legend.append('text').attr('x', 10).attr('y', 0).attr('dominant-baseline', 'central')
      .style('font-size', '11px').style('fill', '#64748b').text('新建');
    legend.append('circle').attr('cx', 60).attr('cy', 0).attr('r', 4).attr('fill', '#22c55e');
    legend.append('text').attr('x', 70).attr('y', 0).attr('dominant-baseline', 'central')
      .style('font-size', '11px').style('fill', '#64748b').text('完成');

  }, []);

  return <div ref={ref} style={{ display: 'flex', justifyContent: 'center' }} />;
};

// ── D3 Chart: 近期庭审时间线 (Gantt-like) ───────────────
const HearingTimeline = () => {
  const ref = React.useRef(null);

  React.useEffect(() => {
    if (!ref.current) return;
    const el = ref.current;
    el.innerHTML = '';

    const events = [
      { label: '中芯国际调解', date: '2025-01-12', color: '#ef4444', type: '调解' },
      { label: '客户周会', date: '2025-01-13', color: '#8b5cf6', type: '会议' },
      { label: '内部讨论', date: '2025-01-15', color: '#8b5cf6', type: '会议' },
      { label: '华为侵权开庭', date: '2025-01-18', color: '#ef4444', type: '开庭' },
      { label: '隆基无效口审', date: '2025-01-20', color: '#ef4444', type: '口审' },
      { label: '百度证据截止', date: '2025-01-25', color: '#f59e0b', type: '期限' },
      { label: '腾讯答辩截止', date: '2025-01-25', color: '#f59e0b', type: '期限' },
    ];

    const margin = { top: 10, right: 20, bottom: 30, left: 120 };
    const width = 420 - margin.left - margin.right;
    const height = 220 - margin.top - margin.bottom;

    const svg = d3.select(el).append('svg')
      .attr('width', width + margin.left + margin.right)
      .attr('height', height + margin.top + margin.bottom)
      .append('g').attr('transform', `translate(${margin.left},${margin.top})`);

    const parseDate = d3.timeParse('%Y-%m-%d');
    const dates = events.map(d => parseDate(d.date));
    const x = d3.scaleTime()
      .domain([d3.timeDay.offset(d3.min(dates), -2), d3.timeDay.offset(d3.max(dates), 2)])
      .range([0, width]);

    const y = d3.scaleBand().domain(events.map(d => d.label)).range([0, height]).padding(0.3);

    // Grid
    svg.selectAll('.gridline').data(x.ticks(d3.timeDay.every(3))).join('line')
      .attr('x1', d => x(d)).attr('x2', d => x(d))
      .attr('y1', 0).attr('y2', height)
      .attr('stroke', '#f1f5f9').attr('stroke-dasharray', '3,3');

    // X axis
    svg.append('g').attr('transform', `translate(0,${height})`)
      .call(d3.axisBottom(x).ticks(d3.timeDay.every(3)).tickFormat(d3.timeFormat('%m/%d')))
      .call(g => g.select('.domain').remove())
      .call(g => g.selectAll('.tick line').attr('stroke', '#e2e8f0'))
      .call(g => g.selectAll('.tick text').style('font-size', '11px').style('fill', '#94a3b8'));

    // Timeline bars
    svg.selectAll('.bar').data(events).join('rect')
      .attr('x', d => x(parseDate(d.date)) - 3)
      .attr('y', d => y(d.label))
      .attr('width', 0)
      .attr('height', y.bandwidth())
      .attr('fill', d => d.color)
      .attr('rx', 4)
      .attr('opacity', 0.15)
      .transition().duration(600).delay((d, i) => i * 80)
      .attr('width', 6);

    // Dots
    svg.selectAll('.dot').data(events).join('circle')
      .attr('cx', d => x(parseDate(d.date)))
      .attr('cy', d => y(d.label) + y.bandwidth() / 2)
      .attr('r', 0)
      .attr('fill', d => d.color)
      .attr('stroke', 'white')
      .attr('stroke-width', 2)
      .transition().duration(400).delay((d, i) => i * 80)
      .attr('r', 5);

    // Labels
    svg.selectAll('.lab').data(events).join('text')
      .attr('x', -8)
      .attr('y', d => y(d.label) + y.bandwidth() / 2)
      .attr('text-anchor', 'end')
      .attr('dominant-baseline', 'central')
      .style('font-size', '12px').style('fill', '#334155')
      .text(d => d.label);

    // Type tags
    svg.selectAll('.tag').data(events).join('text')
      .attr('x', d => x(parseDate(d.date)) + 14)
      .attr('y', d => y(d.label) + y.bandwidth() / 2)
      .attr('dominant-baseline', 'central')
      .style('font-size', '10px').style('fill', d => d.color).style('font-weight', '500')
      .text(d => d.type);

    // Today line
    const today = parseDate('2025-01-11');
    svg.append('line')
      .attr('x1', x(today)).attr('x2', x(today))
      .attr('y1', -4).attr('y2', height + 4)
      .attr('stroke', '#3b82f6').attr('stroke-width', 1.5).attr('stroke-dasharray', '4,4');
    svg.append('text')
      .attr('x', x(today)).attr('y', -8)
      .attr('text-anchor', 'middle')
      .style('font-size', '10px').style('fill', '#3b82f6').style('font-weight', '500')
      .text('今日');

  }, []);

  return <div ref={ref} style={{ display: 'flex', justifyContent: 'center' }} />;
};

// ── Dashboard Page ──────────────────────────────────────
const DashboardPage = () => {
  const { cases, tasks, events, activities } = CasyData;

  const hardScheduleToday = events.filter(e => e.type === 'hearing' && e.date <= '2025-01-20').length;
  const dueToday = tasks.filter(t => t.status === 'today' || t.dueDate === '2025-01-11').length;
  const waitingOverdue = tasks.filter(t => t.status === 'waiting').length;
  const reviewCount = tasks.filter(t => t.status === 'review').length;

  return (
    <div className="fade-in">
      {/* Stats */}
      <div className="stat-cards">
        <div className="stat-card">
          <div className="stat-icon red"><Icons.Calendar size={22} /></div>
          <div>
            <div className="stat-value">{hardScheduleToday}</div>
            <div className="stat-label">近期硬性日程</div>
          </div>
        </div>
        <div className="stat-card">
          <div className="stat-icon amber"><Icons.Clock size={22} /></div>
          <div>
            <div className="stat-value">{dueToday}</div>
            <div className="stat-label">今日到期</div>
          </div>
        </div>
        <div className="stat-card">
          <div className="stat-icon blue"><Icons.AlertTriangle size={22} /></div>
          <div>
            <div className="stat-value">{waitingOverdue}</div>
            <div className="stat-label">等待跟进</div>
          </div>
        </div>
        <div className="stat-card">
          <div className="stat-icon green"><Icons.RefreshCw size={22} /></div>
          <div>
            <div className="stat-value">{reviewCount}</div>
            <div className="stat-label">需回顾</div>
          </div>
        </div>
      </div>

      {/* Charts Row */}
      <div className="grid-23" style={{ marginBottom: 'var(--sp-5)' }}>
        <div className="card">
          <div className="card-header">
            <div className="card-title"><Icons.TrendingUp size={16} /> 月度任务趋势</div>
            <span className="tag tag-blue">近6个月</span>
          </div>
          <div className="card-body">
            <TaskTrendChart />
          </div>
        </div>
        <div className="card">
          <div className="card-header">
            <div className="card-title"><Icons.PieChart size={16} /> 案件状态</div>
          </div>
          <div className="card-body" style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 16 }}>
            <CaseStatusChart />
            <div style={{ display: 'flex', gap: 16, fontSize: 12, color: 'var(--slate-500)' }}>
              <span style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                <span style={{ width: 8, height: 8, borderRadius: '50%', background: '#3b82f6', display: 'inline-block' }} /> 进行中 5
              </span>
              <span style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                <span style={{ width: 8, height: 8, borderRadius: '50%', background: '#f59e0b', display: 'inline-block' }} /> 等待中 2
              </span>
              <span style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
                <span style={{ width: 8, height: 8, borderRadius: '50%', background: '#22c55e', display: 'inline-block' }} /> 已结案 1
              </span>
            </div>
          </div>
        </div>
      </div>

      {/* Timeline + Activity */}
      <div className="grid-23" style={{ marginBottom: 'var(--sp-5)' }}>
        <div className="card">
          <div className="card-header">
            <div className="card-title"><Icons.Calendar size={16} /> 近期庭审 & 期限</div>
            <span className="tag tag-red">{hardScheduleToday} 项待处理</span>
          </div>
          <div className="card-body">
            <HearingTimeline />
          </div>
        </div>
        <div className="card">
          <div className="card-header">
            <div className="card-title"><Icons.BarChart size={16} /> 轨道分布</div>
          </div>
          <div className="card-body">
            <TrackChart />
          </div>
        </div>
      </div>

      {/* Bottom: Next Actions + Activity */}
      <div className="grid-2">
        <div className="card">
          <div className="card-header">
            <div className="card-title"><Icons.Zap size={16} /> 下一步行动</div>
            <span className="badge badge-blue">{tasks.filter(t => t.status === 'next').length}</span>
          </div>
          <div style={{ maxHeight: 280, overflowY: 'auto' }}>
            {tasks.filter(t => t.status === 'next').map(task => (
              <div className="task-row" key={task.id}>
                <div className="task-check" />
                <div className="task-content">
                  <div className="task-name">{task.name}</div>
                  <div className="task-meta">
                    {task.caseId && <span style={{ color: 'var(--blue-500)' }}>{task.caseId}</span>}
                    {task.dueDate && <span>{task.dueDate}</span>}
                  </div>
                </div>
                <span className={`tag ${CasyData.priorityMap[task.priority]?.tagClass || 'tag-gray'}`}>
                  {CasyData.priorityMap[task.priority]?.label || '普通'}
                </span>
              </div>
            ))}
          </div>
        </div>

        <div className="card">
          <div className="card-header">
            <div className="card-title"><Icons.Bell size={16} /> 最近活动</div>
          </div>
          <div style={{ maxHeight: 280, overflowY: 'auto' }}>
            {activities.map(act => (
              <div key={act.id} style={{
                display: 'flex',
                alignItems: 'center',
                gap: 10,
                padding: '10px 16px',
                borderBottom: '1px solid var(--border-light)',
                cursor: 'pointer',
                transition: 'background 0.12s',
              }}
                onMouseEnter={e => e.currentTarget.style.background = 'var(--slate-50)'}
                onMouseLeave={e => e.currentTarget.style.background = 'transparent'}
              >
                <div style={{
                  width: 28, height: 28, borderRadius: '50%',
                  background: 'var(--slate-100)',
                  display: 'flex', alignItems: 'center', justifyContent: 'center',
                  color: 'var(--slate-400)', flexShrink: 0,
                }}>
                  <Icons.FileText size={13} />
                </div>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontSize: 13, color: 'var(--slate-700)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
                    {act.summary}
                  </div>
                </div>
                <span style={{ fontSize: 11, color: 'var(--slate-400)', flexShrink: 0 }}>{act.time}</span>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
};

window.DashboardPage = DashboardPage;
