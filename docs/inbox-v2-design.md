# Casy 收件箱 + 卷宗管理 设计文档 v2.0

> 版本: 2.0 | 日期: 2026-08-01
> 替代: docs/inbox-system-design.md（v1.0）
> 状态: 设计评审中

---

## 一、设计理念变更

### v1.0 的问题

v1.0 设计的收件箱是"自动处理"模式：文件丢进来 → AI 自动分类 → 自动路由到案件/知识库。

**实际问题**：
1. AI 分类有延迟（1-5秒），用户等待体验差
2. 自动路由可能出错，用户需要撤销
3. 大文件无法快速处理
4. 识别不到的文件没有兜底方案

### v2.0 核心原则

**"快判断、先推荐、确认后执行"**

```
一切进入收件箱的内容
    ↓
即时判断（0ms，纯本地逻辑）
    ↓
推荐操作面板（显示给用户）
    ↓
用户确认 → 执行
用户修改 → 按用户意图执行
用户忽略 → 保留在收件箱
```

**关键设计约束**：
- 第一层判断（文件名/类型/大小）必须在 50ms 内完成
- AI 分析是可选的增强，用户主动触发
- 大文件（>5MB）默认不调 AI，仅用本地信息判断
- 任何自动操作都需要用户确认
- 文件拷贝必须有进度条 + 完整性校验

---

## 二、收件箱架构

### 2.1 三层判断模型

```
┌─────────────────────────────────────────────────────────┐
│                    第一层：即时判断                        │
│                    (0ms, 纯本地逻辑)                      │
│                                                         │
│  输入：文件名、扩展名、文件大小、MIME 类型                │
│  输出：推荐操作 + 匹配案件 + 置信度                      │
│  方法：auto_classify() + 正则匹配 + 案号/当事人搜索      │
└──────────────────────────┬──────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────┐
│                    第二层：AI 增强（可选）                 │
│                    (1-5s, 用户主动触发)                   │
│                                                         │
│  输入：提取的文本内容（PDF/Word/OCR）                    │
│  输出：文书类型 + 结构化信息 + 案件匹配                  │
│  方法：classify_document_with_prompt()                   │
│  条件：文件 < 5MB 且用户点击 [AI 分析]                   │
└──────────────────────────┬──────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────┐
│                    第三层：用户确认                        │
│                    (等用户操作)                           │
│                                                         │
│  推荐操作面板 → 用户确认/修改/忽略                       │
│  确认后执行归档/入库/创建任务等动作                       │
└─────────────────────────────────────────────────────────┘
```

### 2.2 即时判断逻辑（第一层）

```rust
fn quick_judge(file_name: &str, file_size: u64, mime_type: &str) -> QuickJudgeResult {
    let mut recommendations = Vec::new();
    let mut confidence = 0.0;
    
    // 1. 文件名关键词匹配
    let category = auto_classify(file_name);
    if category != "other" {
        confidence += 0.6;
    }
    
    // 2. 案号提取（正则）
    let case_no = extract_case_no_from_filename(file_name);
    if let Some(ref cn) = case_no {
        confidence += 0.3;
        // 查数据库匹配案件
        if let Some(case) = find_case_by_no(cn) {
            recommendations.push(Recommendation {
                action: "file_to_case",
                target_case: Some(case),
                target_folder: Some(category_to_folder(category)),
                reason: format!("文件名包含案号 {}", cn),
            });
        }
    }
    
    // 3. 当事人名提取
    let parties = extract_parties_from_filename(file_name);
    for party in &parties {
        if let Some(cases) = find_cases_by_party(party) {
            for case in cases {
                recommendations.push(Recommendation {
                    action: "file_to_case",
                    target_case: Some(case),
                    target_folder: Some(category_to_folder(category)),
                    reason: format!("文件名包含当事人 {}", party),
                });
            }
        }
    }
    
    // 4. 文件大小判断
    let size_strategy = match file_size {
        0..=5_242_880 => SizeStrategy::FullAnalysis,      // < 5MB: 可调 AI
        5_242_881..=52_428_800 => SizeStrategy::QuickOnly, // 5-50MB: 仅本地判断
        _ => SizeStrategy::ManualOnly,                     // > 50MB: 手动处理
    };
    
    // 5. 特殊文件类型
    if mime_type == "message/rfc822" || file_name.ends_with(".eml") {
        recommendations.push(Recommendation {
            action: "parse_email",
            target_case: None,
            target_folder: None,
            reason: "邮件文件，建议解析后归档".into(),
        });
    }
    
    QuickJudgeResult {
        category,
        confidence,
        recommendations,
        size_strategy,
        ai_available: size_strategy == SizeStrategy::FullAnalysis,
    }
}
```

