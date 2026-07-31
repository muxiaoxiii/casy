# Casy 大口袋 — 统一信息入口与数据更新管道

> 版本：v1.0 | 日期：2026-07-31

## 一、设计理念

大口袋不是简单的"收件箱"，它是 Casy 的**信息中枢**：

```
外部世界 ──→ 大口袋 ──→ AI 分类 ──→ 路由到正确的目标
  │                                      │
  ├─ 文件（PDF/Word/图片）               ├─ 案件关联（文件+信息）
  ├─ 邮件                                ├─ 知识库（法条/笔记/邮件）
  ├─ 短信截图                            ├─ 日历（节假日/开庭通知）
  ├─ 手动笔记                            ├─ 内部数据库更新（案由/法院/法条）
  ├─ 剪贴板                              ├─ 待办任务
  └─ 系统托盘/悬浮窗                     └─ 期限提醒
```

**核心原则**：用户只需要"丢进去"，系统自动判断这是什么、该放哪里。

---

## 二、入口设计

### 2.1 入口清单

| 入口 | 说明 | 优先级 |
|------|------|--------|
| **UI 拖拽区** | 主界面中的拖拽区域，支持文件和文本 | P0 |
| **系统托盘菜单** | 右键托盘图标 → "添加到收件箱" | P0 |
| **全局快捷键** | Cmd+Shift+V：粘贴剪贴板内容到收件箱 | P1 |
| **悬浮窗** | 小悬浮球，拖入文件或粘贴文本 | P1 |
| **文件夹监听** | 监听指定文件夹（如 `~/Documents/Casy/inbox/`），新文件自动导入 | P2 |
| **邮件监听** | IMAP 长连接，新邮件自动导入 | P2 |
| **系统通知监听** | 监听法院/国知局的短信通知（需要用户手动转发） | P3 |

### 2.2 系统托盘集成

```rust
// Tauri 2 系统托盘
let _tray = TrayIconBuilder::new()
    .menu(&Menu::with_items(app, &[
        &MenuItem::with_id(app, "add_file", "添加文件到收件箱", true, None::<&str>),
        &MenuItem::with_id(app, "add_note", "添加笔记", true, None::<&str>),
        &MenuItem::with_id(app, "paste_clipboard", "粘贴剪贴板", true, None::<&str>),
        &MenuItem::with_id(app, "open", "打开 Casy", true, None::<&str>),
        &MenuItem::with_id(app, "quit", "退出", true, None::<&str>),
    ])?
    .on_menu_event(|app, event| match event.id.as_ref() {
        "add_file" => { /* 打开文件选择器 → 导入到收件箱 */ }
        "add_note" => { /* 打开笔记输入弹窗 */ }
        "paste_clipboard" => { /* 读取剪贴板 → 导入到收件箱 */ }
        "open" => { /* 显示主窗口 */ }
        "quit" => { app.exit(0); }
        _ => {}
    })
    .build(app)?;
```

### 2.3 剪贴板监听

```rust
// 定期检查剪贴板变化（每 2 秒）
// 如果剪贴板有新内容（文本或图片），托盘图标显示小红点
// 用户点击"粘贴剪贴板"时导入

use arboard::Clipboard;

fn check_clipboard() -> Option<ClipboardContent> {
    let mut clipboard = Clipboard::new().ok()?;
    if let Ok(text) = clipboard.get_text() {
        if !text.is_empty() && text != LAST_CLIPBOARD {
            return Some(ClipboardContent::Text(text));
        }
    }
    if let Ok(image) = clipboard.get_image() {
        return Some(ClipboardContent::Image(image));
    }
    None
}
```

---

## 三、AI 分类与路由

### 3.1 分类类型

