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

/// 项目状态分布（projects 表：legal 由触发器镜像，personal 直管）
#[tauri::command]
pub async fn get_project_status_distribution() -> Result<Vec<NameCount>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT COALESCE(NULLIF(status,''),'active') AS s, COUNT(*)
             FROM projects GROUP BY s ORDER BY COUNT(*) DESC",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(NameCount { label: r.get(0)?, value: r.get(1)? })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    })
    .await
}

/// 案件轨道分布（无效/行政/民事…，来自 case_legal_details）
#[tauri::command]
pub async fn get_track_distribution() -> Result<Vec<NameCount>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT COALESCE(track,'未分类') AS t, COUNT(*)
             FROM case_legal_details GROUP BY t ORDER BY COUNT(*) DESC",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(NameCount { label: r.get(0)?, value: r.get(1)? })
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

/// 月度任务趋势：近 N 个月创建量 vs 完成量
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
                "SELECT COUNT(*) FROM tasks WHERE substr(created_date,1,7) = ?1",
                rusqlite::params![key],
                |r| r.get(0),
            )?;
            let completed: i64 = conn.query_row(
                "SELECT COUNT(*) FROM task_events
                 WHERE event_type = 'completed' AND substr(occurred_at,1,7) = ?1",
                rusqlite::params![key],
                |r| r.get(0),
            )?;
            out.push(MonthTrendPoint { month: key, created, completed });
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
            "SELECT h.id, h.hearing_date, COALESCE(h.hearing_name,'庭审'), c.id, c.case_name
             FROM hearings h JOIN cases c ON c.id = h.case_id
             WHERE h.hearing_date BETWEEN ?1 AND ?2
             ORDER BY h.hearing_date ASC LIMIT 50",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![today.to_string(), end.to_string()], |r| {
                let date: String = r.get(1)?;
                let d = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d")
                    .unwrap_or(today);
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
            "SELECT COUNT(*) FROM hearings WHERE hearing_date = ?1",
            rusqlite::params![today],
            |r| r.get(0),
        )?;
        let due_today: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tasks
             WHERE completed = 0 AND (due_date = ?1 OR deadline = ?1)",
            rusqlite::params![today],
            |r| r.get(0),
        )?;
        let waiting_overdue: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tasks
             WHERE completed = 0 AND task_type = 'waiting'
               AND follow_up_date IS NOT NULL AND follow_up_date < ?1",
            rusqlite::params![today],
            |r| r.get(0),
        )?;
        let review_due: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tasks
             WHERE completed = 0 AND next_review_date IS NOT NULL AND next_review_date <= ?1",
            rusqlite::params![today],
            |r| r.get(0),
        )?;
        Ok(TodayKpis { today_events, due_today, waiting_overdue, review_due })
    })
    .await
}