### 2.3 AI 增强逻辑（第二层）

仅在以下条件满足时可用：
- 文件 < 5MB
- 用户主动点击 [AI 分析] 按钮
- AI 后端已配置

```
AI 分析流程：
1. 提取文本（PDF/Word 直接提取，图片 OCR）
2. 调用 classify_document_with_prompt()
3. 调用 extract_info_with_prompt()
4. 返回结构化结果

AI 分析结果：
- 文书类型（11 种）
- 置信度（0-1）
- 提取字段（案号/当事人/法院/日期/专利号）
- 案件匹配建议
- 附加操作建议（创建任务/更新期限/入库知识库）
```

### 2.4 推荐操作面板（第三层）

```
┌─────────────────────────────────────────────────────────┐
│ 📄 文件: (2024)国知局无效第xxx号_浦项_传票.pdf          │
│    大小: 2.3MB | 类型: PDF | 2026-08-01                 │
├─────────────────────────────────────────────────────────┤
│                                                         │
│ ⚡ 快速判断 (0ms)                                       │
│    识别类型: 传票（文件名匹配）                          │
│    匹配案件: 浦项 v NSC — (2024)国知局无效第xxx号       │
│    置信度: ●●●●○ 0.85                                   │
│                                                         │
│ 📋 推荐操作                                              │
│    ┌─────────────────────────────────────────────────┐  │
│    │ ● 归档到"浦项 v NSC"的 01_传票 目录（推荐）    │  │
│    │ ○ 归档到其他案件...                             │  │
│    │ ○ 仅存入收件箱，稍后处理                        │  │
│    │ ○ 忽略                                          │  │
│    └─────────────────────────────────────────────────┘  │
│                                                         │
│ [🤖 AI 深度分析]  ← 可选，点击后提取更多字段            │
│                                                         │
│                     [确认执行]  [取消]                   │
└─────────────────────────────────────────────────────────┘
```

### 2.5 大文件策略

| 文件大小 | 策略 | 说明 |
|----------|------|------|
| < 5MB | 完整分析 | 可提取文本、可调 AI |
| 5-50MB | 快速判断 | 仅用文件名/扩展名，不提取文本 |
| > 50MB | 手动处理 | 提示"文件较大，请手动选择操作" |

```
大文件推荐面板：
┌─────────────────────────────────────────────────────────┐
│ 📄 文件: 大型案卷扫描件.pdf (120MB)                     │
│                                                         │
│ ⚠️ 文件较大，无法自动分析内容                           │
│                                                         │
│ 请选择操作：                                             │
│ ○ 归档到某个案件 → 选择案件和目录                       │
│ ○ 存入收件箱，稍后处理                                  │
│ ○ 忽略                                                  │
│                                                         │
│                     [确认]  [取消]                       │
└─────────────────────────────────────────────────────────┘
```

### 2.6 无法识别时的兜底

```
┌─────────────────────────────────────────────────────────┐
│ 📄 文件: IMG_20260730_142356.jpg (4.5MB)                │
│                                                         │
│ ⚠️ 无法从文件名判断内容                                 │
│                                                         │
│ 请选择操作：                                             │
│ ○ 归档到某个案件 → 选择案件和目录                       │
│ ○ 存入知识库                                            │
│ ○ 仅存入收件箱                                          │
│ ○ 忽略                                                  │
│                                                         │
│ [🤖 AI 分析（需要 OCR，约 3-5 秒）]                     │
│                                                         │
│                     [确认]  [取消]                       │
└─────────────────────────────────────────────────────────┘
```

---

## 三、卷宗管理模块

### 3.1 设计目标

