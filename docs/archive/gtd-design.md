# Casy GTD 改造设计文档

> **版本**: v1.0  
> **日期**: 2026-08-14  
> **状态**: 设计基准  
> **说明**: 基于对 Things 3 与 OmniFocus 底层实现的逆向分析，推导 Casy 任务/日历模块的 GTD 化改造方案

---

## 一、分析方法论

### 1.1 为什么逆向数据库，而不是看宣传文档

产品宣传文档（官网、评测）只描述**用户可见功能**，不揭示**支撑这些功能的底层设计**。而一个 GTD 应用的好坏，恰恰取决于它如何建模任务的状态、时间、项目关系——这些都在数据模型里。

因此本分析采用 **schema 逆向法**：

1. **Things 3**：macOS 版本使用 SQLite 数据库，位于 `~/Library/Group Containers/JLMPQHK86H.com.culturedcode.ThingsMac/Things Database.thingsdatabase/main.sqlite`。社区（things3-db 项目、things.py、Things3-MCP 等）已完整逆向其表结构。
2. **OmniFocus**：macOS 版本使用 SQLite 数据库（`OmniFocusDatabase2`），表结构经 `ofexport` 等开源项目逆向公开。

### 1.2 分析的两个层次

| 层次 | 看什么 | 回答什么问题 |
|------|--------|-------------|
| 数据结构层 | 表、字段、枚举、外键 | "它怎么存？"——决定能力上限 |
| 机制层 | 字段如何被视图消费 | "它怎么算？"——决定性能与交互 |

---

## 二、Things 3 底层实现分析

### 2.1 数据库全景

```
表结构：
├── TMTask            ← 核心：所有任务/项目/标题（单表模型）
├── TMArea            ← 领域（工作/家庭/健康）
├── TMTag             ← 标签定义
├── TMTaskTag         ← 任务-标签多对多关联
├── TMAreaTag         ← 领域-标签关联
├── TMChecklistItem   ← 待办下的清单子项
├── TMContact         ← 联系人（与委派 delegate 字段关联）
├── TMSettings        ← 应用设置
├── TMTombstone       ← 删除墓碑（同步用）
└── TMMetaItem / BS... ← 内部元数据
```

### 2.2 TMTask 表完整字段分析

```sql
-- 关键字段分组（按职责）
TMTask:
  -- 身份与层级
  uuid                TEXT PRIMARY KEY
  type                INTEGER  -- 0=待办 1=项目 2=标题
  area                TEXT     -- 所属领域（外键→TMArea）
  project             TEXT     -- 所属项目（外键→TMTask，type=1）
  actionGroup         TEXT     -- 所属行动组（子项目）

  -- 生命周期状态
  status              INTEGER  -- 0=进行中 2=已取消 3=已完成
  trashed             INTEGER  -- 0=正常 1=已删除
  creationDate        REAL     -- 创建时间
  userModificationDate REAL   -- 修改时间

  -- ⭐ 时间双轨（本设计最核心）
  startDate           REAL     -- 开始日期（When：决定何时出现在工作清单）
  dueDate             REAL     -- 截止日期（Deadline：真正的最后期限）
  stopDate            REAL     -- 完成日期
  dueDateOffset       INTEGER  -- 截止日期偏移（重复任务的相对偏移）

  -- ⭐ 时间桶枚举（视图推导的关键）
  start               INTEGER  -- 0=Inbox 1=Anytime 2=Someday
  todayIndex          INTEGER  -- Today 列表手动排序索引
  todayIndexReferenceDate REAL -- 排序索引锚定日期

  -- 重复
  recurrenceRule      TEXT     -- 重复规则（iCal RRULE）
  repeatingTemplate   TEXT     -- 重复模板
  instanceCreationStartDate REAL
  instanceCreationPaused INTEGER
  instanceCreationCount INTEGER

  -- 委派
  delegate            TEXT     -- 委派人（外键→TMContact）

  -- 收件箱
  startBucket         INTEGER  -- 同上 start，收件箱阶段桶

  -- 统计缓存
  untrashedLeafActionsCount     INTEGER -- 未删除叶子行动数
  openUntrashedLeafActionsCount INTEGER -- 未完成叶子行动数
  checklistItemsCount           INTEGER -- 清单项数
  openChecklistItemsCount       INTEGER -- 未完成清单项数

  -- 其他
  notes               TEXT     -- 备注
  alarmTimeOffset     INTEGER  -- 提醒时间偏移
  cachedTags          TEXT     -- 标签缓存
```

