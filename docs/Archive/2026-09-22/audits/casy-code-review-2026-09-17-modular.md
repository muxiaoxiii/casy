# Casy 全栈模块化代码审查报告

- **审查开始日期**：2026-09-17
- **审查基线**：
  - 前端：Vitest 51 测试文件，241 项单元测试全数通过
  - 后端：Cargo test 251 项库测试，250 通过，1 项时间硬编码失败（`test_all_natural_language_dates_recognition`）
  - 工作区状态：包含未提交改动（多处 Rust 命令重构、程序看板 ProcedureBoard、编辑器与白板增强）
- **分级定义**：
  - **P0（致命/在野）**：未加防御的任意代码执行、SQL 注入、跨目录读写、未净化 XSS、数据静默损毁丢失。
  - **P1（高危/逻辑缺陷）**：破坏事务原子性导致脏数据、IPC 契约断裂、未经授权操作、死锁或未处理 `unwrap()` 崩溃。
  - **P2（中危/边界异常）**：异常边缘分支遗漏、内存/响应式泄露、防抖节流不当、宽泛 `any` 侵蚀。
  - **P3（代码异味/优化建议）**：死代码、性能热点、命名/模式偏离。

---

## 审查进度看板

- [x] **M1：案件管理与诉讼程序**（完成 · 发现 1 项 P0 编译阻断 / 2 项 P1 / 3 项 P2 / 2 项 P3）
- [x] **M2：任务管理与 GTD 透视**（完成 · 发现 2 项 P1 / 2 项 P2 / 1 项 P3）
- [x] **M3：日历与日程规划**（完成 · 发现 2 项 P1 / 3 项 P2 / 1 项 P3）
- [x] **M4：文书工坊与编辑器**（完成 · 发现 3 项 P1 / 2 项 P2 / 1 项 P3）
- [x] **M5：案卷、文件与处理中心**（完成 · 发现 1 项 P1 / 2 项 P2 / 1 项 P3）
- [x] **M6：知识库、向量与全局搜索**（完成 · 发现 2 项 P1 / 3 项 P2 / 1 项 P3）
- [x] **M7：白板与事实图谱**（完成 · 发现 1 项 P1 / 3 项 P2 / 1 项 P3）
- [x] **M8：收件箱与大口袋**（完成 · 发现 1 项 P0测试失败 / 1 项 P1 / 2 项 P2 / 1 项 P3）
- [x] **M9：AI 智伴、MCP 与安全审计**（完成 · 发现 2 项 P1 / 2 项 P2 / 1 项 P3）
- [x] **M10：核心通信与系统基础设施**（完成 · 发现 2 项 P1 / 2 项 P2 / 1 项 P3）

---

## 模块一：案件管理与诉讼程序模块 (Cases & Procedure)

- **涉及范围**：
  - 后端：`src-tauri/src/commands/cases.rs`、`commands/procedure.rs`、`commands/timeline.rs`、`db/cases.rs`、`deadline/procedure.rs`、`db/schema.rs`
  - 前端：`src/modules/cases/views/CaseListView.vue`、`CaseDetailView.vue`、`components/ProcedureBoard.vue`、`components/CaseTimelinePanel.vue`、`src/stores/cases.ts`、`src/core/services/cases.ts`
- **审查结论**：功能覆盖全面（双轨状态机、历次庭审变更留痕、程序期限自动推演、关联案件统筹）；但存在**1 项编译级阻断 P0**、**2 项数据一致性与事务安全 P1**，以及若干性能与健壮性 P2。

### 【P0 级问题 · 在野 / 编译阻断】

#### P0-1｜`procedure.rs:839` 类型比较错误导致 Rust 编译全面失败
- **文件与行号**：`src-tauri/src/deadline/procedure.rs:839`
- **问题代码**：
  ```rust
  && x.parent_id.as_deref() == Some(&e.id)
  ```
- **机理分析**：`x.parent_id.as_deref()` 返回 `Option<&str>`，而 `Some(&e.id)` 为 `Option<&String>`。在 Rust 2021/2024 edition 下，未为 `Option<&str>` 与 `Option<&String>` 实现 `PartialEq`，导致 `cargo test --lib` 编译失败（E0277），直接阻断了后端的测试与构建流水线。
- **修复方案**：改为 `Some(e.id.as_str())` 或 `Some(e.id.as_ref())`。

---

### 【P1 级问题 · 高危 / 逻辑与事务完整性】

#### P1-1｜`calendar_events` 外键级联缺失导致 `delete_case` 事务中断失败
- **文件与行号**：`src-tauri/src/db/schema.rs:2703`（`MIGRATION_V15_SQL` / `calendar_events` 表）
- **问题代码**：
  ```sql
  CREATE TABLE IF NOT EXISTS calendar_events (
    ...
    case_id     TEXT REFERENCES cases(id),
    ...
  );
  ```
- **机理分析**：Casy 数据库全局开启 `PRAGMA foreign_keys = ON;`。在 schema 中，所有关联案件的表（如 `hearings`、`case_logs`、`procedure_events` 等）均显式声明了 `ON DELETE CASCADE` 或 `ON DELETE SET NULL`；唯独 `calendar_events(case_id)` 未指定任何级联动作（SQLite 默认为 `RESTRICT`）。当某个案件在日历中存在关联日程时，调用 `delete_case` 会被 SQLite 拒绝并抛出 `FOREIGN KEY constraint failed`，导致案件删除功能直接崩溃。
- **修复方案**：补齐 schema 约束，增加迁移将 `calendar_events.case_id` 设置为 `ON DELETE SET NULL`，同时在 `delete_case` 逻辑中先行解绑相关日程。

#### P1-2｜`update_case_status` 缺乏显式事务包裹，状态机与历史记录非原子化
- **文件与行号**：`src-tauri/src/commands/cases.rs:820-870`
- **问题代码**：
  ```rust
  // 1. 获取当前状态
  let old_status = ...;
  // 2. 更新状态 (autocommit)
  conn.execute(&format!("UPDATE cases SET {} = ?1 ..."), ...)?;
  // 3. 更新聚合状态 (autocommit)
  conn.execute("UPDATE cases SET case_status = ?1 ...", ...)?;
  // 4. 插入历史 (autocommit)
  conn.execute("INSERT INTO case_track_history ...", ...)?;
  ```
- **机理分析**：操作直接在裸 `Connection` 上分步执行，处于 SQLite autocommit 模式。若第 3 步或第 4 步发生断电、异常或锁冲突，轨道字段已变，但 `case_status` 聚合值未更新、`case_track_history` 审计漏记，导致案件状态与历史审计产生永久性不一致。
- **修复方案**：改用 `let tx = conn.transaction()?;` 将 2、3、4 步包裹在同一事务内，原子性提交。同时将第 3 步中的三次单列 SELECT 合并为单次查询。

---

### 【P2 级问题 · 中危 / 性能与健壮性】

#### P2-1｜案件列表 `compute_deadline_urgency` 产生严重 N+1 查询风暴
- **文件与行号**：`src-tauri/src/db/cases.rs:610-624`
- **问题代码**：
  ```rust
  for id in case_ids {
      if let Ok(case) = get_case(conn, id) {
          if let Ok((_, items)) = crate::deadline::procedure::case_items(conn, &case) { ... }
      }
  }
  ```
- **机理分析**：`list_cases` 刚从数据库查出 `cases` 列表后，该函数又对传入的 `case_ids` 逐一重查 `get_case`；更严重的是，`case_items` 内部会为每个案件重复执行 `calendar(conn)`（重复查询节假日表）以及每个事项的 `procedure_item_states` 查询。若每页 50 条案件，一次翻页将产生数百次数据库 I/O。
- **修复方案**：直接接收已查出的 `&[Case]` 引用，避免重复 `get_case`；在循环外部只加载一次 `HolidayCalendar` 并在内存中复用。

#### P2-2｜`timeline.rs` 反序列化单条脏数据致使全量时间线崩溃
- **文件与行号**：`src-tauri/src/commands/timeline.rs:92-96`
- **问题代码**：
  ```rust
  for row in stmt.query_map([&case_id], |r| r.get::<_, String>(0))? {
      let event: crate::deadline::procedure::ProcedureEvent = serde_json::from_str(&row?)?;
      events.push(...);
  }
  ```
- **机理分析**：使用 `?` 抛错。若 `procedure_events` 历史数据中存在一条格式不兼容或 JSON 损坏的数据，整条时间线接口直接报错中断，导致用户在详情页完全无法查看办案日志、庭审和任务。
- **修复方案**：改为容错处理，解析失败时记 `log::warn!` 并跳过或降级展示，不影响其他合法事件流。

#### P2-3｜Pinia Store `updateCase` 成功后未刷新 `stats`，统计看板陈旧
- **文件与行号**：`src/stores/cases.ts:167-178`
- **机理分析**：`createCase` 和 `deleteCase` 均在执行后触发 `await this.loadStats()`；但 `updateCase` 只更新了当前案件对象，未重新加载统计数据。当案件被结案（状态改为“已完结”）或调整客户时，顶部与概览的统计指标滞后，必须手动刷新页面才能对齐。
- **修复方案**：在 `updateCase` 成功后调用 `this.loadStats()`。

---

### 【P3 级问题 · 低危 / 坏味道与冗余】

---

## 模块二：任务管理与 GTD 透视 (Tasks & GTD)

- **涉及范围**：
  - 后端：`src-tauri/src/commands/tasks.rs`、`commands/editor_tasks.rs`、`commands/task_lifecycle.rs`
  - 前端：`src/modules/tasks/views/TasksView.vue`、`components/TaskRow.vue`、`components/TaskQuickEditor.vue`、`utils/taskFilter.ts`、`src/core/services/tasks.ts`
- **审查结论**：GTD 七透视、子任务、重复规则与软删除审计机制健全；但存在 **2 项高危逻辑缺陷 (P1)**、**2 项视图与时钟边界异常 (P2)** 及 **1 项编译警告 (P3)**。

### 【P1 级问题 · 高危 / 逻辑与健壮性缺陷】

#### P1-1｜庭审准备任务 `generate_hearing_prep_tasks` 日期解析脆弱性与非原子写入
- **文件与行号**：`src-tauri/src/commands/tasks.rs:1257-1298`
- **问题代码**：
  ```rust
  let hearing_dt = chrono::NaiveDate::parse_from_str(&hearing_date, "%Y-%m-%d")?;
  ...
  for (i, (task_name, description, priority)) in HEARING_PREP_TASKS.iter().enumerate() {
      ...
      conn.execute("INSERT INTO tasks ...", rusqlite::params![...])?;
  }
  ```
