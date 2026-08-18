# Schema v9 迁移记录

> **日期**: 2026-08-18  
> **状态**: 已完成  
> **影响范围**: 数据层、任务系统、案件系统、智伴系统

---

## 一、迁移概述

Schema v9 是 Casy 从基础 CRUD 应用向「全流程秩序引擎」转型的关键数据层改造。本次迁移引入了 GTD（Getting Things Done）方法论的核心数据结构，为后续的任务工作台、主动智伴、数据蒸馏等功能奠定基础。

---

## 二、新增表

### 2.1 领域表（areas）

**用途**: 长期业务方向，如专利诉讼、专利无效、行政诉讼、顾问咨询。

```sql
CREATE TABLE areas (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  description TEXT,
  icon TEXT,
  sort_order INTEGER DEFAULT 0,
  created_at TEXT,
  updated_at TEXT
);
```

**种子数据**:
- 专利诉讼
- 专利无效
- 行政诉讼
- 顾问咨询

### 2.2 行为事件表（task_events）

**用途**: 记录任务的创建、完成、延期、提醒等行为，用于学习用户模式。

```sql
CREATE TABLE task_events (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(id),
  event_type TEXT NOT NULL,  -- created/completed/deferred/snoozed/reminded/overdue/escalated/cancelled/moved
  occurred_at TEXT NOT NULL,
  payload TEXT,  -- JSON
  actor TEXT DEFAULT 'user'  -- user/ai/system
);
```

### 2.3 决策记录表（decisions）

**用途**: 记录用户的关键决策（是否上诉、是否和解等）及 AI 推荐。

```sql
CREATE TABLE decisions (
  id TEXT PRIMARY KEY,
  entity_type TEXT NOT NULL,  -- case/client/task/knowledge
  entity_id TEXT NOT NULL,
  decision_type TEXT NOT NULL,  -- appeal/settle/accept/refuse/other + recommend_*
  decision TEXT NOT NULL,
  basis TEXT,  -- JSON: 决策依据
  ai_advice TEXT,
  ai_model TEXT,
  source_ref TEXT,  -- JSON: 依据来源
  status TEXT DEFAULT 'proposed',  -- proposed/confirmed/rejected/voided
  recursive_checked INTEGER DEFAULT 0,
  confirmed_at TEXT,
  review_due TEXT,
  reviewed_at TEXT
);
```

### 2.4 AI 审计表（ai_runs + ai_context_items）

**用途**: 记录每次 AI 调用的输入输出，实现「模型可见即记录」。

```sql
CREATE TABLE ai_runs (
  id TEXT PRIMARY KEY,
  provider TEXT NOT NULL,
  model TEXT NOT NULL,
  purpose TEXT NOT NULL,
  prompt_version TEXT,
  status TEXT DEFAULT 'pending',
  input_hash TEXT,
  output_hash TEXT,
  job_id TEXT,
  error_message TEXT
);

CREATE TABLE ai_context_items (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL REFERENCES ai_runs(id),
  source_type TEXT NOT NULL,
  source_id TEXT NOT NULL,
  source_field TEXT,
  content_hash TEXT,
  snapshot_version TEXT
);
```

### 2.5 领域事件表（audit_events）

**用途**: 记录领域事务事件，支持事件溯源。

```sql
CREATE TABLE audit_events (
  id TEXT PRIMARY KEY,
  aggregate_type TEXT NOT NULL,
  aggregate_id TEXT NOT NULL,
  event_type TEXT NOT NULL,
  payload TEXT,  -- JSON
  actor TEXT DEFAULT 'user'  -- user/ai/system/mcp/skill
);
```

### 2.6 报表/总结表（smart_summaries）

**用途**: 存储自动生成的日报、周报、月报、项目报告。

```sql
CREATE TABLE smart_summaries (
  id TEXT PRIMARY KEY,
  summary_type TEXT NOT NULL,  -- daily/weekly/monthly/project/client
  entity_type TEXT,
  entity_id TEXT,
  title TEXT NOT NULL,
  content TEXT,  -- Markdown/JSON
  structured_data TEXT,  -- JSON
  ai_model TEXT,
  status TEXT DEFAULT 'draft',  -- draft/confirmed/archived
  period_start TEXT,
  period_end TEXT
);
```

### 2.7 每日统计表（daily_stats）

**用途**: 存储每日任务完成、逾期、开庭等统计数据。

```sql
CREATE TABLE daily_stats (
  id TEXT PRIMARY KEY,
  date TEXT NOT NULL UNIQUE,
  task_done INTEGER DEFAULT 0,
  task_total INTEGER DEFAULT 0,
  overdue_count INTEGER DEFAULT 0,
  overdue_days INTEGER DEFAULT 0,
  hearing_count INTEGER DEFAULT 0,
  deadline_count INTEGER DEFAULT 0,
  waiting_overdue_3d INTEGER DEFAULT 0,
  case_transitions TEXT  -- JSON
);
```

### 2.8 外置记忆表（memory_entries）

