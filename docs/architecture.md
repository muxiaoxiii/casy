# Casy 顶层架构设计

> 版本: 1.0 | 日期: 2026-08-03
> 定位: Casy 整体架构设计，统领所有模块
> 关联: feishu-sync-design.md / inbox-v2-design.md / ui-design.md / Casy-SPEC.md

---

## 一、Casy 的定位

**Casy = 飞书多维表格的"律师专业版"。**

起点是多维表格（灵活的数据结构），但比多维表格更好用：

| 飞书多维表格的局限 | Casy 的解决方案 |
|-------------------|----------------|
| 按案件类型筛选字段很麻烦 | **动态字段系统**：按案由/审级自动显示/隐藏字段 |
| 不同类型案件无法统一筛选 | **跨类型筛选**：统一期限/开庭/状态维度 |
| 任务只有简单截止日期 | **任务系统**：四象限 + 庭审准备模板 + 强提醒 |
| 日历只是日期展示 | **期限引擎**：自动计算答辩期/上诉期/口审准备期 |
| 提醒只有飞书通知 | **多通道提醒**：本地/系统/飞书消息/飞书任务 |
| 没有知识库 | **知识库**：法条/判例/笔记统一存储，写作时检索 |
| 没有收件箱 | **大口袋**：文件/邮件/文本丢进来，自动判断+推荐+归档 |
| 公式只能在表格里看 | **公式引擎**：本地计算 + Copilot 文书引用 |

---

## 二、模块全景

```
┌─────────────────────────────────────────────────────────────┐
│                        Casy 桌面应用                          │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │                    数据层 (SQLite)                      │  │
│  │  cases | hearings | tasks | knowledge | officials      │  │
│  │  inbox_items | case_files | case_logs | calendar       │  │
│  │  feishu_connections | feishu_field_mappings | ...       │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │                    业务模块                             │  │
│  │                                                       │  │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐    │  │
│  │  │ 案件管理 │ │ 任务系统 │ │ 日历    │ │ 知识库  │    │  │
│  │  └─────────┘ └─────────┘ └─────────┘ └─────────┘    │  │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐    │  │
│  │  │ 收件箱   │ │ 文书工坊 │ │ 卷宗管理 │ │ 期限引擎│    │  │
│  │  └─────────┘ └─────────┘ └─────────┘ └─────────┘    │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │                   横切关注点                            │  │
│  │                                                       │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐   │  │
│  │  │ 多通道提醒   │  │ 动态字段    │  │ 跨类型筛选   │   │  │
│  │  │ (全局)      │  │ (全局)      │  │ (全局)      │   │  │
│  │  └─────────────┘  └─────────────┘  └─────────────┘   │  │
│  │  ┌─────────────┐  ┌─────────────┐                     │  │
│  │  │ 公式引擎    │  │ AI 后端     │                     │  │
│  │  │ (全局)      │  │ (全局)      │                     │  │
│  │  └─────────────┘  └─────────────┘                     │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │                   外部集成                              │  │
│  │                                                       │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐   │  │
│  │  │ 飞书同步    │  │ 飞书消息    │  │ 飞书任务     │   │  │
│  │  │ (Bitable)   │  │ (Bot API)   │  │ (Task API)  │   │  │
│  │  └─────────────┘  └─────────────┘  └─────────────┘   │  │
│  │  ┌─────────────┐  ┌─────────────┐                     │  │
│  │  │ WebDAV 同步  │  │ IMAP 邮件   │                     │  │
│  │  └─────────────┘  └─────────────┘                     │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

---

## 三、多通道提醒系统

**这是 Casy 的核心横切能力，不是飞书模块的一部分。**

### 3.1 设计目标

Casy 的提醒应该比飞书更强：
- 不依赖单一通道（飞书通知容易被淹没）
- 根据紧急程度选择通道（普通→飞书消息，紧急→本地弹窗+系统通知）
- 用户可配置每个提醒走哪个通道
- 支持飞书消息、飞书任务、本地弹窗、系统通知、（未来）邮件

### 3.2 架构

```
                    ┌─────────────────┐
                    │   提醒调度器      │
                    │  ReminderEngine  │
                    │  (全局单例)      │
                    └────────┬────────┘
                             │
         ┌───────────┬───────┼───────┬───────────┐
         ↓           ↓       ↓       ↓           ↓
   ┌──────────┐┌──────────┐┌──────────┐┌──────────┐
   │ 本地弹窗  ││ 系统通知  ││ 飞书消息  ││ 飞书任务  │
   │ Tauri    ││ macOS   ││ Bot API ││ Task API │
   │ dialog   ││ notify  ││ im/v1   ││ task/v2  │
   └──────────┘└──────────┘└──────────┘└──────────┘
        ↑           ↑           ↑           ↑
        │           │           │           │
        └───────────┴───────┬───┴───────────┘
                            │
         ┌──────────────────┼──────────────────┐
         │                  │                  │
    ┌────┴────┐      ┌─────┴─────┐     ┌─────┴─────┐
    │ 期限引擎 │      │ 任务系统   │     │ 日历系统   │
    │ deadline │      │ tasks     │     │ calendar  │
    └─────────┘      └───────────┘     └───────────┘
