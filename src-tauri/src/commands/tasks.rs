use super::run_blocking;
use crate::db;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskFilter {
    pub completed: Option<bool>,
    pub case_id: Option<String>,
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

        conn.execute(
            "INSERT INTO tasks (id, case_id, task_name, description, created_date, deadline, priority, completed, assignee, finish_note, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?9, ?10)",
            rusqlite::params![
                id,
                data["caseId"].as_str(),
                data["taskName"].as_str().unwrap_or(""),
                data["description"].as_str().unwrap_or(""),
                data["createdDate"].as_str().unwrap_or(&now),
                data["deadline"].as_str(),
                data["priority"].as_str().unwrap_or("normal"),
                data["assignee"].as_str().unwrap_or(""),
                data["finishNote"].as_str().unwrap_or(""),
                now,
            ],
        )?;

        Ok(serde_json::json!({ "id": id }))
    })
    .await
}

#[tauri::command]
pub async fn toggle_task(id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        conn.execute(
            "UPDATE tasks SET completed = CASE WHEN completed = 0 THEN 1 ELSE 0 END WHERE id = ?1",
            rusqlite::params![id],
        )?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn delete_task(id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        conn.execute("DELETE FROM tasks WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn update_task(id: String, data: serde_json::Value) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        conn.execute(
            "UPDATE tasks SET task_name = ?1, description = ?2, deadline = ?3, priority = ?4, case_id = ?5 WHERE id = ?6",
            rusqlite::params![
                data["taskName"].as_str().unwrap_or(""),
                data["description"].as_str().unwrap_or(""),
                data["deadline"].as_str(),
                data["priority"].as_str().unwrap_or("normal"),
                data["caseId"].as_str(),
                id,
            ],
        )?;
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
            .map_err(|e| anyhow::anyhow!("Invalid hearing date: {}", e))?;

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