| 类型 | 识别方式 | 路由目标 |
|------|---------|---------|
| **传票** | 关键词 + 正则 | 案件关联 + 文件归档 |
| **口审通知书** | 关键词 + 正则 | 案件关联 + 文件归档 |
| **判决/裁定/决定** | 关键词 | 案件关联 + 文件归档 |
| **起诉状/答辩状** | 关键词 | 案件关联 + 文件归档 |
| **证据材料** | 文件名/内容 | 案件关联 + 文件归档 |
| **法条/法规** | 关键词 + 模式匹配 | 知识库 + 内部数据库更新 |
| **节假日通知** | 关键词（放假/假期/调休） | 日历数据库更新 |
| **案由相关** | 关键词（案由/案由规定） | 案由数据库更新 |
| **邮件** | 来源标识 | 知识库 + 案件关联 |
| **笔记** | 手动输入 | 知识库 |
| **其他文件** | 默认 | 收件箱待处理 |

### 3.2 路由决策树

```
输入内容
  │
  ├─ 是文件？
  │   ├─ PDF → OCR 提取文本 → 分类
  │   ├─ Word → 提取文本 → 分类
  │   ├─ 图片 → OCR → 分类
  │   └─ .eml → 解析邮件头+正文 → 分类
  │
  ├─ 是文本？
  │   ├─ 包含"传票/传唤/开庭" → 传票
  │   ├─ 包含"口头审理通知书/口审" → 口审通知书
  │   ├─ 包含"判决书/裁定书/无效决定" → 判决
  │   ├─ 包含"放假/假期/调休/国务院" → 节假日通知
  │   ├─ 包含"案由/案由规定/民事案件案由" → 案由更新
  │   ├─ 包含"法条/法律/法规/第X条" → 法条知识库
  │   ├─ 包含"起诉状/答辩状/请求书" → 诉讼文书
  │   └─ 其他 → 笔记/待处理
  │
  └─ 是图片？
      └─ OCR → 同文本分类逻辑
```

### 3.3 匹配规则

```rust
fn classify_and_route(content: &str, source: &str) -> RoutingDecision {
    // 1. 传票
    if content.contains("传票") || (content.contains("传唤") && content.contains("开庭")) {
        return RoutingDecision {
            category: "summons".into(),
            target: RouteTarget::CaseFile { auto_match: true },
            confidence: 0.85,
        };
    }

    // 2. 节假日通知
    if (content.contains("放假") || content.contains("假期") || content.contains("调休"))
        && (content.contains("国务院") || content.contains("办公厅"))
    {
        return RoutingDecision {
            category: "holiday_notice".into(),
            target: RouteTarget::CalendarUpdate,
            confidence: 0.9,
        };
    }

    // 3. 案由更新
    if content.contains("案由规定") || content.contains("民事案件案由") {
        return RoutingDecision {
            category: "cause_action_update".into(),
            target: RouteTarget::DatabaseUpdate { table: "cause_actions" },
            confidence: 0.8,
        };
    }

    // 4. 法条/法规
    if content.contains("第") && content.contains("条")
        && (content.contains("法") || content.contains("条例") || content.contains("规定"))
    {
        return RoutingDecision {
            category: "legal_provision".into(),
            target: RouteTarget::KnowledgeBase,
            confidence: 0.7,
        };
    }

    // ... 更多规则

    // 默认
    RoutingDecision {
        category: "note".into(),
        target: RouteTarget::Inbox,
        confidence: 0.3,
    }
}
```

---

## 四、内部数据库更新

### 4.1 可更新的数据库

| 数据库 | 更新来源 | 更新方式 |
|--------|---------|---------|
| **法定节假日** | 国务院放假通知 | AI 解析 → 更新 holidays JSON |
| **案由库** | 最高法案由规定 | AI 解析 → 更新 cause_actions 表 |
| **法条库** | 法律法规文本 | AI 解析 → 写入 knowledge_items |
| **法院名录** | 法院名称列表 | 手动或批量导入 |
| **法官信息** | 庭审记录 | 自动提取 → 写入 officials 表 |
| **当事人信息** | 案件文书 | 自动提取 → 写入 clients 表 |

### 4.2 节假日更新流程

