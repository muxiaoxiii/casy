// ═══════════════════════════════════════════════════════════
// Casy UI Upgrade — Cases Management (List / Kanban / Network)
// ═══════════════════════════════════════════════════════════

// ── D3 Force Network Graph ──────────────────────────────
const CaseNetworkGraph = () => {
  const ref = React.useRef(null);
  const [selected, setSelected] = React.useState(null);

  React.useEffect(() => {
    if (!ref.current) return;
    const el = ref.current;
    el.innerHTML = '';

    const { cases, relations } = CasyData;
    const width = el.clientWidth || 700;
    const height = 460;

    const nodes = cases.map(c => ({
      id: c.id,
      label: c.name,
      client: c.client,
      status: c.status,
      track: c.track,
      radius: c.status === '已结案' ? 20 : 28,
    }));

    const links = relations.map(r => ({
      source: r.source,
      target: r.target,
      type: r.type,
      label: r.label,
    }));

    const colorMap = {
      patent_invalidation: '#3b82f6',
      civil_tort: '#8b5cf6',
      admin_litigation: '#f59e0b',
      other: '#94a3b8',
    };

    const statusColor = { '进行中': '#3b82f6', '等待中': '#f59e0b', '已结案': '#22c55e' };

    const svg = d3.select(el).append('svg')
      .attr('width', width).attr('height', height);

    // Arrow marker
    svg.append('defs').append('marker')
      .attr('id', 'arrowhead')
      .attr('viewBox', '0 -5 10 10')
      .attr('refX', 30).attr('refY', 0)
      .attr('markerWidth', 6).attr('markerHeight', 6)
      .attr('orient', 'auto')
      .append('path').attr('d', 'M0,-5L10,0L0,5').attr('fill', '#cbd5e1');

    const simulation = d3.forceSimulation(nodes)
      .force('link', d3.forceLink(links).id(d => d.id).distance(140))
      .force('charge', d3.forceManyBody().strength(-400))
      .force('center', d3.forceCenter(width / 2, height / 2))
      .force('collision', d3.forceCollide().radius(40));

    // Links
    const link = svg.append('g').selectAll('line').data(links).join('line')
      .attr('stroke', '#cbd5e1').attr('stroke-width', 1.5)
      .attr('marker-end', 'url(#arrowhead)');

    // Link labels
    const linkLabel = svg.append('g').selectAll('text').data(links).join('text')
      .style('font-size', '10px').style('fill', '#94a3b8').style('text-anchor', 'middle')
      .text(d => d.label);

    // Node groups
    const node = svg.append('g').selectAll('g').data(nodes).join('g')
      .style('cursor', 'pointer')
      .call(d3.drag()
        .on('start', (e, d) => { if (!e.active) simulation.alphaTarget(0.3).restart(); d.fx = d.x; d.fy = d.y; })
        .on('drag', (e, d) => { d.fx = e.x; d.fy = e.y; })
        .on('end', (e, d) => { if (!e.active) simulation.alphaTarget(0); d.fx = null; d.fy = null; })
      );

    // Node circles
    node.append('circle')
      .attr('r', d => d.radius)
      .attr('fill', d => colorMap[d.track] || '#94a3b8')
      .attr('opacity', 0.15)
      .attr('stroke', d => colorMap[d.track] || '#94a3b8')
      .attr('stroke-width', 2);

    // Status indicator
    node.append('circle')
      .attr('r', 4)
      .attr('fill', d => statusColor[d.status] || '#94a3b8')
      .attr('cx', d => d.radius - 4)
      .attr('cy', d => -(d.radius - 4));

    // Node labels
    node.append('text')
      .attr('text-anchor', 'middle')
      .attr('dy', '0.35em')
      .style('font-size', '11px')
      .style('font-weight', '600')
      .style('fill', '#1e293b')
      .text(d => d.id);

    node.append('text')
      .attr('text-anchor', 'middle')
      .attr('dy', d => d.radius + 14)
      .style('font-size', '10px')
      .style('fill', '#64748b')
      .text(d => d.client);

    // Click
    node.on('click', (e, d) => {
      setSelected(prev => prev === d.id ? null : d.id);
    });

    // Hover
    node.on('mouseenter', function(e, d) {
      d3.select(this).select('circle').transition().duration(200)
        .attr('r', d.radius + 4).attr('opacity', 0.25);
    });
    node.on('mouseleave', function(e, d) {
      d3.select(this).select('circle').transition().duration(200)
        .attr('r', d.radius).attr('opacity', 0.15);
    });

    simulation.on('tick', () => {
      link.attr('x1', d => d.source.x).attr('y1', d => d.source.y)
        .attr('x2', d => d.target.x).attr('y2', d => d.target.y);
      linkLabel.attr('x', d => (d.source.x + d.target.x) / 2)
        .attr('y', d => (d.source.y + d.target.y) / 2 - 8);
      node.attr('transform', d => `translate(${d.x},${d.y})`);
    });

    return () => simulation.stop();
  }, []);

  return <div ref={ref} className="network-container" />;
};

