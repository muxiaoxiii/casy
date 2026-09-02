use super::run_blocking;
use crate::db;

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
    pub start_date: Option<String>,
    pub due_date: Option<String>,
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

#[tauri::command]
pub async fn list_tasks(filter: Option<TaskFilter>) -> Result<Vec<TaskDto>, String> {
    run_blocking(move || {
        db::with_conn(|conn| {
        let mut sql = String::from("SELECT * FROM tasks WHERE 1=1");
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
                    sql.push_str(&format!(" AND case_id = ?{}", idx));
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
                            sql.push_str(&format!(
                                " AND start_bucket = ?{} \
                                 AND (defer_until IS NULL OR defer_until <= date('now','localtime'))",
                                idx
                            ));
                            params.push(Box::new(start_bucket.clone()));
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
            .query_map(param_refs.as_slice(), |row| {
                Ok(TaskDto {
                    id: row.get::<_, String>("id")?,
                    case_id: row.get::<_, Option<String>>("case_id")?,
                    task_name: row.get::<_, String>("task_name")?,
                    description: row.get::<_, Option<String>>("description")?,
                    created_date: row.get::<_, String>("created_date")?,
                    deadline: row.get::<_, Option<String>>("deadline")?,
                    priority: row.get::<_, Option<String>>("priority")?,
                    completed: row.get::<_, i32>("completed")?,
                    assignee: row.get::<_, Option<String>>("assignee")?,
                    finish_note: row.get::<_, Option<String>>("finish_note")?,
                    // GTD 字段
                    task_type: row.get::<_, Option<String>>("task_type")?.unwrap_or_else(|| "action".to_string()),
                    start_date: row.get::<_, Option<String>>("start_date")?,
                    due_date: row.get::<_, Option<String>>("due_date")?,
                    waiting_for: row.get::<_, Option<String>>("waiting_for")?,
                    follow_up_date: row.get::<_, Option<String>>("follow_up_date")?,
                    context: row.get::<_, Option<String>>("context")?,
                    flagged: row.get::<_, Option<i32>>("flagged")?.unwrap_or(0),
                    sequential: row.get::<_, Option<i32>>("sequential")?.unwrap_or(0),
                    blocked: row.get::<_, Option<i32>>("blocked")?.unwrap_or(0),
                    sequence_order: row.get::<_, Option<i32>>("sequence_order")?.unwrap_or(0),
                    start_bucket: row.get::<_, Option<String>>("start_bucket")?.unwrap_or_else(|| "anytime".to_string()),
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
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(tasks)
        })
    })
    .await
}

#[tauri::command]
pub async fn create_task(data: serde_json::Value) -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let mut raw_conn = db::open_db()?;
        // 事务化：token 消费与写入同生共死（写失败则回滚，token 不被白烧）
        let conn = raw_conn.transaction()?;
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

        // A1-7 修复：next_review_date 仅在用户显式设置时写入。
        // 原实现默认填下周日，导致 Review 透视被无回顾意图的任务淹没（噪音缺陷）。

        // ── 案件级顺序项目自动继承（设计哲学 §3.3）─────────────────────
        // 如果关联案件设置了 sequential=1，新任务自动继承 sequential
        let case_id = data["caseId"].as_str();
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
                let existing_count: i32 = conn.query_row(
                    "SELECT COUNT(*) FROM tasks WHERE case_id = ?1 AND sequential = 1 AND completed = 0",
                    rusqlite::params![cid],
                    |row| row.get(0),
                ).unwrap_or(0);
                if existing_count > 0 {
                    blocked = 1;
                }
                sequence_order = existing_count as i64;
            }
        }

        conn.execute(
            "INSERT INTO tasks (id, case_id, task_name, description, created_date, deadline, priority, completed, assignee, finish_note,
             task_type, start_date, due_date, due_time, waiting_for, follow_up_date, context, flagged, sequential, blocked, sequence_order,
             start_bucket, today_index, estimated_minutes, area_id, next_review_date, created_at, parent_task_id, recurrence_rule, is_focus)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29)",
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
                data["parentId"].as_str(),
                data["recurrenceRule"].as_str(),
                data["isFocus"].as_i64().unwrap_or(0),
            ],
        )?;

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

        conn.commit()?;
        Ok(serde_json::json!({ "id": id }))
    })
    .await
}

