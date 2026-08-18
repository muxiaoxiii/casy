# 模块 06 · 日历与期限引擎

> **版本**: v1.0  
> **日期**: 2026-08-14  
> **状态**: 现状校准 + 设计约束  
> **关联**: `00-README.md` / `01-cases.md`（案件期限里程碑）/ `12-reminder.md`（提醒消费）

---

## 一、职责边界

### 1.1 做什么

- 法定/推荐期限规则（`deadline_rules`）的定义与维护。
- 案件实际期限实例（`case_deadlines`）的生成、重算、完成标记。
- 期限引擎（`deadline/engine.rs`）：日期推导（`civil` / `patent` 两种计算口径）+ 法定节假日（`deadline/holidays.rs`）。
- 日历事件聚合（`get_calendar_events`）：开庭/口审/期限/任务到期 → 前端日历视图。
- 首页仪表盘期限预警（`get_dashboard_stats` 内部调用）。

### 1.2 不做什么

- **不负责**提醒触发与送达（见 12，期限引擎只算"结果"）。
- **不负责**案件的创建/修改（见 01，本模块只读案件日期字段）。
- **不负责**任务系统（见 03，任务 `deadline` 与法定期限是两套数据）。

---

## 二、数据模型

### 2.1 `deadline_rules`（期限规则模板）

| 字段 | 说明 |
|---|---|
| `track` / `rule_name` / `legal_basis` | 轨道 / 规则名 / 法律依据（如"民事诉讼法 X 条"） |
| `trigger_field` | 触发字段（如 `verdict_date`、`petitioner_submit_date`） |
| `offset_value` / `offset_unit` | 偏移量 / 单位（`day` / `calendar_month`） |
| `calc_method` | `civil`（按工作日） / `patent`（专利口径） |
| `procedure_types` | 适用程序类型（可选） |
| `deadline_source` | `statutory`（法定）/ `recommended`（推荐） |
| `auto_calculate` / `priority` | 是否自动计算 / 优先级 |

### 2.2 `case_deadlines`（案件期限实例）

| 字段 | 说明 |
|---|---|
| `case_id` / `rule_id` | 案件与规则来源 |
| `deadline_name` / `trigger_date` / `due_date` | 名称 / 触发日 / 截止日（引擎计算） |
| `days_left` | 剩余天数（重算时更新） |
| `deadline_source` | `statutory` / `court` / `manual`（法院指定或手动） |
| `legal_basis` / `court_order_ref` | 依据与法院文书引用 |
| `completed` / `completed_at` | 完成标记 |

### 2.3 `hearings`（开庭/口审）

- 供日历硬性日程消费；`trial_date` 等字段与 01 案件日期里程碑对应。

---

## 三、命令接口

| 命令 | 说明 |
|---|---|
| `get_calendar_events` | 日历聚合事件（月视图数据） |
| `get_deadline_warnings` | 期限预警列表（剩余 N 天/已逾期） |
| `get_dashboard_stats` | 首页聚合（内部含期限预警） |

期限规则/实例的维护命令由 `db/` 与 `commands/cases.rs` 协同（`recalculate_case_formulas` 重算公式字段）。

---

## 四、关键流程

### 4.1 期限生成与重算

```text
案件里程碑日期变更（01 update_case 触发）
  → 期限引擎按 deadline_rules 匹配触发字段
  → 生成/更新 case_deadlines（due_date 按 calc_method + 节假日计算）
  → 更新 days_left
```

### 4.2 每日重算

- `deadline_recalc_scheduler`（每日 00:01，进程内 tokio 任务）：全量重算 `days_left` 与预警状态。

### 4.3 日历聚合

```text
get_calendar_events
  → 合并 hearings（硬性） + case_deadlines（期限） + tasks.deadline（弹性任务）
  → 按日期返回，供日历视图渲染
```

---

## 五、与相邻模块的边界

| 相邻模块 | 交接点 | 约束 |
|---|---|---|
| 01 案件 | 读取 `cases.*_date` 里程碑 | 只读，不修改案件 |
| 03 任务 | `tasks.deadline` 进入日历聚合 | 任务期限与法定期限分开展示（硬性/弹性分区） |
| 12 提醒 | 消费 `case_deadlines.due_date` / `days_left` | 提醒只读期限结果，不重算 |
| 10 同步 | 飞书同步的期限字段映射 | 同步只做字段映射，不触发重算 |

---

## 六、演进方向（目标态）

1. **分级预警 R1-R4**：期限引擎从"到期提醒"升级为分级预警（T-3/T-1/当天/逾期），确定性计算永不走 LLM（设计哲学 §11.2）。
2. **Forecast 双栏日历**：左侧月网格（颜色编码，读 `is_overdue`/`due_soon` 缓存）+ 右侧时间块（设计哲学 §7）。
3. **时间桶视图**：日历与任务双轨融合（硬性日程 vs 弹性行动视觉分区）。

---

## 七、验收标准

1. 15 条法定规则计算正确（规则用例 100% 通过）。
2. 节假日计算准确（`holidays.rs` 覆盖国务院放假安排）。
3. 每日重算不遗漏、不重复（幂等）。
4. 日历聚合区分硬性/弹性来源。