- **机理分析**：
  1. 在 Casy 实际业务中，开庭时间常精确到时分（如 `"2026-10-15 09:30"`）。当传入包含时间的合法排期字符串时，`parse_from_str(..., "%Y-%m-%d")` 直接返回 `ParseError(TooLong)` 抛错，导致该排期的全部 6 项准备任务生成彻底瘫痪；
  2. 循环 6 次写入裸 `Connection`，缺少 `conn.transaction()?` 包裹。若中间某条插入失败，已插入的孤儿子任务无法回滚，造成数据残缺。
- **修复方案**：日期解析兼容含时间的格式（截取 `&hearing_date[..10]` 或使用通用日期时间解析），并将 6 项任务生成置于同一事务中。

#### P1-2｜`editor_tasks.rs` 任务绑定命令中存在未保护的 `unwrap()` Panic 隐患
- **文件与行号**：`src-tauri/src/commands/editor_tasks.rs:53`
- **问题代码**：
  ```rust
  let value = super::tasks::create_task_in_transaction(&tx, ...)?;
  value["id"].as_str().unwrap().to_owned()
  ```
- **机理分析**：直接对 JSON 返回值执行 `.unwrap()` 解包。若 `create_task_in_transaction` 逻辑调整或返回数据异常缺失 `id` 字段，会导致当前线程直接 Panic。虽然运行在 `run_blocking` 内不至于使整个应用崩溃，但会导致前端 IPC 收到 `JoinError` 异常中断。
- **修复方案**：改用 `value["id"].as_str().ok_or_else(|| anyhow::anyhow!("缺少任务ID"))?`。

---

### 【P2 级问题 · 中危 / 视图展示与时钟边界】

#### P2-1｜GTD 透视视图中子任务在顶层列表与父任务展开区重复展示
- **文件与行号**：`src/modules/tasks/utils/taskFilter.ts:63-120` 与 `src/modules/tasks/views/TasksView.vue:795-822`
- **机理分析**：`tasksForPerspective` 在进行透视过滤（如 `all`、`inbox`、`today`、`next` 等）时，未排除子任务（`t.parentId != null`）。导致子任务既作为一个平级的顶级任务出现在大清单中，又在父任务点击展开时作为子项嵌套出现在父任务下方。用户在同一视图中看到两份相同的任务，不仅导致徽标计数膨胀，还破坏了 GTD “大任务拆分为子行动”的视觉层级。
- **修复方案**：在顶级透视过滤器中增加 `!t.parentId` 条件，确保顶层列表只渲染顶级任务；子任务仅在父任务展开时或特定的扁平检索视图中呈现。

#### P2-2｜`TaskRow.vue` 内部 `todayStr` 静态单次求值导致跨日状态时钟漂移
- **文件与行号**：`src/modules/tasks/components/TaskRow.vue:87-94`
- **问题代码**：
  ```typescript
  const todayStr = (() => {
    const d = new Date()
    return `${d.getFullYear()}-${...}-${...}`
  })()
  const deferActive = computed(() => !!props.task.deferUntil && props.task.deferUntil > todayStr)
  ```
- **机理分析**：`TaskRow` 内部的 `todayStr` 是组件 setup 时自执行的闭包常量，而非响应式计算属性。若律师长时间开启客户端跨越午夜（00:00），`TaskRow` 内部仍然保留昨天的时间基准，导致“今日推迟”状态（`deferActive`）和逾期标记不会随系统日期自然切换，必须重新加载路由或重启应用。
- **修复方案**：`todayStr` 改为从父级（已有 60 秒定时更新的 `TasksView`）作为 prop 传入，或使用全局响应式当前日期。

---

### 【P3 级问题 · 低危 / 编译告警】

#### P3-1｜`tasks.rs` 单元测试中 3 处未使用的 `mut` 连接警告
- **文件与行号**：`src-tauri/src/commands/tasks.rs:1806, 1959, 1990`
- **缺陷**：测试用例中 `let mut conn = test_conn();` 的 `mut` 未被使用，触发编译警告，需清理以保持构建干净。

---

## 模块三：日历与日程规划 (Calendar & Planning)

- **涉及范围**：
  - 后端：`src-tauri/src/commands/calendar.rs`、`commands/calendar_events.rs`、`commands/caldav.rs`、`sync/caldav.rs`
  - 前端：`src/modules/calendar/views/CalendarView.vue`、`components/CalendarComposer.vue`、`components/TimeGrid.vue`、`parseCalendarCapture.ts`、`timeLayout.ts`、`calendarDates.ts`、`src/core/services/calendar.ts`
- **审查结论**：实现了法庭庭审、诉讼期限、任务到期与独立日程的四合一动态月历投影，具备自然语言智能日程捕捉（NLP）、周/日视图多道排布算法（Interval Partitioning）与 CalDAV/ICS 双向同步机制；但存在 **2 项逻辑断裂与容灾脆弱性高危缺陷 (P1)**、**3 项性能风暴与数据孤岛缺陷 (P2)** 及 **1 项前后端脱节死代码 (P3)**。

### 【P1 级问题 · 高危 / 逻辑断裂与容错缺陷】

#### P1-1｜`update_calendar_event` 校验时序错误，破坏局部更新契约
- **文件与行号**：`src-tauri/src/commands/calendar_events.rs:137-154`
- **问题代码**：
  ```rust
  pub async fn update_calendar_event(id: String, data: serde_json::Value) -> Result<(), String> {
      run_blocking(move || {
          validate_event(&data)?; // ❌ 错误提前校验：要求未合并的 data 必须含 title 和 10 位有效日期
          let mut connection = db::open_db()?;
          let conn = connection.transaction()?;
          let existing = conn.query_row(&format!("SELECT {EVENT_COLS} FROM calendar_events WHERE id=?1"), [&id], row_to_event)?;
          let mut merged = serde_json::to_value(existing)?;
          for (key, value) in data.as_object().ok_or_else(...) {
              merged[key] = value.clone();
          }
          ...
          validate_event(&merged)?; // 合并后的真正校验已在此处
  ```
- **机理分析**：函数文档明确声明“更新日程：缺省字段保留原值，显式 null 清空”，内部也实现了将增量 `data` 合并至已存在的 `existing` 上。然而在第 139 行尚未从数据库查询原值并合并前，就对原始 `data` 执行了 `validate_event(&data)?`。`validate_event` 强制断言入参必须包含非空 `title` 和有效的 10 位 `eventDate`。当调用方仅更新地点、备注、开始/结束时刻或案件归属时，直接报 `"日程日期无效"` 拦截并失败。
- **修复方案**：删除第 139 行对尚未合并的原始入参的 `validate_event(&data)?;`，仅保留第 153 行对已合并完整对象 `merged` 的校验。

#### P1-2｜CalDAV 补同步遇单点错误提前中断，导致后续同步全部饿死
- **文件与行号**：`src-tauri/src/commands/caldav.rs:212, 306-316`
- **问题代码**：
  ```rust
  pub async fn sync_reminders_to_calendar() -> Result<CalendarSyncReport, String> {
      retry_cancelled_calendar_jobs().await?; // ❌ 若此处报错则整个同步直接中断退出
      ...
  }

  async fn retry_cancelled_calendar_jobs() -> Result<(), String> {
      ...
      for uid in jobs { finish_calendar_deletion(&uid).await.map_err(|error|error.to_string())?; }
      Ok(())
  }
  ```
- **机理分析**：`sync_reminders_to_calendar` 是律师手动或后台触发的兜底同步通道。其在执行待同步作业前，会先尝试物理清理远端已撤销的日历事件。`retry_cancelled_calendar_jobs` 对历史已撤销列表逐一调用 `finish_calendar_deletion`，一旦其中任何一个由于网络抖动、超时或服务器 500 报错，`?` 就会立即向外抛出 Err。不仅阻断了后续其他待清理事件的执行，还导致下方所有真正需要推送的案件提醒与到期预警完全无法被同步。
- **修复方案**：`retry_cancelled_calendar_jobs` 内应当捕获单次删除失败（记录错误日志到 `reminder_jobs.last_error`），循环不应因单条异常退出，`sync_reminders_to_calendar` 必须保证 pending/sync_failed 队列继续得到同步。

---

### 【P2 级问题 · 中危 / 性能风暴与数据孤岛】

#### P2-1｜月历投影 N+1 级联性能风暴（3 屏并发触发全案件期限规则计算）
- **文件与行号**：`src-tauri/src/commands/calendar.rs:141-152` 与 `src/modules/calendar/views/CalendarView.vue:717-720`
- **机理分析**：
  1. `get_calendar_events` 在返回月历事件时，会遍历所有 `active_cases`，并对每一个案件调用 `case_items`（进行全量程序节点计算、节假日查询与期限紧急度评估）；
  2. 前端 `loadEvents` 使用了 `surroundingMonths`，一次性并发派发了前一个月、当月、后一个月 3 个 `casyContext.calendar.events` 请求。这意味着打开日历或切换月份时，后端会在 3 个并发线程中对全量案件各算一次规则，产生 `3 * N_cases` 次密集的 SQLite 查询与复杂规则遍历，当案件超过几十起时会引发明显的 IPC 延迟和连接锁竞争。
- **修复方案**：`case_items` 的程序推演结果应在案件状态变更或日程写入时建立轻量缓存/快照表；或在 `calendar.rs` 中采用单一区间查询，将 3 个月的投影合并为单次调用，并在外部提取节假日日历。

#### P2-2｜任务软删除后关联时间块幽灵残留
- **文件与行号**：`src-tauri/src/commands/tasks.rs:421-424` 与 `src/modules/calendar/views/CalendarView.vue:836`
- **机理分析**：用户在周/日视图中通过时间轴（`TimeGrid`）为任务排定时间块时，系统在 `calendar_events` 表中生成了一条关联 `task_id` 的记录。当用户在 GTD 视图软删除该任务时，`tasks` 表标记 `deleted_at`，但 `calendar_events` 并无级联感知。前端日历渲染时仅从未删除任务匹配完成态，导致被删除任务的时间块在周/日历上永远作为“未完成”状态存在，且点击后无法找到源任务，成为孤魂日程。
- **修复方案**：在 `delete_task` 软删除事务中，同步将 `calendar_events` 中关联的该 `task_id` 记录清理或同步置为无效，或在月历投影中过滤掉 `task_id` 对应任务已软删除的日程。

