# 06 · 核心领域逻辑与数据库层审计

审计日期：2026-09-30。范围：`db/schema.rs`（迁移链）、`db/mod.rs`、`db/{cases,knowledge_index,vector_index,search}.rs`、`deadline/{procedure,holidays,recalc,engine}.rs`、`commands/{tasks,task_plans,reminder,deadline_rules,personal_availability,calendar*,cases,persons,relations,linking}.rs`、`formula/**`。

依据文档：`ARCHITECTURE.md`、`procedure-deadline-rules.md`、`CODE_REVIEW_2026-09-22.md`（R-05）、`DATA_AND_SECURITY.md`。

标注约定：**CONFIRMED** = 已读代码并沿完整调用路径验证；**SUSPECTED** = 代码形态指向该缺陷，但缺少可复现的存量数据或运行时证据。

---

## 概述

这一层的整体工程质量高于同规模项目平均水平，先说结论中值得肯定的部分，避免后面的问题被误读为"整体不可靠"：

- **迁移的事务边界设计是正确的。** `apply_migration_tx`（`db/schema.rs:3110-3117`）把每条迁移的 DDL 与 `PRAGMA user_version` 放在同一个事务里，失败一起回滚；`run_migrations`（`3052-3069`）在进入任何事务前把 `PRAGMA foreign_keys` 置 0、全部迁移结束后再恢复，避免了"`PRAGMA` 在活动事务内切换是 no-op"这个经典坑。裸 `ALTER TABLE ADD COLUMN` 全部放在版本化迁移里（v38/v39/v40），条件补列全部放在 `apply_conditional_segments` 的 `PRAGMA table_info` 探测段（v11/v13/v15/v20/v21/v24 惯例），职责划分清晰。
- **连接复用层有真实并发意识。** `ManagedConnection::drop`（`db/mod.rs:47-62`）在归还连接池前校验 `is_autocommit()` 和 `foreign_keys` pragma，拒绝把"有未完成事务"或"约束被关掉"的连接放回池；`enter_maintenance`（`86-107`）用 `ACTIVE_CONNECTIONS` 计数 + `Condvar` 等待所有在用连接归还后再替换库文件，排他锁语义正确。
- **重复任务没有经典无限增长缺陷。** `next_occurrence` 以"上一个实例自己的日期"为锚（`task_lifecycle.rs:119-126`），链条长度等于用户实际完成次数，不会在一次启动里批量生成实例。
- **R-05 声称的"规则变更与审计同事务"属实。** `commands/deadline_rules.rs` 四个写命令都用 `raw.transaction()` 包裹，`delete_deadline_rule`（`225-242`）还先解绑 `case_deadlines.rule_id` 再删规则，避开了 `NO ACTION` 外键的 RESTRICT。

问题集中在三个地方：**迁移失败时没有退路**、**节假日日历覆盖不足时的降级路径不统一**、**提醒派发对待核对日期与时段外场景的处理不完整**。

一个需要先纠正的文档漂移：`CURRENT_SCHEMA_VERSION` 实际是 **43**（`db/schema.rs:5`），而 `ARCHITECTURE.md:42` 与 `DATA_AND_SECURITY.md:66` 都写"当前版本 42"。v43（`draft_versions` 文书快照）已落地但文档未更新。

---

## Schema 迁移链审计表（v1–v43 关键节点）

| 版本 | 位置 | 性质 | 幂等性 | 数据风险 | 结论 |
| --- | --- | --- | --- | --- | --- |
| v1 | `schema.rs:680` → `SCHEMA_SQL` | 全新建表 + 种子 | `CREATE TABLE IF NOT EXISTS` | 无 | OK。`init_db`（`db/mod.rs:431-437`）在 `user_version=0` 时执行并置 1，随后 `apply_versions` 的 `floor=1` 不会重跑 |
| v2 | `1462-1566` | **inbox_items 全表重建**（收窄 status CHECK + 扩列） | `CREATE TABLE IF NOT EXISTS inbox_items_new` + `DROP` + `RENAME`；因在单事务内，重跑安全 | **高**：`SELECT ... CASE status WHEN 'processing'→'pending' WHEN 'dismissed'→'ignored' ELSE status END` 只枚举了两个旧值，第三个旧枚举值会原样写入新 CHECK 并使 INSERT 失败 | **SUSPECTED 高危**（见 C1） |
| v3 | `1568-1655` | 飞书字段元数据 / 公式缓存列 | 全 `IF NOT EXISTS` / `ADD COLUMN` | 低 | OK |
| v4–v8 | `1657-2098` | 飞书连接/映射/领域表 | `IF NOT EXISTS` | 低 | OK |
| v9 | `2099-2539` | 领域表（areas 等）；`2070` 处另有一次 inbox_items 重建 | `IF NOT EXISTS` | 中 | OK（FK=OFF 下 `DROP TABLE` 不级联，避免误删 `inbox_feedback`，见 `schema.rs:1184` 注释） |
| v10 | `2540-2609` | **memory_entries 重建**（扩 status 枚举） | 单事务 + `DROP IF EXISTS _v10` | **高**：新 CHECK 含 `'pending','merged','dismissed'`，但 `SELECT` 原样复制 `status` | **SUSPECTED**（同 C1 模式） |
| v11 / v13 / v15 / v20 / v21 / v23 / v24 / v25 / v26 | `2610-1090`、`apply_conditional_segments` | 占位 `SELECT 1;` + 条件执行段真实变更 | `PRAGMA table_info` / `sqlite_master.sql` 探测后变更 | 中：条件段含 `cases`、`knowledge_items`、`task_events`、`decisions` 多次整表重建 | 见 C2（单事务全有全无） |
| v12 | `2647-2690` | task_events 重建（recursion_gap） | 条件探测 | 中 | OK |
| v14–v19 | `2623-3034` | 案件↔项目投影表 + 触发器 | `ON CONFLICT DO UPDATE` | 低 | OK |
| v22 | `1251-1262` | 案件↔任务/庭审/日志关联表 | `IF NOT EXISTS` + `ON DELETE CASCADE` | 低 | OK |
| v27 | `1018-1061` | 案件↔任务/庭审/日志关联表 | `IF NOT EXISTS` | 低 | OK |
| v28 | `957-1017` | case_files 软删 + 向量失效触发器 | 裸 `ADD COLUMN` | 低 | OK（单事务 + 版本闸门） |
| v29–v32 | `892-956` | 索引/触发器补齐 | `IF NOT EXISTS` | 低 | OK |
| v33–v36 | `758-891` | 知识标注 job/state 触发器 | `IF NOT EXISTS` | 低 | OK |
| v37 | `726-757` | 案件字段投影 | `ADD COLUMN` | 低 | OK |
| **v38** | `4674-4688` | processing_activities + jobs 扩列 | 裸 `ADD COLUMN`（依赖版本闸门） | 低 | OK |
| **v39** | `4690-4719` | fact_nodes 扩列 + 历史触发器 | `DROP TRIGGER IF EXISTS` → `CREATE IF NOT EXISTS` | 低 | OK。`ADD COLUMN ... REFERENCES knowledge_items(id) ON DELETE SET NULL` 默认 NULL，SQLite 允许 |
| **v40** | `4721-4732` | phase/timing 扩列 | 裸 `ADD COLUMN` | 低 | OK |
| **v41** | `4734-4753` | 删除解绑触发器 + 白板墓碑 | `IF NOT EXISTS` | **正面**：补上了 `calendar_events.case_id` 置空、`tasks.knowledge_id` 置空、`calendar_events.task_id` 级联清理三个孤儿口 | OK |
| **v42** | `4755-4766` | `task_plans` | `IF NOT EXISTS` | 低 | OK |
| **v43** | `4768-4784` | `draft_versions` + 内容快照触发器 | `IF NOT EXISTS` | 低 | OK（文档未更新） |

