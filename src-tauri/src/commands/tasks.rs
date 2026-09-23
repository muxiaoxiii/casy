use super::run_blocking;
use crate::db;
use rusqlite::OptionalExtension;

#[derive(serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TaskFilter {
    pub completed: Option<bool>,
    pub case_id: Option<String>,
    pub area_id: Option<String>,
    pub task_type: Option<String>,
    pub start_bucket: Option<String>,
}

/// 任务列表项（B1 类型化：返回侧 Value → 强类型；字段与前端手写 Task 契约一致）
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TaskDto {
    pub id: String,
    pub case_id: Option<String>,
    pub task_name: String,
    pub description: Option<String>,
    pub created_date: String,
    pub deadline: Option<String>,
    pub priority: Option<String>,
    pub completed: i32,
    pub assignee: Option<String>,
    pub finish_note: Option<String>,
    pub task_type: String,
    pub plan_defined: bool,
    pub planned_start_date: Option<String>,
    pub planned_end_date: Option<String>,
    pub plan_revision: Option<i32>,
    pub start_date: Option<String>,
    pub due_date: Option<String>,
    pub due_time: Option<String>,
    pub time_block: Option<String>,
    pub waiting_for: Option<String>,
    pub follow_up_date: Option<String>,
    pub context: Option<String>,
    pub flagged: i32,
    pub sequential: i32,
    pub blocked: i32,
    pub sequence_order: i32,
    pub start_bucket: String,
    pub today_index: i32,
    pub estimated_minutes: Option<i32>,
    pub actual_minutes: Option<i32>,
    pub is_overdue: i32,
    pub due_soon: i32,
    pub last_review_date: Option<String>,
    pub next_review_date: Option<String>,
    pub area_id: Option<String>,
    pub knowledge_id: Option<String>,
    pub parent_task_id: Option<String>,
    pub recurrence_rule: Option<String>,
    pub is_focus: i32,
    /// W2：OmniFocus 式推迟日（YYYY-MM-DD）；未到期任务在今日/焦点透视隐藏
    pub defer_until: Option<String>,
}

/// 全局搜索结果项（轻量列）
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchTaskDto {
    pub id: String,
    pub task_name: String,
    pub due_date: Option<String>,
    pub completed: i64,
    pub start_bucket: Option<String>,
}

/// 将 tasks 表一行映射为 TaskDto（list_tasks / restore_task 共用，保持字段契约一致）
fn row_to_task_dto(row: &rusqlite::Row) -> rusqlite::Result<TaskDto> {
    Ok(TaskDto {
        id: row.get::<_, String>("id")?,
        case_id: row.get::<_, Option<String>>("case_id")?,
        task_name: row.get::<_, String>("task_name")?,
        description: row.get::<_, Option<String>>("description")?,
        created_date: row.get::<_, String>("created_date")?,
        deadline: row.get::<_, Option<String>>("due_date")?.filter(|v| !v.is_empty()).or(row.get::<_, Option<String>>("deadline")?),
        priority: row.get::<_, Option<String>>("priority")?,
        completed: row.get::<_, i32>("completed")?,
        assignee: row.get::<_, Option<String>>("assignee")?,
        finish_note: row.get::<_, Option<String>>("finish_note")?,
        task_type: row
            .get::<_, Option<String>>("task_type")?
            .unwrap_or_else(|| "action".to_string()),
        plan_defined: row.get::<_, Option<i32>>("plan_revision")?.is_some(),
        planned_start_date: row.get("planned_start_date")?,
        planned_end_date: row.get("planned_end_date")?,
        plan_revision: row.get("plan_revision")?,
        start_date: row.get::<_, Option<String>>("start_date")?,
        due_date: row.get::<_, Option<String>>("due_date")?.filter(|v| !v.is_empty()).or(row.get::<_, Option<String>>("deadline")?),
        due_time: row.get::<_, Option<String>>("due_time")?,
        time_block: row.get::<_, Option<String>>("time_block")?,
        waiting_for: row.get::<_, Option<String>>("waiting_for")?,
        follow_up_date: row.get::<_, Option<String>>("follow_up_date")?,
        context: row.get::<_, Option<String>>("context")?,
        flagged: row.get::<_, Option<i32>>("flagged")?.unwrap_or(0),
        sequential: row.get::<_, Option<i32>>("sequential")?.unwrap_or(0),
        blocked: row.get::<_, Option<i32>>("blocked")?.unwrap_or(0),
        sequence_order: row.get::<_, Option<i32>>("sequence_order")?.unwrap_or(0),
        start_bucket: row
            .get::<_, Option<String>>("start_bucket")?
            .unwrap_or_else(|| "anytime".to_string()),
        today_index: row.get::<_, Option<i32>>("today_index")?.unwrap_or(0),
        estimated_minutes: row.get::<_, Option<i32>>("estimated_minutes")?,
        actual_minutes: row.get::<_, Option<i32>>("actual_minutes")?,
        is_overdue: row.get::<_, Option<i32>>("is_overdue")?.unwrap_or(0),
        due_soon: row.get::<_, Option<i32>>("due_soon")?.unwrap_or(0),
        last_review_date: row.get::<_, Option<String>>("last_review_date")?,
        next_review_date: row.get::<_, Option<String>>("next_review_date")?,
        area_id: row.get::<_, Option<String>>("area_id")?,
        knowledge_id: row.get::<_, Option<String>>("knowledge_id")?,
        parent_task_id: row.get::<_, Option<String>>("parent_task_id")?,
        recurrence_rule: row.get::<_, Option<String>>("recurrence_rule")?,
        is_focus: row.get::<_, i32>("is_focus")?,
        defer_until: row.get::<_, Option<String>>("defer_until")?,
    })
}

const TASK_PROJECTION: &str = "SELECT tasks.*,
    (SELECT start_date FROM task_plans WHERE task_id=tasks.id) AS planned_start_date,
    (SELECT end_date FROM task_plans WHERE task_id=tasks.id) AS planned_end_date,
    (SELECT revision FROM task_plans WHERE task_id=tasks.id) AS plan_revision FROM tasks";

fn load_task_row(conn: &rusqlite::Connection, id: &str) -> anyhow::Result<TaskDto> {
    conn.query_row(
        &format!("{TASK_PROJECTION} WHERE id = ?1"),
        rusqlite::params![id],
        row_to_task_dto,
    )
    .map_err(|e| anyhow::anyhow!("读取还原任务失败 {}: {}", id, e))
}

#[tauri::command]
pub async fn list_tasks(filter: Option<TaskFilter>) -> Result<Vec<TaskDto>, String> {
    run_blocking(move || {
        db::with_conn(|conn| {
        // 软删除：默认过滤已删任务（deleted_at IS NULL），
        // 后续 AND 条件在此之上叠加。
        let mut sql = format!("{TASK_PROJECTION} WHERE deleted_at IS NULL");
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 1;

        if let Some(f) = &filter {
            if let Some(completed) = f.completed {
                sql.push_str(&format!(" AND completed = ?{}", idx));
                params.push(Box::new(completed as i32));
                idx += 1;
            }
            if let Some(case_id) = &f.case_id {
                if !case_id.is_empty() {
                    sql.push_str(&format!(" AND (case_id = ?{0} OR EXISTS (SELECT 1 FROM case_task_links ctl WHERE ctl.task_id=tasks.id AND ctl.case_id=?{0}))", idx));
                    params.push(Box::new(case_id.clone()));
                    idx += 1;
                }
            }
            if let Some(area_id) = &f.area_id {
                if !area_id.is_empty() {
                    sql.push_str(&format!(" AND area_id = ?{}", idx));
                    params.push(Box::new(area_id.clone()));
                    idx += 1;
                }
            }
            if let Some(task_type) = &f.task_type {
                if !task_type.is_empty() {
                    sql.push_str(&format!(" AND task_type = ?{}", idx));
                    params.push(Box::new(task_type.clone()));
                    idx += 1;
                }
            }
            if let Some(start_bucket) = &f.start_bucket {
                if !start_bucket.is_empty() {
                    match start_bucket.as_str() {
                        // W2 已推迟透视：defer_until 非空且未到期（未来日期）的未完成任务
                        "deferred" => {
                            sql.push_str(
                                " AND defer_until IS NOT NULL AND defer_until != '' \
                                 AND defer_until > date('now','localtime') AND completed = 0",
                            );
                        }
                        // W2 今日语义：未到期推迟任务隐藏（defer_until > 今天才藏，到期当天回归）
                        "today" => {
                            sql.push_str(" AND completed=0 AND (
                                COALESCE(NULLIF(due_date,''),NULLIF(deadline,''))<=date('now','localtime')
                                OR ((defer_until IS NULL OR defer_until<=date('now','localtime')) AND start_bucket!='someday'
                                  AND (start_bucket='today' OR (CASE WHEN EXISTS(SELECT 1 FROM task_plans WHERE task_id=tasks.id) THEN (SELECT start_date FROM task_plans WHERE task_id=tasks.id) ELSE NULLIF(start_date,'') END)<=date('now','localtime'))))");
                        }
                        _ => {
                            sql.push_str(&format!(" AND start_bucket = ?{}", idx));
                            params.push(Box::new(start_bucket.clone()));
                        }
                    }
                }
            }
        }

        sql.push_str(" ORDER BY CASE priority WHEN 'urgent_important' THEN 1 WHEN 'urgent' THEN 2 WHEN 'important' THEN 3 ELSE 4 END, deadline ASC");

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let tasks: Vec<TaskDto> = stmt
            .query_map(param_refs.as_slice(), row_to_task_dto)?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(tasks)
        })
    })
    .await
}

fn normalize_optional_task_fields(data: &mut serde_json::Value) {
    for key in ["caseId","areaId","knowledgeId","parentId","parentTaskId","deadline","dueDate","dueTime","startDate","deferUntil","followUpDate","nextReviewDate","lastReviewDate","recurrenceRule","context","timeBlock"] {
        if data[key].as_str().is_some_and(|value|value.trim().is_empty()) {
            data[key] = serde_json::Value::Null;
        }
    }
    // dueDate is canonical; deadline is its legacy alias, including explicit clears.
    if let Some(value) = data.get("dueDate").cloned() { data["deadline"] = value; }
    else if let Some(value) = data.get("deadline").cloned() { data["dueDate"] = value; }
}