#[tauri::command]
pub async fn toggle_task(
    id: String,
    actual_minutes: Option<i64>,
    origin: Option<String>,
    proposal_token: Option<String>,
) -> Result<(), String> {
    let task_id = id.clone();
    let unlock_result = run_blocking(move || {
        let mut raw_conn = db::open_db()?;
        let conn = raw_conn.transaction()?;
        let now = db::now_local();

        // 获取当前状态（错误码试点：CAS-1001 任务不存在）
        let current: i32 = conn
            .query_row(
                "SELECT completed FROM tasks WHERE id = ?1",
                rusqlite::params![id],
                |row| row.get(0),
            )
            .map_err(|e| {
                anyhow::anyhow!(crate::error_code::err(
                    crate::error_code::codes::TASK_NOT_FOUND,
                    format!("任务不存在: {id} ({e})"),
                ))
            })?;

        // P0-2: AI 授权网关（origin='ai' 必须携带有效 proposal token）
        crate::ai::gateway::verify_ai_mutation_authorized(
            &conn,
            origin.as_deref(),
            proposal_token.as_deref(),
            "toggle_task",
            "task",
            Some(&id),
            Some(&crate::ai::gateway::compute_current_entity_hash(&conn, "task", &id)?),
            &serde_json::json!({ "id": id }),
        )?;
        let actor = if origin.as_deref() == Some("ai") { "ai" } else { "user" };

        let new_status = if current == 0 { 1 } else { 0 };

        conn.execute(
            "UPDATE tasks SET completed = ?1 WHERE id = ?2",
            rusqlite::params![new_status, id],
        )?;

        // 完成任务时可同时记录实际耗时（行为学习数据源）
        if new_status == 1 {
            if let Some(mins) = actual_minutes {
                conn.execute(
                    "UPDATE tasks SET actual_minutes = ?1 WHERE id = ?2",
                    rusqlite::params![mins, id],
                )?;
            }
        }

        // 记录 task_event（完成事件 payload 带实际耗时；AI 操作归因 actor='ai'）
        let event_type = if new_status == 1 { "completed" } else { "created" };
        let payload = actual_minutes
            .map(|m| serde_json::json!({ "actualMinutes": m }).to_string());
        conn.execute(
            "INSERT INTO task_events (id, task_id, event_type, occurred_at, payload, actor) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![db::new_id(), id, event_type, now, payload, actor],
        )?;

        // ── A1-5 重复任务：完成后生成下一实例（确定性执行在 Rust · 双路径铁律）──
        if new_status == 1 {
            let rec: Option<(String, Option<String>, Option<String>)> = conn.query_row(
                "SELECT recurrence_rule,
                        COALESCE(due_date, deadline, start_date),
                        COALESCE(start_date, due_date, deadline)
                 FROM tasks WHERE id = ?1 AND completed = 1",
                rusqlite::params![id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            ).ok();

            if let Some((rule, Some(anchor_due), anchor_start)) = rec {
                if let Some(next) = next_occurrence(&rule, &anchor_due) {
                    let parse = |s: &str| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok();
                    let shift = next.parse::<chrono::NaiveDate>().ok()
                        .zip(parse(&anchor_due))
                        .map(|(n, a)| (n - a).num_days())
                        .unwrap_or(0);
                    let next_start = anchor_start.as_deref().and_then(parse)
                        .map(|d| (d + chrono::Duration::days(shift)).format("%Y-%m-%d").to_string());

                    // 生成失败静默：完成动作不受影响
                    let _ = conn.execute(
                        "INSERT INTO tasks (id, case_id, task_name, description, created_date, deadline,
                            priority, assignee, task_type, start_date, due_date, due_time, waiting_for,
                            follow_up_date, context, flagged, sequential, blocked, sequence_order,
                            start_bucket, estimated_minutes, area_id, parent_task_id, recurrence_rule, created_at)
                         SELECT ?1, case_id, task_name, description, ?2, ?3, priority, assignee, task_type,
                                ?4, ?3, due_time, waiting_for, follow_up_date,
                                context, flagged, sequential, blocked, sequence_order, start_bucket,
                                estimated_minutes, area_id, NULL, recurrence_rule, ?2
                         FROM tasks WHERE id = ?5",
                        rusqlite::params![db::new_id(), now, next, next_start, id],
                    );
                }
            }
        }

        // ── 顺序项目自动解锁（设计哲学 §3.3 / §5.4）─────────────────────
        // 如果完成的是一个 sequential 任务，在同一事务内解锁下一个
        let mut unlocked_task_id: Option<String> = None;
        if new_status == 1 {
            let task_info: Option<(String, i32, Option<String>)> = conn.query_row(
                "SELECT id, sequence_order, case_id FROM tasks WHERE id = ?1 AND sequential = 1",
                rusqlite::params![id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?, row.get::<_, Option<String>>(2)?)),
            ).ok();

            if let Some((_, seq_order, case_id)) = task_info {
                // 找到同案件（或无案件）中 sequence_order 更大的下一个 blocked 任务
                let next_task_id: Option<String> = if let Some(cid) = case_id {
                    conn.query_row(
                        "SELECT id FROM tasks WHERE case_id = ?1 AND sequential = 1 AND blocked = 1 AND sequence_order > ?2 ORDER BY sequence_order ASC LIMIT 1",
                        rusqlite::params![cid, seq_order],
                        |row| row.get(0),
                    ).ok()
                } else {
                    conn.query_row(
                        "SELECT id FROM tasks WHERE case_id IS NULL AND sequential = 1 AND blocked = 1 AND sequence_order > ?1 ORDER BY sequence_order ASC LIMIT 1",
                        rusqlite::params![seq_order],
                        |row| row.get(0),
                    ).ok()
                };

                if let Some(next_id) = next_task_id {
                    conn.execute(
                        "UPDATE tasks SET blocked = 0 WHERE id = ?1",
                        rusqlite::params![&next_id],
                    )?;
                    // 记录解锁事件
                    conn.execute(
                        "INSERT INTO task_events (id, task_id, event_type, occurred_at, payload, actor) VALUES (?1, ?2, 'moved', ?3, ?4, 'system')",
                        rusqlite::params![db::new_id(), &next_id, &now, serde_json::json!({"fromBlocked":1,"toBlocked":0,"reason":"sequential_unlock"}).to_string()],
                    )?;
                    unlocked_task_id = Some(next_id);
                }
            }
        }

        conn.commit()?;
        Ok((new_status == 1, unlocked_task_id))
    })
    .await?;

    let completed_now = unlock_result.0;
    let unlocked_id = unlock_result.1;

    // 任务完成后撤销其提醒作业（含已同步到日历的事件，避免误提醒）
    if completed_now {
        if let Err(e) = super::caldav::cancel_jobs_for_entity("task", &task_id).await {
            log::warn!("任务完成后撤销提醒作业失败 (task {}): {}", task_id, e);
        }
        if let Some(ref uid) = unlocked_id {
            log::info!("顺序项目已自动解锁: {}", uid);
        }
    }

    Ok(())
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
        conn.execute("DELETE FROM tasks WHERE id = ?1", rusqlite::params![id])?;
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