### 2.3 Things 3 的核心设计结论

**结论 T1：单表模型**
一个 `TMTask` 表同时装待办（type=0）、项目（type=1）、标题（type=2）。待办"升级"成项目**不需要迁移数据**，只改 `type` 字段。这是为什么 Things 3 的"Convert to Project"（待办转项目）功能瞬间完成。

**结论 T2：When 与 Deadline 分离（时间双轨）**
`startDate` 和 `dueDate` 是两个完全独立的字段：
- `startDate` 控制"任务什么时候进入我的视线"
- `dueDate` 控制"任务真正什么时候必须完成"

典型案例：税务文件"4月1日开始准备，4月15日截止"——`startDate=4/1`，`dueDate=4/15`。

对 Casy 的映射：律师"等判决书到了再准备上诉材料，上诉期 15 天截止"——`startDate=判决日`，`dueDate=判决日+15天`。**Casy 目前只有 dueDate（deadline），完全缺失 startDate，这是最大功能缺口。**

**结论 T3：时间桶是枚举，视图是推导**
`start: 0=Inbox 1=Anytime 2=Someday` 是一个**用户手工放置的桶**，不是日期。Today/Upcoming/Anytime/Someday 这四个视图**不是存储的**，而是：

```
Today    = startDate <= 今天 的任务（在桶里的按 todayIndex 排序）
Upcoming = startDate > 今天 的任务，按日期分组
Anytime  = start = 1（Anytime 桶）且无 startDate
Someday  = start = 2（Someday 桶）
```

关键区别：**Today 列表是"计划做什么"，不是"什么时候到期"**。用户每天早上把今天要做的拖进 Today（或设 startDate=今天），这就是 GTD 的"Engage 执行"阶段。四象限不是这样——四象限是分类，Today 是行动决策。

**结论 T4：手动排序用索引而非日期**
`todayIndex` 是一个整数排序号。用户拖拽重排 Today 列表时，**只改这个整数**，不改日期。这让 Today 列表可以自由排序而保持"今天"语义不变。

**结论 T5：统计字段预缓存**
项目上直接存 `openUntrashedLeafActionsCount`（未完成行动数）。侧边栏项目旁边的数字/进度环**读缓存字段**，不是现算 SQL。

---

## 三、OmniFocus 底层实现分析

### 3.1 数据库全景

```
表结构：
├── Task            ← 核心：所有行动/行动组/项目（单表模型）
├── ProjectInfo     ← 项目统计缓存表（一对一关联 Task 中 type=project 的行）
├── Folder          ← 文件夹（项目分组）
├── Context         ← 上下文（@办公室/@电话，含 GPS）
├── Perspective     ← 透视（自定义视图定义）
├── Setting         ← 设置
├── Attachment      ← 附件
└── ODOMetadata     ← 元数据
```

### 3.2 Task 表完整字段分析