#[tauri::command]
pub async fn create_task(data: serde_json::Value) -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let mut raw_conn = db::open_db()?;
        // 事务化：token 消费与写入同生共死（写失败则回滚，token 不被白烧）
        let conn = raw_conn.transaction()?;
        let result = create_task_in_transaction(&conn, data)?;
        conn.commit()?;
        Ok(result)
    })
    .await
}

pub(super) fn create_task_in_transaction(conn: &rusqlite::Connection, data: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        // P0-2: AI 授权网关（origin='ai' 必须携带有效 proposal token）
        crate::ai::gateway::verify_ai_mutation_authorized(
            &conn,
            data["origin"].as_str(),
            data["proposalToken"].as_str(),
            "create_task",
            "task",
            None,
            None,
            &data,
        )?;
        let id = db::new_id();
        let now = db::now_local();
        let mut data = data;
        if data["taskName"].as_str().is_none_or(|name|name.trim().is_empty()) {
            anyhow::bail!("请输入任务名称");
        }
        normalize_optional_task_fields(&mut data);
        let mut validation = data.clone();
        validation["id"] = serde_json::json!(id);
        serde_json::from_value::<UpdateTaskPatch>(validation)?.validate()?;

        // A1-7 修复：next_review_date 仅在用户显式设置时写入。
        // 原实现默认填下周日，导致 Review 透视被无回顾意图的任务淹没（噪音缺陷）。

        // ── 案件级顺序项目自动继承（设计哲学 §3.3）─────────────────────
        // 如果关联案件设置了 sequential=1，新任务自动继承 sequential
        let case_id = data["caseId"].as_str();
        let parent_id = data["parentId"].as_str().or(data["parentTaskId"].as_str());
        let (mut sequential, mut blocked, mut sequence_order) = (
            data["sequential"].as_i64().unwrap_or(0),
            data["blocked"].as_i64().unwrap_or(0),
            data["sequenceOrder"].as_i64().unwrap_or(0),
        );

        if let Some(cid) = case_id {
            let case_seq: Option<i32> = conn.query_row(
                "SELECT sequential FROM cases WHERE id = ?1",
                rusqlite::params![cid],
                |row| row.get(0),
            ).ok();
            if case_seq == Some(1) && sequential == 0 {
                sequential = 1;
                // 第一个 sequential 任务不阻塞，后续自动阻塞
                let (existing_count, next_order): (i32, i64) = conn.query_row(
                    "SELECT COUNT(*),COALESCE(MAX(sequence_order)+1,0) FROM tasks WHERE case_id = ?1 AND parent_task_id IS ?2 AND sequential = 1 AND completed = 0 AND deleted_at IS NULL",
                    rusqlite::params![cid,parent_id],
                    |row| Ok((row.get(0)?,row.get(1)?)),
                )?;
                if existing_count > 0 {
                    blocked = 1;
                }
                sequence_order = next_order;
            }
        }

        conn.execute(
            "INSERT INTO tasks (id, case_id, task_name, description, created_date, deadline, priority, completed, assignee, finish_note,
             task_type, start_date, due_date, due_time, waiting_for, follow_up_date, context, flagged, sequential, blocked, sequence_order,
             start_bucket, today_index, estimated_minutes, area_id, next_review_date, created_at, parent_task_id, recurrence_rule, is_focus,defer_until,time_block,knowledge_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29,?30,?31,?32)",
            rusqlite::params![
                id,
                case_id,
                data["taskName"].as_str().unwrap_or(""),
                data["description"].as_str().unwrap_or(""),
                data["createdDate"].as_str().unwrap_or(&now),
                data["deadline"].as_str().or(data["dueDate"].as_str()),
                data["priority"].as_str().unwrap_or("normal"),
                data["assignee"].as_str().unwrap_or(""),
                data["finishNote"].as_str().unwrap_or(""),
                // GTD 字段
                data["taskType"].as_str().unwrap_or("action"),
                data["startDate"].as_str(),
                data["dueDate"].as_str().or(data["deadline"].as_str()),
                data["dueTime"].as_str(),
                data["waitingFor"].as_str(),
                data["followUpDate"].as_str(),
                data["context"].as_str(),
                data["flagged"].as_i64().unwrap_or(0),
                sequential,
                blocked,
                sequence_order,
                data["startBucket"].as_str().unwrap_or("anytime"),
                data["todayIndex"].as_i64().unwrap_or(0),
                data["estimatedMinutes"].as_i64(),
                data["areaId"].as_str(),
                data["nextReviewDate"].as_str(), // A1-7：仅显式设置才写入
                now,
                // A1-4/A1-5
                parent_id,
                data["recurrenceRule"].as_str(),
                data["isFocus"].as_i64().unwrap_or(0),
                data["deferUntil"].as_str(),
                data["timeBlock"].as_str(),
                data["knowledgeId"].as_str(),
            ],
        )?;

        super::task_lifecycle::refresh_sequence(&conn, &id)?;
        if let Some(source) = data["inboxSourceId"].as_str() {
            conn.execute("UPDATE tasks SET inbox_source_id=?2 WHERE id=?1", rusqlite::params![id, source])?;
        }
        // 记录 task_event（AI 创建归因 actor='ai'）
        let actor = if data["origin"].as_str() == Some("ai") { "ai" } else { "user" };
        conn.execute(
            "INSERT INTO task_events (id, task_id, event_type, occurred_at, actor) VALUES (?1, ?2, 'created', ?3, ?4)",
            rusqlite::params![db::new_id(), id, now, actor],
        )?;

        // 设置即交接（设计哲学 §11.2）：任务带截止日期 + 日历同步启用 → 立即同步提醒到外部日历
        if let Some(due) = data["dueDate"].as_str().or(data["deadline"].as_str()) {
            let _ = crate::commands::reminder::sync_task_reminder_calendar(
                &conn,
                &id,
                due,
                data["dueTime"].as_str(),
                data["taskName"].as_str().unwrap_or(""),
                data["caseId"].as_str(),
            );
        }

        Ok(serde_json::json!({ "id": id }))
}

#[tauri::command]
pub async fn toggle_task(
    id: String,
    actual_minutes: Option<i64>,
    origin: Option<String>,
    proposal_token: Option<String>,
) -> Result<(), String> {
    if actual_minutes.is_some_and(|value|value < 0 || value > i32::MAX as i64) {
        return Err("实际耗时超出有效范围".into());
    }
    let cancelled = run_blocking(move || {
        let mut connection = db::open_db()?;
        let conn = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let current: i32 = conn.query_row(
            "SELECT completed FROM tasks WHERE id=?1 AND deleted_at IS NULL",
            [&id], |r|r.get(0))?;
        crate::ai::gateway::verify_ai_mutation_authorized(
            &conn, origin.as_deref(), proposal_token.as_deref(), "toggle_task", "task", Some(&id),
            Some(&crate::ai::gateway::compute_current_entity_hash(&conn,"task",&id)?),
            &serde_json::json!({"id":id}),
        )?;
        let completed = if current == 0 {1} else {0};
        conn.execute("UPDATE tasks SET completed=?2 WHERE id=?1",rusqlite::params![id,completed])?;
        if completed==1 {
            if let Some(minutes) = actual_minutes {
                conn.execute("UPDATE tasks SET actual_minutes=?2 WHERE id=?1",rusqlite::params![id,minutes])?;
            }
        }
        let actor = if origin.as_deref()==Some("ai") {"ai"} else {"user"};
        conn.execute("INSERT INTO task_events(id,task_id,event_type,occurred_at,payload,actor) VALUES(?1,?2,?3,?4,?5,?6)",
            rusqlite::params![db::new_id(),id,if completed==1 {"completed"} else {"restored"},db::now_local(),
                serde_json::json!({"actualMinutes":actual_minutes}).to_string(),actor])?;
        let cancelled = super::task_lifecycle::completion_effects(&conn,&id,completed)?;
        conn.commit()?;
        Ok(cancelled)
    }).await?;
    cancel_task_reminders(cancelled).await;
    Ok(())
}

async fn cancel_task_reminders(ids: Vec<String>) {
    for id in ids {
        if let Err(error) = super::caldav::cancel_jobs_for_entity("task",&id).await {
            log::warn!("取消任务提醒失败 ({id}): {error}");
        }
    }
}
#[tauri::command]
pub async fn delete_task(
    id: String,
    origin: Option<String>,
    proposal_token: Option<String>,
) -> Result<(), String> {
    let task_id = id.clone();
    run_blocking(move || {
        let mut raw_conn = db::open_db()?;
        // 事务化：token 消费与写入同生共死（写失败则回滚，token 不被白烧）
        let conn = raw_conn.transaction()?;
        // P0-2: AI 授权网关（origin='ai' 必须携带有效 proposal token）
        crate::ai::gateway::verify_ai_mutation_authorized(
            &conn,
            origin.as_deref(),
            proposal_token.as_deref(),
            "delete_task",
            "task",
            Some(&id),
            Some(&crate::ai::gateway::compute_current_entity_hash(
                &conn, "task", &id,
            )?),
            &serde_json::json!({ "id": id }),
        )?;
        // 软删除（v24）：UPDATE deleted_at 而非 DELETE。行保留，关联提醒/事件/子项不被级联删除。
        let now = db::now_local();
        let rows = conn.execute(
            "UPDATE tasks SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
            rusqlite::params![now, id],
        )?;
        if rows == 0 {
            // 任务不存在 → 报错；已被软删 → 幂等成功（重复删除不视为错误、不重复写审计）。
            let deleted_at: Option<String> = conn
                .query_row(
                    "SELECT deleted_at FROM tasks WHERE id = ?1",
                    rusqlite::params![id],
                    |r| r.get(0),
                )
                .ok();
            if deleted_at.is_none() {
                return Err(anyhow::anyhow!(crate::error_code::err(
                    crate::error_code::codes::TASK_NOT_FOUND,
                    format!("任务不存在: {id}"),
                )));
            }
            conn.commit()?;
            return Ok(());
        }
        // 审计：写 deleted 事件（task_events.event_type 已随 v24 条件重建扩展 'deleted'）
        let actor = if origin.as_deref() == Some("ai") { "ai" } else { "user" };
        let payload = serde_json::json!({ "reason": "soft_delete", "deletedAt": now }).to_string();
        conn.execute(
            "INSERT INTO task_events (id, task_id, event_type, occurred_at, payload, actor) VALUES (?1, ?2, 'deleted', ?3, ?4, ?5)",
            rusqlite::params![db::new_id(), id, now, payload, actor],
        )?;
        super::task_lifecycle::refresh_sequence(&conn, &id)?;
        conn.commit()?;
        Ok(())
    })
    .await?;

    // 任务删除后撤销其提醒作业（含已同步到日历的事件）
    if let Err(e) = super::caldav::cancel_jobs_for_entity("task", &task_id).await {
        log::warn!("任务删除后撤销提醒作业失败 (task {}): {}", task_id, e);
    }

    Ok(())
}

