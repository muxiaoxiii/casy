# 数据链打通 + 对标内核打磨

> **日期**: 2026-08-20
> **背景**: 用户要求"对标要学到内核（交互+数据），数据链要打通支撑自我学习进化"。
> 审计发现数据链断点：task_events 事件覆盖不全（snoozed/reminded/overdue 零写入）、前端无"稍后提醒"交互、完成耗时未上报、提醒无反馈回收。

---

## 一、数据链打通（行为事件补全 + 反馈回收）

### 1. 稍后提醒（snoozed 事件）
- **后端**：`snooze_task` 命令（tasks.rs）——今晚/明天/周末/下周/自定义 → 更新 due_date/start_date/start_bucket + 写 `snoozed` 事件（payload: 选项/新日期/标签）
- **前端**：TasksView 任务下拉新增"稍后：今晚/明天/周末/下周"（Things 3 式推迟交互）

### 2. 提醒反馈回收（reminded 事件）
- **后端链路**：`send_local_notification` 带 taskId/reminderLogId → emit payload 含实体上下文；新增 `record_reminder_feedback` 命令（handled/dismissed/snoozed → 写 `reminded` 事件 + reminder_log 状态更新）
- **前端**：ReminderToast 的"稍后/关闭"→ recordFeedback（设计哲学 §11.2"提醒后反馈回收"落地）

### 3. 逾期事件（overdue）
- `recalc_all_deadlines` 每日重算时扫描逾期任务写 `overdue` 事件（同日去重，actor=system）

### 4. 完成耗时上报（校准数据源）
- TasksView 完成圆圈 → 轻量弹窗可选填实际耗时 → toggle 带 actualMinutes → `completed` 事件 payload 含耗时（设计哲学 §11.6 预估校准数据）

### 数据链全景（支撑自我进化）
```
用户交互（完成/稍后/处理提醒/逾期）→ task_events（行为数据）
  → 学习分析（learning.rs：耗时校准/活跃时段/延期模式）
  → 校准写回 estimated_minutes + AI 智伴学习洞察展示
```

## 二、对标内核打磨

### 日历 Forecast 时间块分区（对标设计哲学 §7.2 / Fantastical 时段视图）
- 右栏改 5 块网格：**上午 06-12 / 下午 12-18 / 晚上 18-22 / 其他 22-06 / 弹性（未定时）**
- 块内按时间排序；硬性（开庭红/期限琥珀）与弹性（蓝）视觉分区不混排
- **时间分配提示**（§7.3）：硬性占 ≥2 块时提示"弹性任务建议安排在 X"（自动选空块）

## 三、验证
- `npm run build` ✅ · `cargo check` ✅ · `cargo test` ✅ 103/103

## 四、遗留
- 收件箱推荐"拒绝"未记录 accepted=0（推荐学习可后续增强）
- task_events 90 天清理调度未启用（设计 §11.9：L1 保留窗口）
- 前端"任务完成耗时"弹窗每次弹出（可加"不再询问"偏好）