卷宗管理是案件文件的**结构化存储系统**：
- 每个案件一个文件夹，含 7 个标准子目录
- 文件名自动规范化（智能重命名）
- 非 inbox 来源的文件也能智能重命名
- 大文件拷贝有进度条 + 完整性校验

### 3.2 文件夹结构

```
~/Documents/Casy/cases/
├── 001_浦项_NSC/                    ← 案件文件夹（自动编号）
│   ├── 01_传票/                     ← 7 个标准子目录
│   ├── 02_证据/
│   ├── 03_交文/                     ← 我方提交的文书
│   ├── 04_收文/                     ← 收到的文书（判决/裁定等）
│   ├── 05_内部/                     ← 内部工作文件
│   ├── 06_通信/                     ← 往来函件
│   └── 07_其他/
├── 002_钛金_高德/
│   └── ...
└── inbox/                           ← 收件箱暂存目录
```

**编号规则**：
- 格式：`{序号}_{客户简称}_{对方简称}`
- 序号：三位数，按创建时间递增（001, 002, 003...）
- 客户/对方简称：取前 4 个字符，去除特殊字符
- 示例：`001_浦项_NSC`、`002_钛金_高德`、`003_隆基_某公司`

### 3.3 文件命名规则（可配置）

在设置页提供三种命名模板选择：

#### 模板 A：四段式（默认）

```
{案号}_{当事人}_{处理人}_{日期}.{ext}

示例：
(2024)国知局无效第xxx号_浦项_张鑫_20260815.pdf
(2024)京73民初xxx号_钛金_李四_20260801.docx

规则：
- 案号：原始案号，去除空格
- 当事人：我方客户名称，取前 8 字符
- 处理人：当前登录用户名称
- 日期：YYYYMMDD 格式
- 超长自动截断，特殊字符自动替换为下划线
```

#### 模板 B：五段式

```
{案号}_{当事人}_{处理人}_{日期}_{分类}.{ext}

示例：
(2024)国知局无效第xxx号_浦项_张鑫_20260815_传票.pdf

比四段式多一个分类字段（传票/证据/交文/收文/内部/通信/其他）
```

#### 模板 C：简化式

```
{日期}_{分类}_{案号简写}_{hash}.{ext}

示例：
2026-08-15_传票_2024guozhiju_a1b2c3d4.pdf

适合不关心详细信息的场景，hash 防覆盖
```

#### 配置存储

```sql
-- 在 settings 表中
INSERT INTO settings (key, value) VALUES ('file_naming_template', 'four_segment');
INSERT INTO settings (key, value) VALUES ('file_naming_user_name', '张鑫');
```

### 3.4 智能重命名（非 inbox 来源）

适用于用户直接操作文件系统的场景（不经过收件箱）。

```
触发方式：
1. 右键案件文件夹中的文件 → "智能重命名"
2. 拖入文件到案件详情页的文件区域 → 自动重命名
3. 批量操作：选中多个文件 → "批量智能重命名"

处理流程：
1. 读取文件内容（PDF/Word 提取文本，图片跳过）
2. AI 提取：案号、当事人、日期、文书类型
3. 按配置的命名模板生成新文件名
4. 预览新旧文件名对比
5. 用户确认 → 重命名
6. 写入 case_files 表记录

批量重命名预览：
┌─────────────────────────────────────────────────────────┐
│ 批量智能重命名                                           │
├─────────────────────────────────────────────────────────┤
│ 原文件名                          → 新文件名            │
│ scan001.pdf                       → (2024)xxx_浦项_张鑫_│
│                                     20260815_传票.pdf   │
│ IMG_20260730.jpg                  → (2024)xxx_浦项_张鑫_│
│                                     20260730_证据.jpg   │
│ doc.pdf                           → ⚠️ 无法识别，跳过   │
├─────────────────────────────────────────────────────────┤
│                           [全部确认]  [逐个确认]  [取消] │
└─────────────────────────────────────────────────────────┘
```

### 3.5 安全拷贝（大文件 + 进度条）

