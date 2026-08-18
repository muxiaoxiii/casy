# Casy 飞书生态设计文档 v4.0

> 版本: 4.0 | 日期: 2026-08-03
> 替代: feishu-sync-design.md v3.0
> 状态: 设计评审中

---

## 一、核心定位

### Casy 是什么

**Casy = 飞书多维表格的"律师专业版"。**

飞书多维表格是通用数据容器，Casy 在它的基础上解决律师的痛点：

| 飞书多维表格的局限 | Casy 的解决方案 |
|-------------------|----------------|
| 按案件类型筛选字段很麻烦（无效/侵权/行政的字段不同）| **动态字段系统**：按案由/审级自动显示/隐藏字段 |
| 任务系统弱，只有简单的截止日期 | **四象限任务管理** + **庭审准备任务模板** + **强提醒** |
| 日历只是日期展示，没有法律期限计算 | **期限引擎**：自动计算答辩期/上诉期/口审准备期 |
| 提醒只有飞书通知，容易错过 | **多通道提醒**：本地弹窗 + 飞书消息 + 飞书任务 + 系统通知 |
| 公式只能在表格里看，不能关联到文书 | **公式引擎本地计算** + **Copilot 文书引用** |
| 没有知识库，法条/判例要另存 | **知识库**：法条/判例/笔记统一存储，写作时自动检索 |
| 没有收件箱，文件要手动归档 | **大口袋**：文件/邮件/文本丢进来，自动判断+推荐+归档 |

### Casy 与飞书的关系

```
Casy 不是飞书的替代品，而是飞书的"上层建筑"：

飞书多维表格 → 通用数据存储 + 团队协作
    ↑ 同步
Casy → 专业案件管理 + 期限计算 + 知识库 + 文书生成 + 强提醒

用户可以在飞书中查看/编辑数据（团队协作场景）
用户在 Casy 中做专业操作（个人工作场景）
两边数据双向同步，无缝衔接
```

---

## 二、当前实现状态检查

### 2.1 已实现 ✅

| 模块 | 代码量 | 说明 |
|------|--------|------|
| 飞书 API 基础 | sync/feishu.rs (1981行) | Token 管理、限流、Pull/Push |
| 表结构发现 | commands/sync.rs | list_tables/list_fields/list_records |
| 字段映射 | commands/sync.rs | compare_table/compare_records/save_mappings |
| 导入引擎 | commands/sync.rs | import_all/import_selected/import_incremental |
| 同步引擎 | commands/sync.rs | sync_pull/sync_push |
| 公式引擎 | formula/ (2600行) | AST/Parser/Evaluator/DependencyGraph |
| JSON 导入 | import_feishu.rs | 5 张表 JSON dump 导入 |
| 前端设置 | FeishuSettings.vue | 连接配置 + 表发现 + 映射 + 导入 |
| 任务管理 | tasks.rs | CRUD + 四象限 + 庭审准备任务 |
| 日历 | calendar.rs | 月视图 + 法定节假日 |
| 期限引擎 | formula/engine.rs | DeadlineEngine + HolidayCalendar |

### 2.2 未实现 ❌

| 模块 | 优先级 | 说明 |
|------|--------|------|
| **飞书消息推送** | P0 | 通过飞书 Bot 发送提醒消息 |
| **飞书任务创建** | P0 | 在飞书中创建任务（期限提醒场景）|
| **多通道提醒系统** | P0 | 统一的提醒调度器（本地/飞书/系统通知）|
| **动态字段系统** | P1 | 按案由/审级自动显示/隐藏字段 |
| **跨类型筛选** | P1 | 不同案件类型的统一筛选视图 |
| **公式缓存列** | P1 | schema 已定义，未写入计算结果 |
| **链接关系同步** | P2 | DuplexLink/SingleLink 的双向同步 |
| **附件同步** | P2 | 飞书附件下载 / 本地文件上传 |
| **冲突解决 UI** | P2 | 字段级冲突的用户选择界面 |

---

## 三、多通道提醒系统（核心新增）

### 3.1 架构