```sql
Task:
  -- 身份与层级
  persistentIdentifier TEXT PRIMARY KEY
  parent               TEXT     -- 父级（行动组/项目）
  projectInfo          TEXT     -- 关联 ProjectInfo（仅项目行）
  containingProjectInfo TEXT    -- 所属项目
  containingProjectContainsSingletons INTEGER

  -- 生命周期
  dateAdded            TIMESTAMP
  dateModified         TIMESTAMP
  dateCompleted        TIMESTAMP
  inInbox              INTEGER  -- 是否在收件箱

  -- ⭐ 时间双轨 + 旗标
  dateDue              TIMESTAMP  -- 截止日期
  dateToStart          TIMESTAMP  -- 推迟日期（Defer）
  flagged              INTEGER    -- 旗标（重要标记）

  -- ⭐ 有效值继承（effective 体系）
  effectiveDateDue     TIMESTAMP  -- 有效截止（含继承）
  effectiveDateToStart TIMESTAMP  -- 有效推迟（含继承）
  effectiveFlagged     INTEGER    -- 有效旗标（含继承）
  effectiveInInbox     INTEGER
  effectiveContainingProjectInfoActive INTEGER
  effectiveContainingProjectInfoRemaining INTEGER

  -- ⭐ 状态推导（关键机制）
  blocked              INTEGER  -- 被阻塞（顺序项目第2+个行动）
  blockedByFutureStartDate INTEGER -- 被未来开始日期阻塞
  isDueSoon            INTEGER  -- 即将到期（缓存：自动算→琥珀色）
  isOverdue            INTEGER  -- 已逾期（缓存→红色）
  containsNextTask     INTEGER  -- 是否包含下一步行动
  nextTaskOfProjectInfo TEXT    -- 项目下一步行动指针

  -- 项目顺序
  sequential           INTEGER  -- 1=顺序项目 0=并行项目
  completeWhenChildrenComplete INTEGER -- 子项全完成则自动完成
  childrenCount        INTEGER
  childrenCountAvailable INTEGER
  childrenCountCompleted INTEGER

  -- 耗时
  estimatedMinutes     INTEGER  -- 预计耗时
  minimumEstimateInTree INTEGER
  maximumEstimateInTree INTEGER

  -- 其他
  name                 TEXT
  noteXMLData          BLOB     -- 富文本备注
  repetitionMethodString TEXT
  repetitionRuleString TEXT     -- 重复规则
  rank                 INTEGER  -- 排序
  creationOrdinal      INTEGER
  hasFlaggedTaskInTree INTEGER
  hasCompletedDescendant INTEGER
```

### 3.3 ProjectInfo 表完整字段分析

```sql
ProjectInfo:
  pk                    TEXT PRIMARY KEY
  task                  TEXT   -- 关联 Task 表
  folder                TEXT   -- 所属文件夹
  status                TEXT   -- 项目状态

  -- ⭐ 项目统计缓存（全部预计算）
  numberOfAvailableTasks  INTEGER -- 可用行动数
  numberOfDueSoonTasks    INTEGER -- 即将到期数（侧边栏琥珀点）
  numberOfOverdueTasks    INTEGER -- 逾期数（侧边栏红点）
  numberOfRemainingTasks  INTEGER -- 剩余数（进度环）
  containsSingletonActions INTEGER
  taskBlocked            INTEGER -- 项目被阻塞

  -- ⭐ 下一步行动缓存
  nextTask               TEXT   -- 项目下一步行动（指针）
  minimumDueDate         TIMESTAMP

  -- ⭐ 回顾机制
  lastReviewDate         TIMESTAMP
  nextReviewDate         TIMESTAMP
  reviewRepetitionString TEXT   -- 回顾周期（如"每周"）

  -- 有效值
  folderEffectiveActive  INTEGER
```

### 3.4 Context 表字段分析

```sql
Context:
  name             TEXT     -- @办公室/@电话/@外出
  parent           TEXT     -- 上下文可嵌套
  allowsNextAction INTEGER  -- 是否允许下一步行动
  locationName     TEXT     -- 地名
  latitude/longitude REAL  -- GPS（位置提醒用）
  radius           REAL     -- 触发半径
  availableTaskCount   INTEGER -- 可用任务数（缓存）
  remainingTaskCount   INTEGER
  localNumberOfDueSoonTasks  INTEGER
  localNumberOfOverdueTasks  INTEGER
  totalNumberOfDueSoonTasks  INTEGER -- 含子级
  totalNumberOfOverdueTasks  INTEGER
```

### 3.5 OmniFocus 的核心设计结论

**结论 O1：effective 继承体系**
`effectiveDateDue`、`effectiveFlagged` 等字段保存**从父级继承后的有效值**。子行动没设截止日期，但所属项目设了，则子行动的有效截止=项目截止。

这是"缓存字段做继承"的典型实现：**写入时计算，读取零成本**。Casy 的案件（项目）若设了期限，子任务自动继承——这就是律师场景"案件整体 3 个月审限，所有子行动共享"的实现方式。