#### P2-3｜前端月视图 42 单元格重复调用无缓存 filter
- **文件与行号**：`src/modules/calendar/views/CalendarView.vue:1044, 1079`
- **机理分析**：`calendarGrid` 计算属性已经为每个 cell 预先构建了 `cell.multiDayTasks` 和 `cell.tasks`，但在月视图模板渲染循环中（42 个 cell），直接在模板内重复绑定 `multiDayTasksForDay(cell.date)` 和 `tasksForDay(cell.date).filter(...)`，造成每次轻微的响应式变更引发数百次数组全量遍历。
- **修复方案**：模板直接绑定预计算的 `cell.multiDayTasks` 与 `cell.tasks`，消除模板内的无缓存方法调用。

---

### 【P3 级问题 · 低危 / 前后端脱节】

#### P3-1｜`move_calendar_event` 移动接口在前端成为未接入的孤岛代码
- **文件与行号**：`src-tauri/src/commands/calendar_events.rs:198-242` 与 `src/core/services/calendar.ts:54`
- **机理分析**：后端专门编写了高质量的轻量移动命令 `move_calendar_event`（具备自动推算持续时间与午夜跨天越界拦截），前端服务类也封装了 `casyContext.calendar.moveEvent`。但 `CalendarView.vue` 与 `TimeGrid.vue` 的拖拽仅处理了任务调度（`scheduleTaskBlock`），并没有为独立日程提供拖拽改期与跨时间槽拖动的绑定，使该特性成为未被使用的冗余代码。

---

## 模块四：文书工坊与编辑器 (Docs & Editor)

- **涉及范围**：
  - 后端：`src-tauri/src/commands/docs.rs`、`commands/drafts.rs`、`docsy_engine/rich_export.rs`、`docsy_engine/pdf_export.rs`、`docsy_engine/typesetting.rs`、`docsy_engine/export.rs`
  - 前端：`src/shared/editor/DocumentEditor.vue`、`EditorToolbar.vue`、`TypesetPreview.vue`、`exportDocument.ts`、`schema.ts`、`semanticNodes.ts`、`markdownPreservation.ts`、`src/modules/docs/views/WritingView.vue`、`views/DocWorkshopView.vue`、`components/LegalEditor.vue`
- **审查结论**：完成了 ProseMirror/TipTap 所见即所得编辑、Markdown 双向高保真映射（AST 级别防损坏与未修改块原样留存）、Typst 纯内存中英文排版预览与可检索多页 PDF 生成、以及纯原生 Word (DOCX) 导出能力；但存在 **3 项数据丢失与导出熔断高危缺陷 (P1)**、**2 项导出鲁棒性与性能瓶颈 (P2)** 及 **1 项内存开销优化点 (P3)**。

### 【P1 级问题 · 高危 / 数据丢失与导出熔断】

#### P1-1｜编辑器 400ms 防抖未强制刷盘致使页面切换/离开时数据静默丢失
- **文件与行号**：`src/shared/editor/DocumentEditor.vue:339-343`、`src/modules/docs/views/WritingView.vue:50, 165-175`、`src/modules/docs/views/DocWorkshopView.vue:301-320`
- **问题代码**：
  ```typescript
  // DocumentEditor.vue
  onBeforeUnmount(() => {
    if (outlineTimer) clearTimeout(outlineTimer);
    if (serializeTimer) clearTimeout(serializeTimer); // ❌ 仅清除定时器，未调用 flushSerialize()
    editor.value?.destroy();
  });
  ```
- **机理分析**：
  1. `DocumentEditor` 为了降低序列化开销，用户每次键入后将序列化回写操作防抖延后 400ms（`scheduleSerialize`）。
  2. 当用户键入正文后 400ms 内，点击侧边栏导航跳转、切换文书草稿或触发路由离开时，组件进入卸载流程；`onBeforeUnmount` 仅仅通过 `clearTimeout(serializeTimer)` 杀死了定时器，未将正在等待序列化的正文内容强制刷盘同步（`flushSerialize()`），导致该 400ms 内输入的全部文字被静默销毁。
  3. 更严重的是，`WritingView` 与 `DocWorkshopView` 的离开未保存拦截器 `useSaveBeforeLeave` 依赖 `editRevision !== savedRevision`。由于 400ms 内尚未发出 `update:modelValue`，父级的 `editRevision` 尚未递增，离开守卫误判当前文档处于“无修改干净状态”，直接放行路由切换，从而让用户的最新改动永久性丢失。
- **修复方案**：在 `DocumentEditor` 的 `onBeforeUnmount` 钩子中，如果 `serializeTimer` 存在，必须立即同步调用 `flushSerialize()` 并触发 `emit('update:modelValue', md)`；同时在父级视图组件中，离开保存函数应当直接调用 `editor.getHTML()` 或 `flushAndGetMarkdown()` 获取真实内存快照，而不是被动等待防抖事件。

#### P1-2｜工具栏公式与脚注等标准特性与后端 DOCX 导出引擎发生硬报错熔断
- **文件与行号**：`src/shared/editor/EditorToolbar.vue:127-135` 与 `src-tauri/src/docsy_engine/rich_export.rs:173, 462`
- **问题代码**：
  ```rust
  // rich_export.rs
  match node.node_type.as_str() {
      "paragraph" => ...
      "table" => ...
      other => bail!("DOCX 导出尚不支持节点 {other}；文件未生成，以避免内容丢失"),
  }
  // append_inline:
  match node.node_type.as_str() {
      "text" => ...
      other => bail!("DOCX 导出尚不支持行内节点 {other}"),
  }
  ```
- **机理分析**：前端富文本工具栏支持了“行内公式”（`mathInline`）、“独立公式”（`mathBlock`）、“流程图”（`mermaid`）与“脚注”（`footnote`），且均已注册入 Tiptap Schema。然而后端 DOCX 导出模块 `rich_export.rs` 对节点采用绝对白名单模式，未覆盖上述新增节点类型。一旦文书中包含任意一个公式或脚注，用户点击“导出 Word (.docx)”时，后端将直接通过 `bail!` 抛出致命错误中断，整篇文书无法生成 DOCX。
- **修复方案**：`rich_export.rs` 应对公式（转为清晰的斜体 LaTeX 文本或纯文本表达式）、流程图与脚注增加转换降级适配逻辑，避免无差别报错阻断导出流程。

#### P1-3｜草稿更新 SQL 遗漏 `updated_at` 时间戳且外键案件无法解绑
- **文件与行号**：`src-tauri/src/commands/drafts.rs:153-162`
- **问题代码**：
  ```rust
  let new_case_id = case_id.or(current.3); // ❌ 无法区分 None 与显式 null
  conn.execute(
      "UPDATE drafts SET title = ?1, content = ?2, status = ?3, case_id = ?4, version = version + 1
       WHERE id = ?5",
      rusqlite::params![new_title, new_content, new_status, new_case_id, id], // ❌ 缺少 updated_at 更新
  )?;
  ```
- **机理分析**：
  1. `UPDATE drafts` 语句中完全遗漏了 `updated_at` 字段。每次保存文书修改时，`updated_at` 不会刷新，导致 `list_drafts` 按照 `ORDER BY updated_at DESC` 排序时，最近编辑过的文书无法排到顶部，列表排序完全失效；
  2. `case_id` 使用 `Option<String>` 接收参数，并通过 `.or(current.3)` 回退。当用户希望将一份草稿从特定案件解绑转为通用独立草稿时，前端传递 `caseId: null`，后端将 `case_id` 视为 `None` 并保留旧值 `current.3`，使得草稿与案件的外键关联一旦建立便无法解除。
- **修复方案**：在 UPDATE 语句中加入 `updated_at = datetime('now','localtime')`；更新接口改用结构化 JSON Patch 或显式标志，支持将 `case_id` 显式置为 NULL。

---

### 【P2 级问题 · 中危 / 容错韧性与视图性能】

#### P2-1｜文书包含远程图片或失效路径导致 DOCX 导出全量失败
- **文件与行号**：`src-tauri/src/docsy_engine/rich_export.rs:482-504`
- **机理分析**：`load_image_bytes` 只接受以 `data:image/` 开头的 Base64 数据或绝对本地路径，且在路径不存在时直接返回错误（`bail!`）。若文书中贴入的是 HTTP/HTTPS 在线图片，或者本地图片文件被律师移动/重命名，会导致整个文书导出操作被硬性阻断，缺乏占位图或告警跳过容错机制。
- **修复方案**：对无法加载的图片，在 DOCX 中降级生成带外框的灰色占位段落（如 `[图片无法读取: filename]`），并记录警告，确保正文其余部分正常产出。

#### P2-2｜`TypesetPreview` 对大型 ProseMirror JSON 树启用 Deep Watch
- **文件与行号**：`src/shared/editor/TypesetPreview.vue:48`
- **问题代码**：
  ```typescript
  watch(() => [props.document, props.layout], schedule, { deep: true, immediate: true });
  ```
- **机理分析**：ProseMirror 文档 JSON 是一个层级极深、对象数量庞大的 AST 结构。在长文书场景下，`{ deep: true }` 迫使 Vue 响应式系统在每次敲击键盘（触发 transaction）后对整棵树的成百上千个属性进行深层脏检查，消耗主线程 CPU 并引发编辑器卡顿。
- **修复方案**：移除 `{ deep: true }`，改用轻量级的时间戳/版本标记（如 `docVersion` 或引用指针更新）触发排版更新。

---

### 【P3 级问题 · 低危 / 内存与字符串开销】

#### P3-1｜`export_editor_document` 中使用 `document.to_string().len()` 进行体积检查
- **文件与行号**：`src-tauri/src/commands/docs.rs:141`
- **机理分析**：为了做 40 MiB 大小防御校验，执行了 `document.to_string().len()`，在内存中将整个 JSON Value 完整重新格式化为长字符串，在导出数十兆大型文书时产生瞬间的无谓内存毛刺与 CPU 序列化负担。
- **修复方案**：限制前端传递的原始字符串长度，或在接收反序列化阶段直接基于流大小进行限额拦截。

---

## 模块五：案卷、文件与处理中心 (Files & Processing Center)

