import re

modal_path = '/Users/only/Documents/PythonProgram/Casy/src/shared/components/BriefingModal.vue'

with open(modal_path, 'r', encoding='utf-8') as f:
    lines = f.readlines()

def get_block(lines, start_str, end_str):
    start_idx = -1
    end_idx = -1
    for i, line in enumerate(lines):
        if start_str in line and start_idx == -1:
            start_idx = i
        if end_str in line and start_idx != -1 and end_idx == -1:
            end_idx = i
            break
    return start_idx, end_idx

# 1. Replace blueprint template with vogue
s1, e1 = get_block(lines, '<!-- ═══ 4. BLUEPRINT', '<!-- ═══ 5. NARRATIVE')
vogue_html = """        <!-- ═══ 4. VOGUE EDITORIAL (时尚杂志) ═══ -->
        <div v-else-if="activeStyle === 'vogue'" class="paper-card vogue-paper">
          <div class="vogue-head">
            <span class="vogue-issue">ISSUE NO. {{ new Date().getMonth()+1 }}</span>
            <h1 class="vogue-title">CASY<br>VOGUE</h1>
            <div class="vogue-date">{{ effectiveDate }}</div>
          </div>
          <div class="paper-scroll-content vogue-body">
            <h2 class="vogue-lead">{{ effectiveTitle }}</h2>
            <div class="vogue-divider"></div>
            <div class="vogue-focus">
              <span class="v-tag">THE FOCUS</span>
              <h3>{{ nextAction?.taskName }}</h3>
              <p>{{ nextAction?.description }}</p>
            </div>
            <div class="vogue-grid">
              <div class="vogue-col">
                <span class="v-tag">DEADLINES</span>
                <div v-for="r in redlines" :key="r.id" class="v-redline">{{ r.title }}<br><small>{{ r.timeText }}</small></div>
              </div>
              <div class="vogue-col">
                <span class="v-tag">HEARINGS</span>
                <div v-for="h in hearings" :key="h.id" class="v-hearing">{{ h.time }} // {{ h.title }}<br><small>{{ h.court }}</small></div>
              </div>
            </div>
          </div>
        </div>
"""
lines = lines[:s1] + [vogue_html] + lines[e1:]

# 2. Replace terminal template with glassmorphism
s2, e2 = get_block(lines, '<!-- ═══ 6. DATA TERMINAL', '<!-- ═══ 7. ACTION')
glass_html = """        <!-- ═══ 6. GLASSMORPHISM (毛玻璃光晕) ═══ -->
        <div v-else-if="activeStyle === 'glassmorphism'" class="paper-card glass-paper">
          <div class="glass-bg-blob blob-1"></div>
          <div class="glass-bg-blob blob-2"></div>
          <div class="glass-content-wrap">
            <div class="glass-header">
              <div class="g-date-pill">{{ effectiveDate }}</div>
              <h1>{{ effectiveTitle }}</h1>
            </div>
            <div class="paper-scroll-content glass-body">
              <div class="glass-card g-focus">
                <strong>⚡️ TODAY'S FOCUS</strong>
                <h2>{{ nextAction?.taskName }}</h2>
                <p>{{ nextAction?.description }}</p>
              </div>
              <div class="glass-row">
                <div class="glass-card g-alert">
                  <strong>🚨 REDLINES</strong>
                  <div v-for="r in redlines" :key="r.id">{{ r.title }} ({{ r.timeText }})</div>
                </div>
                <div class="glass-card g-schedule">
                  <strong>📅 COURT SCHEDULE</strong>
                  <div v-for="h in hearings" :key="h.id">{{ h.time }} - {{ h.title }}</div>
                </div>
              </div>
            </div>
          </div>
        </div>
"""
lines = lines[:s2] + [glass_html] + lines[e2:]