/// 任务"稍后提醒"（设计哲学 §5.4 / §11.9：推迟任务并记录 snoozed 行为事件）
/// option: tonight / tomorrow / weekend / next_week / custom（+new_due_date）
#[tauri::command]
pub async fn snooze_task(
    id: String,
    option: Option<String>,
    new_due_date: Option<String>,
) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let now = db::now_local();
        use chrono::{Datelike, Duration, Local};
        let today = Local::now().date_naive();

        // 计算新日期
        let (new_date, label) = match option.as_deref() {
            Some("tonight") => (today.to_string(), "今晚".to_string()),
            Some("tomorrow") => ((today + Duration::days(1)).to_string(), "明天".to_string()),
            Some("weekend") => {
                let days_to_sat = (6 - today.weekday().num_days_from_monday() + 7) % 7;
                ((today + Duration::days(days_to_sat as i64)).to_string(), "周末".to_string())
            }
            Some("next_week") => ((today + Duration::days(7)).to_string(), "下周".to_string()),
            _ => {
                let d = new_due_date.unwrap_or_else(|| today.to_string());
                (d, "自定义".to_string())
            }
        };

        // 更新任务：到期日 = 新日期；今天 → today 桶，其他 → upcoming
        let is_today = new_date == today.to_string();
        let bucket = if is_today { "today" } else { "upcoming" };
        conn.execute(
            "UPDATE tasks SET due_date = ?1, start_date = ?1, start_bucket = ?2 WHERE id = ?3",
            rusqlite::params![new_date, bucket, id],
        )?;

        // 写 snoozed 行为事件（支撑"懂你的节奏/模式"学习）
        let payload = serde_json::json!({
            "option": option.unwrap_or_else(|| "custom".to_string()),
            "newDueDate": new_date,
            "label": label,
        });
        conn.execute(
            "INSERT INTO task_events (id, task_id, event_type, occurred_at, payload, actor) VALUES (?1, ?2, 'snoozed', ?3, ?4, 'user')",
            rusqlite::params![db::new_id(), id, now, serde_json::to_string(&payload).unwrap_or_default()],
        )?;

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
            "UPDATE tasks SET defer_until = ?1 WHERE id = ?2",
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
            "UPDATE tasks SET defer_until = NULL WHERE id = ?1",
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
                Ok(PatchField::Value(i as i32))
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
    #[serde(default, deserialize_with = "deserialize_patch_string")]
    pub parent_task_id: PatchField<String>,
    #[serde(default, deserialize_with = "deserialize_patch_string")]
    pub recurrence_rule: PatchField<String>,
    #[serde(default, deserialize_with = "deserialize_patch_i32")]
    pub is_focus: PatchField<i32>,
    #[serde(default, deserialize_with = "deserialize_patch_string")]
    pub defer_until: PatchField<String>,
}

