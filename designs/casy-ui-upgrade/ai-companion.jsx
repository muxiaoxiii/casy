// ═══════════════════════════════════════════════════════════
// Casy UI Upgrade — AI Companion Page
// ═══════════════════════════════════════════════════════════

const AICompanionPage = () => {
  const [input, setInput] = React.useState('');
  const [messages] = React.useState([
    { role: 'bot', text: '你好！我是 Casy AI 智伴。我可以帮你分析案件、整理文书、追踪进度。今天有什么需要帮忙的？' },
    { role: 'user', text: '帮我看看隆基244号无效案的最新进展' },
    { role: 'bot', text: '隆基244号无效案（(2024)国知局第244号）当前状态：\n\n📋 进度：65%（进行中）\n⏰ 近期节点：1月20日口头审理\n📌 关键任务：准备口审意见陈述（1月18日前完成）\n👥 承办律师：张律师、李律师\n\n建议优先处理口审准备材料。需要我帮你整理相关的专利文献吗？' },
    { role: 'user', text: '好，帮我整理一下这个案件的证据清单' },
    { role: 'bot', text: '我已为你生成隆基244号案的证据清单框架：\n\n1️⃣ 专利文献类\n- 本专利授权公告文本\n- 对比文件1-3\n- 最接近现有技术说明\n\n2️⃣ 技术证据类\n- 技术方案对比分析报告\n- 公知常识证据\n\n3️⃣ 程序类\n- 无效宣告请求书\n- 口审通知书\n\n需要我从知识库中提取具体的文件链接吗？' },
  ]);

  const quickActions = [
    { icon: '📋', label: '案件摘要', desc: '生成案件进展摘要' },
    { icon: '📝', label: '文书起草', desc: '基于模板起草法律文书' },
    { icon: '🔍', label: '法规检索', desc: '查找相关法条和判例' },
    { icon: '📊', label: '进度分析', desc: '分析所有案件进度' },
  ];

  return (
    <div className="fade-in" style={{ display: 'flex', flexDirection: 'column', height: 'calc(100vh - 120px)' }}>
      {/* Quick Actions */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 12, marginBottom: 'var(--sp-4)' }}>
        {quickActions.map((action, i) => (
          <div key={i} style={{
            background: 'var(--surface)',
            border: '1px solid var(--border)',
            borderRadius: 'var(--r-lg)',
            padding: '14px 16px',
            cursor: 'pointer',
            transition: 'all 0.15s',
            display: 'flex',
            alignItems: 'center',
            gap: 12,
          }}
            onMouseEnter={e => { e.currentTarget.style.borderColor = 'var(--blue-200)'; e.currentTarget.style.boxShadow = 'var(--shadow-md)'; }}
            onMouseLeave={e => { e.currentTarget.style.borderColor = 'var(--border)'; e.currentTarget.style.boxShadow = 'none'; }}
          >
            <span style={{ fontSize: 24 }}>{action.icon}</span>
            <div>
              <div style={{ fontSize: 13, fontWeight: 600, color: 'var(--slate-800)' }}>{action.label}</div>
              <div style={{ fontSize: 11, color: 'var(--slate-400)' }}>{action.desc}</div>
            </div>
          </div>
        ))}
      </div>

      {/* Chat Area */}
      <div className="card" style={{ flex: 1, display: 'flex', flexDirection: 'column' }}>
        <div style={{ flex: 1, overflowY: 'auto', padding: 'var(--sp-4)' }}>
          {messages.map((msg, i) => (
            <div className="ai-message" key={i}>
              <div className={`ai-avatar ${msg.role === 'bot' ? 'bot' : 'user'}`}>
                {msg.role === 'bot' ? <Icons.Sparkles size={16} /> : '你'}
              </div>
              <div className={`ai-bubble ${msg.role === 'bot' ? 'bot' : 'user'}`}>
                {msg.text.split('\n').map((line, j) => <div key={j}>{line || <br />}</div>)}
              </div>
            </div>
          ))}
        </div>

        {/* Input */}
        <div style={{
          padding: 'var(--sp-3) var(--sp-4)',
          borderTop: '1px solid var(--border)',
          display: 'flex',
          gap: 10,
          alignItems: 'center',
        }}>
          <input
            value={input}
            onChange={e => setInput(e.target.value)}
            placeholder="输入你的问题…"
            style={{
              flex: 1,
              border: '1px solid var(--border)',
              borderRadius: 'var(--r-md)',
              padding: '10px 14px',
              fontSize: 13,
              outline: 'none',
              fontFamily: 'var(--font-sans)',
              color: 'var(--slate-700)',
              transition: 'border-color 0.15s',
            }}
            onFocus={e => e.target.style.borderColor = 'var(--blue-400)'}
            onBlur={e => e.target.style.borderColor = 'var(--border)'}
          />
          <button className="btn btn-primary" style={{ padding: '10px 16px' }}>
            <Icons.Send size={14} /> 发送
          </button>
        </div>
      </div>
    </div>
  );
};

window.AICompanionPage = AICompanionPage;