```

### 3.3 提醒规则

```sql
CREATE TABLE IF NOT EXISTS reminder_rules (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    trigger_type    TEXT NOT NULL
                    CHECK(trigger_type IN (
                        'deadline_before',      -- 期限前 N 天
                        'deadline_on',          -- 期限当天
                        'deadline_after',       -- 逾期
                        'hearing_before',       -- 开庭前 N 天
                        'task_due',             -- 任务到期
                        'task_overdue'          -- 任务逾期
                    )),
    trigger_value   INTEGER,                    -- N 天
    channels        TEXT NOT NULL,              -- JSON: ["local","system","feishu_message","feishu_task"]
    message_template TEXT,                      -- 消息模板
    case_types      TEXT,                       -- JSON: 适用案件类型（null=全部）
    enabled         INTEGER DEFAULT 1,
    created_at      TEXT DEFAULT (datetime('now','localtime'))
);
```

### 3.4 默认规则

```
普通提醒（7 天前）：
  通道: 飞书消息
  "答辩期限临近，剩余 7 天"

紧急提醒（3 天前）：
  通道: 飞书消息 + 飞书任务
  "紧急：答辩期限 3 天后"

超紧急（当天）：
  通道: 本地弹窗 + 系统通知 + 飞书消息 + 飞书任务
  "今天是答辩截止日！"

逾期：
  通道: 本地弹窗 + 系统通知 + 飞书消息
  "答辩期限已逾期 N 天"
```

### 3.5 通道实现

| 通道 | 实现 | 说明 |
|------|------|------|
| `local` | Tauri dialog API | 应用内弹窗，最可靠 |
| `system` | macOS Notification | 系统通知栏，应用关闭也能收到 |
| `feishu_message` | POST /im/v1/messages | 飞书 Bot 消息卡片 |
| `feishu_task` | POST /task/v2/tasks | 飞书任务（含截止日期和负责人）|
| `email` | SMTP（未来）| 邮件提醒（暂不实现）|

---

## 四、动态字段系统

**这也是全局能力，影响案件详情页和所有表单。**

### 4.1 问题

不同案件类型需要不同的字段：
- 专利无效：请求人补充意见期限、专利权人陈述意见期限
- 专利侵权：管辖异议、诉讼程序、收到起诉状时间
- 专利行政：行政决定类型、复议信息

飞书多维表格中这些字段都挤在一张表里，很多字段对某些案件是空的。

### 4.2 设计

```sql
-- 字段分组（按案件类型）
CREATE TABLE IF NOT EXISTS field_groups (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,              -- "专利无效专用" / "侵权专用"
    case_types      TEXT,                       -- JSON: ["专利无效"]
    sort_order      INTEGER DEFAULT 0
);

