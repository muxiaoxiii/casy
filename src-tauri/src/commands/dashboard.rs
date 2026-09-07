//! 仪表盘聚合查询（B4 · 数据可视化）
//!
//! 全部为只读 GROUP BY 聚合，避免前端拉全量数据自算。
//! 维度：案件状态 / 案件轨道 / 任务月度趋势 / 近期庭审。

use super::run_blocking;
use crate::db;
use chrono::{Datelike, Duration};

#[derive(serde::Serialize, specta::Type)]
pub struct NameCount {
    pub label: String,
    pub value: i64,
}

/// 非案件项目状态分布。legal 项目是案件的兼容镜像，不在看板重复统计。
#[tauri::command]
pub async fn get_project_status_distribution() -> Result<Vec<NameCount>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT COALESCE(NULLIF(status,''),'active') AS s, COUNT(*)
             FROM projects WHERE kind = 'personal'
             GROUP BY s ORDER BY COUNT(*) DESC",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(NameCount {
                    label: r.get(0)?,
                    value: r.get(1)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    })
    .await
}

/// 案件轨道分布，与案件列表使用同一事实表。
#[tauri::command]
pub async fn get_track_distribution() -> Result<Vec<NameCount>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT COALESCE(NULLIF(track,''),'未分类') AS t, COUNT(*)
             FROM cases GROUP BY t ORDER BY COUNT(*) DESC, t",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(NameCount {
                    label: r.get(0)?,
                    value: r.get(1)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    })
    .await
}

#[derive(serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MonthTrendPoint {
    pub month: String, // YYYY-MM
    pub created: i64,
    pub completed: i64,
}

/// 月度任务趋势：未删除任务的创建量及当前已完成任务的最后完成月份。
/// 审计 P2 修正：按真实日历月回推（原 30 天步进在月末会重复/跳月）
#[tauri::command]
pub async fn get_monthly_task_trend(months: Option<i32>) -> Result<Vec<MonthTrendPoint>, String> {
    run_blocking(move || {
        let n = months.unwrap_or(6).clamp(1, 24);
        let conn = db::open_db()?;
        let today = chrono::Local::now().date_naive();

        // 从当前月起逐月回推，生成 n 个 (year, month)
        let mut ym: Vec<(i32, u32)> = Vec::with_capacity(n as usize);
        let (mut y, mut m) = (today.year(), today.month());
        for _ in 0..n {
            ym.push((y, m));
            if m == 1 {
                y -= 1;
                m = 12;
            } else {
                m -= 1;
            }
        }
        ym.reverse(); // 时间升序

        let mut out = Vec::new();
        for (yy, mm) in &ym {
            let key = format!("{yy:04}-{mm:02}");
            let created: i64 = conn.query_row(
                "SELECT COUNT(*) FROM tasks WHERE substr(created_date,1,7) = ?1 AND deleted_at IS NULL",
                rusqlite::params![key],
                |r| r.get(0),
            )?;
            let completed: i64 = conn.query_row(
                "SELECT COUNT(*) FROM (
                   SELECT e.task_id, MAX(e.occurred_at) AS completed_at
                   FROM task_events e JOIN tasks t ON t.id=e.task_id
                   WHERE e.event_type='completed' AND t.completed=1 AND t.deleted_at IS NULL
                   GROUP BY e.task_id
                 ) WHERE substr(completed_at,1,7)=?1",
                rusqlite::params![key],
                |r| r.get(0),
            )?;
            out.push(MonthTrendPoint {
                month: key,
                created,
                completed,
            });
        }
        Ok(out)
    })
    .await
}

#[derive(serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UpcomingHearing {
    pub id: String,
    pub title: String,
    pub date: String,
    pub case_id: String,
    pub case_name: String,
    pub days_left: i64,
}

/// 近期庭审时间线（未来 N 天内的庭审，含今天）
#[tauri::command]
pub async fn get_upcoming_hearings(days: Option<i32>) -> Result<Vec<UpcomingHearing>, String> {
    run_blocking(move || {
        let n = days.unwrap_or(30).clamp(1, 180);
        let conn = db::open_db()?;
        let today = chrono::Local::now().date_naive();
        let end = today + Duration::days(n as i64);
        let mut stmt = conn.prepare(
            "SELECT h.id, h.hearing_date, COALESCE(NULLIF(h.hearing_name,''),'庭审'), c.id, c.case_name
             FROM hearings h JOIN cases c ON c.id = h.case_id
             WHERE substr(h.hearing_date,1,10) BETWEEN ?1 AND ?2 AND COALESCE(h.actual_status,'未开') != '已开'
             ORDER BY h.hearing_date ASC, h.id",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![today.to_string(), end.to_string()], |r| {
                let date: String = r.get(1)?;
                let d = chrono::NaiveDate::parse_from_str(date.get(..10).unwrap_or(""), "%Y-%m-%d")
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e)))?;
                Ok(UpcomingHearing {
                    id: r.get(0)?,
                    title: r.get(2)?,
                    days_left: (d - today).num_days(),
                    date,
                    case_id: r.get(3)?,
                    case_name: r.get(4)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    })
    .await
}

#[derive(serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TodayKpis {
    pub today_events: i64,
    pub due_today: i64,
    pub waiting_overdue: i64,
    pub review_due: i64,
}

/// 今日 KPI 四项（与首页摘要同口径，SQL 直算）
#[tauri::command]
pub async fn get_today_kpis() -> Result<TodayKpis, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let today = chrono::Local::now().date_naive().to_string();
        let today_events: i64 = conn.query_row(
            "SELECT COUNT(*) FROM hearings WHERE substr(hearing_date,1,10) = ?1",
            rusqlite::params![today],
            |r| r.get(0),
        )?;
        let due_today: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tasks
             WHERE completed = 0 AND (due_date = ?1 OR deadline = ?1) AND deleted_at IS NULL",
            rusqlite::params![today],
            |r| r.get(0),
        )?;
        let waiting_overdue: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tasks
             WHERE completed = 0 AND (task_type = 'waiting' OR COALESCE(waiting_for,'') != '')
               AND NULLIF(follow_up_date,'') IS NOT NULL AND follow_up_date < ?1
               AND deleted_at IS NULL",
            rusqlite::params![today],
            |r| r.get(0),
        )?;
        let review_due: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tasks
             WHERE completed = 0 AND NULLIF(next_review_date,'') IS NOT NULL AND next_review_date <= ?1
               AND deleted_at IS NULL",
            rusqlite::params![today],
            |r| r.get(0),
        )?;
        Ok(TodayKpis {
            today_events,
            due_today,
            waiting_overdue,
            review_due,
        })
    })
    .await
}