**正面结论：迁移可以安全地跑第二遍。** `apply_versions`（`3090-3106`）以库内 `PRAGMA user_version` 为唯一权威（`floor = current_user_version(conn)?.max(from_version)`），裸 `ALTER TABLE` 只出现在版本化迁移里且与版本推进同事务，条件段有 `PRAGMA` 探测。测试 `schema.rs:4229-4267`（中断重跑）、`4258-4267`（当前版本幂等）覆盖了这一点。

**正面结论：v25 的 `cases` 表重建是数据安全的。** `rebuild_cases_for_route_v25`（`3146-3382`）先取 `old_columns` 求交集得到 `copy_columns`，缺 `id`/`case_name` 直接 `bail!`（`3241-3243`），不丢列。17 个 v25 条件列在 `3789-3805` 逐列探测后补齐。

---

## 严重问题

### C1. 迁移前无任何备份，且条件段是"全有或全无"单事务 —— 一次失败即永久锁死启动 【SUSPECTED → 高危】

- `src-tauri/src/lib.rs:88-90`：`recover_interrupted_restore()` 之后直接 `open_db()` → `init_db()`，**迁移前不复制数据库文件、不做快照**。`commands/backup.rs` 存在但是用户手动触发的。
- `db/schema.rs:3590` `apply_conditional_segments` 开启一个事务，事务体覆盖到 `3834 tx.commit()`，中间包含 `cases` 整表重建（`3260-3372`）、`knowledge_items` 重建（`1184-1211`）、`task_events` 两次重建、`decisions` 重建，以及数十条 `ALTER TABLE`。
- `db/mod.rs:440-447`：`init_db` 把 `run_migrations` 的错误用 `?` 直接上抛；`lib.rs:90` 同样用 `?` → Tauri `setup` 返回 `Err` → **应用无法启动**。

具体触发路径（两条，任一命中即锁死）：

1. **存量数据违反新 CHECK。** v2 的 `INSERT ... SELECT`（`1490-1503`）对 `status` 只映射 `'processing'` 和 `'dismissed'`，其余走 `ELSE status`。若某个真实库的 `inbox_items` 里存在第三种旧状态（如 `'new'`、`'read'`），写入新表的 `CHECK(status IN ('pending','processed','filed','archived','ignored','dismissed'))` 时立即失败。v10 的 `memory_entries` 重建（`2580-2586`）是同一模式：新增了三个枚举值但 `SELECT` 原样复制 `status`。
2. **条件段任何一步失败。** 因为整个条件段是一个事务，任意一条语句失败 → 全部回滚 → `user_version` 停在旧值 → 下次启动重跑同样语句 → 同样失败。**没有"跳过这一步继续"的开关，也没有降级路径。**

可观察的错误结果：用户升级后应用再也起不来，日志里只有一条 rusqlite 的 constraint 错误；由于没有迁移前备份，用户手上是一个既打不开、也没有安全副本的资料库。

### C2. 内置节假日日历漏掉 2025-10-08，且只覆盖 2025–2026 【CONFIRMED】

`deadline/holidays.rs:56-62` 的 2025 节假日列表是 `2025-10-01 … 2025-10-07`，`67-74` 的 2025 调休上班日是 `2025-09-28`、`2025-10-11`。国务院办公厅 2025 年放假安排为"国庆节、中秋节 10 月 1 日至 8 日放假调休，共 8 天；9 月 28 日（星期日）、10 月 11 日（星期六）上班"——调休日两项对得上，**假日侧少了 `2025-10-08`（周三）**。

具体触发与错误结果：任一法定期限/指定期限的原始届满日落在 2025-10-08。`is_workday`（`holidays.rs:243-252`）先看 `workdays` 集合（不含）、再看是不是周末（周三，不是）、最后 `!holidays.contains(date)`（**不含 → true**）→ 判定为工作日，**不触发顺延**。正确结果应为顺延至 2025-10-09。可观察症状：一条 15 日答辩期在日历上比实际少 1 天，且没有任何"待核对"标记。

