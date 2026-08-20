use super::run_blocking;
use crate::db;
use chrono::Datelike;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskFilter {
    pub completed: Option<bool>,
    pub case_id: Option<String>,
    pub area_id: Option<String>,
    pub task_type: Option<String>,
    pub start_bucket: Option<String>,
}

#[tauri::command]
pub async fn list_tasks(filter: Option<TaskFilter>) -> Result<Vec<serde_json::Value>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
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
                    sql.push_str(&format!(" AND start_bucket = ?{}", idx));
                    params.push(Box::new(start_bucket.clone()));
                    idx += 1;
                }
            }
        }

        sql.push_str(" ORDER BY CASE priority WHEN 'urgent_important' THEN 1 WHEN 'urgent' THEN 2 WHEN 'important' THEN 3 ELSE 4 END, deadline ASC");

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let tasks: Vec<serde_json::Value> = stmt
            .query_map(param_refs.as_slice(), |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, String>("id")?,
                    "caseId": row.get::<_, Option<String>>("case_id")?,
                    "taskName": row.get::<_, String>("task_name")?,
                    "description": row.get::<_, Option<String>>("description")?,
                    "createdDate": row.get::<_, String>("created_date")?,
                    "deadline": row.get::<_, Option<String>>("deadline")?,
                    "priority": row.get::<_, Option<String>>("priority")?,
                    "completed": row.get::<_, i32>("completed")?,
                    "assignee": row.get::<_, Option<String>>("assignee")?,
                    "finishNote": row.get::<_, Option<String>>("finish_note")?,
                    // GTD 字段
                    "taskType": row.get::<_, Option<String>>("task_type")?.unwrap_or_else(|| "action".to_string()),
                    "startDate": row.get::<_, Option<String>>("start_date")?,
                    "dueDate": row.get::<_, Option<String>>("due_date")?,
                    "waitingFor": row.get::<_, Option<String>>("waiting_for")?,
                    "followUpDate": row.get::<_, Option<String>>("follow_up_date")?,
                    "context": row.get::<_, Option<String>>("context")?,
                    "flagged": row.get::<_, Option<i32>>("flagged")?.unwrap_or(0),
                    "sequential": row.get::<_, Option<i32>>("sequential")?.unwrap_or(0),
                    "blocked": row.get::<_, Option<i32>>("blocked")?.unwrap_or(0),
                    "sequenceOrder": row.get::<_, Option<i32>>("sequence_order")?.unwrap_or(0),
                    "startBucket": row.get::<_, Option<String>>("start_bucket")?.unwrap_or_else(|| "anytime".to_string()),
                    "todayIndex": row.get::<_, Option<i32>>("today_index")?.unwrap_or(0),
                    "estimatedMinutes": row.get::<_, Option<i32>>("estimated_minutes")?,
                    "actualMinutes": row.get::<_, Option<i32>>("actual_minutes")?,
                    "isOverdue": row.get::<_, Option<i32>>("is_overdue")?.unwrap_or(0),
                    "dueSoon": row.get::<_, Option<i32>>("due_soon")?.unwrap_or(0),
                    "lastReviewDate": row.get::<_, Option<String>>("last_review_date")?,
                    "nextReviewDate": row.get::<_, Option<String>>("next_review_date")?,
                    "areaId": row.get::<_, Option<String>>("area_id")?,
                    "knowledgeId": row.get::<_, Option<String>>("knowledge_id")?,
                }))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(tasks)
    })
    .await
}

#[tauri::command]
pub async fn create_task(data: serde_json::Value) -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let id = db::new_id();
        let now = db::now_local();

        // 自动设置 Review 周期：默认下周日（设计哲学 §5.4）
        let next_review = {
            let d = chrono::Local::now().naive_local().date();
            let day = d.weekday().num_days_from_sunday();
            let diff = (7 - day) % 7;
            let diff = if diff == 0 { 7 } else { diff };
            (d + chrono::Duration::days(diff as i64)).format("%Y-%m-%d").to_string()
        };

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
             start_bucket, today_index, estimated_minutes, area_id, next_review_date, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)",
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
                data["nextReviewDate"].as_str().unwrap_or(&next_review),
                now,
            ],
        )?;

        // 记录 task_event
        conn.execute(
            "INSERT INTO task_events (id, task_id, event_type, occurred_at, actor) VALUES (?1, ?2, 'created', ?3, 'user')",
            rusqlite::params![db::new_id(), id, now],
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
    })
    .await
}