```
                    ┌─────────────────┐
                    │   提醒调度器      │
                    │  ReminderEngine  │
                    └────────┬────────┘
                             │
         ┌───────────┬───────┼───────┬───────────┐
         ↓           ↓       ↓       ↓           ↓
   ┌──────────┐┌──────────┐┌──────────┐┌──────────┐┌──────────┐
   │ 本地弹窗  ││ 系统通知  ││ 飞书消息  ││ 飞书任务  ││ 邮件提醒  │
   │ Tauri    ││ macOS   ││ Bot API ││ Task API ││ SMTP    │
   │ dialog   ││ notify  ││ im/v1   ││ task/v2  ││ (future)│
   └──────────┘└──────────┘└──────────┘└──────────┘└──────────┘
```

### 3.2 提醒规则

```sql
CREATE TABLE IF NOT EXISTS reminder_rules (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,              -- 规则名称
    trigger_type    TEXT NOT NULL               -- 触发类型
                    CHECK(trigger_type IN (
                        'deadline_before',      -- 期限前 N 天
                        'deadline_on',          -- 期限当天
                        'deadline_after',       -- 期限后 N 天（逾期）
                        'hearing_before',       -- 开庭前 N 天
                        'task_due',             -- 任务到期
                        'custom'                -- 自定义
                    )),
    trigger_value   INTEGER,                    -- N 天
    channels        TEXT NOT NULL,              -- JSON: ["local","feishu_message","feishu_task","system"]
    message_template TEXT,                      -- 提醒消息模板
    case_types      TEXT,                       -- JSON: 适用案件类型（null=全部）
    enabled         INTEGER DEFAULT 1,
    created_at      TEXT DEFAULT (datetime('now','localtime'))
);

-- 默认提醒规则
INSERT INTO reminder_rules (name, trigger_type, trigger_value, channels) VALUES
('期限前 7 天提醒', 'deadline_before', 7, '["local","feishu_message"]'),
('期限前 3 天紧急提醒', 'deadline_before', 3, '["local","feishu_message","feishu_task","system"]'),
('期限当天强提醒', 'deadline_on', 0, '["local","feishu_message","feishu_task","system"]'),
('开庭前 7 天准备提醒', 'hearing_before', 7, '["local","feishu_message"]'),
('开庭前 1 天最终提醒', 'hearing_before', 1, '["local","feishu_message","feishu_task","system"]'),
('任务到期提醒', 'task_due', 0, '["local","feishu_message"]');
```

### 3.3 飞书消息推送

```rust
/// 通过飞书 Bot 发送消息
async fn send_feishu_message(
    receive_id: &str,           // 用户 open_id 或 chat_id
    receive_id_type: &str,      // "open_id" / "chat_id" / "union_id"
    msg_type: &str,             // "text" / "interactive" (卡片消息)
    content: &str,              // JSON content
) -> Result<String> {
    // POST /im/v1/messages?receive_id_type={type}
    // Headers: Authorization: Bearer {tenant_access_token}
    // Body: { receive_id, msg_type, content }
}
```

**消息卡片模板**（期限提醒）：
```json
{
  "msg_type": "interactive",
  "card": {
    "header": {
      "title": { "tag": "plain_text", "content": "⏰ 期限提醒" },
      "template": "red"
    },
    "elements": [
      {
        "tag": "div",
        "text": {
          "tag": "lark_md",
          "content": "**案件**: 浦项 v NSC — 专利无效\n**期限**: 提交答辩状\n**截止日期**: 2026-08-15\n**剩余**: 3 天"
        }
      },
      {
        "tag": "action",
        "actions": [
          {
            "tag": "button",
            "text": { "tag": "plain_text", "content": "查看案件" },
            "url": "casy://cases/xxx",
            "type": "primary"
          }
        ]
      }
    ]
  }
}
```

### 3.4 飞书任务创建

```rust
/// 在飞书中创建任务
async fn create_feishu_task(
    summary: &str,              -- 任务标题
    description: &str,          -- 任务描述
    due_date: Option<&str>,     -- 截止日期
    members: Vec<&str>,         -- 负责人 open_id
) -> Result<String> {
    // POST /task/v2/tasks
    // Headers: Authorization: Bearer {tenant_access_token}
    // Body: { summary, description, due, members }
}
```

**任务模板**（传票 → 飞书任务）：
```
传票到达 → 自动创建以下飞书任务：

1. 准备证据材料（截止: 开庭前 7 天）
   - 负责人: 案件办案人
   - 描述: 整理并提交证据清单

2. 准备代理词（截止: 开庭前 5 天）
   - 负责人: 案件办案人
   - 描述: 撰写代理词/答辩意见

3. 确认出庭人员（截止: 开庭前 3 天）
   - 负责人: 案件办案人
   - 描述: 确认出庭律师和当事人
```