# 3. Replace all weekly templates
s3, e3 = get_block(lines, '<!-- ═══ 9. DOSSIER', '<!-- ═══ 底部统一工具栏 ═══ -->')
weekly_html = """        <!-- ═══ 9. DOSSIER (绝密归档) ═══ -->
        <div v-else-if="activeStyle === 'dossier'" class="paper-card dossier-paper">
          <div class="dossier-stamp">CONFIDENTIAL</div>
          <div class="dossier-header">
            <span class="dossier-file-no">FILE REF: WK-{{ new Date().getFullYear() }}-{{ String(new Date().getMonth()+1).padStart(2,'0') }}</span>
            <h1 class="dossier-title">{{ effectiveTitle }}</h1>
            <div class="dossier-sub">{{ effectiveDate }} · LEGAL DOSSIER</div>
          </div>
          <div class="dossier-rule"></div>
          <div class="paper-scroll-content dossier-body">
            <div class="dossier-grid-two">
              <div class="dossier-section">
                <h3>[I] MAJOR LITIGATION</h3>
                <p class="d-val">{{ nextAction?.taskName || '重点专案推进' }}</p>
                <p>{{ nextAction?.description }}</p>
              </div>
              <div class="dossier-section">
                <h3>[II] CRITICAL EXPOSURE</h3>
                <div v-for="r in redlines" :key="r.id" class="dossier-bullet">
                  <strong>X</strong> {{ r.title }} <em>({{ r.timeText }})</em>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- ═══ 10. NEON DASHBOARD (暗黑仪表盘) ═══ -->
        <div v-else-if="activeStyle === 'analytics'" class="paper-card neon-dash-paper">
          <div class="neon-head">
            <h1>{{ effectiveTitle }}</h1>
            <span class="neon-badge">{{ effectiveDate }}</span>
          </div>
          <div class="paper-scroll-content neon-body">
            <div class="neon-kpi-grid">
              <div class="n-card">
                <span class="n-lbl">COMPLETED</span>
                <strong class="n-val text-blue">{{ metrics.completedCount || 18 }}</strong>
              </div>
              <div class="n-card">
                <span class="n-lbl">HOURS</span>
                <strong class="n-val text-purple">{{ metrics.committedHours || 32 }}h</strong>
              </div>
              <div class="n-card">
                <span class="n-lbl">PENDING</span>
                <strong class="n-val text-pink">{{ metrics.waitingCount || 4 }}</strong>
              </div>
            </div>
            <div class="n-card glow-card mt-3">
              <span class="n-lbl">PRIORITY METRIC</span>
              <h3>{{ nextAction?.taskName }}</h3>
              <div class="neon-progress"><div class="n-bar" style="width:75%"></div></div>
            </div>
          </div>
        </div>

        <!-- ═══ 11. SUPREME LEDGER (黑金台账) ═══ -->
        <div v-else-if="activeStyle === 'ledger'" class="paper-card luxury-ledger-paper">
          <div class="lux-header">
            <div class="lux-logo">CASY</div>
            <div class="lux-title-box">
              <h2>{{ effectiveTitle }}</h2>
              <span>{{ effectiveDate }} // WEEKLY LEDGER</span>
            </div>
          </div>
          <div class="paper-scroll-content lux-body">
            <table class="lux-table">
              <thead>
                <tr>
                  <th>MATTER / ITEM</th>
                  <th>STATUS</th>
                  <th>VARIANCE</th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <td>{{ nextAction?.taskName }}</td>
                  <td>ACTIVE</td>
                  <td class="t-gold">ON TRACK</td>
                </tr>
                <tr v-for="r in redlines" :key="r.id">
                  <td>{{ r.title }}</td>
                  <td>{{ r.timeText }}</td>
                  <td class="t-red">URGENT</td>
                </tr>
                <tr v-for="h in hearings" :key="h.id">
                  <td>{{ h.title }} ({{ h.court }})</td>
                  <td>{{ h.time }}</td>
                  <td>SCHEDULED</td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>

        <!-- ═══ 12. FLUID MILESTONES (流体时间轴) ═══ -->
        <div v-else-if="activeStyle === 'milestones'" class="paper-card fluid-ms-paper">
          <div class="fluid-head">
            <h1>{{ effectiveTitle }}</h1>
            <p>{{ effectiveDate }}</p>
          </div>
          <div class="paper-scroll-content fluid-body">
            <div class="fluid-track">
              <div class="f-node done">
                <div class="f-bubble"></div>
                <div class="f-content">
                  <strong>Initiation</strong>
                  <p>Filings completed</p>
                </div>
              </div>
              <div class="f-node active">
                <div class="f-bubble glow"></div>
                <div class="f-content">
                  <strong>Current Phase</strong>
                  <p>{{ nextAction?.taskName }}</p>
                </div>
              </div>
              <div class="f-node">
                <div class="f-bubble"></div>
                <div class="f-content">
                  <strong>Upcoming</strong>
                  <p>Awaiting court decision</p>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- ═══ 13. PARTNER LETTER (合伙人复盘信函) ═══ -->
        <div v-else-if="activeStyle === 'partner-brief'" class="paper-card partner-letter-paper">
          <div class="letter-head">
            <div class="letter-firm-name">CASY PARTNERSHIP ATTORNEYS AT LAW</div>
            <div class="letter-meta-row"><span>DATE: {{ effectiveDate }}</span><span>MEMORANDUM TO PARTNERS</span></div>
          </div>
          <div class="letter-divider"></div>
          <div class="paper-scroll-content letter-body">
            <p>Dear Partner,</p>
            <p>本周业务推进平稳有序。我们在<strong>「{{ nextAction?.caseName || '重点专案' }}」</strong>取得了关键进展，完成了庭审证据链反驳工作。</p>
            <p>在风险控制方面，本周共有 {{ redlines.length }} 项法定绝限得到严密监控与执行，无逾期违规情况。</p>
            <p class="letter-sign">Respectfully submitted,<br><span class="sig-font">Casy Lead Counsel</span></p>
          </div>
        </div>

        <!-- ═══ 14. NEO BRUTALISM (新粗野主义) ═══ -->
        <div v-else-if="activeStyle === 'focus-matrix'" class="paper-card brutal-paper">
          <div class="brutal-head">
            <h1>{{ effectiveTitle }}</h1>
            <span class="brutal-tag">{{ effectiveDate }}</span>
          </div>
          <div class="paper-scroll-content brutal-grid">
            <div class="brutal-box b-pink">
              <h3>FOCUS</h3>
              <p>{{ nextAction?.taskName }}</p>
            </div>
            <div class="brutal-box b-yellow">
              <h3>DUE / EXPOSURE</h3>
              <p v-for="r in redlines" :key="r.id">{{ r.title }} ({{ r.timeText }})</p>
            </div>
            <div class="brutal-box b-blue">
              <h3>HEARINGS</h3>
              <p v-for="h in hearings" :key="h.id">{{ h.time }} - {{ h.title }}</p>
            </div>
          </div>
        </div>

        <!-- ═══ 15. CHRONICLE (编年史) ═══ -->
        <div v-else-if="activeStyle === 'chronicle'" class="paper-card dark-chronicle-paper">
          <div class="chronicle-head">
            <h1>THE WEEKLY CHRONICLE</h1>
            <span>{{ effectiveDate }}</span>
          </div>
          <div class="paper-scroll-content dark-chron-flow">
            <div class="dc-row"><div class="dc-dot"></div><strong>MON</strong><span>立案受理与客户沟通会议完成</span></div>
            <div class="dc-row"><div class="dc-dot glow"></div><strong>WED</strong><span class="t-highlight">{{ nextAction?.taskName }}</span></div>
            <div class="dc-row"><div class="dc-dot"></div><strong>FRI</strong><span>法庭庭审质证及周度总结</span></div>
          </div>
        </div>

        <!-- ═══ 16. CYBER MATRIX (赛博战力周报) ═══ -->
        <div v-else class="paper-card cyber-paper">
          <div class="cyber-head">
            <span class="cyber-glitch">[ CYBER_ORDER_MATRIX // WEEKLY ]</span>
            <h2>HUD BATTLE REPORT</h2>
          </div>
          <div class="paper-scroll-content cyber-body">
            <div class="cyber-stat-bar">POWER INDEX: 98.4% // ORDERS COMPLETED: {{ metrics.completedCount || 18 }}</div>
            <div class="cyber-box">
              TARGET LOCKED: {{ nextAction?.taskName }}
            </div>
            <div class="cyber-grid">
              <div v-for="r in redlines" :key="r.id" class="c-danger">[!] {{ r.title }} ({{ r.timeText }})</div>
            </div>
          </div>
        </div>

"""
lines = lines[:s3] + [weekly_html] + lines[e3:]

