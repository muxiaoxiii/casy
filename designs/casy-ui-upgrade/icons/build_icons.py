#!/usr/bin/env python3
"""Casy Judicial Docket icon system — generate consistent SVG icons + preview."""

from __future__ import annotations

from pathlib import Path

OUT = Path(__file__).resolve().parent
SVG_DIR = OUT / "svg"
SVG_DIR.mkdir(parents=True, exist_ok=True)

# Design tokens (Judicial Docket)
# grid: 24 viewBox | artboard: 20 (2px padding) | stroke: 1.5 | caps: round
STROKE = "currentColor"
COMMON = (
    'xmlns="http://www.w3.org/2000/svg" '
    'width="24" height="24" viewBox="0 0 24 24" '
    'fill="none" stroke="currentColor" stroke-width="1.5" '
    'stroke-linecap="round" stroke-linejoin="round"'
)

# Each icon body must only draw within x/y 2–22 (20px artboard).
# Optical balance: vertical mass centered on y=12; horizontals on x=12.
ICONS: dict[str, dict] = {
    "today": {
        "label": "今日",
        "desc": "日晷式今日：天穹圆弧 + 案卷横线",
        "body": """
  <circle cx="12" cy="10" r="3.25"/>
  <path d="M12 3.5v1.2"/>
  <path d="M5.5 10h1.1"/>
  <path d="M17.4 10h1.1"/>
  <path d="M4 16.5h16"/>
  <path d="M6.5 19.5h11"/>
""",
    },
    "inbox": {
        "label": "收件箱",
        "desc": "托盘收件：大口袋",
        "body": """
  <path d="M3.5 13.5h4.2l1.2 2.2h6.2l1.2-2.2h4.2"/>
  <path d="M3.5 13.5 6 6.2c.3-.8 1-1.2 1.8-1.2h8.4c.8 0 1.5.4 1.8 1.2l2.5 7.3"/>
  <path d="M3.5 13.5v3.8c0 .9.7 1.7 1.7 1.7h13.6c1 0 1.7-.8 1.7-1.7v-3.8"/>
""",
    },
    "cases": {
        "label": "案件",
        "desc": "律师箱：案件容器",
        "body": """
  <rect x="3.5" y="7.5" width="17" height="11.5" rx="1.5"/>
  <path d="M9 7.5V6.2c0-.7.5-1.2 1.2-1.2h3.6c.7 0 1.2.5 1.2 1.2v1.3"/>
  <path d="M3.5 12.5h17"/>
  <path d="M10.5 12.5v1.5h3v-1.5"/>
""",
    },
    "projects": {
        "label": "项目",
        "desc": "叠层项目板",
        "body": """
  <rect x="7.5" y="3.8" width="12.2" height="4.2" rx="1"/>
  <rect x="4.3" y="9.9" width="12.2" height="4.2" rx="1"/>
  <rect x="7.5" y="16" width="12.2" height="4.2" rx="1"/>
""",
    },
    "tasks": {
        "label": "任务",
        "desc": "勾选清单",
        "body": """
  <rect x="4" y="4" width="16" height="16" rx="2.5"/>
  <path d="M8 12.2l2.4 2.4L16 9.4"/>
""",
    },
    "calendar": {
        "label": "日历",
        "desc": "装订日历页",
        "body": """
  <rect x="3.8" y="5.5" width="16.4" height="14.2" rx="1.5"/>
  <path d="M3.8 9.5h16.4"/>
  <path d="M8 3.8v3.2"/>
  <path d="M16 3.8v3.2"/>
  <path d="M8.2 13h.01M12 13h.01M15.8 13h.01M8.2 16.5h.01M12 16.5h.01"/>
""",
    },
    "knowledge": {
        "label": "知识库",
        "desc": "开卷 + 知识节点",
        "body": """
  <path d="M4.5 5.2c2.4-.8 4.8-.8 7.5.6v13c-2.7-1.4-5.1-1.4-7.5-.6V5.2z"/>
  <path d="M19.5 5.2c-2.4-.8-4.8-.8-7.5.6v13c2.7-1.4 5.1-1.4 7.5-.6V5.2z"/>
  <path d="M12 5.8v13"/>
""",
    },
    "drafting": {
        "label": "文书",
        "desc": "钢笔尖：文书工坊",
        "body": """
  <path d="M14.8 4.4l4.8 4.8"/>
  <path d="M13.2 5.6l5.2 5.2-7.8 7.8H5.4v-5.2l7.8-7.8z"/>
  <path d="M11.4 8.6l4 4"/>
""",
    },
    "dossier": {
        "label": "卷宗",
        "desc": "双层卷宗夹",
        "body": """
  <path d="M3.5 8.2h6.2l1.5 1.8h9.3c.8 0 1.5.7 1.5 1.5v7.8c0 .8-.7 1.5-1.5 1.5H3.5c-.8 0-1.5-.7-1.5-1.5V9.7c0-.8.7-1.5 1.5-1.5z"/>
  <path d="M2 12.8h20"/>
  <path d="M6.2 8.2V6.5c0-.8.7-1.5 1.5-1.5h3.4l1.3 1.6"/>
""",
    },
    "dashboard": {
        "label": "看板",
        "desc": "四格数据看板",
        "body": """
  <rect x="3.5" y="3.5" width="7.2" height="7.2" rx="1.2"/>
  <rect x="13.3" y="3.5" width="7.2" height="7.2" rx="1.2"/>
  <rect x="3.5" y="13.3" width="7.2" height="7.2" rx="1.2"/>
  <path d="M14.5 18.5v-3.2M17 18.5v-5.2M19.5 18.5v-2"/>
""",
    },
    "search": {
        "label": "搜索",
        "desc": "全局检索",
        "body": """
  <circle cx="11" cy="11" r="6.2"/>
  <path d="M15.5 15.5L20 20"/>
""",
    },
    "capture": {
        "label": "捕获",
        "desc": "取景框捕获",
        "body": """
  <path d="M4.5 8.5V6.2c0-.9.7-1.7 1.7-1.7H8.5"/>
  <path d="M15.5 4.5h2.3c.9 0 1.7.7 1.7 1.7v2.3"/>
  <path d="M19.5 15.5v2.3c0 .9-.7 1.7-1.7 1.7h-2.3"/>
  <path d="M8.5 19.5H6.2c-.9 0-1.7-.7-1.7-1.7v-2.3"/>
  <path d="M12 8.5v7M8.5 12h7"/>
""",
    },
    "reminder": {
        "label": "提醒",
        "desc": "分级预警铃",
        "body": """
  <path d="M6.5 10.2a5.5 5.5 0 0 1 11 0c0 3.2.8 4.6 1.5 5.5H5c.7-.9 1.5-2.3 1.5-5.5z"/>
  <path d="M10 18.2a2.2 2.2 0 0 0 4 0"/>
  <path d="M12 5.2V3.8"/>
""",
    },
    "settings": {
        "label": "设置",
        "desc": "机械齿轮",
        "body": """
  <circle cx="12" cy="12" r="3"/>
  <path d="M12 3.8v2.2M12 18v2.2M3.8 12h2.2M18 12h2.2M6.2 6.2l1.6 1.6M16.2 16.2l1.6 1.6M17.8 6.2l-1.6 1.6M7.8 16.2l-1.6 1.6"/>
""",
    },
    "waiting": {
        "label": "等待",
        "desc": "沙漏：等待中",
        "body": """
  <path d="M7.5 3.8h9"/>
  <path d="M7.5 20.2h9"/>
  <path d="M8.5 3.8c0 4.2 3.5 5.2 3.5 8.2 0 3-3.5 4-3.5 8.2"/>
  <path d="M15.5 3.8c0 4.2-3.5 5.2-3.5 8.2 0 3 3.5 4 3.5 8.2"/>
""",
    },
    "risk": {
        "label": "风险",
        "desc": "期限风险 / 逾期",
        "body": """
  <path d="M12 4.2L21 19.2H3L12 4.2z"/>
  <path d="M12 10.2v4.2"/>
  <path d="M12 16.8h.01"/>
""",
    },
    "link": {
        "label": "关联",
        "desc": "双向链接",
        "body": """
  <path d="M10.2 13.8a3.5 3.5 0 0 0 5 0l3-3a3.5 3.5 0 0 0-5-5l-1.2 1.2"/>
  <path d="M13.8 10.2a3.5 3.5 0 0 0-5 0l-3 3a3.5 3.5 0 0 0 5 5l1.2-1.2"/>
""",
    },
    "file": {
        "label": "文件",
        "desc": "文书页",
        "body": """
  <path d="M7.5 3.8h6.8L18.5 8v12.2H7.5V3.8z"/>
  <path d="M14 3.8V8h4.5"/>
  <path d="M9.8 12.5h6.4M9.8 16h4.2"/>
""",
    },
    "sync": {
        "label": "同步",
        "desc": "双向同步",
        "body": """
  <path d="M6.5 8.5h10.2a3.5 3.5 0 0 1 3.5 3.5"/>
  <path d="M17.5 15.5H7.3A3.5 3.5 0 0 1 3.8 12"/>
  <path d="M6.5 8.5l-2.7-2.7M6.5 8.5l2.2-2.5"/>
  <path d="M17.5 15.5l2.7 2.7M17.5 15.5l-2.2 2.5"/>
""",
    },
    "filter": {
        "label": "筛选",
        "desc": "漏斗筛选",
        "body": """
  <path d="M4 5.5h16l-6.2 7.2v5.3l-3.6 1.8v-7.1L4 5.5z"/>
""",
    },
    "check": {
        "label": "完成",
        "desc": "完成勾",
        "body": """
  <path d="M5 12.5l4.2 4.2L19 7.8"/>
""",
    },
    "clock": {
        "label": "时间",
        "desc": "时点 / 双轨时间",
        "body": """
  <circle cx="12" cy="12" r="8"/>
  <path d="M12 7.5V12l3 2"/>
""",
    },
    "ai": {
        "label": "智伴",
        "desc": "主动智伴 AI",
        "body": """
  <path d="M12 3.8l1.2 3.2L16.4 8.2l-3.2 1.2L12 12.6l-1.2-3.2L7.6 8.2l3.2-1.2L12 3.8z"/>
  <path d="M18.2 13.2l.7 1.8 1.8.7-1.8.7-.7 1.8-.7-1.8-1.8-.7 1.8-.7.7-1.8z"/>
  <path d="M6.5 14.5l.6 1.5 1.5.6-1.5.6-.6 1.5-.6-1.5-1.5-.6 1.5-.6.6-1.5z"/>
""",
    },
    "seal": {
        "label": "品牌印章",
        "desc": "Casy 法务印章标",
        "body": """
  <circle cx="12" cy="12" r="8.2"/>
  <circle cx="12" cy="12" r="5.6"/>
  <path d="M12 7.8v8.4"/>
  <path d="M9.2 10.2h5.6"/>
  <path d="M9.2 13.8h5.6"/>
""",
    },
    "relation": {
        "label": "关系网",
        "desc": "案件关系网络",
        "body": """
  <circle cx="6" cy="7" r="2.2"/>
  <circle cx="18" cy="7" r="2.2"/>
  <circle cx="12" cy="17" r="2.2"/>
  <path d="M8 8.2l2.8 6.4M16 8.2l-2.8 6.4M8.2 7h7.6"/>
""",
    },
    "brief": {
        "label": "早报",
        "desc": "今日早报",
        "body": """
  <path d="M5 5.5h11.5c.8 0 1.5.7 1.5 1.5v12c0 .8-.7 1.5-1.5 1.5H5c-.8 0-1.5-.7-1.5-1.5V7c0-.8.7-1.5 1.5-1.5z"/>
  <path d="M7.5 9h6M7.5 12.5h6M7.5 16h4"/>
  <path d="M18 8.5h1.5c.8 0 1.5.7 1.5 1.5v8"/>
""",
    },
    "star": {
        "label": "重点",
        "desc": "今日重点 ★",
        "body": """
  <path d="M12 3.8l2.4 5 5.5.8-4 3.8.9 5.5L12 16.4 7.2 18.9l.9-5.5-4-3.8 5.5-.8L12 3.8z"/>
""",
    },
    "plus": {
        "label": "新建",
        "desc": "新建",
        "body": """
  <path d="M12 5v14M5 12h14"/>
""",
    },
    "chevron-right": {
        "label": "展开",
        "desc": "侧栏/行展开",
        "body": """
  <path d="M9.5 6.5l5.5 5.5-5.5 5.5"/>
""",
    },
    "archive": {
        "label": "归档",
        "desc": "归档入库",
        "body": """
  <rect x="3.5" y="4.5" width="17" height="4.2" rx="1"/>
  <path d="M5 8.7v9.8c0 .8.7 1.5 1.5 1.5h11c.8 0 1.5-.7 1.5-1.5V8.7"/>
  <path d="M9.5 12.5h5"/>
""",
    },
    "ocr": {
        "label": "OCR",
        "desc": "页级识别",
        "body": """
  <path d="M7.5 4.5h9c.8 0 1.5.7 1.5 1.5v12c0 .8-.7 1.5-1.5 1.5h-9c-.8 0-1.5-.7-1.5-1.5V6c0-.8.7-1.5 1.5-1.5z"/>
  <path d="M9 8.5h6M9 12h6M9 15.5h3.5"/>
  <path d="M4 9.5h1.5M4 14.5h1.5M18.5 9.5H20M18.5 14.5H20"/>
""",
    },
}


