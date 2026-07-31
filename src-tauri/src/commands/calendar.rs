use super::run_blocking;
use crate::db;
use chrono::Datelike;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEvent {
    pub id: String,
    pub date: String,
    pub title: String,
    pub event_type: String,
    pub case_id: String,
    pub case_name: String,
}

#[tauri::command]
pub async fn get_calendar_events(year: i32, month: u32) -> Result<Vec<CalendarEvent>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;

        let start = format!("{:04}-{:02}-01", year, month);
        let last_day = last_day_of_month(year, month);
        let end = format!("{:04}-{:02}-{:02}", year, month, last_day);
        let mut events = Vec::new();

        // 庭审
        let mut stmt = conn.prepare(
            "SELECT h.id, h.hearing_date, h.hearing_name, c.id, c.case_name
             FROM hearings h JOIN cases c ON c.id = h.case_id
             WHERE h.hearing_date BETWEEN ?1 AND ?2",
        )?;
        for row in stmt.query_map(rusqlite::params![start, end], |r| {
            Ok(CalendarEvent {
                id: r.get(0)?,
                date: r.get(1)?,
                title: r.get::<_, Option<String>>(2)?.unwrap_or_else(|| "开庭".into()),
                event_type: "hearing".into(),
                case_id: r.get(3)?,
                case_name: r.get(4)?,
            })
        })? {
            events.push(row?);
        }

        // 任务
        let mut stmt = conn.prepare(
            "SELECT id, deadline, task_name, case_id FROM tasks
             WHERE deadline BETWEEN ?1 AND ?2 AND completed = 0",
        )?;
        for row in stmt.query_map(rusqlite::params![start, end], |r| {
            Ok(CalendarEvent {
                id: r.get(0)?,
                date: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                title: r.get(2)?,
                event_type: "task".into(),
                case_id: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                case_name: String::new(),
            })
        })? {
            events.push(row?);
        }

        // 期限预警（简化：只查 case_deadlines 表）
        let mut stmt = conn.prepare(
            "SELECT cd.id, cd.due_date, cd.deadline_name, c.id, c.case_name
             FROM case_deadlines cd JOIN cases c ON c.id = cd.case_id
             WHERE cd.due_date BETWEEN ?1 AND ?2 AND cd.completed = 0",
        )?;
        for row in stmt.query_map(rusqlite::params![start, end], |r| {
            let due_date: String = r.get(1)?;
            let days_left = {
                let today = chrono::Local::now().naive_local().date();
                let due = chrono::NaiveDate::parse_from_str(&due_date, "%Y-%m-%d")
                    .unwrap_or(today);
                (due - today).num_days()
            };
            let urgency = if days_left <= 3 {
                "deadline_red"
            } else if days_left <= 14 {
                "deadline_yellow"
            } else {
                "deadline_green"
            };
            Ok(CalendarEvent {
                id: r.get(0)?,
                date: due_date,
                title: r.get(2)?,
                event_type: urgency.into(),
                case_id: r.get(3)?,
                case_name: r.get(4)?,
            })
        })? {
            events.push(row?);
        }

        Ok(events)
    })
    .await
}

/// 计算指定年月的最后一天
fn last_day_of_month(year: i32, month: u32) -> u32 {
    // 用下月1号减去1天得到本月最后一天
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let next_first = chrono::NaiveDate::from_ymd_opt(next_year, next_month, 1).unwrap();
    let last = next_first - chrono::Duration::days(1);
    last.day()
}