覆盖范围问题同一处：`HolidayCalendar::builtin()` 只有 2025 和 2026 两组硬编码日期。`entries_for_year`（`177-202`）对 2027 及以后返回空，`year_range()`（`232-240`）返回 `"2025-2026"` 但**全代码库无人调用它**（已 grep 确认）。到期日在 2027-01-01 之后的所有期限，`extend_to_workday` 退化为"只跳过周六周日"。

### C3. `holidays_json` 损坏时三处加载路径行为不一致，规则引擎静默回退到内置日历 【CONFIRMED】

同一个 `settings.holidays_json` 键有三条读取路径，三种失败语义：

| 位置 | 行为 |
| --- | --- |
| `deadline/procedure.rs:109-114` `calendar()` | `?` 传播错误 → 整批程序事项读取失败 |
| `formula/mod.rs:195-198` | `?` 传播错误 |
| `deadline/engine.rs:71-75` | `.ok().flatten()` + `from_json_str(&value).ok()` → **双重吞错，静默回退 `HolidayCalendar::builtin()`** |

具体触发：用户导入官方节假日 JSON 时 JSON 被截断（导入中断、磁盘满）。此后同一份损坏数据下，程序事项全部报错（`engine.rs:183` 造一条 due_date=今天 的红色"程序期限读取失败"占位），而**规则驱动的期限全部改用只含 2025–2026 的内置日历计算，不报错、不打标记**。可观察结果：同一案件里两类期限的日期基准不一致，规则类期限静默使用过期日历。

### C4. 规则引擎路径完全没有"年份未覆盖"标记，而程序事项路径有 【CONFIRMED】

`procedure.rs:723-729` 会在 `cal.entries_for_year(d.year())` 为空（即该年无任何节假日数据）时置 `needs_review = true`，并在 `778-780` 追加"此年份节假日未覆盖，请导入官方日历后核对"。`engine.rs:139-149` 生成 `DeadlineResult` 时**没有任何等价检查**——一条 `deadline_rules` 驱动、落在 2027 年的期限会以正常的 `legal_basis`、正常的 `urgency` 直接输出。

结合 C2：2027-01-01 之后，每一条规则驱动的法定期限都会用"只跳周末"的错误算法计算，并以 `classify_urgency` 给出红/黄/绿分级。`deadline_source` 仍是 `statutory`，`legal_basis` 仍写着法条编号，界面上没有任何"这个日期不可信"的提示。**这正是本次要找的"自信地产生错误日期"而非"缺日期"。**

---

## 期限计算正确性

先说**验证为正确**的部分，避免误伤：

- **自然日期间起算不计入。** `procedure.rs:695-701` 的 `d.checked_add_signed(Duration::days(n))` 与 `holidays.rs:277-281` 的 `from = start + 1; due = from + (days - 1)` 都等于 `start + n`，符合《民事诉讼法》第 85 条"期间开始的时和日，不计算在期间内"和《专利法实施细则》第 5 条。15 日答辩期、10 日裁定上诉期正确。
- **按月期间的月末钳制。** `checked_add_months(Months::new(n))`（`procedure.rs:696`）对 1/31 + 1 月 = 2/28，符合细则第 5 条"没有相应日的，为期限届满月的月末日"。
- **末日休假顺延。** `extend_to_workday`（`holidays.rs:255-265`）先查 `workdays`（调休上班日优先），再跳周末，再跳假日。同日双标记由 `merge_dates`（`160-174`）强制 workdays 覆盖 holidays，语义正确。
- **个人调休与法定日历隔离。** `personal_calendar_days` 独立存储，`personal_availability::rest_intervals(date, official, personal)`（`80-119`）把两个日历分开入参，`parse`（`40-78`）对日期真实性/类型/时长/唯一性做了完整校验。不存在个人假期导致诉讼期限顺延的路径。✓
- **乐观并发。** `procedure.rs` 事件编辑用 revision 校验，`item()`（`540-593`）的 `fingerprint` 绑定了 `RULE_VERSION`、事件全量、`our_role`、`opponent_role`、`suffix`、`due_on`、`source`，事件/角色/日历变化都会让旧处理状态失效。✓

### D1. 待核对日期照样派发最高级别提醒 【CONFIRMED】

`commands/reminder.rs:255-273` 的 `check_procedure_deadline_rules`：

```rust
for i in items.into_iter().filter(|i|i.status=="open") {
    let Some(due)=i.due_on... else {continue};
    let days=(due-today).num_days();
    let applies=match rule.trigger_type.as_str(){"deadline_before"=>(0..=trigger).contains(&days), ...};
    if !applies { continue; }
    ...
    let label=if i.needs_review{"待核对日期"}else ...;
    let message=format!("...{due}...");
    let entry=dispatch_reminder(conn,&rule.id,Some(&case.id),None,"local",&message,level,None)?;
```

`needs_review` 只被用来把标签从"程序期限"换成"待核对日期"，**不阻止派发**。`level` 由 `compute_level(days,false)` 得出，`days<=3` 即 R1 红色。

具体触发：2027-01-01 之后，一个 `patent_invalidation` 案件的无效请求受理通知指定 1 个月答复期，起算日 2026-12-20。届满日 2027-01-20 落在无日历覆盖的年份 → `needs_review=true`（`723-729`）→ 但 `days_diff` 落在 `deadline_before` 规则的 `0..=trigger` 区间 → 派发 R1 本地 + 系统通知，正文写明"待核对日期 日期：2027-01-20"。

可观察的错误结果：一条**明确标记为不可信**的日期，以最高紧急级别弹给律师。同一函数还会把 `source=="internal"`（内部查阅建议）和 `owner=="opponent"`（对方期限）的事项一并按 `compute_level` 派发，与我方法定期限混在同一红/黄/绿序列里 —— 这与 `procedure-deadline-rules.md:50`"不能直接冒充我方义务"的边界不一致。

### D2. 程序期限的 R1/R2 在工作时段外被静默丢弃，而手工期限不会 【CONFIRMED】