/// 撤销删除：按快照还原任务（保留原 id 与 completed）。
///
/// v24 软删除语义：`delete_task` 只写 `tasks.deleted_at`，行与关联提醒/事件/子项
/// 均未被级联删除。因此 `restore_task` 对**已软删原行**执行 undelete（清 `deleted_at`）
/// 并按快照恢复必要字段，写在原行上；仅当原行不存在（旧硬删除遗留）时才走
/// INSERT 全字段重建的向后兼容路径。
///
/// P1-6 修复：`create_task` 忽略入参 id 另生成新 id，并把 completed 硬编码为 0，
/// 导致"撤销删除"恢复出**新 id、未完成**的任务。本命令刻意不走 `create_task` 的
/// 「案件级 sequential 自动继承」与「AI 授权网关」，而是按前端 Task 快照做全字段还原，
/// 返回还原后的完整任务对象供前端对账。
///
/// 原子性：tasks 行 + task_events 事件 + 还原后读取在**同一事务**内完成，
/// 任一失败整体回滚，避免留下半恢复行。
fn restore_task_inner(
    conn: &mut rusqlite::Connection,
    snapshot: &serde_json::Value,
) -> anyhow::Result<TaskDto> {
    let id = snapshot["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("恢复任务失败：快照缺少 id"))?
        .to_string();
    let now = db::now_local();

    let tx = conn.transaction()?;

    // 还原原 id 与 completed（P1-6：completed 必须取快照值，而非 create_task 的字面量 0）
    let completed = snapshot["completed"].as_i64().unwrap_or(0);

    // parent_task_id 兼容两种字段名：前端 Task 用 parentId，生成 DTO 用 parentTaskId
    let parent_task_id = snapshot["parentId"]
        .as_str()
        .or(snapshot["parentTaskId"].as_str());

    // v24 软删除：读取当前行状态（含软删行）。
    //   已软删（deleted_at 非空）→ undelete（清 deleted_at）+ 按快照恢复必要字段；
    //   已存在且未删（deleted_at 为空）→ id 冲突，拒绝覆盖，避免误伤现有任务；
    //   不存在 → 旧硬删除快照还原（向后兼容），INSERT 全字段重建。
    let row_state: Option<Option<String>> = tx
        .query_row(
            "SELECT deleted_at FROM tasks WHERE id = ?1",
            rusqlite::params![id],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()?;

    if let Some(Some(_deleted_at)) = row_state {
        // 已软删原行：undelete。行仍在库，关联提醒/事件/子项从未被级联删除，无需重建。
        // 按快照恢复必要字段，使还原后的任务与前端快照完全一致（应对软删期间字段漂移）。
        tx.execute(
            "UPDATE tasks SET
             case_id = ?2, task_name = ?3, description = ?4, created_date = ?5, deadline = ?6,
             priority = ?7, completed = ?8, assignee = ?9, finish_note = ?10, task_type = ?11,
             start_date = ?12, due_date = ?13, due_time = ?14, waiting_for = ?15, follow_up_date = ?16,
             context = ?17, flagged = ?18, sequential = ?19, blocked = ?20, sequence_order = ?21,
             start_bucket = ?22, today_index = ?23, estimated_minutes = ?24, actual_minutes = ?25,
             is_overdue = ?26, due_soon = ?27, last_review_date = ?28, next_review_date = ?29,
             area_id = ?30, knowledge_id = ?31, parent_task_id = ?32, recurrence_rule = ?33,
             is_focus = ?34, defer_until = ?35,
             deleted_at = NULL, updated_at = ?36, time_block = ?37
             WHERE id = ?1",
            rusqlite::params![
                id,
                snapshot["caseId"].as_str(),
                snapshot["taskName"].as_str().unwrap_or(""),
                snapshot["description"].as_str(),
                snapshot["createdDate"].as_str().unwrap_or(&now),
                snapshot["deadline"].as_str().or(snapshot["dueDate"].as_str()),
                snapshot["priority"].as_str().unwrap_or("normal"),
                completed,
                snapshot["assignee"].as_str(),
                snapshot["finishNote"].as_str(),
                snapshot["taskType"].as_str().unwrap_or("action"),
                snapshot["startDate"].as_str(),
                snapshot["dueDate"].as_str().or(snapshot["deadline"].as_str()),
                snapshot["dueTime"].as_str(),
                snapshot["waitingFor"].as_str(),
                snapshot["followUpDate"].as_str(),
                snapshot["context"].as_str(),
                snapshot["flagged"].as_i64().unwrap_or(0),
                snapshot["sequential"].as_i64().unwrap_or(0),
                snapshot["blocked"].as_i64().unwrap_or(0),
                snapshot["sequenceOrder"].as_i64().unwrap_or(0),
                snapshot["startBucket"].as_str().unwrap_or("anytime"),
                snapshot["todayIndex"].as_i64().unwrap_or(0),
                snapshot["estimatedMinutes"].as_i64(),
                snapshot["actualMinutes"].as_i64(),
                snapshot["isOverdue"].as_i64().unwrap_or(0),
                snapshot["dueSoon"].as_i64().unwrap_or(0),
                snapshot["lastReviewDate"].as_str(),
                snapshot["nextReviewDate"].as_str(),
                snapshot["areaId"].as_str(),
                snapshot["knowledgeId"].as_str(),
                parent_task_id,
                snapshot["recurrenceRule"].as_str(),
                snapshot["isFocus"].as_i64().unwrap_or(0),
                snapshot["deferUntil"].as_str(),
                now,
                snapshot["timeBlock"].as_str(),
            ],
        )?;
        // 审计：写 restored 事件（task_events.event_type 已随 v24 条件重建扩展 'restored'）
        tx.execute(
            "INSERT INTO task_events (id, task_id, event_type, occurred_at, actor) VALUES (?1, ?2, 'restored', ?3, 'user')",
            rusqlite::params![db::new_id(), id, now],
        )?;
    } else if row_state.is_some() {
        // 行已存在且未删（deleted_at 为空）→ id 冲突，拒绝覆盖
        return Err(anyhow::anyhow!("无法恢复任务 {}：该 id 已被占用", id));
    } else {
        // 行不存在 → 旧硬删除快照还原（向后兼容）：INSERT 全字段重建
        tx.execute(
            "INSERT INTO tasks (id, case_id, task_name, description, created_date, deadline, priority, completed, assignee, finish_note,
             task_type, start_date, due_date, due_time, waiting_for, follow_up_date, context, flagged, sequential, blocked, sequence_order,
             start_bucket, today_index, estimated_minutes, actual_minutes, is_overdue, due_soon, last_review_date, next_review_date,
             area_id, knowledge_id, created_at, parent_task_id, recurrence_rule, is_focus, defer_until,time_block)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30,?31,?32,?33,?34,?35,?36,?37)",
            rusqlite::params![
                id,
                snapshot["caseId"].as_str(),
                snapshot["taskName"].as_str().unwrap_or(""),
                snapshot["description"].as_str(),
                snapshot["createdDate"].as_str().unwrap_or(&now),
                snapshot["deadline"].as_str().or(snapshot["dueDate"].as_str()),
                snapshot["priority"].as_str().unwrap_or("normal"),
                completed,
                snapshot["assignee"].as_str(),
                snapshot["finishNote"].as_str(),
                snapshot["taskType"].as_str().unwrap_or("action"),
                snapshot["startDate"].as_str(),
                snapshot["dueDate"].as_str().or(snapshot["deadline"].as_str()),
                snapshot["dueTime"].as_str(),
                snapshot["waitingFor"].as_str(),
                snapshot["followUpDate"].as_str(),
                snapshot["context"].as_str(),
                snapshot["flagged"].as_i64().unwrap_or(0),
                snapshot["sequential"].as_i64().unwrap_or(0),
                snapshot["blocked"].as_i64().unwrap_or(0),
                snapshot["sequenceOrder"].as_i64().unwrap_or(0),
                snapshot["startBucket"].as_str().unwrap_or("anytime"),
                snapshot["todayIndex"].as_i64().unwrap_or(0),
                snapshot["estimatedMinutes"].as_i64(),
                snapshot["actualMinutes"].as_i64(),
                snapshot["isOverdue"].as_i64().unwrap_or(0),
                snapshot["dueSoon"].as_i64().unwrap_or(0),
                snapshot["lastReviewDate"].as_str(),
                snapshot["nextReviewDate"].as_str(),
                snapshot["areaId"].as_str(),
                snapshot["knowledgeId"].as_str(),
                now,
                parent_task_id,
                snapshot["recurrenceRule"].as_str(),
                snapshot["isFocus"].as_i64().unwrap_or(0),
                snapshot["deferUntil"].as_str(),
                snapshot["timeBlock"].as_str(),
            ],
        )?;
        // 审计：硬删除快照还原 → 行被重建，写 'created' 事件（保持既有语义）
        tx.execute(
            "INSERT INTO task_events (id, task_id, event_type, occurred_at, actor) VALUES (?1, ?2, 'created', ?3, 'user')",
            rusqlite::params![db::new_id(), id, now],
        )?;
    }

    super::task_lifecycle::refresh_sequence(&tx, &id)?;
    // 在 commit 前读取还原后的 DTO（事务内自读自己的写入），再做原子提交
    let dto = load_task_row(&tx, &id)?;
    tx.commit()?;
    Ok(dto)
}

