//! calendar_events CRUD（D-7 独立日程实体 · M-CAL-1）
//!
//! 日程是独立事实源：NL 快速建日程直接落此表，不再降级为任务。
//! 与案件/任务为可空外键关联，删除行为遵循默认 NO ACTION（引用方负责清理）。

use super::run_blocking;
use crate::db;

/// 独立日程行（与投影 CalendarEvent 分离；投影时合并进月历数据）（B1 类型化）
#[derive(serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEventRow {
    pub id: String,
    pub title: String,
    pub event_date: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub all_day: bool,
    pub color: Option<String>,
    pub location: Option<String>,
    pub notes: Option<String>,
    pub case_id: Option<String>,
    pub task_id: Option<String>,
}

fn row_to_event(r: &rusqlite::Row) -> rusqlite::Result<CalendarEventRow> {
    Ok(CalendarEventRow {
        id: r.get("id")?,
        title: r.get("title")?,
        event_date: r.get("event_date")?,
        start_time: r.get("start_time")?,
        end_time: r.get("end_time")?,
        all_day: r.get::<_, i64>("all_day")? != 0,
        color: r.get("color")?,
        location: r.get("location")?,
        notes: r.get("notes")?,
        case_id: r.get("case_id")?,
        task_id: r.get("task_id")?,
    })
}

const EVENT_COLS: &str =
    "id, title, event_date, start_time, end_time, all_day, color, location, notes, case_id, task_id";

fn event_all_day(data: &serde_json::Value) -> i64 {
    data["allDay"].as_bool().map(i64::from).or_else(|| data["allDay"].as_i64())
        .unwrap_or(if data["startTime"].is_null() { 1 } else { 0 })
}

fn validate_event(data: &serde_json::Value) -> anyhow::Result<()> {
    anyhow::ensure!(data["title"].as_str().is_some_and(|value| !value.trim().is_empty()), "请输入日程标题");
    let date = data["eventDate"].as_str().unwrap_or("");
    anyhow::ensure!(date.len() == 10 && chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_ok(), "日程日期无效");
    for key in ["startTime", "endTime"] {
        if !data[key].is_null() {
            let time = data[key].as_str().unwrap_or("");
            anyhow::ensure!(time.len() == 5 && chrono::NaiveTime::parse_from_str(time, "%H:%M").is_ok(), "日程时间无效");
        }
    }
    if let (Some(start), Some(end)) = (data["startTime"].as_str(), data["endTime"].as_str()) {
        anyhow::ensure!(end > start, "结束时间必须晚于开始时间");
    }
    anyhow::ensure!(matches!(event_all_day(data), 0 | 1), "全天日程设置无效");
    Ok(())
}

#[tauri::command]
pub async fn list_calendar_events(
    start_date: String,
    end_date: String,
) -> Result<Vec<CalendarEventRow>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(&format!(
            "SELECT {EVENT_COLS} FROM calendar_events
                 WHERE event_date BETWEEN ?1 AND ?2
                 AND NOT EXISTS (SELECT 1 FROM tasks WHERE tasks.id=calendar_events.task_id AND tasks.deleted_at IS NOT NULL)
                 ORDER BY CASE WHEN all_day = 1 THEN 1 ELSE 0 END, start_time, event_date"
        ))?;
        let rows = stmt
            .query_map(rusqlite::params![start_date, end_date], row_to_event)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    })
    .await
}

#[tauri::command]
pub async fn create_calendar_event(data: serde_json::Value) -> Result<CalendarEventRow, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        create_event_in_transaction(&conn, data)
    })
    .await
}