两条路径对"时段外"的处理不同：

- 手工/规则期限走 `dispatch_reminder_at`（`reminder.rs:494-514`）：时段外且有 `cal_ctx` 时调用 `defer_reminder_job`（`656-722`），写 `reminder_jobs(executor='local', status='pending', scheduled_at=下一工作时段起点)` + `reminder_log(status='deferred')`，**重启后仍会补发**。
- 程序期限在 `reminder.rs:264` 就提前短路了：
  ```rust
  if (level=="R1"||level=="R2")&&next_work_start(chrono::Local::now().naive_local(),start,end).is_some(){continue;}
  ```
  `next_work_start`（`632-653`）在时段外返回 `Some`，于是 `continue` —— **不写 `reminder_jobs`、不写 `reminder_log`、不写 receipt**。而且即使删掉这一行，`dispatch_reminder` 的第三个参数 `cal_ctx` 传的是 `None`（`271`），`dispatch_reminder_at:508` 的 `if let Some(ctx)` 也不会成立，延迟派发依然不会发生。

具体触发：默认工作时段 9:00–21:00（`work_hours:610-629` 的 `unwrap_or(9)`/`unwrap_or(21)`）。一个 21:30 变成 R1 的程序期限（`days_left<=3`），引擎 tick 落在 21:30–23:59 之间 → 静默跳过；下一个 tick 落在次日 09:00 之后才发出。可观察结果：提醒日志里这段时间是一段空白，没有任何"已延迟"记录；若律师在 09:00 前关机，这条红色程序期限**永久不会送达**。而同一期限若录在 `case_deadlines` 里，则会留下一条 `reminder_jobs` pending 记录、重启后补发。

### D3. `EDATE(d, -1)` 返回 `d` 本身 【CONFIRMED】

`formula/eval.rs:217-233`：

```rust
let Some(months) = months_val.as_number() else { return Ok(Value::Null); };
let result = add_months_clamp(date, months as u32);
```

`Value::as_number`（`ast.rs:151+`）对负数返回 `Some(-1.0)`。Rust 的 `f64 as u32` 是**饱和转换**（1.45 起），`-1.0 as u32 == 0`。`add_months_clamp(date, 0)`（`eval.rs:405-412`）走 `total = month + 0`，年月不变、日期不变，返回原日期。

具体触发：用户写 `EDATE(收到日期, -1)` 期望上一个月，实际得到同一天。**不报错、不返回 Null，直接给出一个错误的日期。** Excel 语义下应为前一个月。

同函数还有一个 panic 面：`EDATE(d, 1000000000)` → `total` 约 1e9 → `days_in_month`（`eval.rs:414-419`）内部 `NaiveDate::from_ymd_opt(year, month+1, 1).unwrap()`，year 远超 chrono 的 ±262143 上限 → `None.unwrap()` **panic**。全代码库只有 `docsy_engine/rich_export.rs:475` 一处 `catch_unwind`，公式求值路径无保护。

### D4. 硬编码跳过 10 条种子规则，用户对它们的编辑永远不生效但审计显示"已更新" 【CONFIRMED】

`deadline/engine.rs:96`：

```rust
if matches!(rule.id.as_str(), "rule-pi-001"|...|"rule-ct-005") { continue; }
```

这是一条按 **id** 的无条件跳过，位于 `if !rule.auto_calculate` 之后、触发日期读取之前。而 `commands/deadline_rules.rs:141-172` 的 `upsert_deadline_rule` 对这些 id 一视同仁：允许 `UPDATE`、写 `deadline_rule_audit(action='update')`、调 `recalc_track_inner`、提交。

具体触发：律师发现内置的 `rule-al-002`（行政答辩期限）基准不对，打开规则编辑把 `offset_value` 改成 20 并保存。前端提示成功，审计表新增一条 `update` 记录，规则列表显示新值，`recalc_track_inner` 返回"受影响案件 0 个"（因为 `engine.rs:96` 在它之前就跳过了）。**该规则此后仍然永不产出任何期限，且没有任何提示说明编辑被忽略。** 种子 id 见 `schema.rs:3920-4110`。

顺带：`upsert_deadline_rule` 的 UPDATE 语句（`deadline_rules.rs:145-151`）**不含 `auto_calculate`**，所以编辑不会意外改变启停状态（这是对的），但也没有任何字段级校验。

### D5. 规则写入不校验 offset_value / trigger_field，非法规则静默永不产出 【CONFIRMED】

`engine.rs:97-116` 有一串静默 `continue`：

```rust
if rule.offset_value < 1 || rule.offset_value > 3650 || (offset_unit=="calendar_month" && offset_value > 120) { continue; }
... get_case_date_field(case, &rule.trigger_field) ... else { continue; }
NaiveDate::parse_from_str(&trigger_str, "%Y-%m-%d") else { continue; }
```

`get_case_date_field`（`engine.rs:211-228`）是固定字段名的 `match`，`calc_method` 在 `129` 是 `_ =>` 兜底分支（只有 `"patent"` 特殊，其余一律走 civil——当前实现二者算法相同，暂无影响）。

而 `upsert_deadline_rule`（`deadline_rules.rs:134-200`）**只校验 `offset_unit ∈ {day, calendar_month}`**（`137-140`），不校验 `offset_value` 范围、`trigger_field` 是否在白名单内、`calc_method` 是否可识别。

具体触发：用户新建规则时把"触发字段"从 `filing_date` 误输入为 `filingDate`（驼峰），`offset_value` 填 0。写入成功、审计成功、重算返回"受影响 0 个案件"。**这条法定期限从此永不产出，而用户所有的成功反馈都指向"已配置"。** 相比之下 `personal_availability.rs:40-78` 的输入校验是本项目里做得最好的，可以直接作为范式。

### D6. 闰年 / 跨年边界抽查：未发现错误 【CONFIRMED】

逐个手算验证（`task_lifecycle.rs:6-38` 与 `holidays.rs`）：