**用途**: 三层记忆架构（L1 原始/L2 提炼/L3 知识库）的存储。

```sql
CREATE TABLE memory_entries (
  id TEXT PRIMARY KEY,
  layer TEXT NOT NULL,  -- l1/l2/l3
  content TEXT NOT NULL,
  source_ref TEXT,  -- JSON
  status TEXT DEFAULT 'active',  -- active/stale/archived
  confidence REAL DEFAULT 0.5,
  ai_model TEXT,
  last_used_at TEXT,
  merged_from TEXT  -- JSON: 合并来源 ID 列表
);
```

### 2.9 多来源引用表（provenance）

**用途**: 记录结论的多个数据来源，支持溯源。

```sql
CREATE TABLE provenance (
  id TEXT PRIMARY KEY,
  entity_type TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  source_type TEXT NOT NULL,
  source_id TEXT NOT NULL,
  source_field TEXT,
  relation TEXT
);
```

### 2.10 提醒作业表（reminder_jobs）

**用途**: 持久化提醒作业，支持日历同步（CalDAV）。

```sql
CREATE TABLE reminder_jobs (
  id TEXT PRIMARY KEY,
  rule_id TEXT REFERENCES reminder_rules(id),
  entity_type TEXT NOT NULL,  -- case/task/hearing/deadline
  entity_id TEXT NOT NULL,
  channel TEXT NOT NULL,  -- local/system/calendar/email_ics/feishu_message/feishu_task
  executor TEXT DEFAULT 'local',  -- local/calendar
  scheduled_at TEXT NOT NULL,
  timezone TEXT NOT NULL DEFAULT 'Asia/Shanghai',
  calendar_account TEXT,
  calendar_event_id TEXT,
  calendar_etag TEXT,
  content TEXT,
  masked_content TEXT,
  due_snapshot TEXT,
  status TEXT DEFAULT 'pending',  -- pending/synced/sent/delivered/read/sync_failed/delivery_unknown/cancelled/dead_lettered
  attempts INTEGER DEFAULT 0,
  last_error TEXT,
  next_attempt_at TEXT,
  supersedes_id TEXT,
  version INTEGER DEFAULT 1,
  server_msg_id TEXT
);
```

### 2.11 AI 洞察表（ai_insights）

**用途**: 存储 AI 发现的隐性关联和洞察。

```sql
CREATE TABLE ai_insights (
  id TEXT PRIMARY KEY,
  insight_type TEXT NOT NULL,  -- pattern/recommendation/warning/correlation
  entity_type TEXT,
  entity_id TEXT,
  title TEXT NOT NULL,
  content TEXT NOT NULL,
  confidence REAL DEFAULT 0.5,
  source_ref TEXT,
  status TEXT DEFAULT 'pending',  -- pending/confirmed/rejected/archived
  ai_model TEXT
);
```

### 2.12 双路径路由表（command_routes）

**用途**: 标记每个命令是走规则路径还是 AI 路径。

```sql
CREATE TABLE command_routes (
  command_name TEXT PRIMARY KEY,
  route_type TEXT NOT NULL,  -- rule/ai/hybrid
  description TEXT,
  requires_confirmation INTEGER DEFAULT 0,
  min_confirm_level TEXT DEFAULT 'L1'  -- L1/L2/L3
);
```

---

## 三、tasks 表新增字段

### 3.1 GTD 核心字段

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `task_type` | TEXT | 'action' | action/waiting/delegated/someday |
| `start_date` | TEXT | NULL | When（什么时候进入视线） |
| `due_date` | TEXT | NULL | Deadline（截止日期） |
| `waiting_for` | TEXT | NULL | 等谁（法院/对方/客户） |
| `follow_up_date` | TEXT | NULL | 跟进日期 |
| `context` | TEXT | NULL | @办公室/@电话/@法院 |
| `flagged` | INTEGER | 0 | 旗标（重要） |

### 3.2 顺序项目字段

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `sequential` | INTEGER | 0 | 是否顺序项目 |
| `blocked` | INTEGER | 0 | 是否锁定（1=锁定，等待前置任务完成） |
| `sequence_order` | INTEGER | 0 | 顺序号 |

### 3.3 时间桶字段

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `start_bucket` | TEXT | 'anytime' | inbox/anytime/someday/today |
| `today_index` | INTEGER | 0 | Today 列表手动排序号 |

### 3.4 时间预估/实际

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `estimated_minutes` | INTEGER | NULL | 预估耗时（分钟） |
| `actual_minutes` | INTEGER | NULL | 实际耗时（分钟） |

### 3.5 缓存标志

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `is_overdue` | INTEGER | 0 | 是否逾期 |
| `due_soon` | INTEGER | 0 | 是否即将到期 |

### 3.6 回顾周期

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `last_review_date` | TEXT | NULL | 上次回顾日期 |
| `next_review_date` | TEXT | NULL | 下次回顾日期 |

### 3.7 关联

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `area_id` | TEXT | NULL | 关联领域 |
| `knowledge_id` | TEXT | NULL | 关联知识 |

---

## 四、cases 表新增字段