-- 字段与分组关联
CREATE TABLE IF NOT EXISTS field_group_items (
    id              TEXT PRIMARY KEY,
    group_id        TEXT NOT NULL REFERENCES field_groups(id) ON DELETE CASCADE,
    column_name     TEXT NOT NULL,              -- 本地列名
    label           TEXT NOT NULL,              -- 显示标签
    field_type      TEXT NOT NULL,              -- text/number/date/select/textarea
    options         TEXT,                       -- JSON: 选项列表
    required        INTEGER DEFAULT 0,
    sort_order      INTEGER DEFAULT 0,
    UNIQUE(group_id, column_name)
);
```

### 4.3 UI 行为

```
案件详情页：
  读取案件的"案由"字段
    ↓
  加载对应的 field_group
    ↓
  显示：通用字段 + 案由专用字段
  隐藏：不适用于当前案由的字段
    ↓
  用户也可手动展开/收起字段组
```

---

## 五、跨类型筛选

### 5.1 问题

在案件列表中，想看"所有案件的下一个期限"：
- 专利无效：请求人补充意见期限
- 专利侵权：提交答辩状期间
- 专利行政：救济期限

这三个字段名不同，但语义相同（都是"下一个重要期限"）。

### 5.2 设计

```sql
-- 统一筛选视图：将不同类型的字段映射到统一维度
CREATE VIEW IF NOT EXISTS v_case_unified AS
SELECT
    c.id,
    c.case_title,
    c.case_no,
    c.client_name,
    c.case_type,
    c.formula_case_status AS status,
    c.court,
    c.trial_date,

    -- 统一期限：取每种类型对应的期限字段
    COALESCE(
        CASE WHEN c.case_type LIKE '%无效%' THEN c.formula_petitioner_supp END,
        CASE WHEN c.case_type LIKE '%侵权%' THEN c.formula_defense_deadline END,
        CASE WHEN c.case_type LIKE '%行政%' THEN c.relief_deadline END,
        c.formula_estimated_trial_limit
    ) AS next_deadline,

    -- 统一办案人
    c.operator

FROM cases c;
```

### 5.3 UI 行为

```
案件列表页 → 筛选栏：

[全部类型 ▼] [全部状态 ▼] [全部审理机关 ▼]
期限: [未来 7 天 ▼]  开庭: [未来 30 天 ▼]
办案人: [全部 ▼]

结果：
  案件        类型    状态    下一期限    开庭日期
  浦项 v NSC  无效    进行中  8/15       8/20
  钛金 v 高德  侵权    进行中  8/10       —
  隆基 v XX   行政    进行中  8/25       9/01
```

---

## 六、任务系统

### 6.1 现有能力

- CRUD：创建/读取/更新/删除任务
- 四象限：紧急重要/重要不紧急/紧急不重要/不紧急不重要
- 庭审准备任务：传票/口审通知 → 自动生成 5 个准备任务
- 截止日期 + 完成状态

### 6.2 需要增强

| 能力 | 当前 | 目标 |
|------|------|------|
| 本地任务 | ✅ | ✅ |
| 飞书任务同步 | ❌ | ✅ 创建/更新/完成双向同步 |
| 任务提醒 | ❌ | ✅ 到期前 N 天提醒（多通道）|
| 任务模板 | ⚠️ 仅庭审 | ✅ 可自定义任务模板 |
| 任务关联 | ⚠️ 仅案件 | ✅ 关联案件+日历+知识库 |
| 重复任务 | ❌ | ✅ 每周/每月重复 |

### 6.3 任务模板系统

```sql
CREATE TABLE IF NOT EXISTS task_templates (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,              -- 模板名称
    trigger_type    TEXT,                       -- 触发类型：summons/hearing/manual
    tasks_json      TEXT NOT NULL,              -- JSON: [{title, description, days_before}]
    case_types      TEXT,                       -- JSON: 适用案件类型
    enabled         INTEGER DEFAULT 1,
    created_at      TEXT DEFAULT (datetime('now','localtime'))
);
```

---

## 七、日历系统

### 7.1 现有能力

- 月视图：显示开庭/口审日期
- 法定节假日：内置 2026 年数据 + 支持更新
- 期限引擎：自动计算期限并显示

### 7.2 需要增强

| 能力 | 当前 | 目标 |
|------|------|------|
| 月视图 | ✅ | ✅ |
| 法定节假日 | ✅ | ✅ |
| 期限计算 | ✅ | ✅ |
| 日历事件提醒 | ❌ | ✅ 多通道提醒 |
| 飞书日历同步 | ❌ | ✅ 开庭日期同步到飞书日历 |
| 周视图 | ❌ | ✅ |
| 日视图 | ❌ | ✅ |
| 拖拽修改日期 | ❌ | ✅ |

---

## 八、飞书集成（从属关系）

飞书是 Casy 的**外部集成通道之一**，不是核心模块。

### 8.1 飞书的角色

```
飞书在 Casy 中的 3 个角色：

