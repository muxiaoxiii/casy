use super::run_blocking;
use crate::db;

#[derive(serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEvent {
    pub id: String,
    pub date: String,
    pub title: String,
    pub event_type: String,
    pub case_id: String,
    pub case_name: String,
    // M-CAL-1：时间维度（周/日视图定位与时长渲染依赖）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub all_day: Option<bool>,
}

impl CalendarEvent {
    fn projection(
        id: String,
        date: String,
        title: String,
        event_type: &str,
        case_id: String,
        case_name: String,
    ) -> rusqlite::Result<Self> {
        let invalid_date = |error| rusqlite::Error::FromSqlConversionFailure(
            1, rusqlite::types::Type::Text, Box::new(error),
        );
        let day = chrono::NaiveDate::parse_from_str(date.get(..10).unwrap_or(""), "%Y-%m-%d")
            .map_err(invalid_date)?;
        let start_time = if date.len() > 10 {
            let timestamp = ["%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%d %H:%M", "%Y-%m-%dT%H:%M"]
                .iter().find_map(|format| chrono::NaiveDateTime::parse_from_str(&date, format).ok());
            let time = match timestamp {
                Some(value) => value.time(),
                None => chrono::DateTime::parse_from_rfc3339(&date).map_err(invalid_date)?.time(),
            };
            Some(time.format("%H:%M").to_string())
        } else { None };
        Ok(Self {
            id,
            date: day.to_string(),
            title,
            event_type: event_type.into(),
            case_id,
            case_name,
            all_day: Some(start_time.is_none()),
            start_time,
            end_time: None,
        })
    }
}

/// 月历合并投影：案件域事实（庭审/期限）+ 任务到期 + 独立日程（D-7 calendar_events）
#[tauri::command]
pub async fn get_calendar_events(year: i32, month: u32, month_count: Option<u32>) -> Result<Vec<CalendarEvent>, String> {
    run_blocking(move || {
        anyhow::ensure!(chrono::NaiveDate::from_ymd_opt(year, month, 1).is_some(), "无效的日历月份");
        let conn = db::open_db()?;

        let start = format!("{:04}-{:02}-01", year, month);
        let count = month_count.unwrap_or(1);
        anyhow::ensure!((1..=12).contains(&count), "日历查询范围应为 1—12 个月");
        let end_month = month - 1 + count - 1;
        let end_year = year + (end_month / 12) as i32;
        let end_month = end_month % 12 + 1;
        let end = format!("{:04}-{:02}-{:02}", end_year, end_month, last_day_of_month(end_year, end_month));
        let mut events = Vec::new();

        // 庭审
        let mut stmt = conn.prepare(
            "SELECT h.id, h.hearing_date, h.hearing_name, c.id, c.case_name
             FROM hearings h JOIN cases c ON c.id = h.case_id
             WHERE substr(h.hearing_date,1,10) BETWEEN ?1 AND ?2 AND h.lifecycle_status NOT IN ('postponed','cancelled')",
        )?;
        for row in stmt.query_map(rusqlite::params![start, end], |r| {
            CalendarEvent::projection(
                r.get(0)?,
                r.get(1)?,
                r.get::<_, Option<String>>(2)?.filter(|name| !name.is_empty()).unwrap_or_else(|| "开庭".into()),
                "hearing",
                r.get(3)?,
                r.get(4)?,
            )
        })? {
            events.push(row?);
        }

        // 任务到期
        let mut stmt = conn.prepare(
            "SELECT id, deadline, task_name, case_id FROM tasks
             WHERE substr(deadline,1,10) BETWEEN ?1 AND ?2 AND completed = 0 AND deleted_at IS NULL",
        )?;
        for row in stmt.query_map(rusqlite::params![start, end], |r| {
            CalendarEvent::projection(
                r.get(0)?,
                r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                r.get(2)?,
                "task",
                r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                String::new(),
            )
        })? {
            events.push(row?);
        }

        // 期限预警（红黄绿按剩余天数）
        let mut stmt = conn.prepare(
            "SELECT cd.id, cd.due_date, cd.deadline_name, c.id, c.case_name
             FROM case_deadlines cd JOIN cases c ON c.id = cd.case_id
             WHERE substr(cd.due_date,1,10) BETWEEN ?1 AND ?2 AND cd.completed = 0",
        )?;
        for row in stmt.query_map(rusqlite::params![start, end], |r| {
            let due_date: String = r.get(1)?;
            let days_left = {
                let today = chrono::Local::now().naive_local().date();
                let due = chrono::NaiveDate::parse_from_str(due_date.get(..10).unwrap_or(""), "%Y-%m-%d")
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
            CalendarEvent::projection(
                r.get(0)?,
                due_date,
                r.get(2)?,
                urgency,
                r.get(3)?,
                r.get(4)?,
            )
        })? {
            events.push(row?);
        }

        // Procedural dates share the same live projection as the case board.
        let cases = db::cases::active_cases(&conn)?;
        let context = crate::deadline::procedure::ProjectionContext::load(&conn,&cases)?;
        for case in cases {
            let (_,items)=crate::deadline::procedure::case_items_with_context(&case,&context)?;
            for i in items.into_iter().filter(|i|i.status=="open") {
                if let Some(due)=i.due_on {
                    if due>=start && due<=end {
                        let prefix=if i.needs_review {"待核对"} else if i.source=="internal" {"内部"} else if i.owner=="opponent" {"对方"}else{"我方"};
                        events.push(CalendarEvent::projection(i.id,due,format!("[{}] {} · {}",prefix,i.actor_role,i.title),if i.days_left.unwrap_or(0)<=3 {"deadline_red"}else{"deadline_yellow"},i.case_id,i.case_name)?);
                    }
                }
            }
        }

        // D-7 独立日程：并入月历投影，带时刻信息供周/日视图定位
        let mut stmt = conn.prepare(
            "SELECT id, event_date, title, start_time, end_time, all_day,
                    COALESCE(case_id, ''), COALESCE((SELECT case_name FROM cases WHERE id = ce.case_id), '')
             FROM calendar_events ce
             WHERE event_date BETWEEN ?1 AND ?2 AND NOT EXISTS (SELECT 1 FROM tasks t WHERE t.id=ce.task_id AND t.deleted_at IS NOT NULL)",
        )?;
        for row in stmt.query_map(rusqlite::params![start, end], |r| {
            let all_day: i64 = r.get(5)?;
            Ok(CalendarEvent {
                id: r.get(0)?,
                date: r.get(1)?,
                title: r.get(2)?,
                start_time: r.get(3)?,
                end_time: r.get(4)?,
                all_day: Some(all_day != 0),
                event_type: "event".into(),
                case_id: r.get(6)?,
                case_name: r.get(7)?,
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
    use chrono::Datelike;
    chrono::NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .map(|d| (d - chrono::Duration::days(1)).day())
        .unwrap_or(28)
}