```
用户丢入"国务院办公厅关于2026年部分节假日安排的通知"
    ↓
AI 分类 → 节假日通知 (confidence: 0.9)
    ↓
AI 解析 → 提取每个假期的日期和调休工作日
    ↓
显示确认对话框：
┌─────────────────────────────────────────────┐
│  📅 检测到节假日通知                          │
│                                              │
│  解析结果：                                   │
│  元旦：1月1日-3日                             │
│  春节：1月28日-2月3日（调休：1月25日、2月7日）│
│  清明：4月4日-6日                             │
│  ...                                         │
│                                              │
│  [确认更新日历]  [取消]                       │
└─────────────────────────────────────────────┘
    ↓
更新 holidays.json → 期限引擎自动使用新数据
```

### 4.3 案由更新流程

```
用户丢入"最高人民法院关于修改《民事案件案由规定》的决定"
    ↓
AI 分类 → 案由更新 (confidence: 0.8)
    ↓
AI 解析 → 提取新增/修改/删除的案由
    ↓
显示确认对话框：
┌─────────────────────────────────────────────┐
│  📋 检测到案由规定更新                        │
│                                              │
│  新增案由：                                   │
│  · XXX纠纷（一级案由）                       │
│  · YYY纠纷（二级案由）                       │
│                                              │
│  修改案由：                                   │
│  · ZZZ纠纷 → 描述更新                        │
│                                              │
│  [确认更新]  [取消]                           │
└─────────────────────────────────────────────┘
    ↓
更新 cause_actions 表 → 字段推断自动使用新案由
```

### 4.4 法条更新流程

```
用户丢入《专利法实施细则》全文或修订条款
    ↓
AI 分类 → 法规文本 (confidence: 0.7)
    ↓
AI 解析 → 按条提取，记录法律名称、条号、内容
    ↓
写入 knowledge_items 表：
  title: "专利法实施细则第5条"
  category: "legal_provision"
  content: "期限的计算..."
  tags: ["专利法实施细则", "期限", "起算日"]
    ↓
可被 Skill 工作流和 Copilot 引用
```

---

## 五、知识库设计

参考 **LawRefBook**（中国法律 Markdown 数据库）和 **LawVault**（Tauri + SQLite + 向量搜索）。

### 5.1 知识库数据模型

```sql
-- 知识条目（统一存储法条、笔记、邮件、文档摘要）
CREATE TABLE IF NOT EXISTS knowledge_items (
  id            TEXT PRIMARY KEY,
  title         TEXT NOT NULL,
  category      TEXT NOT NULL
                CHECK(category IN (
                  'legal_provision',    -- 法条/法规
                  'case_note',          -- 案件笔记
                  'email',              -- 邮件记录
                  'document_summary',   -- 文档摘要
                  'holiday',            -- 节假日
                  'cause_action',       -- 案由
                  'court_name',         -- 法院名称
                  'judge_info',         -- 法官信息
                  'other'               -- 其他
                )),
  content       TEXT NOT NULL,
  tags          TEXT,                   -- JSON array of tags
  source_type   TEXT,                   -- inbox / manual / auto_extract
  source_id     TEXT,                   -- 来源收件箱项 ID
  linked_case_id TEXT REFERENCES cases(id) ON DELETE SET NULL,
  
  -- 法条专用字段
  law_name      TEXT,                   -- 法律名称（如"专利法实施细则"）
  article_no    TEXT,                   -- 条号（如"第5条"）
  effective_date TEXT,                  -- 生效日期
  status        TEXT DEFAULT 'current'  -- current / amended / repealed
  
  created_at    TEXT DEFAULT (datetime('now','localtime')),
  updated_at    TEXT DEFAULT (datetime('now','localtime'))
);

-- 知识条目版本（跟踪修改历史）
CREATE TABLE IF NOT EXISTS knowledge_versions (
  id            TEXT PRIMARY KEY,
  item_id       TEXT NOT NULL REFERENCES knowledge_items(id) ON DELETE CASCADE,
  content       TEXT NOT NULL,
  changed_at    TEXT DEFAULT (datetime('now','localtime')),
  change_reason TEXT
);

-- 知识条目关系
CREATE TABLE IF NOT EXISTS knowledge_relations (
  id            TEXT PRIMARY KEY,
  source_id     TEXT NOT NULL REFERENCES knowledge_items(id) ON DELETE CASCADE,
  target_id     TEXT NOT NULL REFERENCES knowledge_items(id) ON DELETE CASCADE,
  relation_type TEXT NOT NULL
                CHECK(relation_type IN (
                  'cites',           -- 引用
                  'amends',          -- 修订
                  'supersedes',      -- 替代
                  'implements',      -- 实施
                  'related'          -- 相关
                )),
  UNIQUE(source_id, target_id, relation_type)
);
```