# 4. Replace blueprint CSS with vogue CSS
s4, e4 = get_block(lines, '/* 4. BLUEPRINT', '/* 5. NARRATIVE')
vogue_css = """/* 4. VOGUE EDITORIAL */
.vogue-paper { background: #fff; color: #111; font-family: 'Times New Roman', Times, serif; border: 12px solid #f8f8f8; box-shadow: 0 10px 40px rgba(0,0,0,0.1); }
.vogue-head { text-align: center; border-bottom: 1px solid #111; padding-bottom: 20px; margin-bottom: 20px; }
.vogue-issue { font-family: var(--font-mono); font-size: 10px; letter-spacing: 2px; display: block; margin-bottom: 10px; }
.vogue-title { font-size: 48px; font-weight: 400; line-height: 0.85; margin: 0; letter-spacing: -1px; }
.vogue-date { margin-top: 10px; font-size: 12px; font-style: italic; }
.vogue-lead { font-size: 18px; text-align: center; font-weight: normal; margin: 0 0 10px; }
.vogue-divider { width: 40px; height: 1px; background: #111; margin: 0 auto 20px; }
.vogue-focus { text-align: center; margin-bottom: 20px; }
.vogue-focus h3 { font-size: 20px; margin: 8px 0; font-weight: normal; }
.vogue-focus p { font-size: 14px; color: #555; }
.v-tag { font-family: var(--font-mono); font-size: 9px; letter-spacing: 1px; font-weight: bold; text-transform: uppercase; border-bottom: 1px solid #111; padding-bottom: 2px; margin-bottom: 8px; display: inline-block; }
.vogue-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 20px; text-align: left; }
.vogue-col { border-top: 1px solid #eee; padding-top: 10px; }
.v-redline, .v-hearing { font-size: 13px; margin-bottom: 10px; line-height: 1.4; }

"""
lines = lines[:s4] + [vogue_css] + lines[e4:]