### 3.5 提醒调度器

```rust
/// 提醒调度器：定期检查并触发提醒
pub struct ReminderEngine {
    /// 检查间隔（秒）
    check_interval: u64,
    /// 飞书 Bot 配置
    feishu_bot: Option<FeishuBotConfig>,
}

impl ReminderEngine {
    /// 主循环：每分钟检查一次
    pub async fn run(&self) {
        loop {
            let _ = self.check_and_trigger().await;
            tokio::time::sleep(Duration::from_secs(self.check_interval)).await;
        }
    }

    /// 检查所有活跃案件的期限和任务，触发符合条件的提醒
    async fn check_and_trigger(&self) -> Result<()> {
        let conn = db::open_db()?;

        // 1. 检查期限提醒
        let deadlines = self.check_deadlines(&conn)?;
        for reminder in deadlines {
            self.trigger_reminder(&reminder).await?;
        }

        // 2. 检查开庭提醒
        let hearings = self.check_hearings(&conn)?;
        for reminder in hearings {
            self.trigger_reminder(&reminder).await?;
        }

        // 3. 检查任务到期提醒
        let tasks = self.check_task_due(&conn)?;
        for reminder in tasks {
            self.trigger_reminder(&reminder).await?;
        }

        Ok(())
    }

    /// 根据提醒规则的 channels 配置，发送到对应通道
    async fn trigger_reminder(&self, reminder: &Reminder) -> Result<()> {
        for channel in &reminder.channels {
            match channel.as_str() {
                "local" => self.send_local_notification(reminder),
                "system" => self.send_system_notification(reminder),
                "feishu_message" => self.send_feishu_message(reminder).await?,
                "feishu_task" => self.create_feishu_task(reminder).await?,
                _ => {}
            }
        }
        Ok(())
    }
}
```

---

## 四、动态字段系统

### 4.1 问题

飞书多维表格中，所有案件共用同一套字段。但实际业务中：
- 专利无效案件需要：请求人补充意见期限、专利权人陈述意见期限
- 专利侵权案件需要：管辖异议、诉讼程序（简易/普通）
- 专利行政案件需要：行政复议信息

在飞书中，这些字段都显示在一张表里，导致很多字段对某些案件是空的，筛选困难。

### 4.2 Casy 的解决方案

```sql
-- 字段分组定义
CREATE TABLE IF NOT EXISTS field_groups (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,              -- 分组名称
    description     TEXT,                       -- 分组描述
    case_types      TEXT,                       -- JSON: 适用案件类型
    court_levels    TEXT,                       -- JSON: 适用审级
    sort_order      INTEGER DEFAULT 0,
    created_at      TEXT DEFAULT (datetime('now','localtime'))
);

-- 字段与分组的关联
CREATE TABLE IF NOT EXISTS field_group_items (
    id              TEXT PRIMARY KEY,
    group_id        TEXT NOT NULL REFERENCES field_groups(id) ON DELETE CASCADE,
    column_name     TEXT NOT NULL,              -- 本地列名
    label           TEXT NOT NULL,              -- 显示标签
    field_type      TEXT NOT NULL,              -- text/number/date/select/textarea
    options         TEXT,                       -- JSON: 选项列表（select 类型）
    required        INTEGER DEFAULT 0,
    sort_order      INTEGER DEFAULT 0,
    UNIQUE(group_id, column_name)
);
```

### 4.3 预设字段分组

```
通用字段（所有案件类型）：
  - 案件信息、案号、客户名称、对方名称
  - 我方诉讼地位、对方诉讼地位
  - 审理机关、审级、办案人
  - 立案时间、备注

专利无效专用：
  - 请求人首次无效时间
  - 请求人补充意见期限
  - 请求人收到专利权人意见时间
  - 请求人答复意见期限
  - 专利权人收到受通时间
  - 专利权人陈述意见期限
  - 专利权人补充意见时间

专利侵权专用：
  - 管辖异议
  - 诉讼程序（简易/普通）
  - 收到起诉状时间
  - 提交答辩状期间
  - 预估审限

行政诉讼专用：
  - 行政决定类型
  - 复议信息
```

### 4.4 UI 效果