**结论 O2：blocked 状态（顺序项目的灵魂）**
`blocked=1` 表示"该行动当前不可用"。顺序项目（sequential）中：
- 第一个行动：blocked=0（可用）
- 后续行动：blocked=1（被阻塞）
- 第一个完成 → 第二个自动变 blocked=0

这就是 GTD "项目只有下一步行动可做"的**数据库实现**。对 Casy 的映射：
- 专利无效程序是天然的**顺序项目**：提无效请求 → 答复 → 口审 → 决定
- 律师只能"看到"当前这一步行动，做完解锁下一步
- **Casy 完全没有这个机制**——所有任务平铺，律师要自己盯流程顺序

**结论 O3：统计预缓存**
ProjectInfo 表存 `numberOfOverdueTasks` 等字段。侧边栏项目的红点（逾期）、进度环（剩余/总数）**直接读缓存**。Casy 的 `case_stats` 每次现算 SQL，案件多时会有性能问题，且无法在列表页实时显示"逾期数徽标"。

**结论 O4：isDueSoon / isOverdue 是缓存标志**
Task 表直接存 `isOverdue` 布尔值。Forecast 日历网格里"红色=当天有逾期，琥珀=快到期"的颜色编码，**读的就是这两个字段**。不是每次渲染时现算。

**结论 O5：Review 回顾机制有数据支撑**
`lastReviewDate`、`nextReviewDate`、`reviewRepetitionString` 三件套。项目到"下次回顾时间"就自动出现在 Review 透视，直到用户"标记已回顾"。Casy 没有——律师的"案件定期复盘"没地方落地。

**结论 O6：Context 是实体表（含位置）**
`@办公室/@电话/@法院` 是**数据库实体**（Context 表），不是字符串标签。支持嵌套、位置提醒、任务计数。Casy 如果做上下文，应建表而不是加个文本字段。

---

## 四、两强对比

| 维度 | Things 3 | OmniFocus | Casy 现状 |
|------|----------|-----------|-----------|
| 数据模型 | 单表 TMTask | 单表 Task + ProjectInfo 缓存 | tasks 表（单表，无缓存） |
| When/Deadline | ✅ startDate + dueDate | ✅ dateToStart + dateDue | ❌ 只有 deadline |
| 时间桶 | ✅ Inbox/Anytime/Someday | ✅（用 defer 实现类似） | ❌ 四象限 |
| 视图推导 | 从 startDate 推导 Today/Upcoming | Perspectives 推导 | ❌ 存储的四象限 |
| 手动排序 | ✅ todayIndex 索引 | ✅ rank | ❌ 无 |
| 顺序项目 | ⚠️ 部分（actionGroup） | ✅ sequential + blocked | ❌ 无 |
| 有效值继承 | 部分（cachedTags） | ✅ effective* 体系 | ❌ 无 |
| 统计缓存 | ✅ openLeafActionsCount | ✅ ProjectInfo 全量 | ❌ 每次现算 |
| 上下文 | ⚠️ 标签模拟 | ✅ Context 实体表 | ❌ 无 |
| 回顾 | ⚠️ 无专门机制 | ✅ Review 三件套 | ❌ 无 |
| 旗标/重要 | ⚠️ tag 模拟 | ✅ flagged | ❌ 优先级字段（弱） |
| 委派 | ✅ delegate | ✅（通过 context） | ⚠️ assignee 字段（有但未用） |
| 逾期缓存 | 部分 | ✅ isOverdue | ❌ 现算 |
| 重复任务 | ✅ recurrenceRule | ✅ repetitionRuleString | ❌ 无 |

---

## 五、Casy 律师 GTD 改造方案

### 5.1 数据模型改造（tasks 表）

基于以上分析，Casy 的 `tasks` 表改造为：