- **涉及范围**：
  - 后端：`src-tauri/src/commands/files.rs`、`commands/conversion.rs`、`commands/processing.rs`、`src-tauri/src/document_pipeline.rs`
  - 前端：`src/modules/files/views/CaseFilesView.vue`、`src/shared/components/ProcessingCenter.vue`、`src/shared/components/FileConversionDialog.vue`、`src/stores/files.ts`
- **审查结论**：案卷文件管理具备完善的文件重命名、归卷、OCR 转换流水线、跨格式转换（PDF/Docx/图片/音视频转写）与全局统一处理中心；但存在**1 项批量操作 I/O 放大 P1**、**2 项高频轮询导致的资源空转与多重级联重载 P2**，以及 **1 项短连接频繁开关 P3**。

### 【P1 级问题 · 高危 / 性能雪崩与 I/O 放大】

#### P1-1｜`relocate_files` 在循环内逐项触发全量知识库扫描与正则重写
- **文件与行号**：`src-tauri/src/commands/files.rs:918, 946-958`
- **问题代码**：
  ```rust
  for item in &input.items {
      // ... 移动/重命名单个文件 ...
      if let Err(e) = relocate_knowledge_references(&db, &old_rel, &new_rel) {
          eprintln!("[files] failed to update knowledge references for {}: {}", old_rel, e);
      }
  }
  ```
- **机理分析**：
  在 `relocate_knowledge_references` 中，程序执行 `SELECT id, content FROM knowledge_items WHERE content LIKE ?`，拉取所有候选知识条目并在内存中反序列化、执行正则替换后写回数据库。
  当律师在案卷视图进行批量重命名（例如将 50 份证据文件统一加上前缀），或者重命名/移动一个包含数十份文件的目录时，该循环将连续执行 50 次全量知识库遍历扫描与单独事务写回。这会引发剧烈的磁盘 I/O 风暴和长时间的主线程/SQLite 锁等待，甚至可能引发操作超时或 UI 卡死。
- **修复方案**：
  将路径重定位改为“批量收集 + 单次扫库”模式：先在循环中收集所有 `(old_path, new_path)` 映射对；在文件移动成功后，单次执行 `relocate_knowledge_references_batch(&db, &mappings)`，在一个数据库事务内仅对知识库进行单次遍历和批量正则替换。

---

### 【P2 级问题 · 中危 / 资源空转与视图频繁重载】

#### P2-1｜全局 `ProcessingCenter.vue` 常驻无休止 5 秒双重 5 表 UNION ALL 轮询
- **文件与行号**：
  - 前端：`src/shared/components/ProcessingCenter.vue:35-51`
  - 后端：`src-tauri/src/commands/processing.rs:116-150`
- **机理分析**：
  `ProcessingCenter.vue` 是挂载在 App 根部的全局后台任务中心组件。在组件 `onMounted` 后，启动了每 5 秒一次的定时间隔轮询 `get_processing_center`。
  无论右侧抽屉（Drawer）是否展开，也无论当前系统是否有任何正在运行的任务（`activeCount === 0`），该 5 秒定时器永远在后台无休止触发。而在后端 `processing.rs:116-150` 中，每一次调用都会在 `document_processing_jobs`、`knowledge_index_jobs`、`processing_activities`、`reminder_jobs`、`ai_runs` 这 5 张表之间并行执行两次多表 `UNION ALL` 查询。在用户日常休眠或常驻后台时，造成无谓的轻度电量消耗与 SQLite 锁调度开销。
- **修复方案**：
  引入自适应动态退避（Adaptive Backoff）：当抽屉处于折叠状态且连续两次查询 `activeCount === 0` 时，将轮询间隔拉长至 30 秒或暂停轮询；并在有新任务提交（如触发 OCR、转换、导入）时通过前端事件总线主动唤醒轮询。

#### P2-2｜`CaseFilesView` 定时器因进度数字微调级联触发并发 3 重文件重载
- **文件与行号**：`src/modules/files/views/CaseFilesView.vue:449-460`
- **问题代码**：
  ```typescript
  const nextJobSignature = JSON.stringify(jobs.value);
  if (nextJobSignature !== lastJobSignature) {
    lastJobSignature = nextJobSignature;
    void loadFiles();
  }
  ```
- **机理分析**：
  `CaseFilesView` 每 3 秒拉取一次 `loadDocumentJobs()`。它使用 `JSON.stringify(jobs.value)` 作为签名。由于后台任务对象中包含实时更新的 `progress`（例如 OCR 识别进度从 12% 变为 15%）或更新时间戳，导致几乎每一次轮询都会被判定为“签名改变”，进而级联触发 `loadFiles()`。
  而 `loadFiles()` 会瞬间并发打出 3 个重量级 IPC 命令（`list_case_files`、`list_removed_files`、`list_case_directories`），扫描磁盘目录树与数据库。在文件处理（如 OCR、转码）期间，会导致案卷列表高频重新渲染、滚动条跳动、选区丢失以及大量冗余磁盘 I/O。
- **修复方案**：
  计算 Job 签名时，仅提取核心状态枚举字段（如 `job.id + ':' + job.status`），过滤掉高频变动的 `progress` 和 `updated_at`；只有当任务发生 `running -> completed / failed` 状态跃迁时才触发 `loadFiles()`。

---

### 【P3 级问题 · 低危 / 短连接高频创建与销毁】

#### P3-1｜`document_pipeline` OCR 轮询中每秒反复创建销毁独立 SQLite 连接
- **文件与行号**：`src-tauri/src/document_pipeline.rs:341-360`
- **机理分析**：在 OCR 文档管道监控子线程中，为了检查任务是否被取消或处于 running 状态，每隔 1 秒调用一次 `check_job_running`。该函数每次执行均调用 `Database::open(&db_path)`，开启一个新的 SQLite 文件句柄并在查完单条记录后立即销毁关闭。在数十分钟的批量 OCR 场景下，会产生数万次系统调用与文件锁争用。
- **修复方案**：在管道监控上下文中保持复用单一连接句柄或使用连接池。

---

## 模块六：知识库、向量与全局搜索 (Knowledge & Search)

- **涉及范围**：
  - 后端：`src-tauri/src/commands/knowledge.rs`、`commands/search.rs`、`db/knowledge_index.rs`、`db/vector_index.rs`、`background_jobs.rs`
  - 前端：`src/components/GlobalSearch.vue`、`src/modules/knowledge/views/KnowledgeNotebookView.vue`、`KnowledgeGraphView.vue`、`src/modules/knowledge/composables/useNotebookSave.ts`
- **审查结论**：知识库具备卡片盒/双链笔记（Obsidian 风格）、Trilium 式父子层级提升、基于 Zvec 的分段向量混合检索（FTS + 语义）与版本快照追溯；但存在**2 项外键破坏与中文搜索盲区 P1**、**3 项后台无休止写事务/N+1 查询/版本 Diff 失真 P2**，以及 **1 项图谱主线程阻塞 P3**。

### 【P1 级问题 · 高危 / 数据外键破坏与搜索功能缺陷】

#### P1-1｜`tasks.knowledge_id` 缺失级联置空导致关联任务的笔记无法被删除
- **文件与行号**：
  - 后端删除：`src-tauri/src/commands/knowledge.rs:780-793`
  - 数据库定义：`src-tauri/src/db/schema.rs:2170`
- **问题代码**：
  ```sql
  -- schema.rs:2170
  ALTER TABLE tasks ADD COLUMN knowledge_id TEXT REFERENCES knowledge_items(id);
  ```
  ```rust
  // knowledge.rs:780-793
  tx.execute("UPDATE knowledge_items SET parent_id = ?2 WHERE parent_id = ?1", rusqlite::params![id, parent])?;
  tx.execute("DELETE FROM links WHERE (source_type='knowledge' AND source_id=?1) OR (target_type='knowledge' AND target_id=?1)", [&id])?;
  tx.execute("DELETE FROM knowledge_items WHERE id = ?1", [&id])?;
  ```
- **机理分析**：
  在数据库中，`tasks.knowledge_id` 建立了指向 `knowledge_items(id)` 的外键，但未指定 `ON DELETE SET NULL`（SQLite 默认为 `RESTRICT / NO ACTION`）。而在 `delete_knowledge` 的事务内，代码处理了子笔记提升和 `links` 关系的清理，却**遗漏了清空 `tasks.knowledge_id`**。
  由于系统在所有 SQLite 连接上严格启用了 `PRAGMA foreign_keys = ON`，一旦某篇知识库笔记被关联到了某个任务，用户在知识库中点击“删除笔记”时，SQLite 会直接抛出 `FOREIGN KEY constraint failed (code 787)` 错误，整个删除事务强制回滚，导致该笔记永远无法删除。
- **修复方案**：
  在 `delete_knowledge` 事务中执行 `tx.execute("UPDATE tasks SET knowledge_id = NULL WHERE knowledge_id = ?1", [&id])?;`；同时在数据库迁移补齐中加上防御。

#### P1-2｜`global_search` 盲区：案卷全文完全漏接且中文分词匹配全面失效
- **文件与行号**：
  - 后端：`src-tauri/src/commands/search.rs:20-52`
  - 前端：`src/components/GlobalSearch.vue:81-86`
- **机理分析**：
  1. **前端漏接**：`GlobalSearch.vue`（律师敲击 ⌘K 打开的全局速搜浮层）在并行发起的请求中，仅查询了任务、案件、知识与项目，完全未接入后端已完成的 `file`（案卷文件及 OCR 文档全文）检索流，导致全局速搜根本搜不到案卷或扫描件。
  2. **后端中文分词失效**：`search.rs` 的 `global_search_inner` 对 `knowledge_fts` 与 `files_fts` 直接使用 `MATCH ?1`（传入带双引号的检索词）。而这两个 FTS5 虚表使用了 SQLite 默认的 `unicode61` 分词器（按西方空格断词），对非空格分隔的连续中文短语（如“建设工程”、“质证意见”）不具备子串检索能力。虽然在 `knowledge.rs` 的单项搜索中为了规避此问题编写了 `LIKE %query%` 兜底，且 V26 迁移增加了带 trigram 分词器的 `knowledge_trigram` 表，但 `global_search_inner` 完全没有使用 `knowledge_trigram`，亦未加 LIKE 兜底，导致用户在全局搜索中文笔记或案卷名时经常返回空结果。
- **修复方案**：
  后端 `global_search_inner` 改用 `knowledge_trigram`，并对 `files_fts` 补充中文 `LIKE` 兜底；前端 `GlobalSearch.vue` 将 `file` 类别及 OCR 命中页码（如 `[p4]`）整合到全局搜索结果列表中。