# 5. Replace terminal CSS with glassmorphism CSS
s5, e5 = get_block(lines, '/* 6. DATA TERMINAL', '/* 7. ACTION')
glass_css = """/* 6. GLASSMORPHISM */
.glass-paper { background: rgba(255, 255, 255, 0.1); border: 1px solid rgba(255,255,255,0.2); backdrop-filter: blur(20px); -webkit-backdrop-filter: blur(20px); overflow: hidden; }
.glass-bg-blob { position: absolute; border-radius: 50%; filter: blur(40px); z-index: 0; opacity: 0.6; }
.blob-1 { width: 200px; height: 200px; background: #a855f7; top: -50px; left: -50px; }
.blob-2 { width: 150px; height: 150px; background: #ec4899; bottom: -20px; right: -20px; }
.glass-content-wrap { position: relative; z-index: 1; display: flex; flex-direction: column; height: 100%; color: #fff; text-shadow: 0 1px 2px rgba(0,0,0,0.1); }
.glass-header { margin-bottom: 20px; }
.g-date-pill { display: inline-block; padding: 4px 12px; background: rgba(255,255,255,0.2); border-radius: 20px; font-size: 10px; font-weight: bold; letter-spacing: 1px; margin-bottom: 8px; }
.glass-header h1 { font-size: 24px; font-weight: 800; margin: 0; letter-spacing: -0.5px; }
.glass-card { background: rgba(255,255,255,0.15); border: 1px solid rgba(255,255,255,0.3); border-radius: 12px; padding: 16px; margin-bottom: 12px; backdrop-filter: blur(10px); }
.g-focus strong, .g-alert strong, .g-schedule strong { font-size: 11px; opacity: 0.9; margin-bottom: 6px; display: block; }
.g-focus h2 { font-size: 18px; margin: 0 0 4px; }
.g-focus p { font-size: 13px; opacity: 0.8; margin: 0; }
.glass-row { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
.g-alert div, .g-schedule div { font-size: 12px; margin-bottom: 4px; padding-bottom: 4px; border-bottom: 1px solid rgba(255,255,255,0.1); }

"""
lines = lines[:s5] + [glass_css] + lines[e5:]