// ── Cases List View ─────────────────────────────────────
const CasesListView = () => {
  const { cases, trackMap, statusMap } = CasyData;

  return (
    <div className="card">
      <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
        <thead>
          <tr style={{ background: 'var(--slate-50)', borderBottom: '1px solid var(--border)' }}>
            {['案件名称', '客户', '轨道', '状态', '案号', '期限', '进度', ''].map((h, i) => (
              <th key={i} style={{ padding: '10px 16px', textAlign: 'left', fontWeight: 500, color: 'var(--slate-500)', fontSize: 12 }}>
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {cases.map(c => {
            const track = trackMap[c.track] || trackMap.other;
            const status = statusMap[c.status] || { color: 'gray' };
            const isOverdue = c.dueDate && c.dueDate < '2025-01-11';
            return (
              <tr key={c.id} style={{ borderBottom: '1px solid var(--border-light)', cursor: 'pointer', transition: 'background 0.12s' }}
                onMouseEnter={e => e.currentTarget.style.background = 'var(--slate-50)'}
                onMouseLeave={e => e.currentTarget.style.background = 'transparent'}
              >
                <td style={{ padding: '12px 16px' }}>
                  <div style={{ fontWeight: 500, color: 'var(--slate-800)' }}>{c.name}</div>
                  <div style={{ fontSize: 11, color: 'var(--slate-400)', marginTop: 2 }}>{c.attorneys.join(' · ')}</div>
                </td>
                <td style={{ padding: '12px 16px', color: 'var(--slate-600)' }}>{c.client}</td>
                <td style={{ padding: '12px 16px' }}>
                  <span className={`tag tag-${track.color}`}>{track.label}</span>
                </td>
                <td style={{ padding: '12px 16px' }}>
                  <span style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}>
                    <span style={{ width: 6, height: 6, borderRadius: '50%', background: status.color === 'blue' ? '#3b82f6' : status.color === 'amber' ? '#f59e0b' : '#22c55e' }} />
                    <span style={{ color: 'var(--slate-600)', fontSize: 12 }}>{c.status}</span>
                  </span>
                </td>
                <td style={{ padding: '12px 16px', fontSize: 12, color: 'var(--slate-500)', fontFamily: 'var(--font-mono)' }}>{c.caseNo}</td>
                <td style={{ padding: '12px 16px', fontSize: 12, color: isOverdue ? 'var(--red-500)' : 'var(--slate-500)' }}>
                  {c.dueDate || '—'}
                </td>
                <td style={{ padding: '12px 16px' }}>
                  <div style={{ width: 80, height: 4, background: 'var(--slate-100)', borderRadius: 2, overflow: 'hidden' }}>
                    <div style={{ width: `${c.progress}%`, height: '100%', background: c.progress === 100 ? '#22c55e' : '#3b82f6', borderRadius: 2, transition: 'width 0.3s' }} />
                  </div>
                </td>
                <td style={{ padding: '12px 16px' }}>
                  <Icons.MoreHorizontal size={16} style={{ color: 'var(--slate-400)', cursor: 'pointer' }} />
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
};

// ── Cases Kanban View ───────────────────────────────────
const CasesKanbanView = () => {
  const { cases, trackMap, statusMap } = CasyData;
  const columns = [
    { key: '进行中', color: '#3b82f6' },
    { key: '等待中', color: '#f59e0b' },
    { key: '已结案', color: '#22c55e' },
  ];

  return (
    <div className="kanban-board">
      {columns.map(col => {
        const colCases = cases.filter(c => c.status === col.key);
        return (
          <div className="kanban-column" key={col.key}>
            <div className="kanban-column-header" style={{ borderBottomColor: col.color }}>
              <div className="kanban-column-title">
                <span style={{ width: 8, height: 8, borderRadius: '50%', background: col.color, display: 'inline-block' }} />
                {col.key}
                <span className="badge" style={{ background: col.color, marginLeft: 4 }}>{colCases.length}</span>
              </div>
            </div>
            {colCases.map(c => {
              const track = trackMap[c.track] || trackMap.other;
              return (
                <div className="kanban-card" key={c.id}>
                  <div style={{ fontSize: 13, fontWeight: 500, color: 'var(--slate-800)', marginBottom: 6 }}>{c.name}</div>
                  <div style={{ fontSize: 12, color: 'var(--slate-500)', marginBottom: 8 }}>{c.client}</div>
                  <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                    <span className={`tag tag-${track.color}`} style={{ fontSize: 10 }}>{track.label}</span>
                    <div style={{ display: 'flex', gap: 4 }}>
                      {c.attorneys.slice(0, 2).map((a, i) => (
                        <div key={i} style={{
                          width: 22, height: 22, borderRadius: '50%',
                          background: 'var(--slate-100)', color: 'var(--slate-500)',
                          display: 'flex', alignItems: 'center', justifyContent: 'center',
                          fontSize: 10, fontWeight: 500,
                        }}>{a[0]}</div>
                      ))}
                    </div>
                  </div>
                  {c.progress < 100 && (
                    <div style={{ marginTop: 8, height: 3, background: 'var(--slate-100)', borderRadius: 2, overflow: 'hidden' }}>
                      <div style={{ width: `${c.progress}%`, height: '100%', background: col.color, borderRadius: 2 }} />
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        );
      })}
    </div>
  );
};

// ── Cases Page ──────────────────────────────────────────
const CasesPage = () => {
  const [view, setView] = React.useState('list');
  const views = [
    { key: 'list', label: '列表', icon: Icons.Layout },
    { key: 'kanban', label: '看板', icon: Icons.GripVertical },
    { key: 'network', label: '关系图谱', icon: Icons.Network },
  ];

  return (
    <div className="fade-in">
      {/* Toolbar */}
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 'var(--sp-4)' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <div className="input-search">
            <Icons.Search size={14} />
            <input placeholder="搜索案件名称、客户、案号…" />
          </div>
          <button className="btn btn-ghost btn-sm"><Icons.Filter size={14} /> 筛选</button>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
          <div className="tabs-bar">
            {views.map(v => {
              const VIcon = v.icon;
              return (
                <div key={v.key} className={`tab-item ${view === v.key ? 'active' : ''}`} onClick={() => setView(v.key)}>
                  <VIcon size={13} style={{ marginRight: 4, verticalAlign: -2 }} /> {v.label}
                </div>
              );
            })}
          </div>
          <button className="btn btn-primary btn-sm"><Icons.Plus size={14} /> 新建案件</button>
        </div>
      </div>

      {/* Content */}
      {view === 'list' && <CasesListView />}
      {view === 'kanban' && <CasesKanbanView />}
      {view === 'network' && (
        <div className="card">
          <div className="card-header">
            <div className="card-title"><Icons.Network size={16} /> 案件关系网络</div>
            <span className="tag tag-blue">{CasyData.relations.length} 条关联</span>
          </div>
          <div className="card-body">
            <CaseNetworkGraph />
          </div>
        </div>
      )}
    </div>
  );
};

window.CasesPage = CasesPage;