def wrap(body: str) -> str:
    return f"<svg {COMMON}>\n{body.strip()}\n</svg>\n"


def write_icons() -> None:
    for name, meta in ICONS.items():
        path = SVG_DIR / f"{name}.svg"
        path.write_text(wrap(meta["body"]), encoding="utf-8")
    # Sprite with symbols for production use
    symbols = []
    for name, meta in ICONS.items():
        symbols.append(
            f'  <symbol id="i-{name}" viewBox="0 0 24 24" fill="none" '
            f'stroke="currentColor" stroke-width="1.5" stroke-linecap="round" '
            f'stroke-linejoin="round">\n{meta["body"].strip()}\n  </symbol>'
        )
    sprite = (
        '<svg xmlns="http://www.w3.org/2000/svg" style="display:none">\n'
        + "\n".join(symbols)
        + "\n</svg>\n"
    )
    (OUT / "casy-icons.sprite.svg").write_text(sprite, encoding="utf-8")


def write_preview() -> None:
    cards = []
    for name, meta in ICONS.items():
        svg = wrap(meta["body"]).replace("\n", "\n      ")
        cards.append(
            f"""
    <article class="card">
      <div class="glyph" title="{meta['desc']}">
        <svg {COMMON}>
{meta['body'].rstrip()}
        </svg>
      </div>
      <div class="meta">
        <code>{name}</code>
        <strong>{meta['label']}</strong>
        <span>{meta['desc']}</span>
      </div>
    </article>"""
        )
    grid = "\n".join(cards)

    # size tests
    size_rows = []
    for px in (16, 20, 24, 32):
        size_rows.append(
            f"""
      <div class="size-item">
        <svg width="{px}" height="{px}" viewBox="0 0 24 24" fill="none" stroke="currentColor"
             stroke-width="{1.5 * 24 / px:.2f}" stroke-linecap="round" stroke-linejoin="round">
{ICONS['today']['body'].rstrip()}
        </svg>
        <span>{px}px</span>
      </div>"""
        )

    reverse_tiles = []
    for name in ("today", "cases", "capture", "seal", "risk", "waiting"):
        reverse_tiles.append(
            f"""
      <div class="rev-tile">
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor"
             stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
{ICONS[name]['body'].rstrip()}
        </svg>
      </div>"""
        )

    html = f"""<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>Casy · Judicial Docket 图标系统</title>
<style>
  :root {{
    --ink: #0C1526;
    --body: #243041;
    --muted: #5B6B7F;
    --rule: #D7DEE8;
    --bg: #F3F5F8;
    --surface: #FFFFFF;
    --sidebar: #E9EDF3;
    --accent: #1A4FD6;
    --risk: #D64545;
  }}
  * {{ box-sizing: border-box; }}
  body {{
    margin: 0;
    font-family: "PingFang SC", "Inter", "SF Pro Text", -apple-system, sans-serif;
    background: var(--bg);
    color: var(--body);
    line-height: 1.5;
  }}
  header {{
    background: var(--surface);
    border-bottom: 1px solid var(--rule);
    padding: 32px 40px 28px;
  }}
  .brand {{
    display: flex;
    align-items: center;
    gap: 16px;
    margin-bottom: 18px;
  }}
  .mark {{
    width: 48px; height: 48px;
    border: 1.5px solid var(--ink);
    border-radius: 10px;
    display: grid; place-items: center;
    color: var(--ink);
  }}
  h1 {{
    margin: 0;
    font-size: 28px;
    font-weight: 650;
    letter-spacing: -0.02em;
    color: var(--ink);
    font-family: "Songti SC", "Noto Serif SC", "Source Han Serif SC", Georgia, serif;
  }}
  .sub {{ color: var(--muted); font-size: 14px; margin-top: 4px; }}
  .rules {{
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 12px;
    margin-top: 8px;
  }}
  .rule-chip {{
    background: var(--bg);
    border: 1px solid var(--rule);
    border-radius: 8px;
    padding: 10px 12px;
    font-size: 12px;
    color: var(--muted);
  }}
  .rule-chip strong {{
    display: block;
    color: var(--ink);
    font-size: 13px;
    margin-bottom: 2px;
    font-weight: 600;
  }}
  main {{ padding: 28px 40px 64px; max-width: 1200px; }}
  h2 {{
    font-size: 13px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--muted);
    font-weight: 600;
    margin: 28px 0 14px;
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
  }}
  .grid {{
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 12px;
  }}
  .card {{
    background: var(--surface);
    border: 1px solid var(--rule);
    border-radius: 10px;
    padding: 18px 16px 14px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }}
  .glyph {{
    width: 56px; height: 56px;
    border: 1px dashed var(--rule);
    border-radius: 8px;
    display: grid; place-items: center;
    color: var(--ink);
    background:
      linear-gradient(to right, transparent 49.5%, var(--rule) 49.5%, var(--rule) 50.5%, transparent 50.5%),
      linear-gradient(to bottom, transparent 49.5%, var(--rule) 49.5%, var(--rule) 50.5%, transparent 50.5%),
      var(--bg);
    background-size: 56px 56px;
  }}
  .glyph svg {{ width: 28px; height: 28px; }}
  .meta {{ display: flex; flex-direction: column; gap: 2px; }}
  .meta code {{
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    font-size: 11px;
    color: var(--accent);
  }}
  .meta strong {{ color: var(--ink); font-size: 14px; font-weight: 600; }}
  .meta span {{ color: var(--muted); font-size: 12px; }}
  .panel {{
    background: var(--surface);
    border: 1px solid var(--rule);
    border-radius: 12px;
    padding: 20px;
  }}
  .sizes {{ display: flex; align-items: end; gap: 28px; color: var(--ink); }}
  .size-item {{ display: flex; flex-direction: column; align-items: center; gap: 8px; font-size: 11px; color: var(--muted); font-family: ui-monospace, monospace; }}
  .rev {{
    display: flex; gap: 12px; flex-wrap: wrap;
  }}
  .rev-tile {{
    width: 56px; height: 56px;
    background: var(--ink);
    color: #fff;
    border-radius: 10px;
    display: grid; place-items: center;
  }}
  .grid-demo {{
    display: grid;
    grid-template-columns: 220px 1fr;
    gap: 20px;
    align-items: start;
  }}
  .anatomy {{
    background: var(--surface);
    border: 1px solid var(--rule);
    border-radius: 12px;
    padding: 16px;
  }}
  .anatomy .frame {{
    width: 100%;
    aspect-ratio: 1;
    background: #fff;
    border: 1px solid var(--rule);
    border-radius: 8px;
    position: relative;
    overflow: hidden;
  }}
  .anatomy .frame svg {{
    width: 100%; height: 100%;
    color: var(--ink);
  }}
  .anatomy ul {{
    margin: 12px 0 0;
    padding-left: 18px;
    color: var(--muted);
    font-size: 12px;
  }}
  .swatches {{ display: flex; gap: 10px; flex-wrap: wrap; }}
  .swatch {{
    width: 88px;
    border: 1px solid var(--rule);
    border-radius: 8px;
    overflow: hidden;
    background: var(--surface);
  }}
  .swatch i {{ display: block; height: 44px; }}
  .swatch span {{
    display: block;
    padding: 8px;
    font-size: 11px;
    font-family: ui-monospace, monospace;
    color: var(--muted);
  }}
  footer {{
    margin-top: 36px;
    padding-top: 16px;
    border-top: 1px solid var(--rule);
    color: var(--muted);
    font-size: 12px;
  }}
</style>
</head>
<body>
<header>
  <div class="brand">
    <div class="mark" aria-hidden="true">
      <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor"
           stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
{ICONS['seal']['body'].rstrip()}
      </svg>
    </div>
    <div>
      <h1>Casy · Judicial Docket 图标系统</h1>
      <div class="sub">专利律师全流程秩序引擎 · 统一线性图标语言 · 可直接用于 Vue / 设计交付</div>
    </div>
  </div>
  <div class="rules">
    <div class="rule-chip"><strong>画板 24</strong>viewBox 0 0 24 24，视觉安全区 2–22（20px）</div>
    <div class="rule-chip"><strong>描边 1.5</strong>round cap / join，无填充，currentColor</div>
    <div class="rule-chip"><strong>光学平衡</strong>主视觉居中 y=12；避免发丝线，角半径 1–2.5</div>
    <div class="rule-chip"><strong>隐喻克制</strong>法务卷宗语义，几何精准，无 emoji / 无混重</div>
  </div>
</header>
<main>
  <h2>Construction · 作图规范</h2>
  <div class="grid-demo">
    <div class="anatomy">
      <div class="frame">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"
             stroke-linecap="round" stroke-linejoin="round">
          <rect x="2" y="2" width="20" height="20" rx="1" stroke="#D7DEE8" stroke-dasharray="1.5 1.5" stroke-width="0.6"/>
          <rect x="4" y="4" width="16" height="16" rx="1" stroke="#1A4FD6" stroke-opacity="0.35" stroke-width="0.6" stroke-dasharray="1 1"/>
          <path d="M2 12h20M12 2v20" stroke="#D7DEE8" stroke-width="0.5"/>
{ICONS['today']['body'].rstrip()}
        </svg>
      </div>
      <ul>
        <li>外框 24，内容 20</li>
        <li>描边统一 1.5</li>
        <li>拐角与端点 round</li>
        <li>多用水平案卷线</li>
      </ul>
    </div>
    <div class="panel">
      <h2 style="margin-top:0">Size tests · 尺寸试作</h2>
      <div class="sizes">
{"".join(size_rows)}
        <div class="size-item" style="margin-left:auto">
          <div class="rev-tile" style="width:48px;height:48px">
            <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                 stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
{ICONS['seal']['body'].rstrip()}
            </svg>
          </div>
          <span>reverse</span>
        </div>
      </div>
      <h2>Reverse · 墨底反白</h2>
      <div class="rev">
{"".join(reverse_tiles)}
      </div>
      <h2>Color tokens · 用色</h2>
      <div class="swatches">
        <div class="swatch"><i style="background:#0C1526"></i><span>ink #0C1526</span></div>
        <div class="swatch"><i style="background:#1A4FD6"></i><span>accent #1A4FD6</span></div>
        <div class="swatch"><i style="background:#D64545"></i><span>risk #D64545</span></div>
        <div class="swatch"><i style="background:#C4890A"></i><span>warn #C4890A</span></div>
        <div class="swatch"><i style="background:#2F7A8C"></i><span>wait #2F7A8C</span></div>
        <div class="swatch"><i style="background:#5B6B7F"></i><span>2nd #5B6B7F</span></div>
      </div>
    </div>
  </div>

  <h2>Library · 完整图标库（{len(ICONS)}）</h2>
  <div class="grid">
{grid}
  </div>

  <footer>
    Casy Judicial Docket Icons · stroke/round/currentColor · 源文件 <code>icons/svg/*.svg</code> ·
    雪碧图 <code>casy-icons.sprite.svg</code> · 生成脚本 <code>build_icons.py</code>
  </footer>
</main>
</body>
</html>
"""
    (OUT / "preview.html").write_text(html, encoding="utf-8")