# 6. Replace weekly CSS 
s6, e6 = get_block(lines, '/* 9. DOSSIER', '/* 底部操作栏 */')
weekly_css = """/* 9. DOSSIER */
.dossier-paper { background: #efebe4; color: #2a2826; border: 2px solid #8b3a3a; font-family: 'Courier New', monospace; box-shadow: inset 0 0 30px rgba(0,0,0,0.05); }
.dossier-stamp { position: absolute; right: 20px; top: 30px; border: 3px solid #b91c1c; color: #b91c1c; padding: 4px 12px; font-size: 14px; font-weight: 900; transform: rotate(15deg); opacity: 0.8; letter-spacing: 2px; }
.dossier-file-no { font-size: 10px; color: #8b3a3a; font-weight: 700; display: block; margin-bottom: 4px; }
.dossier-title { font-size: 22px; font-weight: 800; margin: 0; color: #111; font-family: 'Times New Roman', serif; }
.dossier-sub { font-size: 12px; color: #555; }
.dossier-rule { height: 2px; background: #8b3a3a; margin: 12px 0; }
.dossier-grid-two { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }
.dossier-section h3 { font-size: 14px; font-weight: 900; margin: 0 0 8px; color: #8b3a3a; border-bottom: 1px dashed #8b3a3a; padding-bottom: 4px; }
.d-val { font-weight: bold; font-size: 14px; }
.dossier-bullet { font-size: 12px; margin-bottom: 6px; }

/* 10. NEON DASHBOARD */
.neon-dash-paper { background: #0f172a; color: #e2e8f0; border: 1px solid #1e293b; font-family: Inter, sans-serif; }
.neon-head { display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px; }
.neon-head h1 { font-size: 20px; font-weight: 800; margin: 0; background: linear-gradient(90deg, #3b82f6, #8b5cf6); -webkit-background-clip: text; color: transparent; }
.neon-badge { background: rgba(59,130,246,0.2); color: #60a5fa; padding: 4px 10px; border-radius: 12px; font-size: 10px; font-weight: bold; border: 1px solid #3b82f6; }
.neon-kpi-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 12px; }
.n-card { background: #1e293b; border-radius: 10px; padding: 12px; border: 1px solid #334155; }
.n-lbl { font-size: 10px; color: #94a3b8; font-weight: 700; display: block; margin-bottom: 4px; }
.n-val { font-size: 24px; font-weight: 900; }
.text-blue { color: #3b82f6; text-shadow: 0 0 10px rgba(59,130,246,0.5); }
.text-purple { color: #8b5cf6; text-shadow: 0 0 10px rgba(139,92,246,0.5); }
.text-pink { color: #ec4899; text-shadow: 0 0 10px rgba(236,72,153,0.5); }
.glow-card { border-color: #3b82f6; box-shadow: 0 0 15px rgba(59,130,246,0.15); }
.glow-card h3 { font-size: 16px; margin: 0 0 12px; color: #fff; }
.neon-progress { height: 6px; background: #0f172a; border-radius: 3px; overflow: hidden; }
.n-bar { height: 100%; background: linear-gradient(90deg, #3b82f6, #ec4899); box-shadow: 0 0 8px #ec4899; }

/* 11. SUPREME LEDGER */
.luxury-ledger-paper { background: #111; color: #d4d4d4; border: 2px solid #d4af37; border-radius: 4px; }
.lux-header { display: flex; align-items: center; gap: 16px; border-bottom: 1px solid #333; padding-bottom: 16px; margin-bottom: 16px; }
.lux-logo { font-size: 28px; font-family: 'Times New Roman', serif; color: #d4af37; border-right: 1px solid #333; padding-right: 16px; }
.lux-title-box h2 { font-size: 16px; margin: 0; color: #fff; font-weight: 600; letter-spacing: 1px; }
.lux-title-box span { font-size: 10px; color: #d4af37; font-family: var(--font-mono); letter-spacing: 2px; }
.lux-table { width: 100%; border-collapse: collapse; font-size: 12px; font-family: var(--font-mono); }
.lux-table th { text-align: left; padding: 10px 8px; border-bottom: 1px solid #d4af37; color: #d4af37; font-weight: normal; }
.lux-table td { padding: 12px 8px; border-bottom: 1px solid #222; }
.t-gold { color: #d4af37; }
.t-red { color: #ef4444; }

/* 12. FLUID MILESTONES */
.fluid-ms-paper { background: #fdf2f8; border: none; border-top: 6px solid #ec4899; box-shadow: 0 10px 30px rgba(236,72,153,0.1); }
.fluid-head h1 { font-size: 22px; color: #831843; margin: 0; font-weight: 800; }
.fluid-head p { font-size: 12px; color: #be185d; margin: 4px 0 16px; }
.fluid-track { display: flex; flex-direction: column; gap: 0; position: relative; padding-left: 20px; }
.fluid-track::before { content: ''; position: absolute; left: 24px; top: 10px; bottom: 10px; width: 2px; background: linear-gradient(to bottom, #ec4899, #fbcfe8); }
.f-node { display: flex; gap: 16px; padding: 12px 0; position: relative; z-index: 1; }
.f-bubble { width: 10px; height: 10px; border-radius: 50%; background: #fbcfe8; border: 2px solid #fff; margin-top: 4px; flex-shrink: 0; }
.f-node.done .f-bubble { background: #ec4899; }
.f-node.active .f-bubble.glow { background: #db2777; box-shadow: 0 0 0 4px rgba(219, 39, 119, 0.2); }
.f-content strong { display: block; font-size: 14px; color: #831843; }
.f-content p { margin: 2px 0 0; font-size: 12px; color: #9d174d; }

/* 13. PARTNER LETTER */
.partner-letter-paper { background: #fdfbf7; border: 1px solid #d4c5b9; font-family: Georgia, serif; }
.letter-firm-name { font-size: 13px; font-weight: 700; letter-spacing: 1px; color: #433; text-align: center; margin-bottom: 10px; }
.letter-meta-row { display: flex; justify-content: space-between; font-size: 11px; color: #655; text-transform: uppercase; }
.letter-divider { height: 1px; background: #a89f91; margin: 12px 0; }
.letter-body { font-size: 14px; line-height: 1.8; color: #222; }
.letter-sign { margin-top: 20px; }
.sig-font { font-family: 'Brush Script MT', cursive, serif; font-size: 24px; color: #111; display: block; margin-top: 8px; }

/* 14. NEO BRUTALISM */
.brutal-paper { background: #fff; border: 4px solid #000; box-shadow: 8px 8px 0px #000; border-radius: 0; }
.brutal-head { border-bottom: 4px solid #000; padding-bottom: 12px; margin-bottom: 16px; display: flex; justify-content: space-between; align-items: flex-end; }
.brutal-head h1 { font-size: 28px; font-weight: 900; margin: 0; text-transform: uppercase; letter-spacing: -1px; }
.brutal-tag { font-family: var(--font-mono); font-weight: bold; background: #000; color: #fff; padding: 4px 8px; font-size: 12px; }
.brutal-grid { display: flex; flex-direction: column; gap: 16px; }
.brutal-box { border: 3px solid #000; padding: 12px; box-shadow: 4px 4px 0px #000; }
.b-pink { background: #fbcfe8; }
.b-yellow { background: #fef08a; }
.b-blue { background: #bfdbfe; }
.brutal-box h3 { font-size: 14px; font-weight: 900; margin: 0 0 8px; border-bottom: 2px solid #000; display: inline-block; padding-bottom: 2px; }
.brutal-box p { font-size: 13px; font-weight: 600; margin: 4px 0; }

/* 15. CHRONICLE */
.dark-chronicle-paper { background: #18181b; color: #fafafa; border: 1px solid #3f3f46; border-radius: 12px; }
.chronicle-head { margin-bottom: 20px; }
.chronicle-head h1 { font-size: 22px; margin: 0 0 4px; font-weight: 800; letter-spacing: -0.5px; }
.chronicle-head span { font-size: 12px; color: #a1a1aa; }
.dark-chron-flow { display: flex; flex-direction: column; gap: 16px; }
.dc-row { display: flex; align-items: center; gap: 12px; font-size: 13px; background: #27272a; padding: 12px; border-radius: 8px; }
.dc-dot { width: 8px; height: 8px; border-radius: 50%; background: #52525b; }
.dc-dot.glow { background: #10b981; box-shadow: 0 0 8px #10b981; }
.dc-row strong { font-family: var(--font-mono); color: #d4d4d8; }
.t-highlight { color: #34d399; font-weight: 600; }

/* 16. CYBER MATRIX */
.cyber-paper { background: #020617; color: #0ea5e9; border: 1px solid #0369a1; font-family: var(--font-mono); }
.cyber-head { display: flex; flex-direction: column; gap: 4px; border-bottom: 1px dashed #0369a1; padding-bottom: 12px; margin-bottom: 12px; }
.cyber-glitch { font-size: 10px; color: #38bdf8; letter-spacing: 2px; }
.cyber-head h2 { font-size: 20px; font-weight: 800; margin: 0; color: #bae6fd; }
.cyber-stat-bar { background: #0c4a6e; color: #e0f2fe; padding: 8px; font-size: 11px; font-weight: 700; margin-bottom: 12px; border-left: 4px solid #38bdf8; }
.cyber-box { border: 1px solid #0284c7; background: rgba(2,132,199,0.1); padding: 12px; font-size: 12px; margin-bottom: 12px; }
.c-danger { color: #f43f5e; font-size: 11px; margin-bottom: 6px; }

"""
lines = lines[:s6] + [weekly_css] + lines[e6:]

with open(modal_path, 'w', encoding='utf-8') as f:
    f.writelines(lines)