impl UpdateTaskPatch {
    pub fn validate(&self) -> Result<(), anyhow::Error> {
        if self.id.trim().is_empty() {
            return Err(anyhow::anyhow!("Task ID cannot be empty"));
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
    run_blocking(move || {
        let patch: UpdateTaskPatch = serde_json::from_value(data.clone())
            .map_err(|e| anyhow::anyhow!("Failed to parse UpdateTaskPatch: {}", e))?;

        patch.validate()?;

        let mut conn = db::open_db()?;
        let tx = conn.transaction()?;
        let now = db::now_local();

        // 1. 校验任务是否存在并获取旧数据
        let old_info: (Option<String>, i32, String) = tx
            .query_row(
                "SELECT due_date, completed, start_bucket FROM tasks WHERE id = ?1",
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

        // 完成状态检测
        if let PatchField::Value(new_done) = &patch.completed {
            if *new_done != old_completed {
                let event_type = if *new_done == 1 {
                    "completed"
                } else {
                    "created"
                };
                tx.execute(
                    "INSERT INTO task_events (id, task_id, event_type, occurred_at, actor) VALUES (?1, ?2, ?3, ?4, 'user')",
                    rusqlite::params![db::new_id(), patch.id, event_type, now],
                )?;
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
        if let PatchField::Value(due) = &patch.due_date {
            let due_time_val = patch.due_time.value().map(|s| s.as_str());
            let task_name_val = patch.task_name.value().map(|s| s.as_str()).unwrap_or("");
            let case_id_val = patch.case_id.value().map(|s| s.as_str());
            let _ = crate::commands::reminder::sync_task_reminder_calendar(
                &conn,
                &patch.id,
                due,
                due_time_val,
                task_name_val,
                case_id_val,
            );
        }

        Ok(())
    })
    .await
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
        let conn = db::open_db()?;
        let now = db::now_local();

        // 解析庭审日期，计算截止日期（庭审前 3 天、1 天等）
        let hearing_dt = chrono::NaiveDate::parse_from_str(&hearing_date, "%Y-%m-%d")
            ?;

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
            "SELECT id, task_name, due_date, completed, start_bucket
             FROM tasks
             WHERE task_name LIKE ?1
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
fn next_occurrence(rule: &str, from: &str) -> Option<String> {
    use chrono::{Datelike, Duration, NaiveDate};
    let d = NaiveDate::parse_from_str(from, "%Y-%m-%d").ok()?;
    let next = match rule.trim() {
        "daily" => d + Duration::days(1),
        "weekdays" => {
            let mut n = d + Duration::days(1);
            while n.weekday().num_days_from_monday() >= 5 {
                n += Duration::days(1);
            }
            n
        }
        r if r.starts_with("weekly:") => {
            let target: u32 = r.split(':').nth(1)?.trim().parse().ok()?;
            if !(1..=7).contains(&target) {
                return None;
            }
            let cur = d.weekday().num_days_from_monday() + 1;
            let delta = (target + 7 - cur) % 7;
            d + Duration::days(if delta == 0 { 7 } else { delta } as i64)
        }
        r if r.starts_with("monthly:") => {
            let day: u32 = r.split(':').nth(1)?.trim().parse().ok()?;
            if !(1..=31).contains(&day) {
                return None;
            }
            let (mut y, mut m) = (d.year(), d.month());
            loop {
                m += 1;
                if m > 12 {
                    m = 1;
                    y += 1;
                }
                let dim = NaiveDate::from_ymd_opt(y, m + if m == 12 { 0 } else { 1 }, 1)
                    .map(|first| (first - Duration::days(1)).day())
                    .unwrap_or(28);
                if let Some(nd) = NaiveDate::from_ymd_opt(y, m, day.min(dim)) {
                    break nd;
                }
            }
        }
        _ => return None,
    };
    Some(next.format("%Y-%m-%d").to_string())
}

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
}