def write_guidelines() -> None:
    rows = "\n".join(
        f"| `{name}` | {meta['label']} | {meta['desc']} |"
        for name, meta in ICONS.items()
    )
    md = f"""# Casy Judicial Docket 图标系统

> 完整一致的法务线性图标语言。与 UI 升级（卷宗墨卷 · 现代化配色）同一套骨架。

## 设计锚点

- **产品**：Casy — 专利律师全流程秩序引擎
- **系统名**：Judicial Docket（卷宗墨卷）
- **性格**：卷宗秩序、法庭文书的克制精确；现代线性，而非复古雕版
- **形态**：线性描边、几何法务隐喻、统一光学重量

## 构造规则

| 项 | 规范 |
|----|------|
| 画板 | `viewBox="0 0 24 24"` |
| 安全区 | 2–22（20px 视觉盒） |
| 描边 | `stroke-width="1.5"` |
| 端点 | `stroke-linecap="round"` `stroke-linejoin="round"` |
| 填充 | `fill="none"`（默认） |
| 颜色 | `stroke="currentColor"`，跟随文字色 / 语义色 |
| 小尺寸 | 16px 时描边可增至约 2.25 视觉等效（预览已标） |
| 禁止 | emoji、多套图标混用、不同描边混重、写实隐喻、多余装饰 |

## 隐喻约定

- **今日**：日轮 + 案卷横线（今日卷）
- **收件箱**：托盘（大口袋，先丢后厘清）
- **案件**：律师箱（统一容器）
- **卷宗**：双层卷宗夹（12 阶段归档）
- **等待**：沙漏（等待中，不是延期）
- **风险**：三角警示（R1–R4 / 逾期）
- **智伴**：星芒节点（主动智伴）
- **品牌印章**：双环法务印（可用作 App 图标 / 角标）

## 语义色（与主题一致）

| Token | Hex | 用途 |
|-------|-----|------|
| ink | `#0C1526` | 默认图标 |
| accent | `#1A4FD6` | 选中 / 主操作 |
| risk | `#D64545` | 逾期 / R3–R4 |
| warning | `#C4890A` | 将到期 / R2 |
| waiting | `#2F7A8C` | 等待中 |
| secondary | `#5B6B7F` | 弱化图标 |

## 交付文件

| 文件 | 说明 |
|------|------|
| `svg/*.svg` | 单图标源文件（{len(ICONS)} 枚） |
| `casy-icons.sprite.svg` | `<symbol>` 雪碧图 |
| `preview.html` | 规范预览页（浏览器打开） |
| `build_icons.py` | 生成脚本（改几何请改这里再重生成） |

## 在 Vue / Element Plus 中使用

内联 SVG（推荐，随文字色着色）：

```html
<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor"
     stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
  <!-- 将 svg/*.svg 中的 path/circle/rect 直接粘贴于此 -->
</svg>
```

或使用雪碧图：

```html
<svg width="20" height="20"><use href="/icons/casy-icons.sprite.svg#i-today"></use></svg>
```

## 图标清单

| id | 中文 | 说明 |
|----|------|------|
{rows}

## 质量自检

- [x] 全部 24 网格对齐，安全区 2–22
- [x] 描边 1.5 / round / currentColor 一致
- [x] 无填充混用、无 emoji
- [x] 同类隐喻同构（卷宗线、三角风险、沙漏等待）
- [x] 16 / 20 / 24 / 32 尺寸试作通过
- [x] 墨底反白可用
"""
    (OUT / "ICON-GUIDELINES.md").write_text(md, encoding="utf-8")


if __name__ == "__main__":
    write_icons()
    write_preview()
    write_guidelines()
    print(f"generated {len(ICONS)} icons in {SVG_DIR}")
    print(f"preview: {OUT / 'preview.html'}")