#[tauri::command]
pub async fn restore_task(snapshot: serde_json::Value) -> Result<TaskDto, String> {
    run_blocking(move || {
        let mut conn = db::open_db()?;
        restore_task_inner(&mut conn, &snapshot)
    })
    .await
}

/// 任务"稍后提醒"（设计哲学 §5.4 / §11.9：推迟任务并记录 snoozed 行为事件）
/// option: tonight / tomorrow / weekend / next_week / custom（+new_due_date）
#[tauri::command]
pub async fn snooze_task(
    id: String,
    option: Option<String>,
    new_due_date: Option<String>,
) -> Result<(), String> {
    run_blocking(move || {
        let mut connection = db::open_db()?;
        let conn = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let now = db::now_local();
        use chrono::{Datelike, Duration, Local};
        let today = Local::now().date_naive();

        // 计算新日期
        let (new_date, label) = match option.as_deref() {
            Some("tonight") => (today.to_string(), "今晚".to_string()),
            Some("tomorrow") => ((today + Duration::days(1)).to_string(), "明天".to_string()),
            Some("weekend") => {
                let days_to_sat = (5 + 7 - today.weekday().num_days_from_monday()) % 7;
                ((today + Duration::days(days_to_sat as i64)).to_string(), "周末".to_string())
            }
            Some("next_week") => ((today + Duration::days(7)).to_string(), "下周".to_string()),
            None | Some("custom") => {
                let d = new_due_date.ok_or_else(||anyhow::anyhow!("请选择计划日期"))?;
                (d, "自定义".to_string())
            }
            _ => anyhow::bail!("无效计划选项"),
        };
        chrono::NaiveDate::parse_from_str(&new_date,"%Y-%m-%d")?;

        let has_plan: bool=conn.query_row("SELECT EXISTS(SELECT 1 FROM task_plans WHERE task_id=?1)",[&id],|r|r.get(0))?;
        anyhow::ensure!(!has_plan,"此任务使用独立计划，请在甘特图中调整并确认日期");
        // Snooze changes the plan, never a legal deadline.
        // 软删拒绝：仅对活跃任务生效，0 行命中即任务不存在/已软删。
        let is_today = new_date == today.to_string();
        let bucket = if is_today { "today" } else { "anytime" };
        let rows = conn.execute(
            "UPDATE tasks SET start_date=?1,start_bucket=?2,time_block=CASE WHEN ?4 THEN 'evening' ELSE time_block END WHERE id=?3 AND deleted_at IS NULL AND completed=0",
            rusqlite::params![new_date, bucket, id,option.as_deref()==Some("tonight")],
        )?;
        if rows != 1 {
            return Err(anyhow::anyhow!(crate::error_code::err(
                crate::error_code::codes::TASK_NOT_FOUND,
                format!("任务不存在: {}", id),
            )));
        }

        // 写 snoozed 行为事件（支撑"懂你的节奏/模式"学习）
        let payload = serde_json::json!({
            "option": option.unwrap_or_else(|| "custom".to_string()),
            "newStartDate": new_date,
            "label": label,
        });
        conn.execute(
            "INSERT INTO task_events (id, task_id, event_type, occurred_at, payload, actor) VALUES (?1, ?2, 'snoozed', ?3, ?4, 'user')",
            rusqlite::params![db::new_id(), id, now, serde_json::to_string(&payload).unwrap_or_default()],
        )?;
        conn.commit()?;
        Ok(())
    })
    .await
}

/// W2 OmniFocus 式推迟日：设置 defer_until（YYYY-MM-DD）
///
/// 与 snooze_task（改 due/start 日期）正交：defer_until 只做"可见性闸门"——
/// 未到期任务在今日/焦点透视隐藏，到期当天自动回归，不改任务本身的计划日期。
#[tauri::command]
pub async fn defer_task(task_id: String, until: String) -> Result<(), String> {
    run_blocking(move || {
        // 校验日期格式（YYYY-MM-DD）
        chrono::NaiveDate::parse_from_str(&until, "%Y-%m-%d").map_err(|_| {
            anyhow::anyhow!(
                "Invalid date format for defer until: expected YYYY-MM-DD, got '{}'",
                until
            )
        })?;

        let conn = db::open_db()?;
        let now = db::now_local();

        let rows = conn.execute(
            "UPDATE tasks SET defer_until = ?1 WHERE id = ?2 AND deleted_at IS NULL",
            rusqlite::params![until, task_id],
        )?;
        if rows != 1 {
            return Err(anyhow::anyhow!(crate::error_code::err(
                crate::error_code::codes::TASK_NOT_FOUND,
                format!("任务不存在: {}", task_id),
            )));
        }

        // 写 deferred 行为事件（actor='user'，payload 带推迟目标日期）
        let payload = serde_json::json!({ "until": until }).to_string();
        conn.execute(
            "INSERT INTO task_events (id, task_id, event_type, occurred_at, payload, actor) VALUES (?1, ?2, 'deferred', ?3, ?4, 'user')",
            rusqlite::params![db::new_id(), task_id, now, payload],
        )?;

        Ok(())
    })
    .await
}

/// W2 提前结束推迟：清除 defer_until（任务立即回到今日/焦点可见集合）
#[tauri::command]
pub async fn clear_task_defer(task_id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let rows = conn.execute(
            "UPDATE tasks SET defer_until = NULL WHERE id = ?1 AND deleted_at IS NULL",
            rusqlite::params![task_id],
        )?;
        if rows != 1 {
            return Err(anyhow::anyhow!(crate::error_code::err(
                crate::error_code::codes::TASK_NOT_FOUND,
                format!("任务不存在: {}", task_id),
            )));
        }
        Ok(())
    })
    .await
}

use crate::types::PatchField;
use serde::Deserialize;

fn deserialize_patch_string<'de, D>(deserializer: D) -> Result<PatchField<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde_json::Value;
    let opt = Option::<Value>::deserialize(deserializer)?;
    match opt {
        None => Ok(PatchField::Null),
        Some(Value::Null) => Ok(PatchField::Null),
        Some(Value::String(s)) => Ok(PatchField::Value(s)),
        Some(Value::Number(n)) => Ok(PatchField::Value(n.to_string())),
        Some(Value::Bool(b)) => Ok(PatchField::Value(b.to_string())),
        Some(other) => Err(serde::de::Error::custom(format!(
            "expected string or null, got {:?}",
            other
        ))),
    }
}

fn deserialize_patch_i32<'de, D>(deserializer: D) -> Result<PatchField<i32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde_json::Value;
    let opt = Option::<Value>::deserialize(deserializer)?;
    match opt {
        None => Ok(PatchField::Null),
        Some(Value::Null) => Ok(PatchField::Null),
        Some(Value::Number(n)) => {
            if let Some(i) = n.as_i64() {
                i32::try_from(i).map(PatchField::Value).map_err(|_|serde::de::Error::custom("integer out of range"))
            } else {
                Err(serde::de::Error::custom("invalid integer number"))
            }
        }
        Some(Value::Bool(b)) => Ok(PatchField::Value(if b { 1 } else { 0 })),
        Some(other) => Err(serde::de::Error::custom(format!(
            "expected number, bool, or null, got {:?}",
            other
        ))),
    }
}

/// 任务类型化更新契约（P1-2：三态字段 + 服务端校验 + 影响行数断言）
#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTaskPatch {
    pub id: String,

    // AI 授权凭证（P0-2：服务端不可绕过授权网关）
    #[serde(default)]
    pub origin: Option<String>,
    #[serde(default)]
    pub proposal_token: Option<String>,

    // 业务字段三态 Patch
    #[serde(default, deserialize_with = "deserialize_patch_string")]
    pub task_name: PatchField<String>,
    #[serde(default, deserialize_with = "deserialize_patch_string")]
    pub description: PatchField<String>,
    #[serde(default, deserialize_with = "deserialize_patch_string")]
    pub deadline: PatchField<String>,
    #[serde(default, deserialize_with = "deserialize_patch_string")]
    pub priority: PatchField<String>,
    #[serde(default, deserialize_with = "deserialize_patch_i32")]
    pub completed: PatchField<i32>,
    #[serde(default, deserialize_with = "deserialize_patch_string")]
    pub assignee: PatchField<String>,
    #[serde(default, deserialize_with = "deserialize_patch_string")]
    pub finish_note: PatchField<String>,
    #[serde(default, deserialize_with = "deserialize_patch_string")]
    pub task_type: PatchField<String>,
    #[serde(default, deserialize_with = "deserialize_patch_string")]
    pub start_date: PatchField<String>,
    #[serde(default, deserialize_with = "deserialize_patch_string")]
    pub due_date: PatchField<String>,
    #[serde(default, deserialize_with = "deserialize_patch_string")]
    pub due_time: PatchField<String>,
    #[serde(default, deserialize_with = "deserialize_patch_string")]
    pub waiting_for: PatchField<String>,
    #[serde(default, deserialize_with = "deserialize_patch_string")]
    pub follow_up_date: PatchField<String>,
    #[serde(default, deserialize_with = "deserialize_patch_string")]
    pub context: PatchField<String>,
    #[serde(default, deserialize_with = "deserialize_patch_i32")]
    pub flagged: PatchField<i32>,
    #[serde(default, deserialize_with = "deserialize_patch_i32")]
    pub sequential: PatchField<i32>,
    #[serde(default, deserialize_with = "deserialize_patch_i32")]
    pub blocked: PatchField<i32>,
    #[serde(default, deserialize_with = "deserialize_patch_string")]
    pub blocked_reason: PatchField<String>,
    #[serde(default, deserialize_with = "deserialize_patch_i32")]
    pub sequence_order: PatchField<i32>,
    #[serde(default, deserialize_with = "deserialize_patch_string")]
    pub start_bucket: PatchField<String>,
    #[serde(default, deserialize_with = "deserialize_patch_i32")]
    pub today_index: PatchField<i32>,
    #[serde(default, deserialize_with = "deserialize_patch_i32")]
    pub estimated_minutes: PatchField<i32>,
    #[serde(default, deserialize_with = "deserialize_patch_i32")]
    pub actual_minutes: PatchField<i32>,
    #[serde(default, deserialize_with = "deserialize_patch_string")]
    pub area_id: PatchField<String>,
    #[serde(default, deserialize_with = "deserialize_patch_string")]
    pub case_id: PatchField<String>,
    #[serde(default, deserialize_with = "deserialize_patch_string")]
    pub time_block: PatchField<String>,
    #[serde(default, alias = "parentId", deserialize_with = "deserialize_patch_string")]
    pub parent_task_id: PatchField<String>,
    #[serde(default, deserialize_with = "deserialize_patch_string")]
    pub recurrence_rule: PatchField<String>,
    #[serde(default, deserialize_with = "deserialize_patch_i32")]
    pub is_focus: PatchField<i32>,
    #[serde(default, deserialize_with = "deserialize_patch_string")]
    pub defer_until: PatchField<String>,
    #[serde(default, deserialize_with = "deserialize_patch_string")]
    pub next_review_date: PatchField<String>,
    #[serde(default, deserialize_with = "deserialize_patch_string")]
    pub last_review_date: PatchField<String>,
}