- `weekly:1`（周一）从 2028-02-28（闰年周一）：`(1+7-1)%7=0` → +7 → 2028-03-06 ✓
- `monthly:29` 从 2028-01-29：`first=2028-02-01`，`last = 2028-03-01.pred_opt() = 2028-02-29` → `min(29,29)=29` → **2028-02-29** ✓ 闰年 2 月 29 日正确产出
- `monthly:31` 从 2026-01-31：→ 2026-02-28 → 2026-03-31 → 2026-04-30 → 2026-05-31。逐月钳制，与 Excel 行为一致，属设计选择而非缺陷。
- 自然日跨年：`Duration::days` 在 chrono `NaiveDate` 上按公历推进，2026-12-20 + 15 = 2027-01-04，无跳变 ✓（该日期后续会被 C2/C4 标记问题影响，但算术本身正确）

**未发现自然日/月的 off-by-one。** 真正的问题全在 C2/C3/C4（覆盖与降级）和 D1/D2（提醒投递）。

---

## 重复任务

### 正面结论

- **无无限增长。** `completion_effects`（`task_lifecycle.rs:97-172`）在 `completed==1` 且 `existing.is_none()` 时才生成一个后继，`task_recurrence_instances.source_task_id` 是 PRIMARY KEY（`schema.rs:928-932`）保证一个源任务最多一个后继。链条长度 = 用户实际完成次数。✓
- **锚点是上一个实例自己的日期，不是原始日期。** `anchor` 取 `COALESCE(due_date, deadline, plan.start_date, start_date, date('now'))`（`119-121`），因此不会出现"每周任务在 N 次后日期膨胀"的问题。✓
- **计划跨度平移保持长度。** `move_date`（`136-142`）用同一个 `shift` 平移 `task_plans.start_date/end_date`，并校验结果 ≤ `9999-12-31`。✓
- **修改过的后继实例不会被误删。** `150-164` 用 `snapshot_hash`（`47-66`，覆盖任务本体 + case_task_links + task_events + 子任务 + links 五类引用）比对，只在完全未被改动时软删。这条设计是对的。✓
- **写入在事务内。** `commands/tasks.rs:376-398` 的 `toggle_task` 用 `transaction_with_behavior(Immediate)`，`tasks.rs:1193` 的批量 patch 用 `&tx`，`editor_tasks.rs:72` 用 `&tx`，三者都在提交前完成 `completion_effects`。✓

### R1. 编辑过后继实例后，"完成→撤销→再完成"使重复链永久停止 【CONFIRMED】

`task_lifecycle.rs:150-165` 的撤销分支：

```rust
} else if let Some((successor, expected)) = existing {
    if snapshot_hash(conn, &successor)? == expected {
        ... 软删后继 ...
        conn.execute("DELETE FROM task_recurrence_instances WHERE source_task_id=?1", [id])?;
    }
    // 快照不匹配：什么都不做，task_recurrence_instances 行保留
}
```

具体触发序列：完成每周任务 T0 → 生成 T1 + `task_recurrence_instances(T0→T1)` → 用户编辑 T1（改标题/加关联案件）→ 撤销 T0 的完成 → 因哈希不匹配，T1 保留、`task_recurrence_instances(T0→T1)` **也保留** → 再次完成 T0 → 走 `108-115` 的 `if completed == 1`，`rule.filter(|_| existing.is_none())` 因 `existing.is_some()` 而为 false → **不生成 T2**。

可观察结果：T0 反复完成/撤销一次之后，这条每周重复任务就再也不产生下一个实例了，且界面上没有任何提示。文档 `DATA_AND_SECURITY.md:68` 只说"完成撤销仍保留已被用户修改的下一实例"，没有覆盖"再完成"这一路径。

### R2. 软删源任务不撤销已生成的后继实例 【CONFIRMED】

`commands/tasks.rs:412-467` 的 `delete_task` 是软删除（`UPDATE tasks SET deleted_at=?`），只 `refresh_sequence` + 写 `deleted` 审计事件 + 撤销该任务自己的 CalDAV 作业。它**不查 `task_recurrence_instances`、不处理已生成的后继**。

具体触发：完成每周任务生成 T1 → 软删 T0。T1 继续留在待办列表并会按时提醒。`task_recurrence_instances.source_task_id REFERENCES tasks(id) ON DELETE CASCADE` 对软删除无效（行还在）。可观察结果：用户以为删掉了这条重复任务，下周仍然冒出一个新实例。（相比之下 `delete_task` 对 CalDAV 作业的处理是完整的，所以这是口径不一致而非整体缺失。）

### R3. 顺序组的 `blocked` 依赖被继承但未被修正（低） 【SUSPECTED】

后继实例的 INSERT（`task_lifecycle.rs:128-134`）显式写 `blocked=0`，同时复制 `sequential`。若源任务是顺序组的一员，正确值应为 1。当前实现之所以没出问题，是因为 `refresh_sequence(conn, id)`（`166`）在同一次调用末尾重新扫描整组（`85-86` 的 `WHERE case_id IS ?1 AND parent_task_id IS ?2 AND sequential=1 AND completed=0 AND deleted_at IS NULL`），后继实例因 `completed` 默认 0 而落在组内被一并修正。**但这依赖 `tasks.completed` 的 DEFAULT 0**（`schema.rs:224`）和后继 INSERT 未显式给 `completed`——两个隐式契约耦合在一起，值得加断言。

---

## 事务与原子性

### T1. 迁移期 FK 关闭范围过大，与运行时共用同一连接池 【CONFIRMED，设计权衡】

`run_migrations`（`3055-3067`）把 `foreign_keys` 置 0 直到全部版本化迁移 + 条件段结束。条件段包含 `cases`、`knowledge_items` 等大表重建，期间**外键完全不生效**。若此时另一条连接正在写（理论上不会——`init_db` 在 `setup` 里同步执行，早于 `start_background_worker`/`start_reminder_engine`），理论上可以写入孤儿行。`ManagedConnection::drop`（`db/mod.rs:52`）会拒绝把 `foreign_keys=false` 的连接放回池 ✓，所以这个窗口不会泄漏到运行时。**当前调用时序是安全的，但安全性依赖"迁移只在 setup 里跑"这个未被类型系统强制的约定。**