#[tauri::command]
pub async fn toggle_task(id: String, actual_minutes: Option<i64>) -> Result<(), String> {
    let task_id = id.clone();
    let unlock_result = run_blocking(move || {
        let conn = db::open_db()?;
        let now = db::now_local();

        // 获取当前状态
        let current: i32 = conn.query_row(
            "SELECT completed FROM tasks WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        )?;

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

        // 记录 task_event（完成事件 payload 带实际耗时）
        let event_type = if new_status == 1 { "completed" } else { "created" };
        let payload = actual_minutes
            .map(|m| serde_json::json!({ "actualMinutes": m }).to_string());
        conn.execute(
            "INSERT INTO task_events (id, task_id, event_type, occurred_at, payload, actor) VALUES (?1, ?2, ?3, ?4, ?5, 'user')",
            rusqlite::params![db::new_id(), id, event_type, now, payload],
        )?;

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
pub async fn delete_task(id: String) -> Result<(), String> {
    let task_id = id.clone();
    run_blocking(move || {
        let conn = db::open_db()?;
        conn.execute("DELETE FROM tasks WHERE id = ?1", rusqlite::params![id])?;
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

#[tauri::command]
pub async fn update_task(data: serde_json::Value) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let id = data["id"].as_str().ok_or_else(|| anyhow::anyhow!("Missing task id"))?;
        let now = db::now_local();

        // 读取旧 due_date，用于检测延期（deferred 事件）
        let old_due_date: Option<String> = conn
            .query_row(
                "SELECT due_date FROM tasks WHERE id = ?1",
                rusqlite::params![id],
                |row| row.get(0),
            )
            .ok()
            .flatten();

        let new_due_date = data["dueDate"].as_str().or(data["deadline"].as_str());

        conn.execute(
            "UPDATE tasks SET
                task_name = COALESCE(?1, task_name),
                description = COALESCE(?2, description),
                deadline = ?3,
                due_date = ?4,
                due_time = ?5,
                priority = COALESCE(?6, priority),
                case_id = ?7,
                task_type = COALESCE(?8, task_type),
                start_date = ?9,
                waiting_for = ?10,
                follow_up_date = ?11,
                context = ?12,
                flagged = COALESCE(?13, flagged),
                start_bucket = COALESCE(?14, start_bucket),
                today_index = COALESCE(?15, today_index),
                estimated_minutes = ?16,
                area_id = ?17,
                time_block = ?18,
                updated_at = ?19,
                actual_minutes = COALESCE(?20, actual_minutes)
             WHERE id = ?19",
            rusqlite::params![
                data["taskName"].as_str(),
                data["description"].as_str(),
                data["deadline"].as_str(),
                new_due_date,
                data["dueTime"].as_str(),
                data["priority"].as_str(),
                data["caseId"].as_str(),
                data["taskType"].as_str(),
                data["startDate"].as_str(),
                data["waitingFor"].as_str(),
                data["followUpDate"].as_str(),
                data["context"].as_str(),
                data["flagged"].as_i64(),
                data["startBucket"].as_str(),
                data["todayIndex"].as_i64(),
                data["estimatedMinutes"].as_i64(),
                data["areaId"].as_str(),
                data["timeBlock"].as_str(),
                now,
                id,
                data["actualMinutes"].as_i64(),
            ],
        )?;

        // due_date 被推迟（新值 > 旧值）→ 记录 deferred 事件（YYYY-MM-DD 字符串可直接比较）
        if let (Some(old_due), Some(new_due)) = (old_due_date.as_deref(), new_due_date) {
            if new_due > old_due {
                let payload = serde_json::json!({
                    "from": old_due,
                    "to": new_due,
                })
                .to_string();
                conn.execute(
                    "INSERT INTO task_events (id, task_id, event_type, occurred_at, payload, actor) VALUES (?1, ?2, 'deferred', ?3, ?4, 'user')",
                    rusqlite::params![db::new_id(), id, now, payload],
                )?;
            }
        }

        // 记录 task_event
        conn.execute(
            "INSERT INTO task_events (id, task_id, event_type, occurred_at, payload, actor) VALUES (?1, ?2, 'moved', ?3, ?4, 'user')",
            rusqlite::params![db::new_id(), id, now, serde_json::to_string(&data).unwrap_or_default()],
        )?;

        // 改期联动（设计哲学 §11.2）：due_date/due_time 变化 → 重新同步 CalDAV（同 UID 幂等更新）
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

        Ok(())
    })
    .await
}

/// 庭审准备任务模板
const HEARING_PREP_TASKS: &[(&str, &str, &str)] = &[
    ("准备证据材料", "整理并提交本案相关证据材料，包括证据清单、证据原件及复印件", "important"),
    ("准备代理词/法律意见书", "撰写庭审代理词或法律意见书，梳理案件事实和法律依据", "important"),
    ("确认出庭人员", "确认出庭律师、当事人及其他相关人员是否能够按时出庭", "urgent"),
    ("检查案件材料完整性", "检查案件卷宗材料是否齐全，包括起诉状、答辩状、证据材料等", "normal"),
    ("准备庭审提纲", "准备庭审发言提纲，包括举证质证要点、辩论要点等", "important"),
    ("确认庭审时间和地点", "核实庭审具体时间、地点及法庭编号，确保准时到达", "urgent"),
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

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
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