impl UpdateTaskPatch {
    pub fn validate(&self) -> Result<(), anyhow::Error> {
        if self.id.trim().is_empty() {
            return Err(anyhow::anyhow!("Task ID cannot be empty"));
        }
        if let PatchField::Value(value) = self.completed {
            if ![0,1].contains(&value) { anyhow::bail!("completed 必须为 0 或 1"); }
        }
        if let PatchField::Value(rule) = &self.recurrence_rule {
            super::task_lifecycle::validate_recurrence(rule)?;
        }
        if let PatchField::Value(time) = &self.due_time {
            if !time.is_empty() { chrono::NaiveTime::parse_from_str(time,"%H:%M").or_else(|_|chrono::NaiveTime::parse_from_str(time,"%H:%M:%S"))?; }
        }

        if let PatchField::Value(name) = &self.task_name {
            if name.trim().is_empty() {
                return Err(anyhow::anyhow!("Task name cannot be empty"));
            }
        }

        if let PatchField::Value(bucket) = &self.start_bucket {
            let valid = ["inbox", "anytime", "someday", "today"];
            if !valid.contains(&bucket.as_str()) {
                return Err(anyhow::anyhow!(
                    "Invalid start_bucket '{}', must be one of {:?}",
                    bucket,
                    valid
                ));
            }
        }

        if let PatchField::Value(p) = &self.priority {
            let valid = [
                "urgent_important",
                "urgent_not_important",
                "not_urgent_important",
                "not_urgent_not_important",
                "urgent",
                "important",
                "normal",
                "low",
                "high",
            ];
            if !valid.contains(&p.as_str()) {
                return Err(anyhow::anyhow!("Invalid priority '{}'", p));
            }
        }

        if let PatchField::Value(tb) = &self.time_block {
            let valid = ["morning", "afternoon", "evening", "night", "flex"];
            if !valid.contains(&tb.as_str()) {
                return Err(anyhow::anyhow!("Invalid time_block '{}'", tb));
            }
        }

        // 校验日期格式 (YYYY-MM-DD)
        let validate_date = |field_name: &str, val: &str| -> Result<(), anyhow::Error> {
            if !val.trim().is_empty() {
                chrono::NaiveDate::parse_from_str(val, "%Y-%m-%d").map_err(|_| {
                    anyhow::anyhow!(
                        "Invalid date format for {}: expected YYYY-MM-DD, got '{}'",
                        field_name,
                        val
                    )
                })?;
            }
            Ok(())
        };

        if let PatchField::Value(d) = &self.due_date {
            validate_date("dueDate", d)?;
        }
        if let PatchField::Value(d) = &self.deadline {
            validate_date("deadline", d)?;
        }
        if let PatchField::Value(d) = &self.start_date {
            validate_date("startDate", d)?;
        }
        if let PatchField::Value(d) = &self.follow_up_date {
            validate_date("followUpDate", d)?;
        }
        if let PatchField::Value(d) = &self.defer_until {
            validate_date("deferUntil", d)?;
        }
        if let PatchField::Value(d) = &self.next_review_date { validate_date("nextReviewDate",d)?; }
        if let PatchField::Value(d) = &self.last_review_date { validate_date("lastReviewDate",d)?; }

        if let PatchField::Value(m) = &self.estimated_minutes {
            if *m < 0 {
                return Err(anyhow::anyhow!("estimatedMinutes must be non-negative"));
            }
        }
        if let PatchField::Value(m) = &self.actual_minutes {
            if *m < 0 {
                return Err(anyhow::anyhow!("actualMinutes must be non-negative"));
            }
        }

        Ok(())
    }
}

#[tauri::command]
pub async fn update_task(data: serde_json::Value) -> Result<(), String> {
    let cancelled = run_blocking(move || {
        let mut normalized = data.clone();
        normalize_optional_task_fields(&mut normalized);
        let patch: UpdateTaskPatch = serde_json::from_value(normalized)
            .map_err(|e| anyhow::anyhow!("Failed to parse UpdateTaskPatch: {}", e))?;

        patch.validate()?;

        let mut conn = db::open_db()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let now = db::now_local();

        // 1. 校验任务是否存在并获取旧数据
        let old_info: (Option<String>, i32, String) = tx
            .query_row(
                "SELECT due_date, completed, start_bucket FROM tasks WHERE id = ?1 AND deleted_at IS NULL",
                rusqlite::params![patch.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|e| {
                anyhow::anyhow!(crate::error_code::err(
                    crate::error_code::codes::TASK_NOT_FOUND,
                    format!("任务不存在: {} ({})", patch.id, e),
                ))
            })?;

        let (old_due_date, old_completed, old_start_bucket) = old_info;
        // A legacy start-date edit must not silently diverge from an independent plan.
        if !matches!(patch.start_date, PatchField::Unset) {
            let (has_plan, start): (bool, Option<String>) = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM task_plans WHERE task_id=tasks.id),start_date FROM tasks WHERE id=?1", [&patch.id], |r|Ok((r.get(0)?,r.get(1)?)))?;
            let requested=match &patch.start_date {PatchField::Value(v)=>Some(v.clone()),_=>None};
            anyhow::ensure!(!has_plan || requested==start, "此任务使用独立计划，请在甘特图中调整并确认日期");
        }

        // 2. P0-2: AI 授权网关校验
        crate::ai::gateway::verify_ai_mutation_authorized(
            &tx,
            patch.origin.as_deref(),
            patch.proposal_token.as_deref(),
            "update_task",
            "task",
            Some(&patch.id),
            Some(&crate::ai::gateway::compute_current_entity_hash(&tx, "task", &patch.id)?),
            &data,
        )?;

        // 3. 构建动态 UPDATE SET 语句
        let mut sets: Vec<String> = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        macro_rules! apply_patch_text {
            ($field:expr, $col:expr) => {
                match &$field {
                    PatchField::Unset => {}
                    PatchField::Null => {
                        sets.push(format!("{} = NULL", $col));
                    }
                    PatchField::Value(v) => {
                        sets.push(format!("{} = ?", $col));
                        params.push(Box::new(v.clone()));
                    }
                }
            };
        }

        macro_rules! apply_patch_i32 {
            ($field:expr, $col:expr) => {
                match &$field {
                    PatchField::Unset => {}
                    PatchField::Null => {
                        sets.push(format!("{} = NULL", $col));
                    }
                    PatchField::Value(v) => {
                        sets.push(format!("{} = ?", $col));
                        params.push(Box::new(*v));
                    }
                }
            };
        }

        apply_patch_text!(patch.task_name, "task_name");
        apply_patch_text!(patch.description, "description");
        apply_patch_text!(patch.deadline, "deadline");
        apply_patch_text!(patch.priority, "priority");
        apply_patch_i32!(patch.completed, "completed");
        apply_patch_text!(patch.assignee, "assignee");
        apply_patch_text!(patch.finish_note, "finish_note");
        apply_patch_text!(patch.task_type, "task_type");
        apply_patch_text!(patch.start_date, "start_date");
        apply_patch_text!(patch.due_date, "due_date");
        apply_patch_text!(patch.due_time, "due_time");
        apply_patch_text!(patch.waiting_for, "waiting_for");
        apply_patch_text!(patch.follow_up_date, "follow_up_date");
        apply_patch_text!(patch.context, "context");
        apply_patch_i32!(patch.flagged, "flagged");
        apply_patch_i32!(patch.sequential, "sequential");
        apply_patch_i32!(patch.blocked, "blocked");
        apply_patch_text!(patch.blocked_reason, "blocked_reason");
        apply_patch_i32!(patch.sequence_order, "sequence_order");
        apply_patch_text!(patch.start_bucket, "start_bucket");
        apply_patch_i32!(patch.today_index, "today_index");
        apply_patch_i32!(patch.estimated_minutes, "estimated_minutes");
        apply_patch_i32!(patch.actual_minutes, "actual_minutes");
        apply_patch_text!(patch.area_id, "area_id");
        apply_patch_text!(patch.case_id, "case_id");
        apply_patch_text!(patch.time_block, "time_block");
        apply_patch_text!(patch.parent_task_id, "parent_task_id");
        apply_patch_text!(patch.recurrence_rule, "recurrence_rule");
        apply_patch_i32!(patch.is_focus, "is_focus");
        apply_patch_text!(patch.defer_until, "defer_until");
        apply_patch_text!(patch.next_review_date, "next_review_date");
        apply_patch_text!(patch.last_review_date, "last_review_date");

        sets.push("updated_at = ?".to_string());
        params.push(Box::new(now.clone()));

        if !sets.is_empty() {
            let sql = format!("UPDATE tasks SET {} WHERE id = ?", sets.join(", "));
            params.push(Box::new(patch.id.clone()));

            let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|b| b.as_ref()).collect();
            let rows_affected = tx.execute(&sql, rusqlite::params_from_iter(params_refs))?;
            if rows_affected != 1 {
                return Err(anyhow::anyhow!(
                    "Expected exactly 1 row updated, got {}",
                    rows_affected
                ));
            }
        }

        // 4. 事件记录（单事务内原子写入）
        // 推迟检测
        if let PatchField::Value(new_due) = &patch.due_date {
            if let Some(old_due) = &old_due_date {
                if new_due > old_due {
                    let payload = serde_json::json!({
                        "from": old_due,
                        "to": new_due,
                    })
                    .to_string();
                    tx.execute(
                        "INSERT INTO task_events (id, task_id, event_type, occurred_at, payload, actor) VALUES (?1, ?2, 'deferred', ?3, ?4, 'user')",
                        rusqlite::params![db::new_id(), patch.id, now, payload],
                    )?;
                }
            }
        }

        // 桶移动检测
        if let PatchField::Value(new_bucket) = &patch.start_bucket {
            if new_bucket != &old_start_bucket {
                let payload = serde_json::json!({
                    "fromBucket": old_start_bucket,
                    "toBucket": new_bucket,
                })
                .to_string();
                tx.execute(
                    "INSERT INTO task_events (id, task_id, event_type, occurred_at, payload, actor) VALUES (?1, ?2, 'moved', ?3, ?4, 'user')",
                    rusqlite::params![db::new_id(), patch.id, now, payload],
                )?;
            }
        }

        let mut cancelled = Vec::new();
        // Completion side effects are shared with toggle_task.
        if let PatchField::Value(new_done) = &patch.completed {
            if *new_done != old_completed {
                let event_type = if *new_done == 1 {
                    "completed"
                } else {
                    "restored"
                };
                tx.execute(
                    "INSERT INTO task_events (id, task_id, event_type, occurred_at, actor) VALUES (?1, ?2, ?3, ?4, 'user')",
                    rusqlite::params![db::new_id(), patch.id, event_type, now],
                )?;
                cancelled = super::task_lifecycle::completion_effects(&tx,&patch.id,*new_done)?;
            }
        }

        // 常规审计日志（脱敏：剔除 origin/proposalToken；AI 操作归因 actor='ai'）
        let mut sanitized = data.clone();
        if let Some(obj) = sanitized.as_object_mut() {
            obj.remove("origin");
            obj.remove("proposalToken");
            obj.remove("proposal_token");
        }
        let edit_actor = if patch.origin.as_deref() == Some("ai") { "ai" } else { "user" };
        tx.execute(
            "INSERT INTO task_events (id, task_id, event_type, occurred_at, payload, actor) VALUES (?1, ?2, 'edited', ?3, ?4, ?5)",
            rusqlite::params![db::new_id(), patch.id, now, serde_json::to_string(&sanitized).unwrap_or_default(), edit_actor],
        )?;

        // 提交事务
        tx.commit()?;

        // 5. 日历同步联动
        if !matches!(patch.due_date, PatchField::Unset) || !matches!(patch.due_time, PatchField::Unset)
            || !matches!(patch.task_name, PatchField::Unset) || !matches!(patch.case_id, PatchField::Unset) {
            let task = load_task_row(&conn, &patch.id)?;
            if task.completed == 0 {
                if let Some(due) = task.due_date.as_deref().or(task.deadline.as_deref()) {
                    let _ = crate::commands::reminder::sync_task_reminder_calendar(
                        &conn, &patch.id, due, task.due_time.as_deref(), &task.task_name, task.case_id.as_deref(),
                    );
                } else {
                    cancelled.push(patch.id.clone());
                }
            }
        }

        Ok(cancelled)
    })
    .await?;
    cancel_task_reminders(cancelled).await;
    Ok(())
}