```
拷贝流程：

1. 获取源文件元数据
   - 文件大小
   - 修改时间
   - MIME 类型

2. 创建目标路径
   - 目标目录：{案件文件夹}/{子目录}/
   - 目标文件名：按命名规则生成
   - 临时文件：{目标文件名}.tmp

3. 分块拷贝（带进度）
   - 块大小：64KB（小文件）/ 1MB（大文件）
   - 每块写入后更新进度
   - 进度通过 Tauri event 发送到前端

4. 完整性校验
   - 计算源文件 SHA-256
   - 计算目标文件 SHA-256
   - 比对校验和

5. 原子重命名
   - .tmp → 正式文件名
   - 写入 case_files 表记录

6. 错误处理
   - 拷贝中断 → 删除 .tmp 文件
   - 校验失败 → 删除 .tmp 文件，提示重试
   - 磁盘空间不足 → 提前检查，不足则提示
```

**进度条 UI**：

```
┌─────────────────────────────────────────────────────────┐
│ 正在拷贝文件...                                          │
│                                                         │
│ 📄 (2024)国知局无效第xxx号_浦项_传票.pdf                 │
│ ████████████████████░░░░░░░░░░  67%  1.5MB/2.3MB       │
│ 速度: 12MB/s | 预计剩余: < 1秒                          │
│                                                         │
│                              [取消]                      │
└─────────────────────────────────────────────────────────┘
```

**后端实现**：

```rust
#[tauri::command]
pub async fn copy_file_with_progress(
    source_path: String,
    target_case_id: String,
    target_category: String,
    naming_template: Option<String>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let source = Path::new(&source_path);
    let file_size = std::fs::metadata(source)
        .map_err(|e| format!("无法读取文件: {}", e))?
        .len();
    
    // 检查磁盘空间
    let target_dir = get_case_folder(&target_case_id, &target_category)?;
    if !has_enough_space(&target_dir, file_size) {
        return Err("磁盘空间不足".into());
    }
    
    // 生成目标文件名
    let case = db::cases::get_case(&conn, &target_case_id)?;
    let new_name = generate_filename(&source, &case, &target_category, naming_template);
    let target = target_dir.join(&new_name);
    let tmp_target = target.with_extension(format!("{}.tmp", 
        target.extension().and_then(|e| e.to_str()).unwrap_or("")));
    
    // 分块拷贝（带进度）
    let mut src_file = std::fs::File::open(source)?;
    let mut dst_file = std::fs::File::create(&tmp_target)?;
    let mut hasher = Sha256::new();
    let mut copied: u64 = 0;
    let block_size = if file_size > 10_000_000 { 1_048_576 } else { 65_536 };
    let mut buffer = vec![0u8; block_size];
    
    loop {
        let bytes_read = src_file.read(&mut buffer)?;
        if bytes_read == 0 { break; }
        
        dst_file.write_all(&buffer[..bytes_read])?;
        hasher.update(&buffer[..bytes_read]);
        copied += bytes_read as u64;
        
        // 发送进度事件
        let _ = app.emit("file-copy-progress", serde_json::json!({
            "copied": copied,
            "total": file_size,
            "percent": (copied * 100 / file_size) as u32,
        }));
    }
    
    dst_file.flush()?;
    drop(dst_file);
    
    // SHA-256 校验
    let src_hash = compute_sha256(source)?;
    let dst_hash = compute_sha256(&tmp_target)?;
    if src_hash != dst_hash {
        std::fs::remove_file(&tmp_target)?;
        return Err("文件校验失败，请重试".into());
    }
    
    // 原子重命名
    std::fs::rename(&tmp_target, &target)?;
    
    // 写入数据库
    let file_id = db::new_id();
    conn.execute(
        "INSERT INTO case_files (id, case_id, file_name, file_path, category, source_type, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'manual', ?6)",
        params![file_id, target_case_id, new_name, target.display(), target_category, db::now_local()],
    )?;
    
    Ok(file_id)
}
```

---

## 四、收件箱处理流程（完整）

### 4.1 文本/剪贴板进入

```
用户粘贴文本 / Cmd+Shift+V
    ↓
即时判断（正则匹配关键词）
    ├─ 匹配到"传票/口审/判决/起诉状" → 推荐：关联案件 + 执行动作
    ├─ 匹配到"放假/假期" → 推荐：更新日历
    ├─ 匹配到"第X条/法条" → 推荐：存入知识库
    ├─ 匹配到案号 → 推荐：关联到该案件
    └─ 无匹配 → 推荐：存为笔记 / AI 分析
    ↓
显示推荐面板 → 用户确认 → 执行
```