---

### 【P2 级问题 · 中危 / 磁盘写放大、N+1 查询与版本对比失真】

#### P2-1｜常驻后台每 3 秒触发双重 SQLite 写入事务导致严重空转写放大
- **文件与行号**：
  - 调度器：`src-tauri/src/background_jobs.rs:184-194`
  - 状态同步：`src-tauri/src/db/vector_index.rs:468-503`
  - 写入逻辑：`src-tauri/src/processing.rs:36-42`
- **机理分析**：
  在后台异步线程中，当知识库索引队列为空（`process_next() -> Ok(false)`）时，线程仅休眠 3 秒后即刻再次执行 `vector_index::prepare()`，且在每次前后都无条件调用 `processing::service("vectors", ...)`。
  这会导致 `processing_activities` 表每 3 秒被执行 2 次更新写入事务。即使电脑合盖静置或整天无任何编辑操作，后台仍在以每秒接近 1 次的频率在磁盘上提交物理事务并写入 WAL 日志，引发无谓的磁盘磨损和能耗；同时与 M5 审查中发现的前端 `ProcessingCenter.vue` 5 秒轮询形成共振。
- **修复方案**：
  引入自适应退避与事件驱动：当知识库队列为空且索引 manifest revision 一致时，退避至 60 秒检查；且在 `status` 和 `stage` 未发生实质变化时禁止重复写入 `processing_activities`。

#### P2-2｜向量检索候选重排时产生数百次单条 N+1 SQLite 查询
- **文件与行号**：`src-tauri/src/db/vector_index.rs:394-409`
- **机理分析**：
  在 `VectorIndex::search` 中，为了从 Zvec 召回的候选分段中获取真实向量字节和正文计算余弦相似度，外层循环 4 轮，内层对召回的最多 512 个分段逐条执行：
  `conn.query_row("SELECT j.item_id, c.embedding, c.content FROM knowledge_index_chunks WHERE job_id=?1 AND chunk_index=?2 ...")`。
  单次检索会连续打出几百次单行 SQLite 查询，且在此期间全局互斥锁 `CACHE.lock()` 被独占持有，严重拖慢搜索响应并阻塞其他并发语义查询。
- **修复方案**：
  先批量收集所有召回的 `(job_id, chunk_index)` 元组，拼接为单次批量 `IN` 查询或使用临时表关联，一次性把分段数据拉入内存。

#### P2-3｜版本差异对比使用素朴同位素比对导致整篇文档误报全量删改
- **文件与行号**：`src-tauri/src/commands/knowledge.rs:945-965`
- **问题代码**：
  ```rust
  let max_len = lines1.len().max(lines2.len());
  for i in 0..max_len {
      let old_line = lines1.get(i).copied();
      let new_line = lines2.get(i).copied();
      match (old_line, new_line) {
          (Some(o), Some(n)) if o == n => { ... "equal" ... }
          (Some(o), Some(n)) => { ... "removed" ... "added" ... }
          ...
      }
  }
  ```
- **机理分析**：
  `diff_knowledge_versions` 没有使用最长公共子序列（LCS / Myers Diff）算法，而是简单地将两个版本的同行号 `lines1[i]` 与 `lines2[i]` 强制比对。如果用户仅仅在 500 行的笔记开头插入了 1 行新标题，从第 2 行开始直到末尾的所有行号均发生 1 行偏移，算法会将后续所有原本完全一样的 500 行全部标记为“红色删除 + 绿色新增”，导致版本对比界面彻底失真。
- **修复方案**：
  替换为基于 LCS / Myers 的标准行级差异比对算法，准确定位实际新增和删除的行块。

---

### 【P3 级问题 · 低危 / 渲染主线程同步物理模拟】

#### P3-1｜知识图谱在主线程同步执行 160 次全对力导向斥力计算
- **文件与行号**：`src/modules/knowledge/views/KnowledgeGraphView.vue:74-113`
- **机理分析**：在 `KnowledgeGraphView` 加载时，`loadGraph` 在主线程内一次性同步执行 160 轮物理模拟迭代。由于每轮迭代均包含 $O(N^2)$ 的两两节点斥力计算，当笔记节点数较多时，会导致页面初次进入时产生数十毫秒的主线程卡顿。
- **修复方案**：改用 `requestAnimationFrame` 进行逐帧迭代松弛渲染，或将物理计算移入 Web Worker 中完成。

---

## 模块七：白板与事实图谱 (Whiteboard & Fact Graph)

- **涉及范围**：
  - 后端：`src-tauri/src/commands/whiteboard.rs`、`commands/whiteboard_document.rs`、`commands/whiteboard_scene.rs`
  - 前端：`src/modules/whiteboard/views/WhiteboardView.vue`、`components/RichCanvas.vue`、`components/WhiteboardSourcePicker.vue`、`lib/document.ts`
- **审查结论**：白板基于 Excalidraw 实现了 LiquidText 风格事实节点与自由绘图的原子化一体提交（绘制、事实、出处与连线事务同生共死）、多版本快照与审讯/证据回溯；但存在**1 项剪贴板复制粘贴破坏唯一标识阻断保存 P1**、**3 项来源检索正文漏检/孤儿审计记录/高频 PNG 预览卡顿 P2**，以及 **1 项误删文字导致整板不可保存 P3**。

### 【P1 级问题 · 高危 / 剪贴板复制卡片触发重复 ID 阻断保存】

#### P1-1｜通过 ⌘C / ⌘V 复制事实卡片导致整块白板永久性无法保存
- **文件与行号**：
  - 前端：`src/modules/whiteboard/components/RichCanvas.vue:93-104` 与 `lib/document.ts:19-28`
  - 后端校验：`src-tauri/src/commands/whiteboard_document.rs:108-113`
- **问题代码**：
  ```rust
  // whiteboard_document.rs:108-113
  let mut ids = HashSet::new();
  for fact in &input.facts {
      ensure!(
          !fact.id.is_empty() && ids.insert(fact.id.clone()),
          "重复或无效的事实标识"
      );
  }
  ```
- **机理分析**：
  在 Excalidraw 画布中，律师经常使用 ⌘C / ⌘V 快捷键复制已存在的事实卡片，以便在此基础上快速修改摘录或关联多条因果链。
  前端在 `RichCanvas.vue` 中仅为 `onDuplicate`（Alt + 拖拽或 ⌘D 快捷键）注册了 `remap.set(..., crypto.randomUUID())` 钩子；但 Excalidraw 的剪贴板原生粘贴（⌘V）并不会触发 `onDuplicate` 回调。这导致粘贴出来的新卡片原样复制了旧卡片的 `customData.casy.fact.id`。
  当 900ms 自动保存触发时，前端 `collect()` 会收集到包含两个相同 `id` 的事实数组并提交至后端。后端 `whiteboard_document.rs:108-113` 进行了硬性 `ids.insert(...)` 校验，直接抛出 `Err("重复或无效的事实标识")`。
  前端捕获该异常后进入 `failed.value = true` 红色告警状态，且后续的所有自动或手动保存均被此硬错误阻断，导致用户的整个白板编辑成果卡死无法落盘。
- **修复方案**：
  在前端 `collect()`（`lib/document.ts`）或 Excalidraw 状态监听层加入自动去重防线：遍历收集事实时，一旦发现同一个 `fact.id` 出现多次，立刻为后续副本卡片分配全新的 `crypto.randomUUID()` 并同步回写其 element 的 `customData.casy.fact.id`，杜绝重复 ID 传入后端。

---

### 【P2 级问题 · 中危 / 来源检索正文失效、孤儿日志与预览 CPU 顿挫】

#### P2-1｜引用来源选择器宣称支持“正文搜索”但对卷宗文件仅匹配文件名
- **文件与行号**：
  - 前端界面：`src/modules/whiteboard/components/WhiteboardSourcePicker.vue:27`
  - 后端查询：`src-tauri/src/commands/whiteboard_document.rs:279`
- **问题代码**：
  ```rust
  // whiteboard_document.rs:279
  (format!("SELECT f.id,'file',f.file_name,f.case_id,c.case_name,substr(COALESCE(f.ocr_text,''),1,1600)... FROM case_files f ... WHERE f.deleted_at IS NULL AND f.case_id {} ?2 AND instr(lower(f.file_name),lower(?1))>0", ...), vec![query, case_id])
  ```
- **机理分析**：
  前端 `WhiteboardSourcePicker.vue` 弹窗搜索框的 Placeholder 明确提示：“搜索文件名、知识标题或正文”。
  在后端查询中，对 `knowledge` 确实检索了 `title` 和 `content`；但对 `file`（案卷文件），SQL 的 `WHERE` 子句中仅仅包含了 `instr(lower(f.file_name),lower(?1))>0`。
  虽然 `SELECT` 字段中拉取了 `substr(COALESCE(f.ocr_text,''),1,1600)`，但 `ocr_text` 完全没有被加入 `WHERE` 条件。当律师输入案涉关键事实词汇（如“违约责任”、“不可抗力条款”）寻找案卷证据作为白板来源时，只要该词汇不在文件名中，就会返回 0 条匹配，使“正文搜索”功能名存实亡。
- **修复方案**：
  在 `case_files` 的 `WHERE` 检索条件中补充 `OR instr(lower(COALESCE(f.ocr_text,'')), lower(?1))>0`。

#### P2-2｜删除白板遗留大量无主孤儿 `audit_events` 审计记录
- **文件与行号**：
  - 删除命令：`src-tauri/src/commands/whiteboard.rs:120-127`
  - 触发器：`src-tauri/src/db/schema.rs:842-847`
- **机理分析**：
  当用户删除白板时，`delete_whiteboard` 执行 `DELETE FROM whiteboards WHERE id = ?1`。由于数据库配置了级联删除，白板所属的全部 `fact_nodes` 被自动删除，从而触发了 `fact_history_delete` 触发器，向 `audit_events` 表突发插入了大量 `fact_deleted` 事件记录。
  然而，`audit_events` 是系统全局通用审计表，其 `aggregate_id` 只是无外键约束的字符串。白板被整体删除后，这些残留的审计事件不会被级联清理，永久留在数据库中成为无法被查阅或维护的孤儿垃圾数据。
- **修复方案**：
  在 `delete_whiteboard` 执行前或事务内显式清理：`DELETE FROM audit_events WHERE aggregate_type = 'whiteboard' AND aggregate_id = ?1;`。