```
案件详情页 → 根据"案由"自动切换字段分组：

选择"专利无效" → 显示：通用字段 + 专利无效专用字段
选择"专利侵权" → 显示：通用字段 + 专利侵权专用字段
选择"专利行政" → 显示：通用字段 + 行政诉讼专用字段

用户也可以手动添加/隐藏字段
```

---

## 五、跨类型筛选

### 5.1 问题

飞书多维表格中，不同类型的案件在同一张表里，但筛选时：
- 想看"所有案件的开庭日期"→ 有的案件没有开庭日期字段
- 想看"所有案件的期限"→ 不同类型的期限计算规则不同

### 5.2 Casy 的解决方案

**统一筛选视图**：跨案件类型的通用筛选字段

```sql
-- 虚拟筛选字段：将不同类型的字段映射到统一的筛选维度
CREATE VIEW IF NOT EXISTS v_case_unified_fields AS
SELECT
    c.id,
    c.case_title,
    c.case_no,
    c.client_name,
    c.case_status,
    c.case_type,

    -- 统一期限字段：从公式缓存列获取
    COALESCE(
        c.formula_defense_deadline,          -- 侵权：答辩期
        c.formula_petitioner_supp,           -- 无效：请求人补充意见期限
        c.relief_deadline                    -- 行政：救济期限
    ) AS next_deadline,

    -- 统一开庭日期
    c.trial_date,

    -- 统一案件状态
    c.formula_case_status,

    -- 统一审理机关
    c.court

FROM cases c;
```

**筛选 UI**：
```
┌─────────────────────────────────────────────────────────┐
│  筛选: [全部类型 ▼] [全部状态 ▼] [全部审理机关 ▼]       │
│  期限: [未来 7 天 ▼]  开庭: [未来 30 天 ▼]              │
├─────────────────────────────────────────────────────────┤
│  统一视图:                                               │
│  案件        类型    状态    下一期限    开庭日期         │
│  浦项 v NSC  无效    进行中  8/15       8/20            │
│  钛金 v 高德  侵权    进行中  8/10       —               │
│  隆基 v XX   行政    进行中  8/25       9/01            │
└─────────────────────────────────────────────────────────┘
```

---

## 六、飞书同步完善

### 6.1 当前缺失项

| 缺失 | 说明 | 实现方案 |
|------|------|----------|
| 飞书消息推送 | Bot 发送提醒消息 | POST /im/v1/messages |
| 飞书任务创建 | 在飞书中创建任务 | POST /task/v2/tasks |
| 公式缓存写入 | 计算结果写入 formula_ 列 | 公式引擎 → UPDATE |
| 链接关系同步 | DuplexLink 双向同步 | 解析 record_ids → 本地关联 |
| 附件同步 | 飞书附件下载 | GET /drive/v1/medias/{file_token} |
| 冲突解决 UI | 字段级冲突选择 | 前端弹窗 |

### 6.2 飞书 API 清单（完整）

```
已使用：
  ✅ POST /auth/v3/tenant_access_token/internal  — Token
  ✅ GET  /bitable/v1/apps/{app}/tables          — 列出表
  ✅ GET  /bitable/v1/apps/{app}/tables/{t}/fields — 列出字段
  ✅ GET  /bitable/v1/apps/{app}/tables/{t}/records — 列出记录
  ✅ POST /bitable/v1/apps/{app}/tables/{t}/records — 创建记录
  ✅ PUT  /bitable/v1/apps/{app}/tables/{t}/records/{r} — 更新记录

需要新增：
  ❌ POST /im/v1/messages?receive_id_type=...    — 发送消息
  ❌ POST /task/v2/tasks                         — 创建任务
  ❌ GET  /drive/v1/medias/{file_token}/download — 下载附件
  ❌ POST /drive/v1/medias/upload_all            — 上传附件
  ❌ GET  /bitable/v1/apps/{app}/tables/{t}/records/{r} — 获取单条记录
```

### 6.3 飞书 Bot 配置

```sql
-- 飞书 Bot 配置（在 feishu_connections 表扩展）
ALTER TABLE feishu_connections ADD COLUMN bot_webhook TEXT;    -- Webhook URL（简单模式）
ALTER TABLE feishu_connections ADD COLUMN bot_app_id TEXT;     -- Bot App ID（高级模式）
ALTER TABLE feishu_connections ADD COLUMN bot_app_secret TEXT; -- Bot App Secret
ALTER TABLE feishu_connections ADD COLUMN notify_chat_id TEXT; -- 默认通知群 ID
ALTER TABLE feishu_connections ADD COLUMN notify_user_id TEXT; -- 默认通知用户 open_id
```