### T2. `dispatch_due_local_jobs` 无事务，作业状态与投递日志可能不一致 【CONFIRMED】

`reminder.rs:786-796`：

```rust
conn.execute("UPDATE reminder_jobs SET status=?1, last_error=?2, attempts=attempts+1 WHERE id=?3", ...)?;
let log_id = db::new_id();
conn.execute("INSERT INTO reminder_log (...) VALUES (...)", ...)?;
```

两条语句在 autocommit 下先后执行。中间任何失败（磁盘满、锁超时、进程被杀）→ 作业已标 `sent` 但无投递审计记录。反向不一致不会发生（UPDATE 在前）。同一函数其余的 `push_inbox_notification` 用了 `let _ =` 显式 best-effort（`797`），风格上是有意识的，但这两条没被纳入同一保护。对比 `commands/tasks.rs:376`、`task_plans.rs:62`、`persons.rs:168`、`relations.rs:164` 都正确使用了 `Immediate` 事务，这里的遗漏是不一致而非设计。

### T3. `db::cases::delete_case` 自身不是原子的（唯一调用方已包装） 【CONFIRMED，现状安全】

`db/cases.rs:486-509` 是四条语句（两次清 `links`、一次断父子链、一次 `DELETE FROM cases`），函数签名 `conn: &Connection` 不要求事务。`commands/cases.rs:163-179` 用 `raw_conn.transaction()` 完整包裹 ✓，所以当前唯一的生产路径是原子的。**但函数契约没有强制这一点**——下一个调用者漏包就会得到"links 已清、案件还在"的半提交状态。建议在函数内 `unchecked_transaction()` 或把签名改成 `&Transaction`。

### T4. 任务创建的外部日历同步静默吞错 【CONFIRMED】

`commands/tasks.rs:351-357` 与 `1219-1225`：

```rust
let _ = crate::commands::reminder::sync_task_reminder_calendar(conn, &id, due, ...);
```

`sync_task_reminder_calendar`（`reminder.rs:906-957`）内部会 `INSERT ... ON CONFLICT DO UPDATE` 写 `reminder_jobs`，其内部错误（如 `masked_calendar_summary` 读到异常的 `settings`、CHECK 约束冲突）会返回 `Err`，在这里被 `let _ =` 完整丢弃。

具体触发：任务带截止日 + 已配置 CalDAV，但 `settings.caldav_user` 内容异常或 `reminder_jobs` 触发器抛错 → 任务创建成功、`reminder_jobs` 无行、外部日历无此事件，**用户界面没有任何提示**。`procedure-deadline-rules.md:46` 已明确程序事项"暂不生成外部日历或飞书提醒任务"，说明这条路径对期限提醒是有意保守的；但**任务截止日**的提醒被静默丢弃仍属缺陷——律师可能已把提醒责任完全外包给外部日历。

### T5. 提醒引擎在持有租约连接时执行阻塞网络调用 【CONFIRMED，程度中等】

`send_via_channel`（`reminder.rs:572-608`）的 `feishu_message`/`feishu_task` 分支用 `tauri::async_runtime::block_on(...)` 同步等待 HTTP；`send_system_notification`（`1071-1105`）`Command::new("osascript")` 每条提醒派生一次子进程。这些都发生在 `check_and_trigger(&conn)` 之内，而 `conn` 是 `open_db()` 租用的独占连接（`db/mod.rs:110-127`）。

由于没有任何显式写事务，WAL 模式下不持写锁，**不会冻结 UI** ✓。真实代价是：(a) 飞书端点超时期间该连接被占，`ACTIVE_CONNECTIONS` 计数不降，30 秒后 `enter_maintenance` 会失败并提示"仍有操作进行中"；(b) 200 条 R1 提醒 = 200 次 `osascript` fork。`MAX_IDLE_CONNECTIONS = 4` 只限制空闲池，不限制并发活跃连接。

---

## 外键与孤儿

### 正面结论

`PRAGMA foreign_keys=ON` 在**每一条**连接创建路径上都设置了：`db/mod.rs:294`（测试模式）、`305`（新建）、`322`（正常打开）、`342`（明文迁移后）。连接池复用不会丢失该 pragma，且 `drop`（`52`）会主动把 `foreign_keys=false` 的连接踢出池。`WAL`（`304/321/341`）与 `busy_timeout=5000`（`306/313/343`）也都设了。**这是本层做得最扎实的一块。**

v41 迁移（`schema.rs:4734-4753`）主动补了三个删除解绑口：`calendar_events.case_id → NULL`、`tasks.knowledge_id → NULL`、`calendar_events.task_id → 级联删除`，并留下 `whiteboard_tombstones` 记录被删白板。`delete_deadline_rule`（`commands/deadline_rules.rs:230-234`）先解绑 `case_deadlines.rule_id` 再删规则，避开 `NO ACTION` 的 RESTRICT。这两处说明作者已经识别过孤儿问题。

### F1. 四张表完全没有外键也没有删除清理，案件删除后永久残留 【CONFIRMED】

| 表 | 定义位置 | 问题 |
| --- | --- | --- |
| `procedure_audit` | `schema.rs:815-822` | `case_id TEXT NOT NULL` **无 REFERENCES**。`db::cases::delete_case`（`486-509`）不清理它 |
| `procedure_item_states` | `schema.rs:804-812` | 只有 `item_id TEXT PRIMARY KEY`，`item_id = "{event_id}:{suffix}"`，无 case 维度，无外键 |
| `procedure_reminder_receipts` | `813-816` | 同上，且 `sent_on` 决定 R1/R2 去重窗口 |
| `reminder_log` | `schema.rs:1762-1775` | `case_id TEXT` 无外键（`rule_id` 有） |