### 4.2 文件进入（拖拽/托盘/文件夹监听）

```
文件进入收件箱
    ↓
即时判断（文件名 + 扩展名 + 大小）
    ↓
┌─ 文件名含案件关键词 + 匹配到案件
│  → 推荐：归档到该案件的对应子目录
│
├─ 文件名含案号，匹配到案件
│  → 推荐：归档到该案件
│
├─ 文件名含当事人名，匹配到案件
│  → 推荐：归档到该案件
│
├─ .eml 文件
│  → 推荐：解析邮件 → 存入知识库 + 关联案件
│
├─ 文件名无法识别
│  → 推荐：手动选择操作 + [AI 分析] 按钮
│
└─ 大文件（>50MB）
   → 推荐：手动选择操作（不提供 AI 分析）
    ↓
显示推荐面板 → 用户确认 → 安全拷贝（带进度条 + SHA-256 校验）
```

### 4.3 邮件进入（IMAP 监听）

```
新邮件到达
    ↓
邮件白名单过滤
    ├─ 来自发件人白名单 → 推荐：解析 + 归档
    └─ 不在白名单 → 推荐：忽略 / 手动处理
    ↓
解析邮件正文 + 附件
    ↓
对每个附件分别走文件进入流程
对正文走文本进入流程
```

---

## 五、数据库变更

### 5.1 新增表：file_naming_rules

```sql
CREATE TABLE IF NOT EXISTS file_naming_rules (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,           -- 规则名称
    template    TEXT NOT NULL,           -- 模板代码: four_segment / five_segment / simple
    pattern     TEXT NOT NULL,           -- 文件名模式
    is_default  INTEGER DEFAULT 0,       -- 是否默认规则
    created_at  TEXT DEFAULT (datetime('now','localtime')),
    updated_at  TEXT DEFAULT (datetime('now','localtime'))
);

-- 默认规则
INSERT INTO file_naming_rules (id, name, template, pattern, is_default)
VALUES ('rule-default', '四段式', 'four_segment', '{case_no}_{client}_{user}_{date}', 1);
```

### 5.2 新增表：inbox_recommendations

```sql
CREATE TABLE IF NOT EXISTS inbox_recommendations (
    id              TEXT PRIMARY KEY,
    inbox_item_id   TEXT NOT NULL REFERENCES inbox_items(id) ON DELETE CASCADE,
    action          TEXT NOT NULL,       -- file_to_case / parse_email / save_to_knowledge / create_task
    target_case_id  TEXT,
    target_folder   TEXT,
    reason          TEXT,                -- 推荐理由
    confidence      REAL,
    accepted        INTEGER,             -- NULL=待确认, 1=接受, 0=拒绝
    created_at      TEXT DEFAULT (datetime('now','localtime'))
);
```

### 5.3 inbox_items 表新增字段

```sql
ALTER TABLE inbox_items ADD COLUMN quick_category TEXT;      -- 即时判断的分类
ALTER TABLE inbox_items ADD COLUMN quick_confidence REAL;    -- 即时判断的置信度
ALTER TABLE inbox_items ADD COLUMN ai_analyzed INTEGER DEFAULT 0; -- 是否已调 AI 分析
ALTER TABLE inbox_items ADD COLUMN copy_progress INTEGER;    -- 拷贝进度 0-100
ALTER TABLE inbox_items ADD COLUMN file_hash TEXT;           -- SHA-256 校验和
```

---

## 六、前端交互设计

### 6.1 收件箱主页面

