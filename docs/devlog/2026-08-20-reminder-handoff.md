# 提醒时间交接完善（设计哲学 §11.2 离线送达：设置即交接）

> **日期**: 2026-08-20
> **背景**: 用户指出"提醒是时间问题，目前通过外部协议（CalDAV/ICS）进行，设置提醒就要做好交接"。
> 审计发现交接缺口：① 提醒日历事件固定 09:00，任务的 due_time（具体时间点）没进入事件；
> ② 任务设置/改期不联动外部日历；③ 交接要等引擎扫描触发才发生。

---

## 一、时间点进入提醒事件

- **`CalendarJobCtx` 加 `due_time`**：任务/庭审/期限上下文携带具体时间点
- **`parse_due_datetime(due_date, due_time)`**（新）：due_time（HH:MM）优先 → 事件用真实时间点；无则默认 09:00（兼容旧行为）
- 任务扫描 SQL 增加 `t.due_time` 列 → 提醒引擎触发时事件时间 = 任务真实时间点

## 二、设置即交接（核心）

- **`sync_task_reminder_calendar`**（新公共函数）：任务带截止日期 + 日历同步启用 → **创建任务的瞬间**立即创建/更新 CalDAV 事件（不等引擎扫描）
- 无 task_due/task_overdue 日历规则或未启用同步时优雅跳过（保持本地提醒）

## 三、幂等与联动

- **幂等**：按任务查已有 calendar job 复用 job_id（ICS UID = job_id）→ PUT 同 UID 天然更新不重复；`INSERT ... ON CONFLICT(id) DO UPDATE` UPSERT
- **改期联动**：update_task 检测 due_date/due_time 变化 → 重新同步（同 UID 更新外部事件）
- **撤销联动**（已有）：任务完成/删除 → `cancel_reminder_jobs_for`（取消 job + DELETE 日历事件）
- 脱敏（masked_calendar_summary）、时区（本地浮动时间，中国单时区可接受）不变

## 交接链路全景

```
用户设置任务（due_date + due_time）
  → sync_task_reminder_calendar（立即）
    → reminder_jobs（executor='calendar'，UID=job_id 幂等）
      → 异步 PUT CalDAV（Google/Apple/Outlook 日历）
        → 日历服务商按时推送（离线也准时）
改期/编辑 → 同 UID PUT 更新；完成/删除 → cancel + DELETE
```

## 四、验证

- `npm run build` ✅ · `cargo check` ✅ · `cargo test` ✅ 103/103