具体触发：删除一个已完结案件 → 该案全部程序事项、处理状态、投递回执、提醒日志全部成为孤儿行。可观察结果有两层：(a) 存储无界增长；(b) **`procedure_reminder_receipts` 的孤儿行如果 item_id 恰好被复用（新事件拿到相同 id 的概率极低，实际不成立），会误抑制提醒**——所以这条主要是数据卫生问题，不是正确性问题。`procedure_audit` 尤其值得修：它是程序事件的**法律审计轨迹**，案件删除后审计记录仍在、但已无法与任何案件关联，等于审计链断裂。

### F2. 跨案 `parent_task_id` 在删除案件时未置空，可致删除永久失败 【SUSPECTED】

`db/cases.rs:500-505`：

```rust
conn.execute("UPDATE tasks SET parent_task_id = NULL
  WHERE case_id = ?1 AND parent_task_id IN (SELECT id FROM tasks WHERE case_id = ?1)", params![id])?;
```

只断开**同案**父子链。而 `tasks.parent_task_id TEXT REFERENCES tasks(id)`（`schema.rs:2721`）**没有 ON DELETE**。同时 v25 引入的 `trg_cases_preserve_shared_nodes`（`schema.rs:3807-3815`）会在删除案件时把**共享子任务改挂到另一个案件**：

```sql
UPDATE tasks SET case_id=(SELECT min(case_id) FROM case_task_links WHERE task_id=tasks.id AND case_id!=OLD.id) ...
```

具体触发序列：A 案与 B 案共享任务 S。删除 A 案时，触发器把 S 的 `case_id` 改挂到 B 案。此时若 B 案存在任务 T 且 `T.parent_task_id = S`（在触发器执行前就建立的跨案父子关系），删除 A 案 → `DELETE FROM cases(A)` 级联删掉 S（按旧的 case_id 归属）→ T 仍指向已删的 S → 外键检查失败 → `delete_case` 整个事务回滚。

可观察结果：用户在界面上无法删除该案件，错误信息是裸 SQL 的 `FOREIGN KEY constraint failed`，且没有任何说明指向"共享子任务的跨案父子链"。触发条件较绕（需要跨案父子链），因此标 SUSPECTED，但修复成本低：把 `parent_task_id` 的置空条件从"同案"扩展为"任何指向本 `id` 集合的引用"。

### F3. `tasks.inbox_source_id` 无 ON DELETE，删除收件箱项可能失败 【SUSPECTED】

`schema.rs:1541`（v2 内）：`ALTER TABLE tasks ADD COLUMN inbox_source_id TEXT REFERENCES inbox_items(id);` —— 无 ON DELETE。`commands/tasks.rs:340-343` 在建任务后单独 `UPDATE tasks SET inbox_source_id=?2 WHERE id=?1`。若该收件箱项被删除，v2 重建后的 `inbox_items` 上没有指向 `tasks` 的约束，而 `tasks` 这一侧是 NO ACTION → 删除收件箱项时报 FK 失败或（在 FK 失效时）留下悬空 id。优先级低于 F1/F2。

### F4. `delete_person` 静默级联清除案件角色记录 【CONFIRMED，设计后果】

`schema.rs:1398-1405` 的 `case_persons.person_id ... ON DELETE CASCADE` + `commands/persons.rs:150-155` 的无条件 `DELETE FROM persons WHERE id=?1`。删除一名主审法官会静默清除其在**所有**案件上的 `role` 记录（"主审法官"），且 `delete_person` 连存在性检查都没有（对比 `delete_task` 在 `tasks.rs:439-455` 有完整的 not-found / already-deleted 分流）。这是外键设计的正常后果，不是代码缺陷，但值得在 UI 上提示"该人员关联 N 个案件"。

---

## 并发

### X1. 迁移期与运行时的连接隔离是干净的，但缺少迁移前备份 【CONFIRMED，见 C1】

`enter_maintenance`（`db/mod.rs:86-107`）的排他语义正确：先 CAS 置位 `MAINTENANCE_MODE`（`87-92`），再 `reset_shared_conn()` 清空空闲池（`96`），再在 `ACTIVE_CONNECTIONS` 上等 30 秒（`97-104`）。`open_db`（`110-127`）在**持有同一把计数锁**的情况下检查 `is_maintenance_mode()` 并递增计数，所以不存在"检查通过后、计数递增前"被抢占的窗口。✓ 备份/恢复路径因此是安全的。

**但迁移没有走这条路径。** `init_db` 直接在普通租约连接上跑，没有进入维护模式、没有备份。

### X2. 提醒引擎不跨周期持有连接 【CONFIRMED，正面】

`start_reminder_engine`（`reminder.rs:1317-1352`）的循环体是 `match db::open_db() { Ok(conn) => { ... } }`，`conn` 在 match 臂结束即 drop，**在 300 秒 `thread::sleep` 之前**。不会长期占用连接槽位。✓

### X3. `check_and_trigger` 的失败是自愈的 【CONFIRMED】

`engine.rs:...` 实际上 `check_and_trigger` 内的写入是逐条 autocommit。若某条 `INSERT reminder_log` 撞上 5 秒 `busy_timeout` 失败，`check_and_trigger` 返回 `Err`，`1318` 行只 `log::error!` 后进入下一轮。因为 D1 的 receipt 写入在派发之后，失败时 receipt 未落库 → 下一轮重试。**自愈成立** ✓。但代价是一次锁冲突会丢弃该轮**已成功派发但状态未记录**的通知（通知已发、receipt 未写 → 下一轮重复发）。低概率、低影响。

### X4. 事务内 `spawn` 异步任务读未提交数据 【SUSPECTED，自愈】