```
┌─────────────────────────────────────────────────────────┐
│  收件箱                           [全部] [待处理] [已归档]│
├─────────────────────────────────────────────────────────┤
│                                                         │
│  ┌──────────────────────────────────────────────────┐   │
│  │ 📄 (2024)xxx_浦项_传票.pdf         ⚡ 快速判断    │   │
│  │    2.3MB | 传票 | 匹配: 浦项 v NSC               │   │
│  │    [确认归档] [AI分析] [忽略]                     │   │
│  └──────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────┐   │
│  │ 📄 IMG_20260730.jpg                ⚠️ 无法识别    │   │
│  │    4.5MB | 图片                                   │   │
│  │    [选择案件] [AI分析] [忽略]                     │   │
│  └──────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────┐   │
│  │ 📝 粘贴的文本                      ⚡ 法条        │   │
│  │    "根据《专利法》第65条..."                      │   │
│  │    [存入知识库] [关联案件] [忽略]                 │   │
│  └──────────────────────────────────────────────────┘   │
│                                                         │
│  ─────────── 拖拽区域 ───────────                       │
│  拖入文件或粘贴文本到此处                                │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

### 6.2 推荐确认弹窗（归档操作）

```
┌─────────────────────────────────────────────────────────┐
│  确认归档                                                │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  📄 (2024)国知局无效第xxx号_浦项_传票.pdf                │
│                                                         │
│  归档到：                                                │
│  案件: [浦项 v NSC (2024)国知局无效第xxx号) ▼]          │
│  目录: [01_传票 ▼]                                      │
│  文件名: [(2024)xxx_浦项_张鑫_20260815.pdf        ]     │
│          ↑ 可编辑                                        │
│                                                         │
│  附加操作：                                              │
│  ☑ 创建庭审准备任务（如有开庭日期）                      │
│  ☑ 记录办案日志                                          │
│  ☐ 更新案件开庭日期                                      │
│                                                         │
│                     [确认归档]  [取消]                   │
└─────────────────────────────────────────────────────────┘
```

### 6.3 拷贝进度条

```
┌─────────────────────────────────────────────────────────┐
│  正在拷贝文件...                                          │
│                                                         │
│  📄 (2024)xxx_浦项_传票.pdf                              │
│  ████████████████████░░░░░░░░░░  67%  1.5MB / 2.3MB    │
│  速度: 12 MB/s | 剩余: < 1 秒                           │
│                                                         │
│                              [取消]                      │
└─────────────────────────────────────────────────────────┘
```

---

## 七、配置项

### 7.1 设置页新增配置

```
收件箱设置：
├─ 文件命名规则
│  ○ 四段式：案号_当事人_处理人_日期
│  ○ 五段式：案号_当事人_处理人_日期_分类
│  ○ 简化式：日期_分类_案号_hash
│  处理人名称: [张鑫]
│
├─ AI 分析
│  ☑ 启用 AI 深度分析（可选按钮）
│  自动分析阈值: [< 5 MB]
│
├─ 文件夹监听
│  ☑ 启用
│  监听目录: [~/Documents/Casy/inbox/]
│
└─ 安全拷贝
   ☑ 启用 SHA-256 校验
   ☑ 大文件显示进度条
```

---

## 八、与现有模块的集成

| 模块 | 集成方式 |
|------|----------|
| 案件管理 | 归档 → 创建 case_log + case_file |
| 知识库 | 法条/笔记/邮件 → knowledge_items |
| 任务管理 | 传票/口审 → 创建庭审准备任务 |
| 期限引擎 | 判决/起诉状 → 更新日期字段 → 重算期限 |
| 日历 | 节假日通知 → 解析日期 → 提示更新 |
| 文书工坊 | 知识库内容 → Copilot 检索源 |
| 卷宗管理 | 文件拷贝 → 7 子目录分类 + 智能重命名 |

---

## 九、实施计划

| 序号 | 任务 | 工作量 | 依赖 |
|------|------|--------|------|
| 1 | 即时判断逻辑（auto_classify + 案号/当事人匹配）| 1 天 | 无 |
| 2 | 推荐操作面板前端 | 1 天 | 1 |
| 3 | 安全拷贝 + 进度条 + SHA-256 | 1 天 | 无 |
| 4 | 文件命名规则配置 | 0.5 天 | 无 |
| 5 | 智能重命名（非 inbox）| 0.5 天 | 4 |
| 6 | 大文件策略 | 0.5 天 | 3 |
| 7 | 数据库变更 + 迁移 | 0.5 天 | 无 |
| 8 | 集成测试 + 文档更新 | 0.5 天 | 1-7 |
| **总计** | | **5.5 天** | |

---

> 最后更新：2026-08-01