---

## 七、与各模块的完整集成

### 7.1 Inbox → 飞书

```
Inbox 收到传票
    ↓
归档到案件 + 创建本地任务
    ↓
如果启用了飞书同步：
  ├─ 创建飞书任务（准备证据/代理词/出庭确认）
  └─ 发送飞书消息提醒（"收到传票，开庭日期 8/15"）
```

### 7.2 期限引擎 → 飞书

```
期限引擎计算出答辩期限
    ↓
写入 cases.formula_defense_deadline
    ↓
提醒调度器检查：
  ├─ 期限前 7 天 → 飞书消息："答辩期限临近，剩余 7 天"
  ├─ 期限前 3 天 → 飞书消息 + 飞书任务："紧急：答辩期限 3 天后"
  └─ 期限当天   → 飞书消息 + 飞书任务 + 系统通知："今天是答辩截止日！"
```

### 7.3 任务管理 → 飞书

```
本地创建/更新任务
    ↓
如果启用了飞书同步：
  ├─ 新任务 → 飞书创建对应任务
  ├─ 任务完成 → 飞书更新任务状态
  └─ 任务删除 → 飞书归档任务
```

### 7.4 日历 → 飞书

```
开庭日期变更
    ↓
更新 cases.trial_date
    ↓
提醒调度器：
  ├─ 开庭前 7 天 → 飞书消息 + 创建准备任务
  └─ 开庭前 1 天 → 飞书消息 + 系统通知
```

### 7.5 知识库 → 飞书

```
新法条入库
    ↓
如果有相关活跃案件：
  └─ 飞书消息："新入库：专利法实施细则第 X 条，与案件 XX 相关"
```

---

## 八、实施计划

| 序号 | 任务 | 工作量 | 优先级 | 依赖 |
|------|------|--------|--------|------|
| 1 | 多通道提醒系统（ReminderEngine + 规则引擎） | 3d | P0 | 无 |
| 2 | 飞书消息推送（Bot API + 消息卡片模板） | 2d | P0 | 1 |
| 3 | 飞书任务创建（Task API + 任务模板） | 2d | P0 | 1 |
| 4 | 公式缓存写入（公式引擎 → formula_ 列） | 1d | P1 | 无 |
| 5 | 动态字段系统（field_groups + 自动显示/隐藏） | 3d | P1 | 无 |
| 6 | 跨类型筛选视图（v_case_unified_fields） | 1d | P1 | 5 |
| 7 | 链接关系同步（DuplexLink 双向） | 2d | P2 | 无 |
| 8 | 附件同步（下载/上传） | 2d | P2 | 无 |
| 9 | 冲突解决 UI | 2d | P2 | 无 |
| 10 | 飞书 Bot 配置 UI | 1d | P0 | 2 |
| 11 | 提醒规则配置 UI | 1d | P0 | 1 |
| 12 | 测试 + 文档 | 2d | P0 | 1-11 |
| **总计** | | **~22d** | | |

### 优先级说明

**P0（核心差异）**：提醒系统 + 飞书消息 + 飞书任务
→ 这是 Casy 区别于飞书多维表格的核心能力

**P1（增强体验）**：公式缓存 + 动态字段 + 跨类型筛选
→ 让 Casy 比飞书更好用

**P2（完善功能）**：链接同步 + 附件 + 冲突解决
→ 完善飞书同步的完整性

---

## 九、技术栈总结

```
Casy 技术栈：
├── 前端: Vue 3 + Element Plus + Vite
├── 后端: Tauri 2 + Rust
├── 数据库: SQLite
├── 飞书集成:
│   ├── Bitable API（表格 CRUD）
│   ├── IM API（消息推送）
│   ├── Task API（任务创建）
│   └── Drive API（附件下载/上传）
├── AI: Ollama / OpenAI 兼容 API
├── 公式引擎: Rust nom 解析器
└── 提醒系统: 本地调度器 + 多通道推送
```

---

> 最后更新: 2026-08-03 (v4.0: 从"飞书同步"升级为"比飞书更好用的案件管理系统")