/// 庭审准备任务模板
const HEARING_PREP_TASKS: &[(&str, &str, &str)] = &[
    (
        "准备证据材料",
        "整理并提交本案相关证据材料，包括证据清单、证据原件及复印件",
        "important",
    ),
    (
        "准备代理词/法律意见书",
        "撰写庭审代理词或法律意见书，梳理案件事实和法律依据",
        "important",
    ),
    (
        "确认出庭人员",
        "确认出庭律师、当事人及其他相关人员是否能够按时出庭",
        "urgent",
    ),
    (
        "检查案件材料完整性",
        "检查案件卷宗材料是否齐全，包括起诉状、答辩状、证据材料等",
        "normal",
    ),
    (
        "准备庭审提纲",
        "准备庭审发言提纲，包括举证质证要点、辩论要点等",
        "important",
    ),
    (
        "确认庭审时间和地点",
        "核实庭审具体时间、地点及法庭编号，确保准时到达",
        "urgent",
    ),
];

/// 从庭审自动生成准备任务
/// 当创建庭审时调用，自动关联创建准备任务
#[tauri::command]
pub async fn generate_hearing_prep_tasks(
    case_id: String,
    hearing_id: String,
    hearing_date: String,
) -> Result<Vec<serde_json::Value>, String> {
    run_blocking(move || {
        let connection = db::open_db()?;
        let conn = connection.unchecked_transaction()?;
        let now = db::now_local();

        // 解析庭审日期，计算截止日期（庭审前 3 天、1 天等）
        let hearing_dt = parse_hearing_date(&hearing_date)?;

        let mut created_tasks = Vec::new();

        for (i, (task_name, description, priority)) in HEARING_PREP_TASKS.iter().enumerate() {
            let id = db::new_id();

            // 根据任务类型设置不同的截止日期
            let deadline = match i {
                0 | 1 | 4 => {
                    // 证据、代理词、提纲：庭审前 3 天
                    let d = hearing_dt - chrono::Duration::days(3);
                    d.format("%Y-%m-%d").to_string()
                }
                2 | 5 => {
                    // 确认人员、确认时间地点：庭审前 1 天
                    let d = hearing_dt - chrono::Duration::days(1);
                    d.format("%Y-%m-%d").to_string()
                }
                _ => {
                    // 其他：庭审前 2 天
                    let d = hearing_dt - chrono::Duration::days(2);
                    d.format("%Y-%m-%d").to_string()
                }
            };

            conn.execute(
                "INSERT INTO tasks (id, case_id, task_name, description, created_date, deadline, priority, completed, assignee, finish_note, source_log_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, '', '', ?8, ?9)",
                rusqlite::params![
                    id,
                    case_id,
                    format!("【庭审准备】{}", task_name),
                    description,
                    now,
                    deadline,
                    priority,
                    hearing_id,
                    now,
                ],
            )?;

            created_tasks.push(serde_json::json!({
                "id": id,
                "taskName": format!("【庭审准备】{}", task_name),
                "description": description,
                "deadline": deadline,
                "priority": priority,
                "caseId": case_id,
            }));
        }

        conn.commit()?;
        Ok(created_tasks)
    })
    .await
}

// ============================================================
// 任务模板系统
// ============================================================

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TaskTemplate {
    pub id: String,
    pub name: String,
    pub trigger_type: Option<String>,
    pub tasks_json: String,
    pub case_types: Option<String>,
    pub enabled: bool,
    pub created_at: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TaskTemplateItem {
    pub title: String,
    pub description: String,
    pub days_before: i64,
}

#[tauri::command]
pub async fn list_task_templates() -> Result<Vec<TaskTemplate>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, trigger_type, tasks_json, case_types, enabled, created_at
             FROM task_templates ORDER BY created_at",
        )?;

        let templates = stmt
            .query_map([], |row| {
                Ok(TaskTemplate {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    trigger_type: row.get(2)?,
                    tasks_json: row.get(3)?,
                    case_types: row.get(4)?,
                    enabled: row.get::<_, i32>(5)? != 0,
                    created_at: row.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(templates)
    })
    .await
}

#[tauri::command]
pub async fn create_task_template(data: serde_json::Value) -> Result<TaskTemplate, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let id = db::new_id();

        let tasks_json = data["tasksJson"].to_string();

        conn.execute(
            "INSERT INTO task_templates (id, name, trigger_type, tasks_json, case_types, enabled)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                id,
                data["name"].as_str().unwrap_or(""),
                data["triggerType"].as_str(),
                tasks_json,
                data["caseTypes"].as_str(),
                data["enabled"].as_i64().unwrap_or(1) as i32,
            ],
        )?;

        Ok(TaskTemplate {
            id,
            name: data["name"].as_str().unwrap_or("").to_string(),
            trigger_type: data["triggerType"].as_str().map(|s| s.to_string()),
            tasks_json,
            case_types: data["caseTypes"].as_str().map(|s| s.to_string()),
            enabled: data["enabled"].as_i64().unwrap_or(1) != 0,
            created_at: Some(db::now_local()),
        })
    })
    .await
}

/// 从模板生成任务
/// template_id: 模板 ID
/// case_id: 关联案件 ID
/// trigger_date: 触发日期 (YYYY-MM-DD)，任务截止日期 = trigger_date - days_before
#[tauri::command]
pub async fn apply_task_template(
    template_id: String,
    case_id: String,
    trigger_date: String,
) -> Result<Vec<serde_json::Value>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let now = db::now_local();

        // 读取模板
        let tasks_json: String = conn.query_row(
            "SELECT tasks_json FROM task_templates WHERE id = ?1 AND enabled = 1",
            rusqlite::params![template_id],
            |row| row.get(0),
        )?;

        let items: Vec<TaskTemplateItem> =
            serde_json::from_str(&tasks_json)?;

        let trigger_dt = chrono::NaiveDate::parse_from_str(&trigger_date, "%Y-%m-%d")
            ?;

        let mut created = Vec::new();

        for item in &items {
            let id = db::new_id();
            let deadline_dt = trigger_dt - chrono::Duration::days(item.days_before);
            let deadline = deadline_dt.format("%Y-%m-%d").to_string();

            conn.execute(
                "INSERT INTO tasks (id, case_id, task_name, description, created_date, deadline, priority, completed, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'normal', 0, ?7)",
                rusqlite::params![
                    id,
                    case_id,
                    item.title,
                    item.description,
                    now,
                    deadline,
                    now,
                ],
            )?;

            created.push(serde_json::json!({
                "id": id,
                "taskName": item.title,
                "description": item.description,
                "deadline": deadline,
                "caseId": case_id,
            }));
        }

        Ok(created)
    })
    .await
}