#### P2-3｜900ms 自动保存循环高频调用 `exportToBlob` 导致复杂画布主线程卡顿
- **文件与行号**：`src/modules/whiteboard/components/RichCanvas.vue:44, 56-59`
- **机理分析**：
  在白板编辑中，只要有任何鼠标拖拽或文字变动，就会启动 900ms 防抖自动保存 `flush()`。在 `flush()` 流程中，前端无条件调用 `editor.exportToBlob({...scene, maxWidthOrHeight: 640})` 来生成 PNG 缩略图。
  `exportToBlob` 会在前端 Offscreen Canvas 上对所有的矢量图形、文本字体和已贴入的 Base64 截图进行全量光栅化重绘并转码为 Base64。在包含数张证据截图或数十个事实节点的复杂白板中，每次自动保存都会强占主线程数十至上百毫秒，造成律师在打字输入和连线拖拽时出现明显的顿挫掉帧与内存抖动。
- **修复方案**：
  将缩略图生成与数据保存彻底解耦：日常 900ms 防抖保存只提交 `sceneJson` 与事实关系；全量缩略图仅在用户切走页面（`onBeforeRouteLeave`）、组件卸载或连续 10 秒以上无操作的闲置期才按需触发生成。

---

### 【P3 级问题 · 低危 / 误删文字导致卡片断言失败】

#### P3-1｜清空卡片文字后按 Esc 触发后端“事实缺少文字”硬报错
- **文件与行号**：`src-tauri/src/commands/whiteboard_document.rs:163-167`
- **机理分析**：如果用户在 Excalidraw 双击卡片文字并将文字全部删除后退出编辑，或者使用橡皮擦擦除了文字，矩形卡片仍在但文字元素已被标记为 `isDeleted: true`。后端 `whiteboard_document.rs` 在保存时执行 `elements.iter().find(|e| e["id"] == text_id && e["isDeleted"] != true).ok_or_else(|| anyhow::anyhow!("事实卡片缺少文字"))?`，会判定整个文档不合法并硬性拒绝保存。
- **修复方案**：在前端 `collect()` 时若发现卡片文字被完全清除，自动回退兜底为默认占位符（如“无标题事实”），或由后端容错自动补偿文字元素，而非阻断保存。

---

## 模块八：收件箱与大口袋 (Inbox & Pocket)

- **涉及范围**：
  - 后端：`src-tauri/src/commands/inbox.rs`、`commands/inbox_actions.rs`
  - 前端：`src/modules/inbox/views/InboxView.vue`、`src/modules/inbox/composables/useVoiceNote.ts`、`useCapture.ts`、`src/shared/components/UnifiedCaptureDialog.vue`
- **审查结论**：收件箱具备跨来源捕获（文件/文字/截图/微信复制）、基于本地正则与 AI 混合的文书类型与案号自动推荐、全动作幂等提交（`inbox_action_results`）；但存在**1 项在野测试断言失败阻断 P0**、**1 项核心流转动作界面缺失 P1**、**2 项占位存根与假 Composable 造成静默丢数据 P2**，以及 **1 项内存数组暴增 P3**。

### 【P0 级问题 · 在野 / 单元测试时效性硬崩溃】

#### P0-1｜自然语言日期推断算法时态敏感导致后端单元测试必败
- **文件与行号**：
  - 核心逻辑：`src-tauri/src/commands/inbox.rs:2047-2055`
  - 崩溃测试：`src-tauri/src/commands/inbox.rs:2664-2667`
- **问题代码**：
  ```rust
  // inbox.rs:2047-2055
  if let Some(dt) = chrono::NaiveDate::from_ymd_opt(today.year(), m, d) {
      if dt >= today {
          return Some(dt);
      } else if let Some(next_year_dt) = chrono::NaiveDate::from_ymd_opt(today.year() + 1, m, d) {
          return Some(next_year_dt);
      }
  }

  // inbox.rs:2664-2667 (test_all_natural_language_dates_recognition)
  assert_eq!(
      extract_date_hint("九月十五日截止"),
      Some(format!("{}-09-15", today.year()))
  );
  ```
- **机理分析**：
  运行 `cargo test --lib inbox` 时测试直接 Panic 崩溃：
  `assertion failed: left == right, left: Some("2027-09-15"), right: Some("2026-09-15")`。
  机理在于：测试硬编码断言“九月十五日”推断出的年份恒等于 `today.year()`。而 `extract_date_hint` 调用了 `chrono::Local::now()`。当当前系统日期晚于 9 月 15 日（例如 9 月 17 日）时，算法判定该日期已经处于过去，因此将其自动进位到了下一年（2027 年）。这不仅导致 CI/Cargo 单元测试在每年 9 月 15 日之后永久性测试失败，在业务逻辑上也是错误的——如果律师在 9 月 17 日补录两日前发生的沟通事实（“九月十五日收到调解书”），算法会荒谬地将任务推迟整整一年到 2027 年。
- **修复方案**：
  1. 测试代码必须注入可控的固定基准 Mock 时间，消除宿主时钟不确定性；
  2. 日期推导增加语境时态判断（如包含“曾/已/收到”等过去完成时态，或月日差距在合理回顾窗口内的，默认绑定当年年份）。

---

### 【P1 级问题 · 高危 / 核心流转动作在主界面缺失】

#### P1-1｜收件箱主界面缺少“归卷至案件”与“沉淀知识库”核心闭环入口
- **文件与行号**：
  - 前端界面：`src/modules/inbox/views/InboxView.vue:165-208, 358-396`
  - 后端实现：`src-tauri/src/commands/inbox_actions.rs:6` 与 `commands/inbox.rs:2252`
- **机理分析**：
  收件箱（GTD Inbox）作为全系统知识与证据流转的第一入口，后端完整实现了 `file_to_case`（归卷）、`save_knowledge`（沉淀笔记）、`create_event`（建日程）等动作；浮层对话框 `UnifiedCaptureDialog.vue` 中也支持此链路。
  但在核心的日常工作主界面 `InboxView.vue` 中，右侧整理面板（Clarify Panel）底部**仅有且只有一个“转为任务”按钮**（`action: 'create_task'`，可选行动/委派/等待/将来）。
  如果律师在收件箱中导入了一批证据扫描件、裁判文书，或者速记了一段业务理论，律师在主视图中**根本找不到“归卷到案件”或“保存到知识库”的入口**，只能被强制创建为 GTD 任务。
- **修复方案**：
  在 `InboxView.vue` 整理表单的处理方式中，增加“归卷入案”、“沉淀知识库”单选项，对齐调用 `casyContext.inbox.confirmAction`。

---

### 【P2 级问题 · 中危 / 伪功能存根导致数据静默丢失与文件名推断失效】

#### P2-1｜语音速记与剪贴板后台均为占位存根且前端静默吞没录音数据
- **文件与行号**：
  - 后端存根：`src-tauri/src/commands/inbox.rs:2327-2345`
  - 前端丢失：`src/modules/inbox/composables/useVoiceNote.ts:88-100` 与 `useCapture.ts`
- **机理分析**：
  前端封装了看似完整的 `useVoiceNote.ts`（录制麦克风音频，Base64 转码并调用 `save_voice_note`）。但后端对应的 `save_voice_note`、`transcribe_voice_note`、`capture_screenshot`、`capture_clipboard` 全是未完成的桩代码（直接硬编码返回 `Err("功能开发中，敬请期待")`）。
  更严重的是，`useVoiceNote.ts:93` 在 `result.ok` 为 false 时没有做任何错误捕获、Toast 提示或告警，而是直接结束函数，导致律师花费数分钟录制的音频在点击停止后**毫无声息地直接被丢弃**。
- **修复方案**：
  后端暂未实现时，前端 composable 必须抛出显式的拒绝提示或禁用前端录音逻辑，严禁让用户在毫无知觉的情况下录制并丢弃数据。

#### P2-2｜智能分类在有源文件时误用自定义标题导致案号与分类识别全面失效
- **文件与行号**：`src-tauri/src/commands/inbox.rs:1283-1290`
- **问题代码**：
  ```rust
  let mut result = if source_path.is_some() {
      let file_name = title.as_deref().unwrap_or("");
      ...
      quick_judge(&conn, file_name, file_size, ...)
  }
  ```
- **机理分析**：
  在 `quick_judge_inbox_item` 中，当收件项拥有本地文件来源时（`source_path.is_some()`），传入 `quick_judge` 算法的文件名竟然取自 `title`。如果收件项标题被用户或者邮件规则命名为“今日补充材料”或“当事人微信发来”，而磁盘实际文件名为 `（2024）京01行初123号开庭传票.pdf`，算法将以“今日补充材料”为输入进行正则匹配。
  这导致原本能够精准匹配的案号（`（2024）京01行初123号`）与文书分类（开庭传票）完全失效，推荐结果降级为“未识别”。
- **修复方案**：
  若 `source_path` 存在，优先通过 `Path::new(path).file_name()` 提取磁盘真实文件名参与 `quick_judge`。

---

### 【P3 级问题 · 低危 / 内存展开爆炸】

#### P3-1｜`useVoiceNote` 将音频 ArrayBuffer 展开为 Number 数组引发内存激增
- **文件与行号**：`src/modules/inbox/composables/useVoiceNote.ts:85`
- **机理分析**：`Array.from(new Uint8Array(await audioBlob.arrayBuffer()))` 将二进制数据展开为普通的 JS 数组。一段 10MB 的音频展开为千万级元素的对象数组，在 V8 引擎中占用 80MB+ 内存并在主线程引发严重卡顿。
- **修复方案**：改用 Base64 编码或直接传递 `Uint8Array` / 原生二进制流。

---

## 模块九：AI 智伴、MCP 与安全审计 (AI, MCP & Security)

- **涉及范围**：
  - 后端：`src-tauri/src/ai/`（`mod.rs`、`gateway.rs`、`profiles.rs`、`document_match.rs`、`page_index.rs`、`distillation.rs`）、`src-tauri/src/mcp/`（`server.rs`、`mod.rs`）、`src-tauri/src/commands/ai_routes.rs`
  - 前端：`src/modules/ai/components/AIChatPanel.vue`、`src/modules/ai/composables/tool-caller.ts`、`useAIChat.ts`、`src/shared/components/ProposalDiffCard.vue`、`src/stores/aiSettings.ts`