pub(super) fn create_event_in_transaction(conn: &rusqlite::Connection, data: serde_json::Value) -> anyhow::Result<CalendarEventRow> {
        validate_event(&data)?;
        let id = db::new_id();
        let now = db::now_local();
        let title = data["title"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing event title"))?;
        let event_date = data["eventDate"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing eventDate"))?;

        conn.execute(
            "INSERT INTO calendar_events
             (id, title, event_date, start_time, end_time, all_day, color, location, notes, case_id, task_id, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?12)",
            rusqlite::params![
                id,
                title,
                event_date,
                data["startTime"].as_str(),
                data["endTime"].as_str(),
                event_all_day(&data),
                data["color"].as_str(),
                data["location"].as_str(),
                data["notes"].as_str(),
                data["caseId"].as_str(),
                data["taskId"].as_str(),
                now,
            ],
        )?;

        let ev = conn.query_row(
            &format!("SELECT {EVENT_COLS} FROM calendar_events WHERE id = ?1"),
            rusqlite::params![id],
            row_to_event,
        )?;
        Ok(ev)
}

/// 更新日程：缺省字段保留原值，显式 null 清空。
#[tauri::command]
pub async fn update_calendar_event(id: String, data: serde_json::Value) -> Result<(), String> {
    run_blocking(move || {
        let mut connection = db::open_db()?;
        let conn = connection.transaction()?;
        let existing = conn.query_row(&format!("SELECT {EVENT_COLS} FROM calendar_events WHERE id=?1"), [&id], row_to_event)?;
        let mut merged = serde_json::to_value(existing)?;
        for (key, value) in data.as_object().ok_or_else(|| anyhow::anyhow!("日程数据无效"))? {
            anyhow::ensure!(["title", "eventDate", "startTime", "endTime", "allDay", "color", "location", "notes", "caseId", "taskId"].contains(&key.as_str()), "不支持的日程字段：{key}");
            merged[key] = value.clone();
        }
        if data.get("startTime").is_some() && data.get("allDay").is_none() {
            merged["allDay"] = serde_json::Value::Bool(data["startTime"].is_null());
        }
        if data.get("startTime").is_some_and(|v| v.is_null()) && data.get("endTime").is_none() {
            merged["endTime"] = serde_json::Value::Null;
        }
        validate_event(&merged)?;
        let data = merged;
        let now = db::now_local();

        let title = data["title"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing event title"))?;
        let event_date = data["eventDate"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing eventDate"))?;

        let changed = conn.execute(
            "UPDATE calendar_events SET
                title = ?1, event_date = ?2, start_time = ?3, end_time = ?4,
                all_day = ?5, color = ?6, location = ?7, notes = ?8,
                case_id = ?9, updated_at = ?10, task_id = ?12
             WHERE id = ?11",
            rusqlite::params![
                title,
                event_date,
                data["startTime"].as_str(),
                data["endTime"].as_str(),
                event_all_day(&data),
                data["color"].as_str(),
                data["location"].as_str(),
                data["notes"].as_str(),
                data["caseId"].as_str(),
                now,
                id,
                data["taskId"].as_str(),
            ],
        )?;
        if changed == 0 {
            return Err(anyhow::anyhow!(crate::error_code::err(
                crate::error_code::codes::CALENDAR_NOT_FOUND,
                "日程不存在",
            )));
        }
        conn.commit()?;
        Ok(())
    })
    .await
}

/// 移动日程（拖拽改期/改时刻专用轻量命令）
#[tauri::command]
pub async fn move_calendar_event(
    id: String,
    new_date: String,
    new_start: Option<String>,
) -> Result<(), String> {
    run_blocking(move || {
        let mut conn = db::open_db()?;
        let tx = conn.transaction()?;
        let (old_start, old_end): (Option<String>, Option<String>) = tx.query_row(
            "SELECT start_time,end_time FROM calendar_events WHERE id=?1", [&id], |r| Ok((r.get(0)?,r.get(1)?)))?;
        let new_end = match (old_start.as_deref(),old_end.as_deref(),new_start.as_deref()) {
            (Some(start),Some(end),Some(next)) => {
                let start = chrono::NaiveTime::parse_from_str(start,"%H:%M")?;
                let end = chrono::NaiveTime::parse_from_str(end,"%H:%M")?;
                let next = chrono::NaiveTime::parse_from_str(next,"%H:%M")?;
                let (shifted, overflow) = next.overflowing_add_signed(end - start);
                anyhow::ensure!(overflow==0 && shifted>next,"移动后日程跨越午夜，请拆分为两天的日程");
                Some(shifted.format("%H:%M").to_string())
            },
            _ => None,
        };
        validate_event(&serde_json::json!({"title":"move","eventDate":new_date,"startTime":new_start,"endTime":new_end}))?;
        let changed = tx.execute(
            "UPDATE calendar_events SET event_date = ?1, start_time = ?2,
                all_day = ?3, updated_at = ?4,end_time=?6 WHERE id = ?5",
            rusqlite::params![
                new_date,
                new_start,
                if new_start.is_none() { 1 } else { 0 },
                db::now_local(),
                id,
                new_end
            ],
        )?;
        if changed == 0 {
            return Err(anyhow::anyhow!(crate::error_code::err(
                crate::error_code::codes::CALENDAR_NOT_FOUND,
                "日程不存在",
            )));
        }
        tx.commit()?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn delete_calendar_event(id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        conn.execute(
            "DELETE FROM calendar_events WHERE id = ?1",
            rusqlite::params![id],
        )?;
        Ok(())
    })
    .await
}
