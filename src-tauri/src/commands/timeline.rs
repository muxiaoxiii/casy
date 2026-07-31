use super::run_blocking;
use crate::db;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEvent {
    pub id: String,
    pub source_table: String,
    pub source_id: String,
    pub event_date: String,
    pub event_type: String,
    pub title: String,
    pub detail: Option<String>,
    pub icon: String,
    pub color: String,
}

#[tauri::command]
pub async fn get_case_timeline(case_id: String) -> Result<Vec<TimelineEvent>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut events = Vec::new();

        // 办案日志
        let mut stmt = conn.prepare(
            "SELECT id, event_date, event_type, event_summary, content
             FROM case_logs WHERE case_id = ?1",
        )?;
        for row in stmt.query_map(rusqlite::params![case_id], |r| {
            let event_type: String = r.get(2)?;
            Ok(TimelineEvent {
                id: r.get(0)?,
                source_table: "case_logs".into(),
                source_id: r.get::<_, String>(0)?,
                event_date: r.get(1)?,
                icon: match_event_icon(&event_type),
                color: match_event_color(&event_type),
                event_type,
                title: r.get(3)?,
                detail: r.get(4)?,
            })
        })? {
            events.push(row?);
        }

        // 庭审
        let mut stmt = conn.prepare(
            "SELECT id, hearing_date, hearing_name, venue
             FROM hearings WHERE case_id = ?1",
        )?;
        for row in stmt.query_map(rusqlite::params![case_id], |r| {
            Ok(TimelineEvent {
                id: r.get(0)?,
                source_table: "hearings".into(),
                source_id: r.get::<_, String>(0)?,
                event_date: r.get(1)?,
                icon: "📅".into(),
                color: "#3b82f6".into(),
                event_type: "hearing".into(),
                title: r.get::<_, Option<String>>(2)?.unwrap_or_else(|| "开庭".into()),
                detail: r.get::<_, Option<String>>(3)?,
            })
        })? {
            events.push(row?);
        }

        // 任务
        let mut stmt = conn.prepare(
            "SELECT id, created_date, task_name, description, completed
             FROM tasks WHERE case_id = ?1",
        )?;
        for row in stmt.query_map(rusqlite::params![case_id], |r| {
            let completed: i32 = r.get(4)?;
            Ok(TimelineEvent {
                id: r.get(0)?,
                source_table: "tasks".into(),
                source_id: r.get::<_, String>(0)?,
                event_date: r.get(1)?,
                icon: if completed == 1 { "✅" } else { "📌" }.into(),
                color: "#8b5cf6".into(),
                event_type: "task".into(),
                title: r.get(2)?,
                detail: r.get(3)?,
            })
        })? {
            events.push(row?);
        }

        // 按日期倒序
        events.sort_by(|a, b| b.event_date.cmp(&a.event_date));
        Ok(events)
    })
    .await
}

#[tauri::command]
pub async fn add_case_log(
    case_id: String,
    event_summary: String,
    event_type: String,
    event_date: String,
    content: Option<String>,
) -> Result<String, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let id = db::new_id();
        conn.execute(
            "INSERT INTO case_logs (id, case_id, event_summary, event_type, event_date, content, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                id, case_id, event_summary, event_type, event_date,
                content.unwrap_or_default(), db::now_local(),
            ],
        )?;
        Ok(id)
    })
    .await
}

#[tauri::command]
pub async fn delete_case_log(id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        conn.execute("DELETE FROM case_logs WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    })
    .await
}

fn match_event_icon(event_type: &str) -> String {
    match event_type {
        "submitted" => "📤",
        "received" => "📥",
        "record" => "📝",
        "task" => "📌",
        _ => "📄",
    }
    .into()
}

fn match_event_color(event_type: &str) -> String {
    match event_type {
        "submitted" => "#22c55e",
        "received" => "#3b82f6",
        "record" => "#6b7280",
        "task" => "#8b5cf6",
        _ => "#9ca3af",
    }
    .into()
}