1. 数据同步通道（Bitable API）
   - 连接任意飞书多维表格
   - 字段映射 + 比较 + 导入 + 双向同步
   - 详见: feishu-sync-design.md §二~§七

2. 提醒通道之一（Bot API）
   - 飞书消息推送（期限提醒/开庭提醒）
   - 飞书任务创建（庭审准备任务）
   - 由 ReminderEngine 统一调度
   - 详见: 本文档 §三

3. 团队协作平台
   - 飞书端查看/编辑案件数据
   - Casy 端做专业操作（期限计算/文书生成/知识检索）
   - 两边数据双向同步
```

### 8.2 飞书 API 清单

```
已使用：
  ✅ POST /auth/v3/tenant_access_token/internal  — Token
  ✅ GET  /bitable/v1/apps/{app}/tables           — 列出表
  ✅ GET  /bitable/v1/apps/{app}/tables/{t}/fields — 列出字段
  ✅ GET  /bitable/v1/apps/{app}/tables/{t}/records — 列出记录
  ✅ POST /bitable/v1/apps/{app}/tables/{t}/records — 创建记录
  ✅ PUT  /bitable/v1/apps/{app}/tables/{t}/records/{r} — 更新记录

需要新增（提醒通道）：
  ❌ POST /im/v1/messages?receive_id_type=...     — 发送消息
  ❌ POST /task/v2/tasks                          — 创建任务
  ❌ PUT  /task/v2/tasks/{task_id}                — 更新任务

需要新增（数据同步）：
  ❌ GET  /drive/v1/medias/{file_token}/download  — 下载附件
  ❌ POST /drive/v1/medias/upload_all             — 上传附件
```

---

## 九、模块依赖关系

```
提醒系统（全局）
    ↑
    ├── 期限引擎 → 计算期限 → 触发提醒
    ├── 任务系统 → 任务到期 → 触发提醒
    ├── 日历系统 → 开庭日期 → 触发提醒
    └── Inbox → 收到传票 → 创建任务 + 触发提醒

动态字段系统（全局）
    ↑
    ├── 案件详情页 → 根据案由显示字段
    ├── 案件列表页 → 根据类型筛选字段
    └── 飞书同步 → 按字段分组映射

公式引擎（全局）
    ↑
    ├── 期限引擎 → 计算答辩期/上诉期/审限
    ├── 飞书同步 → 本地执行飞书公式
    ├── 知识库 → Copilot 检索
    └── 文书工坊 → AI 写作辅助

收件箱（全局）
    ↑
    ├── 文件/邮件/文本 → 分类 → 归档
    ├── 归档 → 创建任务
    ├── 归档 → 触发期限计算
    └── 归档 → 飞书同步推送
```

---

## 十、实施路线图

### Phase A：提醒系统 + 任务增强（P0，5 天）
- ReminderEngine 核心（规则引擎 + 调度循环）
- 本地弹窗 + 系统通知通道
- 飞书消息推送通道
- 飞书任务创建通道
- 提醒规则配置 UI
- 任务模板系统

### Phase B：动态字段 + 跨类型筛选（P1，4 天）
- field_groups / field_group_items 表
- 预设字段分组（无效/侵权/行政）
- 案件详情页字段动态显示
- v_case_unified 视图
- 案件列表跨类型筛选

### Phase C：公式缓存 + 链接同步（P1，3 天）
- 公式引擎 → formula_ 缓存列写入
- DuplexLink 双向同步
- SingleLink 同步

### Phase D：附件同步 + 冲突解决（P2，4 天）
- 飞书附件下载
- 本地文件上传
- 冲突检测 + 解决 UI

### Phase E：日历增强 + 测试（P2，3 天）
- 周视图 / 日视图
- 拖拽修改日期
- 飞书日历同步
- 全面测试

**总计：~19 天**

---

> 最后更新: 2026-08-03
