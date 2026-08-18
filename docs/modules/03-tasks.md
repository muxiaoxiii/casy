# 模块 03 · 任务系统（GTD）

> **版本**: v1.0  
> **日期**: 2026-08-14  
> **状态**: 现状校准 + GTD 化设计约束  
> **关联**: `00-README.md` / `architecture.md` §5 / `01-cases.md`（案件上下文）/ `06-calendar-deadline.md`（期限）

---

## 一、职责边界

### 1.1 做什么

- 任务（`tasks`）CRUD 与完成切换。
- 任务模板（`task_templates`）与庭审准备任务自动生成。
- GTD 化任务工作台的数据支撑（时间双轨、时间桶、等待、回顾——目标态）。
- 顺序项目（案件内任务按顺序解锁，`blocked` 机制——目标态）。

### 1.2 不做什么

- **不负责**案件状态机（见 02）。
- **不负责**期限计算（见 06；任务 `deadline` 与期限引擎是两套数据）。
- **不负责**收件箱厘清（见 04；厘清转任务的命令在 04）。

---

## 二、数据模型

### 2.1 `tasks` 表（现状字段）

| 字段 | 说明 |
|---|---|
| `id` / `case_id` | 主键 + 案件外键（`ON DELETE CASCADE`） |
| `task_name` / `description` | 名称与描述 |
| `created_date` / `deadline` | 创建日期 / 截止日期（现状只有 deadline，缺 start_date） |
| `priority` | 枚举 `urgent_important / important / urgent / normal` |
| `completed` / `finish_note` | 完成标志（0/1）与完成备注 |
| `assignee` | 负责人（字段存在，当前未深度使用） |
| `source_log_id` | 来源案件日志（从时间线生成任务时回填） |
| `created_at` | 创建时间 |

### 2.2 GTD 化目标字段（迁移 v9，设计草案）

| 字段 | 语义 |
|---|---|
| `task_type` | `action / waiting / delegated / someday` |
| `start_date` | When（开始日期）—— 什么时候开始出现在清单 |
| `due_date` | Deadline（截止日期，现状 `deadline` 演进） |
| `waiting_for` / `follow_up_date` | 等谁（法院/对方/客户）/ 跟进日期 |
| `context` | @办公室 / @电话 / @法院 |
| `flagged` | 旗标（重要） |
| `sequential` / `blocked` | 顺序项目：0=解锁，1=锁定 |
| `start_bucket` | `inbox / anytime / someday / today`（时间桶） |
| `today_index` | Today 列表手动排序号 |
| `estimated_minutes` / `actual_minutes` | 预估 + 实际（学习数据） |
| `is_overdue` / `due_soon` | 缓存标志（日历颜色编码数据源） |
| `last_review_date` / `next_review_date` | 回顾周期 |

> 字段语义以 architecture.md §5 为准；v9 为设计草案，未落库前不得写成现状。

---

## 三、命令接口

| 命令 | 说明 |
|---|---|
| `list_tasks` | 任务列表（按案件/筛选） |
| `create_task` | 创建任务 |
| `toggle_task` | 完成/取消完成 |
| `update_task` | 更新任务 |
| `delete_task` | 删除任务 |
| `generate_hearing_prep_tasks` | 从开庭/口审自动生成庭审准备任务（如"核对证据清单/确认出庭时间"） |
| `list_task_templates` / `create_task_template` / `apply_task_template` | 任务模板的 CRUD 与应用 |

---

## 四、GTD 化设计（目标态，承接设计哲学第五章）

### 4.1 任务工作台 7 透视（数据推导，非存储）

```
收件箱   = start_bucket='inbox'（来自 04 厘清后待转）
今天     = startDate<=今天 或手动拖入，按 today_index 排序
计划中   = startDate>今天，按日期分组（Upcoming）
随时     = start_bucket='anytime'
等待     = task_type='waiting'，显示"等谁 / 已等 N 天"，超阈值建议催办
回顾     = nextReviewDate<=今天 的案件/任务
某天     = start_bucket='someday'
```

### 4.2 顺序项目 + blocked（律师流程）

- 案件默认 `sequential=1`。
- 只有 `blocked=0` 的行动对用户可见（下一步行动）。
- 完成当前步 → 事务内解锁下一步（blocked: 1→0）。
- 案件详情显示进度环（读缓存字段 `remaining/total`）。

典型：专利无效案件线性流程 `提无效请求 → 答复 → 口审 → 无效决定`。

### 4.3 数据一致性策略

- 缓存字段（`is_overdue` / `due_soon`）写入时计算，不每次查询现算。
- 统计字段（`remaining_task_count` / `overdue_task_count` / `next_action_id`）事务内更新。
- 完成任务的命令必须与"解锁下一步"在同一事务内。

---

## 五、与相邻模块的边界

| 相邻模块 | 交接点 | 约束 |
|---|---|---|
| 01 案件 | `tasks.case_id`；案件详情"下一步行动"卡读 blocked=0 | 顺序项目的解锁逻辑在任务侧，案件侧只读 |
| 04 收件箱 | 厘清"转行动"→ `create_task`；`inbox_items` 记录 `tasks.inbox_source_id` | 不自动建任务，人工确认后转 |
| 06 期限 | `deadline` 供日历/提醒消费 | 任务 deadline 与法定期限（`case_deadlines`）分开，不混用 |
| 12 提醒 | 任务到期提醒（R2/R3） | 提醒只消费 `deadline`，不修改任务 |
| 13 AI | 时间预估校准（`estimated_minutes` vs `actual_minutes`） | AI 只写建议，落库走确认 |

---

## 六、验收标准

1. 任务的完成/解锁下一步在单事务内完成，失败回滚。
2. 透视视图全部从字段推导，无重复存储。
3. 等待视图能正确显示"已等 N 天"并触发催办建议。
4. 任务模板生成庭审准备任务后，用户可编辑、可删除。