### 4.1 顺序项目

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `sequential` | INTEGER | 1 | 是否顺序项目（案件默认是） |
| `next_action_id` | TEXT | NULL | 下一步行动的 task_id |

### 4.2 统计缓存

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `overdue_task_count` | INTEGER | 0 | 逾期任务数 |
| `remaining_task_count` | INTEGER | 0 | 剩余任务数 |

### 4.3 回顾

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `next_review_date` | TEXT | NULL | 下次回顾日期 |

### 4.4 客户/领域关联

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `client_id` | TEXT | NULL | 关联客户 |
| `area_id` | TEXT | NULL | 关联领域 |

### 4.5 案件类型

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `case_type` | TEXT | 'exploratory' | computational/exploratory/growth |
| `case_goal` | TEXT | NULL | 案件目标（30字内） |

---

## 五、clients 表新增字段

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `aliases` | TEXT | NULL | JSON: 别名列表 |
| `normalized_name` | TEXT | NULL | 标准化名称（用于归一） |

---

## 六、新增索引

### tasks 表
- `idx_tasks_type` - task_type
- `idx_tasks_start_date` - start_date
- `idx_tasks_due_date` - due_date
- `idx_tasks_bucket` - start_bucket
- `idx_tasks_flagged` - flagged
- `idx_tasks_blocked` - blocked
- `idx_tasks_overdue` - is_overdue
- `idx_tasks_area` - area_id

### cases 表
- `idx_cases_sequential` - sequential
- `idx_cases_next_action` - next_action_id
- `idx_cases_review_date` - next_review_date
- `idx_cases_client_id` - client_id
- `idx_cases_area_id` - area_id
- `idx_cases_case_type` - case_type

---

## 七、触发器

### 7.1 areas 表
- `trg_areas_updated` - 自动更新 updated_at

### 7.2 tasks 表
- `trg_tasks_updated` - 自动更新 updated_at

### 7.3 decisions 表
- `trg_decisions_updated` - 自动更新 updated_at

### 7.4 smart_summaries 表
- `trg_summaries_updated` - 自动更新 updated_at

### 7.5 memory_entries 表
- `trg_memory_updated` - 自动更新 updated_at

### 7.6 reminder_jobs 表
- `trg_reminder_jobs_updated` - 自动更新 updated_at

### 7.7 ai_insights 表
- `trg_insights_updated` - 自动更新 updated_at

---

## 八、种子数据

### 8.1 领域（areas）
- 专利诉讼
- 专利无效
- 行政诉讼
- 顾问咨询

### 8.2 命令路由（command_routes）
- create_case → rule
- update_case → rule
- delete_case → rule (L3)
- create_task → rule
- update_task → rule
- toggle_task → rule
- delete_task → rule (L3)
- process_inbox_item → ai (L2)
- generate_writing_suggestion → ai (L2)
- classify_document_with_prompt → ai (L2)
- extract_info_with_prompt → ai (L2)

---

## 九、设计原则遵循

### 9.1 原则一：案件即流程
- `cases.sequential = 1` 标记案件为顺序项目
- `tasks.blocked` 控制任务解锁
- `cases.next_action_id` 指向当前可执行任务

### 9.2 原则二：时间双轨
- `tasks.start_date` + `tasks.due_date` 实现双轨
- `tasks.start_bucket` 实现时间桶

### 9.3 原则三：先捕获，后整理
- `tasks.start_bucket = 'inbox'` 标记待厘清任务

### 9.4 原则四：数据有限，视图无限
- 四大元信息（cases/tasks/areas/knowledge）
- 透视由字段推导（start_bucket, task_type 等）

### 9.5 原则六：主动智伴
- `task_events` 记录行为数据
- `ai_runs` + `ai_context_items` 实现 AI 审计
- `decisions` 记录决策链
- `ai_insights` 存储隐性关联

### 9.6 原则七：数据蒸馏
- `memory_entries` 实现三层记忆架构
- `daily_stats` + `smart_summaries` 实现报表体系

### 9.7 原则八：双向开放
- `command_routes` 标记命令路径
- `reminder_jobs` 支持日历同步

---

## 十、后续工作

1. **P1**: 基于新 schema 重写任务工作台（5 透视）
2. **P1**: 实现三层导航架构
3. **P1**: 实现案件详情页（项目书）
4. **P2**: 实现日历 Forecast 重写
5. **P2**: 实现知识库职能化改造
6. **P3**: 实现 AI 推荐决策引擎
7. **P3**: 实现数据蒸馏循环

---

## 十一、技术细节

### 11.1 迁移方式
- 增量迁移，通过 `PRAGMA user_version` 控制
- 从 v8 升级到 v9
- 兼容旧数据，新增字段均有默认值

### 11.2 兼容性
- 所有新增字段均为可选（有默认值）
- 不影响现有功能
- 旧数据自动获得默认值

### 11.3 性能考虑
- 新增索引覆盖常用查询
- 缓存字段（overdue_task_count 等）减少实时计算
- 触发器自动维护 updated_at