`commands/tasks.rs:351` 的 `sync_task_reminder_calendar` 在 `create_task_in_transaction` 的事务内被调用，其内部 `reminder.rs:889-893` 用 `tauri::async_runtime::spawn` 派发 `execute_calendar_job(payload)`。被 spawn 的任务会在**调用方事务提交之前**尝试用自己的连接访问 `reminder_jobs`。由于 payload 已自带 `job_id`/`summary`/`dtstart`，`execute_calendar_job` 大概率先 PUT 到 CalDAV 再回写状态，因此实际影响有限；且引擎每轮 `1344-1348` 会 `spawn(sync_reminders_to_calendar())` 重扫 pending 作业，**自愈成立**。标 SUSPECTED 而非 CONFIRMED，因为没有读过 `execute_calendar_job` 的完整实现。

---

## 修复建议（按收益排序）

### P0 — 迁移安全网（对应 C1、C2）

1. **迁移前自动快照。** 在 `lib.rs:88-90` 的 `init_db` 之前检测 `user_version < CURRENT_SCHEMA_VERSION`，用 `VACUUM INTO` 或 `backup.rs` 已有的快照逻辑写一份 `pre-migrate-v{N}-{timestamp}.db`，失败则中止迁移。约 20 行，把"永久锁死 + 无副本"降级为"用户可自行恢复"。**这是本清单里投入产出比最高的一项。**

2. **拆分 `apply_conditional_segments` 的单一事务。** 现状（`schema.rs:3590-3834`）是一个事务包住全部条件段。改为每条独立变更（`cases` 重建、`knowledge_items` 重建、`task_events` 重建、每组 `ALTER`）各自一个事务 + 各自的"已完成"标记（可复用 `PRAGMA user_version` 的细粒度化，或在 `settings` 里存 `applied_segments`）。这样单点失败不再回滚全部，也让"重跑"真正自愈。

3. **给版本化迁移的 CHECK 收窄加前置扫描。** v2（`1490-1503`）和 v10（`2580-2586`）的 `INSERT ... SELECT` 在执行前先 `SELECT DISTINCT status FROM inbox_items WHERE status NOT IN (...)`，命中就把新枚举值加入 CHECK 或中止并给出可读错误，而不是让 `apply_migration_tx` 抛裸 constraint 错误。

4. **补 `2025-10-08`，并把 `year_range()` 接进降级路径。** `holidays.rs:56-62` 加入 `"2025-10-08"`（同时 `builtin_holiday_name` 的 `(2025,10,6)=>中秋` 保持不变）。更重要的是：`HolidayCalendar` 增加 `covers_year(i32) -> bool`，`engine.rs:139` 生成 `DeadlineResult` 时对未覆盖年份强制 `deadline_source = "unconfirmed_calendar"` + 前缀 `[待核对]`，与 `procedure.rs:723-729` 已有的处理对齐（对应 C4）。

### P1 — 提醒投递的"待核对"与"时段外"缺口（对应 D1、D2）

5. **`check_procedure_deadline_rules` 跳过 `needs_review` 项的 R1/R2 派发。** 在 `reminder.rs:255` 的循环里加 `if i.needs_review && (level=="R1"||level=="R2") { continue; }`，或降级为 `monitor` 级（不弹系统通知、只进通知中心）。当前行为是"把明确标记为不可信的日期以最高级别弹出"，方向完全反了。

6. **把程序期限接进 `defer_reminder_job`。** 删掉 `reminder.rs:264` 的 `continue`，改为构造一个 `CalendarJobCtx { entity_type: "procedure_item", entity_id: i.id, ... }` 传给 `dispatch_reminder`，让 `dispatch_reminder_at:505-513` 的延迟分支生效（或在派发前自行调 `defer_reminder_job`）。否则程序期限的时段外提醒是"碰运气"派发，重启即丢。

### P2 — 规则引擎的静默失效（对应 D4、D5）

7. **把 `engine.rs:96` 的 id 硬编码换成数据驱动标记。** 给 `deadline_rules` 加一列 `seeded_legacy INTEGER`（或在 `settings` 里存一份 id 列表），`upsert_deadline_rule` 在 UPDATE 时**清除该标记**，跳过条件改为 `rule.seeded_legacy && rule.id == original_id`。这样用户编辑后规则立即生效，审计与行为一致。

8. **`upsert_deadline_rule` 复用 `engine.rs` 的校验。** 把 `offset_value ∈ [1,3650]`、`calendar_month ≤ 120`、`trigger_field ∈ get_case_date_field` 的白名单、`calc_method ∈ {patent, civil}` 提取成一个 `validate_rule()`，在写入前调用（失败即拒写）。可以直接抄 `personal_availability.rs:40-78` 的校验风格。

### P3 — 数据一致性与契约（对应 F1、F2、T3）

9. **给 `procedure_audit` / `procedure_item_states` / `procedure_reminder_receipts` 补 case 维度。** 新增一列 `case_id TEXT` + 索引，在 `db::cases::delete_case` 中一并清理。`procedure_audit` 优先级最高——它是程序事件的审计轨迹，案件删除后残留即审计链断裂。

10. **扩展 `delete_case` 的父子链解空范围。** `db/cases.rs:500-505` 的条件从"同案"改为"指向本 `id` 集合或被本集合指向"，并在同一事务内。

11. **把 `db::cases::delete_case` 的签名改成 `&rusqlite::Transaction`**（或函数内自行开事务），让原子性成为类型层面的要求而非调用者的记忆负担。

### P4 — 健壮性小项

12. **`formula/eval.rs:230` 的 `months as u32` 改为 `i32` 并支持负数**（照 Excel：`EDATE(d,-1)` = 上一个月），同时给 `days_in_month`（`414-419`）的 `from_ymd_opt(...).unwrap()` 换成 `?` 或范围钳制，消除 panic 面（对应 D3）。

13. **修正文档漂移**：`ARCHITECTURE.md:42` 和 `DATA_AND_SECURITY.md:66` 的"当前版本 42"改为 43。

14. **T4 的 `let _ =` 改为记录 `reminder_jobs` 写入失败**（写 `log::warn!` + 在任务响应里带一个 `calendarSyncPending` 标志），让律师知道外部日历没有这条提醒。
