# 模块 12 · 提醒系统

> **版本**: v1.0  
> **日期**: 2026-08-14  
> **状态**: 现状校准 + 设计约束（离线提醒口径以 architecture.md §9 为准）  
> **关联**: `00-README.md` / `06-calendar-deadline.md`（期限结果）/ `architecture.md` §9（离线提醒决策）

---

## 一、职责边界

### 1.1 做什么

- 提醒规则（`reminder_rules`）CRUD 与种子数据。
- 提醒日志（`reminder_log`）与同日去重。
- 提醒引擎（`start_reminder_engine`）周期检查与分发。
- 多通道分发（本地/系统通知/飞书消息/飞书任务——现状飞书为占位）。

### 1.2 不做什么

- **不负责**期限计算（见 06，只消费 `case_deadlines` / `tasks.deadline`）。
- **不负责**任务系统（见 03）。
- **不负责**"离线准时送达"的自我实现（M1 交给日历服务，见 architecture.md §9）。

---

## 二、数据模型

### 2.1 `reminder_rules`（提醒规则）

- 规则定义：触发对象（case/task/hearing/deadline）× 触发条件（`deadline_before` / `deadline_on` / `deadline_after` / `hearing_before` / `task_due` / `task_overdue`）× `trigger_days` × 通道 × 强度。

### 2.2 `reminder_log`（提醒日志）

- `status` 取值 `sent / failed / snoozed`——表示"本地声称已发出"，**不等于服务端接受或用户已读**（architecture.md §5.2、§9.11）。
- 去重：`rule_id + date(sent_at) + case_id + task_id` 组合查询（`already_sent`），是本地同日去重，**不是端到端幂等键**。

### 2.3 `reminder_jobs`（目标态，M1，设计草案）

- 作业表：`id`（幂等键）/ `entity_type` / `channel` / `executor`（local/calendar）/ `scheduled_at`（UTC + IANA 时区）/ `calendar_event_id` / `calendar_etag` / `status`（pending → synced → sent → delivered → read）/ `attempts` / `supersedes_id`。
- 状态机与可靠性机制详见 architecture.md §9.6。

---

## 三、命令接口

| 命令 | 说明 |
|---|---|
| `list_reminder_rules` / `create_reminder_rule` / `update_reminder_rule` / `delete_reminder_rule` | 规则 CRUD |
| `test_reminder` | 规则触发测试 |
| `start_reminder_engine` | 引擎启动（默认 5 分钟周期检查） |
| `get_reminder_log` | 提醒日志查询 |
| `send_feishu_reminder_async` / `create_feishu_task_reminder_async` | 飞书通道（现状 dead_code，占位） |

---

## 四、关键流程

### 4.1 当前流程（M0 本地）

```text
start_reminder_engine（现状未在启动时拉起，见 architecture.md §9.3）
  → check_and_trigger 周期扫描
  → days_diff == trigger_days 严格相等触发（deadline_before 等）
  → dispatch_reminder 分发
    ├── local/system → 写 reminder_log(sent) + 系统通知
    └── feishu_message/task → 占位，直接返回成功并写 sent（未调飞书 API）
  → already_sent 同日去重
```

### 4.2 M1 日历同步（目标态）

```text
本地规则引擎算触发时间（确定性）
  → 生成 reminder_job（scheduled_at=UTC + IANA 时区）
  → 生成 iCalendar 事件（UID=job id，summary=脱敏标题，dtstart，alarm）
  → CalDAV PUT 到用户日历（Google/Apple/Outlook）
  → 日历返回 ETag → status=synced（服务端接受）
  → 日历服务商在到期前按 alarm 跨设备推送（移动/邮件/桌面）
  → Casy 离线/关机/退出不影响
```

**高敏案件**：案件级通知策略 ∈ {完整, 脱敏摘要, 不出端}，默认脱敏摘要；开启前弹风险提示（architecture.md §9.8）。

---

## 五、与相邻模块的边界

| 相邻模块 | 交接点 | 约束 |
|---|---|---|
| 06 期限 | 消费 `case_deadlines` / `hearings` | 只读期限结果，不重算 |
| 03 任务 | 消费 `tasks.deadline` | 任务到期提醒 |
| 10 同步 | M1 CalDAV 同步链路 | 同步执行在 10，作业状态在 12 |
| 13 AI | 提醒反馈学习（用户是否处理） | 目标态，需要回执 |
| 14 数据层 | `settings` / keyring（CalDAV 凭据） | 凭据只存 OS keyring |

---

## 六、演进方向（目标态）

1. **M0 补齐**：`start_reminder_engine` 启动时拉起；补偿扫描改区间触发（`0 <= days_diff <= trigger_days`）；设置/日志明示"M0 尽力而为"。
2. **M1 日历同步（方案 D）**：CalDAV + ICS email 实现离线准时（architecture.md §9.4-9.5）。
3. **R1-R4 分级预警**：T-3 温和 / T-1 明确 / 当天强提醒 / 逾期追踪（设计哲学 §11.2）。
4. **时机智能**：学习用户活跃时段，提醒尽量落在活跃期。
5. **飞书通道接通**：`send_feishu_reminder_async` 真实调用（M0 本地在线时的补充渠道）。

---

## 七、验收标准

1. 规则测试用例 100% 通过（M0）。
2. `reminder_log` 不虚报"已送达/已读"（只写 sent/failed/snoozed）。
3. M1 模拟桌面端离线 48h，日历事件按 alarm 正常触发（architecture.md §9.10）。
4. 同一 UID 不产生重复日程。