```sql
-- 迁移 v9: 任务 GTD 化
ALTER TABLE tasks ADD COLUMN task_type TEXT DEFAULT 'action'
  CHECK(task_type IN ('action','waiting','delegated','someday'));
-- GTD 类型：action=行动 waiting=等待 delegated=委派 someday=某天

ALTER TABLE tasks ADD COLUMN start_date TEXT;      -- 开始日期（When）
ALTER TABLE tasks ADD COLUMN waiting_for TEXT;     -- 等待对象（如"法院"/"对方律师"）
ALTER TABLE tasks ADD COLUMN follow_up_date TEXT;  -- 跟进日期（等待类专用）
ALTER TABLE tasks ADD COLUMN context TEXT;         -- 上下文（@办公室/@电话/@法院）
ALTER TABLE tasks ADD COLUMN flagged INTEGER DEFAULT 0; -- 旗标（重要）
ALTER TABLE tasks ADD COLUMN sequential INTEGER DEFAULT 0; -- 是否顺序项目
ALTER TABLE tasks ADD COLUMN blocked INTEGER DEFAULT 0;    -- 是否被阻塞
ALTER TABLE tasks ADD COLUMN start_bucket TEXT DEFAULT 'inbox'
  CHECK(start_bucket IN ('inbox','anytime','someday','today'));
ALTER TABLE tasks ADD COLUMN today_index INTEGER DEFAULT 0; -- Today 排序
ALTER TABLE tasks ADD COLUMN estimated_minutes INTEGER;     -- 预计耗时
ALTER TABLE tasks ADD COLUMN is_overdue INTEGER DEFAULT 0;  -- 逾期缓存
ALTER TABLE tasks ADD COLUMN due_soon INTEGER DEFAULT 0;    -- 快到期缓存
ALTER TABLE tasks ADD COLUMN last_review_date TEXT;         -- 上次回顾
ALTER TABLE tasks ADD COLUMN next_review_date TEXT;         -- 下次回顾
```

### 5.2 案件项目化（cases 表补充）

```sql
-- 迁移 v9: 案件作为顺序项目
ALTER TABLE cases ADD COLUMN sequential INTEGER DEFAULT 1;  -- 案件默认顺序项目
ALTER TABLE cases ADD COLUMN next_action_id TEXT;           -- 当前下一步行动
ALTER TABLE cases ADD COLUMN overdue_task_count INTEGER DEFAULT 0; -- 逾期任务缓存
ALTER TABLE cases ADD COLUMN remaining_task_count INTEGER DEFAULT 0; -- 剩余任务缓存
ALTER TABLE cases ADD COLUMN last_review_date TEXT;         -- 案件回顾
ALTER TABLE cases ADD COLUMN next_review_date TEXT;
```

### 5.3 任务工作台（前端重构）

**Perspectives 侧边栏**（仿 OmniFocus 内置透视 + Things 视图）：

```
任务工作台
├── 📥 收件箱        ← start_bucket='inbox'，待厘清
├── ✅ 今天          ← startDate<=今天 或 手动拖入，按 todayIndex 排序
├── 📅 计划中        ← startDate>今天，按日期分组（Upcoming）
├── 🕐 随时          ← start_bucket='anytime'，可随时做
├── ⏳ 等待          ← task_type='waiting'，显示等待对象+已等天数
├── 🔄 回顾          ← nextReviewDate<=今天 的案件/任务
└── 💤 某天          ← start_bucket='someday'
```

**核心交互**：
- 收件箱项 → "厘清"操作：选类型（行动/等待/委派/某天）→ 进对应视图
- Today 列表拖拽重排 → 改 `today_index`
- 任务卡片显示：上下文标签、旗标、预计耗时、逾期红/快到期琥珀

### 5.4 顺序项目 + blocked（律师流程）

案件默认 `sequential=1`：
- 后端命令 `get_case_next_actions(case_id)` 只返回 blocked=0 的行动
- 完成一个行动 → 事务内解锁下一个（blocked: 1→0）
- 案件详情显示"项目进度环"（remaining/total 读缓存字段）

典型：专利无效案件的线性流程——
```
提无效请求(可用) → 答复(阻塞→完成①后可用) → 口审(阻塞) → 无效决定(阻塞)
```

### 5.5 日历 Forecast 视图（前端重构）

**左右布局**（仿 OmniFocus Forecast）：