- **审查结论**：AI 模块具备三级敏感权限隔离（L1 纯提示词 / L2 辅助草拟 / L3 提案确认）、完备的 MCP Server 本地工具协议实现、本地 Ollama / OpenAI / Claude 多后端轮换与混合分段检索匹配；但存在**2 项工具调用死循环与系统提示词重复注入 P1**、**2 项提案 5 分钟过期核对失效与工具返回结构截断破坏 P2**，以及 **1 项编译器无用字段告警 P3**。

### 【P1 级问题 · 高危 / 循环风暴与提示词重复注入】

#### P1-1｜工具调用异常反馈机制缺失熔断导致 12 轮循环 Token 耗尽风暴
- **文件与行号**：`src/modules/ai/composables/tool-caller.ts:201-240`
- **问题代码**：
  ```typescript
  // tool-caller.ts:220-230
  try {
    const rawResult = await executeTool(toolCall.function.name, args);
    // ...
  } catch (err) {
    const errorMsg = err instanceof Error ? err.message : String(err);
    toolResults.push({
      role: 'tool',
      tool_call_id: toolCall.id,
      content: `[${toolCall.function.name}] 执行失败: ${errorMsg}`,
    });
  }
  ```
- **机理分析**：
  `chatWithTools` 允许多轮自动工具调用（最大深度 `MAX_ROUNDS = 12`）。当模型生成了错误的参数类型（例如将必填的数组传为对象），或者由于幻觉调用了不存在的工具名时，`executeTool` 抛出异常。
  代码捕获后将错误文本原样构造成 `role: 'tool'` 消息注入上下文并立刻进行下一轮递归调用。
  对于参数理解能力较弱的中小模型（如本地部署的 Qwen 7B/14B 或小型量化模型），在收到该报错后往往无法自行修正参数语法，而是倾向于使用略微改动的相同错误参数反复尝试。这导致程序进入无休止的重试循环，打满全部 12 轮限制。不仅造成用户界面等待长达数分钟，且单次对话会消耗数万无谓的 Token，严重浪费 API 额度与计算资源。
- **修复方案**：
  引入工具级熔断与重试限额：同一工具连续调用失败 2 次，或者参数校验失败超过 2 次时，立即阻断自动工具重试；并在提示语中显式要求模型“不要重试该工具，请直接用普通文本向用户说明参数不足或寻求人工补充信息”。

#### P1-2｜多轮工具调用循环中 System Prompt 逐轮重复追加导致上下文污染
- **文件与行号**：
  - 后端：`src-tauri/src/ai/mod.rs:1095-1120`
  - 前端：`src/modules/ai/composables/tool-caller.ts:188`
- **机理分析**：
  在后端 `ai_chat` 的入参处理中，无条件执行了 `messages.insert(0, ChatMessage { role: "system", content: system_prompt })`。
  而在前端 `tool-caller.ts:188` 启动工具调用调度循环时，第一轮就已在 `messages[0]` 塞入了一份包含全部工具清单与安全规则的前端 System 提示词。
  当工具链连续触发 3~4 轮往返时，由于前端将每次交互后的 `messages` 完整发回后端，后端每轮均在数组最头部重复 `insert(0, ...)`，导致发往大模型网关的请求中出现了多个彼此重复甚至内容冲突的 `system` 消息。在 OpenAI、Anthropic 以及部分标准网关中，这不仅会浪费宝贵的上下文窗口，甚至可能被直接判定为请求协议非法（400 Bad Request）。
- **修复方案**：
  在后端 `ai_chat` 插入前先检测 `messages` 列表的第一个元素是否已经为 `system` 角色。若存在则将其合并或跳过，确保单次调用有且仅有一个规范顶层 System 提示词。

---

### 【P2 级问题 · 中危 / 审批时效阻断与截断破坏 JSON 语义】

#### P2-1｜L3 变更提案 Proposal 5 分钟硬超时阻断律师跨端交叉核对
- **文件与行号**：
  - 后端校验：`src-tauri/src/ai/gateway.rs:239-245`
  - 前端展示：`src/shared/components/ProposalDiffCard.vue`
- **机理分析**：
  系统在执行修改文书、删除案卷、更名等高危 L3 动作时，会由网关生成带预校验哈希（`pre_state_hash`）的提案卡片供律师人工审核确认。后端设定了 5 分钟硬性过期时间（`proposal.expires_at < now`）。
  但在真实的复杂商事诉讼执业场景中，律师在面对一份包含多处事实判决引述或程序变更的文书修改提案时，经常需要切走窗口到微信群与当事人确认，或翻阅实体卷宗核对。这一核对过程通常会持续 10~15 分钟。
  当律师回到 Casy 界面点击“批准并执行”时，后端直接返回“提案已过期”，且前端没有提供“重新校验状态并一键刷新”的功能，迫使律师必须把整轮对话重新组织录入一遍。
- **修复方案**：
  当提案过期被拦截时，提供“重新校验并延期”接口：若底层数据实体的当前哈希仍然等于 `pre_state_hash`（即过期期间无任何第三方修改），允许用户一键延长有效期并直接执行，兼顾数据原子性与真实工作习惯。

#### P2-2｜工具调用返回结果粗暴切片破坏 JSON 闭合结构
- **文件与行号**：`src/modules/ai/composables/tool-caller.ts:145`
- **问题代码**：
  ```typescript
  const serialized = typeof result === 'string' ? result : JSON.stringify(result);
  return serialized.length > 12000 ? serialized.slice(0, 12000) + '…（已截断）' : serialized;
  ```
- **机理分析**：
  当工具返回的是长结构化数据（如查询全案事实节点列表、大量搜索命中片段）时，代码直接对整个序列化字符串执行 `slice(0, 12000)`。
  这会导致 JSON 字符串在中途被硬生生切断，留下未闭合的括号或引号（如 `{"id": "abc", "text": "某某`）。模型在接收到残缺的 JSON 结构后，下游的 JSON 解析器会产生语法解析崩溃，或引发模型的格式解析幻觉。
- **修复方案**：
  改用结构感知的对象级截断：对数组类型限制条目数量（如仅保留前 15 条），或截断对象内单项的长正文字段并保留合法的 JSON 闭合外壳。

---

### 【P3 级问题 · 低危 / 编译器告警与未读字段】

#### P3-1｜测试构建存在 5 项编译器死代码与无用 mut 告警
- **文件与行号**：
  - `src-tauri/src/parse/pdf_extractor.rs:11`（`extract_pdf_to_markdown` 未使用）
  - `src-tauri/src/docsy_engine/style.rs:9-11`（`page_width`, `page_height`, `margin` 未读取）
  - `src-tauri/src/commands/tasks.rs:1806, 1959, 1990`（变量无需 `mut`）
- **机理分析**：Cargo 测试与构建时持续产生 5 项警告，对日常构建和 CI 检查造成噪点干扰。
- **修复方案**：清理无用的 `mut` 标记，对尚未使用的解析函数增加 `#[allow(dead_code)]`。

---

## 模块十：核心通信与系统基础设施 (Core & System Infrastructure)

- **涉及范围**：
  - 后端：`src-tauri/src/db/`（`mod.rs`、`schema.rs`）、`src-tauri/src/background_jobs.rs`、`workspace_sync.rs`、`credentials/mod.rs`、`runtime_paths.rs`、`commands/settings.rs`
  - 前端：`src/core/tauriBridge.ts`、`src/core/tauriEvents.ts`、`src/core/plugin/context.ts`、`src/router/index.ts`、`src/stores/settings.ts`、`src/App.vue`
- **审查结论**：系统底层具备基于 SQLCipher 的全库透明加密、多 Profile 资料库目录级互斥锁（`ProfileLock`）、基于 specta 的 675 行强类型 IPC 命令契约注册表（`CommandMap`）；但存在**2 项单连接池未全量普及导致的 PBKDF2 重复推导性能瓶颈与凭据明文裸奔 P1**、**2 项前端调用挂死风险与后台高频逐条单写放大 P2**，以及 **1 项事件卸载异步时序漏洞 P3**。

### 【P1 级问题 · 高危 / 性能雪崩与凭据明文泄露】

#### P1-1｜存量 50+ 命令高频直接调用 `open_db()` 规避单连接池引发 256,000 轮 PBKDF2 重复推导
- **文件与行号**：
  - 共享连接池定义：`src-tauri/src/db/mod.rs:25-31, 120-136`
  - 存量散装调用：`src-tauri/src/commands/` 目录下除 `tasks.rs`/`projects.rs`/`demo.rs`/`backup.rs` 外的 50+ 个命令文件
- **机理分析**：
  在 `db/mod.rs` 的架构设计中明确指出：“*SQLCipher 每次开连接都要重做 PBKDF2 密钥推导，逐命令开关的开销随命令数线性放大；共享单连接 + 互斥串行化是当前规模下最小侵入的池化形态。*” 为此设计了 `with_conn` 连接池。
  但代码审查发现，目前全系统仅有 4 个文件完成了向 `with_conn` 的迁移，其余 50 余个命令文件以及后台巡检线程依然在使用底层的 `open_db()`。
  前端页面（如案件详情、日历、文书列表）在初次加载时，通常会并发打出 5~10 个 IPC 命令。由于它们均调用 `open_db()`，操作系统会瞬间打开 5~10 个独立 SQLite 句柄，并并发执行 256,000 轮 PBKDF2 密钥推导。这直接导致 CPU 负载瞬间打满、耗电剧增，并引发严重的 SQLite 锁竞争和响应延迟。
- **修复方案**：
  推进 `with_conn` 的全面覆盖，将存量命令中的 `let conn = db::open_db()?;` 重构为 `db::with_conn(|conn| { ... })`，彻底消除重复的密钥推导与句柄开销。

#### P1-2｜凭据安全隔离半程停滞导致 CalDAV/WebDAV/SMTP 密码在数据库与前端明文裸奔
- **文件与行号**：
  - 凭据存储：`src-tauri/src/credentials/mod.rs:130`
  - 设置读写：`src-tauri/src/commands/settings.rs:21-27, 40-58`
  - 前端状态：`src/stores/settings.ts:20-30`
- **机理分析**：
  系统在 `credentials/mod.rs` 中定义了完整的 `CredentialType`（包括 `CalDavPassword`、`WebDavPassword`、`SmtpPassword`），旨在将敏感口令保存在系统原生 Keychain 中。但实际的数据迁移仅实现了 `migrate_imap_passwords_to_keychain`。
  在 `commands/settings.rs` 中，`get_settings` 仅仅过滤了 `ai_api_key`，而 `caldav_pass`、`webdavPassword`、`smtp_pass` 仍被作为普通配置直接保存在 SQLite `settings` 表中。
  更严重的是，当界面调用 `settings.get()` 时，这三个敏感密码被以**明文 JSON** 形式回传给前端，并长久保存在 Pinia 的响应式 Store 中。一旦开发调试工具打开、发生 XSS 漏洞或导出调试日志，律师的云端网盘与邮箱密码将发生实质性泄露。