/// ⌘K 全局搜索的任务域查询（A1-6）
/// 本地规模用 LIKE 足够；FTS 升级待 tasks_fts 落地（B1 可选）
#[tauri::command]
pub async fn search_tasks(query: String) -> Result<Vec<SearchTaskDto>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let q = query.trim();
        if q.is_empty() {
            return Ok(Vec::new());
        }
        let like = format!("%{}%", q.replace('%', ""));
        let mut stmt = conn.prepare(
            "SELECT id, task_name, COALESCE(NULLIF(due_date,''),deadline) AS due_date, completed, start_bucket
             FROM tasks
             WHERE task_name LIKE ?1 AND deleted_at IS NULL
             ORDER BY completed ASC,
                      CASE WHEN due_date IS NULL THEN 1 ELSE 0 END,
                      due_date ASC
             LIMIT 20",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![like], |r| {
                Ok(SearchTaskDto {
                    id: r.get::<_, String>("id")?,
                    task_name: r.get::<_, String>("task_name")?,
                    due_date: r.get::<_, Option<String>>("due_date")?,
                    completed: r.get::<_, i64>("completed")?,
                    start_bucket: r.get::<_, Option<String>>("start_bucket")?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    })
    .await
}

/// A1-5：RRULE 极简子集的下一到期日
/// 'daily' | 'weekdays' | 'weekly:<1-7>'(周一=1) | 'monthly:<DD>'
#[cfg(test)]
use super::task_lifecycle::next_occurrence;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_occurrence_rules() {
        assert_eq!(
            next_occurrence("daily", "2026-08-28"),
            Some("2026-08-29".to_string())
        );
        // 2026-08-28 is Friday -> next weekday is Monday 2026-08-31
        assert_eq!(
            next_occurrence("weekdays", "2026-08-28"),
            Some("2026-08-31".to_string())
        );
        // weekly on Tuesday (2) from Friday (5) -> next Tuesday is +4 days = 2026-09-01
        assert_eq!(
            next_occurrence("weekly:2", "2026-08-28"),
            Some("2026-09-01".to_string())
        );
        // monthly on 15th from 2026-08-28 -> 2026-09-15
        assert_eq!(
            next_occurrence("monthly:15", "2026-08-28"),
            Some("2026-09-15".to_string())
        );
    }

    fn test_conn() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::schema::SCHEMA_SQL).unwrap();
        conn.execute_batch("PRAGMA user_version = 1;").unwrap();
        crate::db::schema::run_migrations(&conn, 1).unwrap();
        conn
    }

    // P1-6：撤销删除必须恢复原 id 与 completed（原缺陷是 create_task 另生成 id + completed 硬编码 0）
    #[test]
    fn test_restore_task_preserves_original_id_and_completed() {
        let mut conn = test_conn();
        // 模拟一个「已完成任务」随后被硬删除
        conn.execute(
            "INSERT INTO tasks (id, task_name, created_date, completed) VALUES ('t-abc', '旧任务', '2026-09-01', 1)",
            [],
        )
        .unwrap();
        conn.execute("DELETE FROM tasks WHERE id = 't-abc'", [])
            .unwrap();

        let snapshot = serde_json::json!({
            "id": "t-abc",
            "taskName": "旧任务",
            "description": "待还原描述",
            "createdDate": "2026-09-01",
            "deadline": null,
            "priority": "important",
            "completed": 1,
            "assignee": null,
            "finishNote": null,
            "taskType": "action",
            "startDate": null,
            "dueDate": "2026-09-05",
            "dueTime": null,
            "waitingFor": null,
            "followUpDate": null,
            "context": "@办公室",
            "flagged": 1,
            "sequential": 0,
            "blocked": 0,
            "sequenceOrder": 0,
            "startBucket": "today",
            "todayIndex": 2,
            "estimatedMinutes": 30,
            "actualMinutes": null,
            "isOverdue": 0,
            "dueSoon": 0,
            "lastReviewDate": null,
            "nextReviewDate": null,
            "areaId": null,
            "knowledgeId": null,
            "caseId": null,
            "parentId": null,
            "recurrenceRule": null,
            "isFocus": 0,
            "deferUntil": null,
        });

        let restored = restore_task_inner(&mut conn, &snapshot).unwrap();
        assert_eq!(
            restored.id, "t-abc",
            "撤销删除应恢复原 id，而非 create_task 新生成的 id"
        );
        assert_eq!(
            restored.completed, 1,
            "撤销删除应保留 completed=1，而非被清零"
        );
        assert_eq!(restored.task_name, "旧任务");
        assert_eq!(restored.priority.as_deref(), Some("important"));
        assert_eq!(restored.due_date.as_deref(), Some("2026-09-05"));
        assert_eq!(restored.context.as_deref(), Some("@办公室"));
        assert_eq!(restored.flagged, 1);
        assert_eq!(restored.start_bucket, "today");
        assert_eq!(restored.today_index, 2);
        assert_eq!(restored.estimated_minutes, Some(30));
    }

    // P1-6：原 id 仍被占用时（误用还原或 id 冲突）应拒绝覆盖
    #[test]
    fn test_restore_task_rejects_id_collision() {
        let mut conn = test_conn();
        conn.execute(
            "INSERT INTO tasks (id, task_name, created_date, completed) VALUES ('t-x', '占用', '2026-09-01', 0)",
            [],
        )
        .unwrap();
        let snapshot = serde_json::json!({
            "id": "t-x",
            "taskName": "尝试还原",
            "completed": 1,
        });
        assert!(
            restore_task_inner(&mut conn, &snapshot).is_err(),
            "id 已存在时撤销删除应拒绝覆盖"
        );
    }

    // P1-6：撤销删除不得触发 create_task 的「案件级 sequential 自动继承 / sequence_order 重算」副作用
    #[test]
    fn test_restore_task_does_not_inherit_case_sequential() {
        let mut conn = test_conn();
        // cases.sequential 默认为 1（见 schema ALTER）——命中 create_task 的继承条件
        conn.execute(
            "INSERT INTO cases (id, case_name, client_name) VALUES ('case1', '测试案件', '委托人')",
            [],
        )
        .unwrap();

        let snapshot = serde_json::json!({
            "id": "t-seq",
            "taskName": "顺序任务",
            "createdDate": "2026-09-01",
            "completed": 0,
            "caseId": "case1",
            "sequential": 0,
            "blocked": 0,
            "sequenceOrder": 3,
            "taskType": "action",
            "startBucket": "anytime",
            "todayIndex": 0,
            "flagged": 0,
            "isOverdue": 0,
            "dueSoon": 0,
            "isFocus": 0,
        });

        let restored = restore_task_inner(&mut conn, &snapshot).unwrap();
        assert_eq!(
            restored.sequential, 0,
            "撤销删除不得自动继承案件级 sequential"
        );
        assert_eq!(restored.case_id.as_deref(), Some("case1"));
        assert_eq!(
            restored.sequence_order, 3,
            "sequence_order 应按快照还原，而非按案件已有计数重算"
        );
    }

    // 原子性：成功时 tasks 行与 task_events 'created' 事件同批提交
    #[test]
    fn test_restore_task_atomic_commits_task_and_event() {
        let mut conn = test_conn();
        let snapshot = serde_json::json!({
            "id": "t-atomic",
            "taskName": "原子还原",
            "createdDate": "2026-09-01",
            "completed": 1,
            "taskType": "action",
            "startBucket": "today",
        });
        let restored = restore_task_inner(&mut conn, &snapshot).unwrap();
        assert_eq!(restored.id, "t-atomic");

        let task_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tasks WHERE id = 't-atomic'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(task_count, 1, "还原成功应落库 1 行任务");
        let event_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM task_events WHERE task_id = 't-atomic' AND event_type = 'created'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(event_count, 1, "还原成功应同批提交 1 条 created 事件");
    }

    // 原子性：tasks 写入成功后 event 写入失败 → 整体回滚，不留半恢复行
    #[test]
    fn test_restore_task_rolls_back_leaves_no_partial_rows() {
        let mut conn = test_conn();
        // 强制 task_events 插入失败，模拟「task 已插入但 event 写入失败」的中间态
        conn.execute_batch(
            "CREATE TRIGGER force_event_failure AFTER INSERT ON task_events
             BEGIN SELECT RAISE(ABORT, 'forced event insert failure'); END;",
        )
        .unwrap();

        let snapshot = serde_json::json!({
            "id": "t-rollback",
            "taskName": "回滚",
            "createdDate": "2026-09-01",
            "completed": 0,
            "taskType": "action",
            "startBucket": "anytime",
        });
        assert!(
            restore_task_inner(&mut conn, &snapshot).is_err(),
            "event 写入失败应使还原失败"
        );

        let task_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tasks WHERE id = 't-rollback'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(task_count, 0, "失败后不应残留半恢复的任务行");
        let event_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM task_events WHERE task_id = 't-rollback'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(event_count, 0, "失败后不应残留 task_events 行");
    }

    // 父关系：兼容前端 parentId 与后端 DTO 的 parentTaskId，避免父关系丢失
    #[test]
    fn test_restore_task_preserves_parent_from_both_field_names() {
        let mut conn = test_conn();
        conn.execute(
            "INSERT INTO tasks (id, task_name, created_date, completed) VALUES ('t-parent', '父', '2026-09-01', 0)",
            [],
        )
        .unwrap();

        // 后端 DTO 字段名：parentTaskId
        let snap_dto = serde_json::json!({
            "id": "t-child-1", "taskName": "子1", "createdDate": "2026-09-01",
            "completed": 0, "taskType": "action", "startBucket": "anytime",
            "parentTaskId": "t-parent",
        });
        let r1 = restore_task_inner(&mut conn, &snap_dto).unwrap();
        assert_eq!(
            r1.parent_task_id.as_deref(),
            Some("t-parent"),
            "应兼容 parentTaskId"
        );

        // 前端 Task 字段名：parentId
        let snap_fe = serde_json::json!({
            "id": "t-child-2", "taskName": "子2", "createdDate": "2026-09-01",
            "completed": 0, "taskType": "action", "startBucket": "anytime",
            "parentId": "t-parent",
        });
        let r2 = restore_task_inner(&mut conn, &snap_fe).unwrap();
        assert_eq!(
            r2.parent_task_id.as_deref(),
            Some("t-parent"),
            "应兼容 parentId"
        );
    }

    /// 模拟 delete_task 的服务端软删除（UPDATE deleted_at + 写 deleted 审计事件）。
    fn soft_delete(conn: &rusqlite::Connection, id: &str, now: &str) {
        conn.execute(
            "UPDATE tasks SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
            rusqlite::params![now, id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO task_events (id, task_id, event_type, occurred_at, payload, actor)
             VALUES (?1, ?2, 'deleted', ?3, ?4, 'user')",
            rusqlite::params![
                db::new_id(),
                id,
                now,
                serde_json::json!({ "reason": "soft_delete", "deletedAt": now }).to_string(),
            ],
        )
        .unwrap();
    }

    /// v24 软删除：删除是 UPDATE（行保留），且关联提醒/事件/子项不被级联删除；
    /// list_tasks 的默认查询（deleted_at IS NULL）将软删任务过滤掉。
    #[test]
    fn test_soft_delete_preserves_row_and_associated_records() {
        let conn = test_conn();
        let now = "2026-09-01 12:00:00";

        // 一个主任务 + 关联的事件、提醒作业、子任务（软删后都应存活）
        conn.execute(
            "INSERT INTO tasks (id, task_name, created_date, completed) VALUES ('t-del', '主任务', '2026-09-01', 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO task_events (id, task_id, event_type, occurred_at, actor)
             VALUES ('te-1', 't-del', 'created', '2026-09-01', 'user')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO reminder_jobs (id, entity_type, entity_id, channel, scheduled_at, status)
             VALUES ('rj-1', 'task', 't-del', 'local', '2026-09-01 09:00:00', 'pending')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tasks (id, task_name, created_date, completed, parent_task_id)
             VALUES ('t-child', '子任务', '2026-09-01', 0, 't-del')",
            [],
        )
        .unwrap();

        soft_delete(&conn, "t-del", now);

        // 主任务行仍在库（软删标记），未被 DELETE
        let (still_exists, deleted_at): (i64, Option<String>) = conn
            .query_row(
                "SELECT COUNT(*), deleted_at FROM tasks WHERE id = 't-del'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(still_exists, 1, "软删后主任务行应保留");
        assert!(deleted_at.is_some(), "软删后 deleted_at 应被写入");

        // 关联引用行未被级联删除
        let ev_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM task_events WHERE task_id = 't-del'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            ev_count, 2,
            "软删不应级联删除 task_events（created + deleted）"
        );
        let rj_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM reminder_jobs WHERE entity_id = 't-del'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(rj_count, 1, "软删不应删除提醒作业行");
        let child_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tasks WHERE id = 't-child'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(child_count, 1, "软删不应删除子任务行");

        // 默认任务查询（list_tasks 语义：deleted_at IS NULL）应过滤掉软删任务
        let visible: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tasks WHERE deleted_at IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(visible, 1, "软删任务应被默认查询过滤（只剩子任务可见）");
    }

    /// v24 软删除还原：对已软删原行执行 undelete（清 deleted_at）并按快照恢复字段，
    /// 关联引用行仍保留；写 restored 审计事件。
    #[test]
    fn test_restore_undeletes_soft_deleted_task() {
        let mut conn = test_conn();
        let now = "2026-09-01 12:00:00";

        conn.execute(
            "INSERT INTO tasks (id, task_name, created_date, completed, priority, start_bucket)
             VALUES ('t-u', '软删任务', '2026-09-01', 1, 'important', 'today')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO task_events (id, task_id, event_type, occurred_at, actor)
             VALUES ('te-1', 't-u', 'created', '2026-09-01', 'user')",
            [],
        )
        .unwrap();
        soft_delete(&conn, "t-u", now);

        let snapshot = serde_json::json!({
            "id": "t-u",
            "taskName": "软删任务",
            "createdDate": "2026-09-01",
            "completed": 1,
            "priority": "urgent_important",
            "taskType": "action",
            "startBucket": "today",
            "flagged": 1,
            "isFocus": 0,
        });

        let restored = restore_task_inner(&mut conn, &snapshot).unwrap();
        assert_eq!(restored.id, "t-u");
        assert_eq!(restored.completed, 1);
        assert_eq!(
            restored.priority.as_deref(),
            Some("urgent_important"),
            "还原应按快照恢复字段"
        );
        assert_eq!(restored.flagged, 1);

        let (deleted_at_after, deleted_event): (Option<String>, i64) = conn
            .query_row(
                "SELECT deleted_at,
                        (SELECT COUNT(*) FROM task_events WHERE task_id='t-u' AND event_type='restored')
                 FROM tasks WHERE id='t-u'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!(
            deleted_at_after.is_none(),
            "undelete 后 deleted_at 应为 NULL"
        );
        assert_eq!(deleted_event, 1, "还原应写 restored 审计事件");
        // 关联事件未丢
        let ev_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM task_events WHERE task_id='t-u'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            ev_count, 3,
            "关联事件应保留（created + deleted + restored）"
        );
    }

    /// v24：重复删除已软删任务 → 幂等成功（不重复写审计）；行不被二次变更。
    #[test]
    fn test_idempotent_soft_delete_no_duplicate_event() {
        let conn = test_conn();
        let now = "2026-09-01 12:00:00";
        conn.execute(
            "INSERT INTO tasks (id, task_name, created_date, completed) VALUES ('t-d2', '任务', '2026-09-01', 0)",
            [],
        )
        .unwrap();
        soft_delete(&conn, "t-d2", now);

        // delete_task 的重复删除路径：UPDATE 命中 0 行（已删），视为幂等成功，不写新 deleted 事件
        let rows = conn
            .execute(
                "UPDATE tasks SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
                rusqlite::params![now, "t-d2"],
            )
            .unwrap();
        assert_eq!(rows, 0, "已删任务二次 UPDATE 应命中 0 行");
        let deleted_events: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM task_events WHERE task_id='t-d2' AND event_type='deleted'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(deleted_events, 1, "重复删除不应重复写 deleted 审计事件");
    }

    /// 软删任务对**写操作按 id**应被拒绝（存在性检查返回空 → TASK_NOT_FOUND），
    /// 对**显示/统计**应被过滤（by-id 显示查询返回空、聚合计数为 0）。
    #[test]
    fn test_soft_deleted_task_hidden_from_mutations_and_display() {
        let conn = test_conn();
        let now = "2026-09-01 12:00:00";

        // 先建领域以满足 tasks.area_id 外键约束
        conn.execute(
            "INSERT INTO areas (id, name) VALUES ('area1', '领域一')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tasks (id, task_name, created_date, completed, area_id)
             VALUES ('t-sd', '软删任务', '2026-09-01', 0, 'area1')",
            [],
        )
        .unwrap();
        soft_delete(&conn, "t-sd", now);

        // 写操作存在性检查（toggle 语义：SELECT completed ... AND deleted_at IS NULL）→ 空
        let toggle_ok: bool = conn
            .query_row(
                "SELECT completed FROM tasks WHERE id = 't-sd' AND deleted_at IS NULL",
                [],
                |_r| Ok(true),
            )
            .optional()
            .unwrap()
            .is_none();
        assert!(
            toggle_ok,
            "toggle 对软删任务的存在性检查应返回空（→TASK_NOT_FOUND）"
        );

        // 更新存在性检查（update_task old_info 语义）→ 空
        let update_ok: bool = conn
            .query_row(
                "SELECT due_date, completed, start_bucket FROM tasks WHERE id = 't-sd' AND deleted_at IS NULL",
                [],
                |_r| Ok(true),
            )
            .optional()
            .unwrap()
            .is_none();
        assert!(update_ok, "update_task 对软删任务的存在性检查应返回空");

        // 显示/链接标签查询（linking resolve_title 语义）→ 空
        let label_ok: bool = conn
            .query_row(
                "SELECT task_name FROM tasks WHERE id = 't-sd' AND deleted_at IS NULL",
                [],
                |_r| Ok(true),
            )
            .optional()
            .unwrap()
            .is_none();
        assert!(label_ok, "软删任务不应返回显示标签");

        // 统计计数（areas delete_area 语义：活跃任务计数）→ 0
        let active_in_area: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tasks WHERE area_id = 'area1' AND deleted_at IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(active_in_area, 0, "软删任务不应计入活跃统计");

        // 实体存在性（recursive_check entity_exists 语义）→ 0
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tasks WHERE id = 't-sd' AND deleted_at IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(exists, 0, "软删任务应被视为不存在（entity_exists=false）");
    }
}

fn parse_hearing_date(value: &str) -> anyhow::Result<chrono::NaiveDate> {
    if let Ok(date) = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d") { return Ok(date); }
    if let Ok(date) = chrono::DateTime::parse_from_rfc3339(value) { return Ok(date.date_naive()); }
    for format in ["%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M"] {
        if let Ok(date) = chrono::NaiveDateTime::parse_from_str(value, format) { return Ok(date.date()); }
    }
    anyhow::bail!("庭审日期无效，请使用 YYYY-MM-DD 或完整日期时间")
}

#[cfg(test)]
mod hearing_date_tests {
    #[test]
    fn accepts_dates_and_valid_timestamps_only() {
        for input in ["2026-09-17", "2026-09-17 09:30", "2026-09-17T09:30:00+08:00"] {
            assert_eq!(super::parse_hearing_date(input).unwrap().to_string(), "2026-09-17");
        }
        for input in ["2026-09-17garbage", "2026-09-17 99:99", "错误日期"] {
            assert!(super::parse_hearing_date(input).is_err());
        }
    }
}