### 5.2 法条存储格式

参考 **LawRefBook** 的 Markdown 结构：

```markdown
# 专利法实施细则

## 第五条

期限的计算，按照下列方式计算：

（一）期限以年或者月计算的，以其最后一日的届满日为期限届满日；

（二）期限以日计算的，以开始之日的次日为期限起算日；

（三）期限届满日是法定休假日的，以休假日后的第一个工作日为期限届满日。
```

在数据库中存储为：

```json
{
  "title": "专利法实施细则第5条",
  "law_name": "专利法实施细则",
  "article_no": "第5条",
  "content": "期限的计算...",
  "tags": ["期限", "起算日", "休假日顺延"],
  "status": "current",
  "effective_date": "2024-01-20"
}
```

### 5.3 搜索机制

参考 **LawVault** 的双引擎方案：

| 引擎 | 用途 | 实现 |
|------|------|------|
| **FTS5** | 关键词搜索 | SQLite 内置，已在 knowledge_fts 表 |
| **语义搜索** | 含义相似的查询 | 可选：本地向量模型（后续） |

```sql
-- FTS5 全文搜索
SELECT ki.* FROM knowledge_fts f
JOIN knowledge_items ki ON ki.rowid = f.rowid
WHERE knowledge_fts MATCH '期限 起算日'
ORDER BY rank
LIMIT 20;

-- 按类别筛选
SELECT * FROM knowledge_items
WHERE category = 'legal_provision'
AND law_name = '专利法实施细则'
ORDER BY article_no;
```

---

## 六、完整数据流

```
                        ┌─────────────────┐
                        │    大口袋入口     │
                        │                 │
                        │ · UI 拖拽       │
                        │ · 系统托盘      │
                        │ · 快捷键        │
                        │ · 悬浮窗        │
                        │ · 文件夹监听    │
                        │ · 邮件监听      │
                        └────────┬────────┘
                                 │
                                 ↓
                        ┌─────────────────┐
                        │   文本提取层     │
                        │                 │
                        │ · PDF → OCR     │
                        │ · Word → XML    │
                        │ · 图片 → OCR    │
                        │ · .eml → 解析   │
                        │ · 文本 → 直接   │
                        └────────┬────────┘
                                 │
                                 ↓
                        ┌─────────────────┐
                        │   AI 分类层      │
                        │                 │
                        │ · 正则规则匹配  │
                        │ · AI 分类（可选）│
                        │ · 置信度评分    │
                        └────────┬────────┘
                                 │
                ┌────────────────┼────────────────┐
                ↓                ↓                ↓
        ┌──────────┐    ┌──────────┐    ┌──────────┐
        │ 案件关联  │    │ 知识库   │    │ 系统更新  │
        │          │    │          │    │          │
        │ · 文件   │    │ · 法条   │    │ · 节假日  │
        │ · 信息   │    │ · 笔记   │    │ · 案由    │
        │ · 日志   │    │ · 邮件   │    │ · 法院    │
        └──────────┘    │ · 摘要   │    └──────────┘
                        └──────────┘
```

---

## 七、与现有模块的集成

| 模块 | 集成点 |
|------|--------|
| **案件管理** | 收件箱归档 → 创建 case_log + case_file |
| **日历** | 节假日通知 → 更新 holidays.json → 期限引擎 |
| **期限引擎** | 法条更新 → 可能影响期限计算规则 |
| **文书工坊** | 法条知识库 → Copilot 补全数据源 |
| **邮件记录** | 邮件 → email_records 表 + 知识库 |
| **字段推断** | 案由/法院更新 → 推断规则自动增强 |