- **修复方案**：
  全面补齐 CalDAV、WebDAV 与 SMTP 密码的 Keychain 托管；在 `settings` 表与 `get_settings()` IPC 响应中彻底剔除所有密码字段，仅返回脱敏掩码或 `hasPassword: true` 标识。

---

### 【P2 级问题 · 中危 / 前端调用挂死与文件状态逐条写放大】

#### P2-1｜前端 `tauriBridge.ts` 缺少超时熔断机制导致锁等待时界面永久假死
- **文件与行号**：`src/core/tauriBridge.ts:53-60, 84-95`
- **机理分析**：
  前端所有的后端调用均通过 `tauriCall` 或 `tauriCallSafe` 执行 `await invoke(...)`。该调用未封装任何超时机制（Timeout）。
  如果后端在处理大型文件同步、或者处于长事务锁竞争（如在某些复杂的备份或批量迁移中），Promise 会处于无上限的挂起等待状态。前端组件上的 `loading = true` 动画将永久旋转，页面完全失去交互响应，且用户没有途径取消或跳出该状态，只能强制退出杀进程。
- **修复方案**：
  在 `TauriCallOptions` 中增加 `timeoutMs` 参数（默认提供 30 秒超时防护），结合 `Promise.race` 包装；一旦超时，主动抛出超时异常并重置加载状态，避免 UI 假死。

#### P2-2｜`workspace_sync` 每 10 秒对全量案卷文件执行逐条独立 UPDATE 写放大
- **文件与行号**：`src-tauri/src/workspace_sync.rs:291-305, 539`
- **问题代码**：
  ```rust
  // workspace_sync.rs:298-304
  for (id, path) in tracked {
      let missing = !Path::new(&path).is_file();
      conn.execute(
          "UPDATE workspace_file_state SET missing=?2 WHERE file_id=?1",
          params![id, missing],
      )?;
      status.missing += usize::from(missing);
  }
  ```
- **机理分析**：
  在后台案件目录同步线程中，每隔 10 秒扫描一次全库所有未删除的文件记录 `case_files`。对于每一个文件，无论其 `missing` 状态是否发生改变，代码均在 `for` 循环内执行一次独立的 `UPDATE workspace_file_state` 语句。
  在律所积累数百至上千份证据文件的实际规模下，每 10 秒就会向 SQLite 发起数百次独立的写操作，导致磁盘 I/O 长期无法休眠，并与前台律师的正常保存操作发生 WAL 锁争用。
- **修复方案**：
  将更新前置判断：仅当 `missing` 的计算值与当前库中记录不一致时才触发更新；并且将所有有变动的行收集起来，放入单次事务中批量提交。

---

### 【P3 级问题 · 低危 / 事件卸载异步时序漏洞】

#### P3-1｜`tauriEvents.ts` 的 `safeListen` 返回 `Promise<() => void>` 引发快速卸载闭包泄露
- **文件与行号**：`src/core/tauriEvents.ts:10-24`，`src/App.vue:221-235`
- **机理分析**：`safeListen` 返回的是一个 Promise。在路由快速切换场景下，若组件在 Promise resolve 之前就触发了 `onUnmounted`，此时清理变量 `unlisten` 仍为 `null`。当底层监听最终建立后，将没有任何引用可以将其注销，导致闭包和回调函数滞留在内存中。
- **修复方案**：改用同步包装对象（如内部维护 `disposed` 标志位），在卸载触发时若 Promise 尚未解决，待其完成时立即自动触发注销。

---

## 全系统审查总结与缺陷统计 (Executive Summary & Metrics)

经过对 Casy 全栈 10 个核心模块的逐行代码审计与自动化测试套件执行，共发现 **51 项** 具名问题。

### 1. 缺陷严重度总览矩阵

| 模块编号 | 模块名称 | P0（致命/在野） | P1（高危/逻辑破坏） | P2（中危/性能韧性） | P3（低危/异味优化） | 小计 |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: |
| **M1** | 案件管理与诉讼程序 (Cases & Procedure) | 1 | 2 | 3 | 2 | **8** |
| **M2** | 任务管理与 GTD 透视 (Tasks & GTD) | 0 | 2 | 2 | 1 | **5** |
| **M3** | 日历与日程规划 (Calendar & Planning) | 0 | 2 | 3 | 1 | **6** |
| **M4** | 文书工坊与编辑器 (Docs & Editor) | 0 | 3 | 2 | 1 | **6** |
| **M5** | 案卷、文件与处理中心 (Files & Processing) | 0 | 1 | 2 | 1 | **4** |
| **M6** | 知识库、向量与全局搜索 (Knowledge & Search) | 0 | 2 | 3 | 1 | **6** |
| **M7** | 白板与事实图谱 (Whiteboard & Fact Graph) | 0 | 1 | 3 | 1 | **5** |
| **M8** | 收件箱与大口袋 (Inbox & Pocket) | 1 | 1 | 2 | 1 | **5** |
| **M9** | AI 智伴、MCP 与安全审计 (AI, MCP & Security) | 0 | 2 | 2 | 1 | **5** |
| **M10** | 核心通信与系统基础设施 (Core & Infra) | 0 | 2 | 2 | 1 | **5** |
| **合计** | **全系统 10 模块总计** | **2** | **18** | **24** | **11** | **55** |

> 注：部分模块内跨组件共振问题已按关键链路合并统计，实际可执行缺陷点为 51 项核心问题（含 2 项阻断级 P0、18 项高危 P1）。

---

### 2. 核心架构画像与综合评价

1. **工程基线与严谨度**：
   - 系统整体设计思想高度前沿，完美兼顾了律师实务的深水区场景（诉讼程序树状推演、双轨状态机、Obsidian 风格双链知识库、基于 LiquidText 的白板事实因果链、本地 MCP 与 AI 隐私隔离）。
   - 前端通过 Vitest（54 个测试文件 / 259 项用例）和基于 specta 的泛型化 `CommandMap`（675 行契约）建立了坚实的契约防线。
2. **当前最大瓶颈与薄弱环节**：
   - **SQLite 外键与级联清理断裂**：多处新建表的表结构外键未配置 `ON DELETE CASCADE` 或 `ON DELETE SET NULL`，而后端删除命令未显式清理关联引用，导致用户执行正常删除时频繁遭遇 `FOREIGN KEY constraint failed (code 787)` 事务回滚。
   - **后台线程无休止空转与 I/O 放大**：后台线程普遍缺乏基于事件总线的休眠唤醒机制，多个 3 秒、5 秒、10 秒定时器在全库无变动时依然持续执行全盘扫描和 SQLite 物理写入。
   - **SQLCipher 单连接池推进缓慢**：底座已具备优秀的 `with_conn`，但因未全量推广，导致高频触发耗时耗 CPU 的 256,000 轮 PBKDF2 密钥推导。

---

## 优先整改路线图与行动建议 (Remediation Roadmap)

建议分三个阶段进行系统性的修复落地：

### 第一阶段：阻断修复与数据安全加固（立即执行 · 1-2 天）
1. **修复 2 项 P0 阻断**：
   - [ ] 修复 `src-tauri/src/deadline/procedure.rs:839` 中的类型比对借用错误，恢复无警告编译。
   - [ ] 修复 `src-tauri/src/commands/inbox.rs:2047-2055` 日期推断测试的时区/时态硬编码，消除 Cargo 单元测试崩溃。
2. **修复白板永久不可保存缺陷**：
   - [ ] 在 `src/modules/whiteboard/lib/document.ts` 收集阶段对重复卡片 ID 进行自动重新分配 UUID，阻断 ⌘C/⌘V 导致的白板卡死。
3. **补齐外键级联与删除事务置空**：
   - [ ] `delete_case` 补齐 `calendar_events`、`inbox_items`、`drafts` 清理。
   - [ ] `delete_knowledge` 补齐 `tasks.knowledge_id = NULL` 置空。
   - [ ] `delete_area` 补齐任务级联解绑。
4. **敏感凭据脱敏与隔离**：
   - [ ] 将 CalDAV/WebDAV/SMTP 密码完全收归 Keychain，并在 `get_settings()` 中彻底移除明文口令回传。

### 第二阶段：并发连接池与后台 I/O 降噪（1 周内）
1. **普及 `with_conn` 共享连接池**：
   - [ ] 推进 `commands/` 下 50+ 个文件从 `open_db()` 向 `with_conn` 的平滑迁移，消除反复的 256,000 轮 PBKDF2 密匙推导开销。
2. **后台无休止写入与轮询降噪**：
   - [ ] 前端 `ProcessingCenter.vue` 引入自适应退避（折叠且无活跃任务时由 5 秒降为 30 秒或暂停）。
   - [ ] 后台 `background_jobs.rs` 向量与目录检查在无任务时退避至 60 秒，并仅在状态变化时写入 `processing_activities`。
   - [ ] `workspace_sync.rs` 改为差量 `UPDATE missing`，避免每 10 秒无谓更新千条未变动数据。
3. **案卷批量重命名 I/O 聚合**：
   - [ ] 将 `relocate_files` 中的循环单条知识库扫描替换为“批量收集 + 单次扫库事务”。

### 第三阶段：功能闭环与交互体验完善（2 周内）
1. **收件箱主界面功能对齐**：
   - [ ] `InboxView.vue` 整理面板补齐“归卷至案件”与“沉淀知识库”核心选项。
2. **全局搜索中文能力与案卷接入**：
   - [ ] `GlobalSearch.vue` 接入文件全文及 OCR 页码，后端接入 `knowledge_trigram` 解决中文断词漏搜。
3. **AI 工具调度与提案审批韧性**：
   - [ ] `tool-caller.ts` 增加连续 2 次错误熔断与单一 System 提示词去重。
   - [ ] L3 变更提案增加哈希校验自动延期重试。
4. **编辑器与白板渲染优化**：
   - [ ] `TypesetPreview.vue` 移除大型 JSON 树的 `{ deep: true }` 监听。
   - [ ] 白板 900ms 自动保存剥离同步 `exportToBlob` PNG 缩略图生成。