```
┌─────────────────────────────────────────────┐
│ 左侧:月份网格           │ 右侧:选中日时间线      │
│ ┌───┬───┬───┬───┐      │ ┌─────────────────┐ │
│ │ 1 │ 2 │ 3 │ 4 │      │ │ 硬性日程         │ │
│ │   │2● │   │1● │      │ │ 09:00 开庭-张三案│ │
│ ├───┼───┼───┼───┤      │ │ 14:00 口审-李四案│ │
│ │ 5 │ 6 │ 7 │ 8 │      │ ├─────────────────┤ │
│ │3● │   │   │   │      │ │ 到期任务         │ │
│ ├───┼───┼───┼───┤      │ │ ⚠ 提交上诉状-王五│ │
│ │...│   │   │   │      │ ├─────────────────┤ │
│ └───┴───┴───┴───┘      │ │ 开始日期任务     │ │
│  ●=事件数，红=逾期      │ │ ▸ 起草答辩状     │ │
│  琥珀=快到期            │ └─────────────────┘ │
└─────────────────────────────────────────────┘
```

- 左侧网格颜色编码：读 `is_overdue` / `due_soon` 缓存字段
- 拖拽任务到另一天 = 改 `start_date`（或 due_date）
- 硬性日程（开庭/口审/期限）与弹性任务**视觉分区**，不混排

### 5.6 后端命令新增

| 命令 | 说明 |
|------|------|
| `clarify_inbox_item` | 收件箱项厘清：转行动/等待/委派/某天 |
| `get_today_tasks` | Today 视图（含排序索引） |
| `get_upcoming_tasks` | 计划视图（按 startDate 分组） |
| `get_waiting_tasks` | 等待视图（含等待时长） |
| `get_review_items` | 回顾视图（到期的案件/任务） |
| `complete_task` | 完成任务 + 解锁顺序项目下一步 |
| `get_case_next_actions` | 案件当前可用行动 |
| `recalc_task_flags` | 重算逾期/快到期缓存标志 |
| `get_forecast` | 日历 Forecast 聚合数据 |

### 5.7 数据一致性策略

- **缓存字段写入时计算**：`is_overdue` / `due_soon` 在任务创建/更新时由后端计算，不每次查询现算
- **统计字段事务内更新**：完成任务时，在同一事务里更新案件的 `remaining_task_count` / `overdue_task_count` / `next_action_id`
- **Review 触发**：每日定时器检查 `next_review_date`，到期项目自动进入回顾视图

---

## 六、实施优先级

| 阶段 | 内容 | 依赖 |
|------|------|------|
| P0 | tasks/cases 表迁移 v9 + 字段补全 | 无 |
| P1 | 任务工作台：5 个透视视图 + 收件箱厘清流程 | P0 |
| P2 | 顺序项目 + blocked + 案件项目进度 | P0 |
| P3 | 日历 Forecast 视图（左右布局 + 颜色编码 + 拖拽改期） | P0 + get_forecast |
| P4 | 回顾机制 + 逾期缓存重算定时器 | P0 |

---

## 七、风险与取舍

| 决策 | 取舍 |
|------|------|
| 单表 vs 分表 | 沿用现有 tasks 单表，加字段（避免大重构） |
| 时间桶 vs 日期严格分类 | 采用 Things 的枚举桶 + startDate 推导，保留手动 Today 拖拽 |
| 上下文用标签 vs 实体表 | 先加 `context` 文本字段，后续需要位置提醒再升级实体表 |
| 四象限保留？ | 作为"随时"视图内的辅助分组，不删除现有数据（兼容） |
| 顺序项目默认开启？ | 案件默认 sequential=1，但提供开关（部分案件非流程化） |

---

## 八、参考文献

1. things3-db（GitHub）：Things 3 SQLite 数据库逆向
2. things.py / Things3-MCP：Things 3 数据访问库（API 文档）
3. ofexport（GitHub）：OmniFocus SQLite 数据库逆向
4. OmniFocus 官方文档（Glossary / Forecast / Perspectives）
5. OmniFocus 4.7 发布公告：Planned Dates 第三日期概念
6. David Allen《Getting Things Done》（GTD 方法论）
