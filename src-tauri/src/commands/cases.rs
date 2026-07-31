use serde::Serialize;

use super::run_blocking;
use crate::db;
use crate::db::cases::CaseFilter;
use crate::formula::engine::DeadlineResult;

#[tauri::command]
pub async fn list_cases(
    filter: db::cases::CaseFilter,
) -> Result<db::cases::CaseListResult, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        db::cases::list_cases(&conn, &filter)
    })
    .await
}

#[tauri::command]
pub async fn get_case(id: String) -> Result<db::cases::Case, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        db::cases::get_case(&conn, &id)
    })
    .await
}

#[tauri::command]
pub async fn create_case(data: serde_json::Value) -> Result<db::cases::Case, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut case: db::cases::Case = serde_json::from_value(data)?;
        case.id = db::new_id();
        case.created_at = Some(db::now_local());
        case.updated_at = Some(db::now_local());

        // 自动创建客户（如果不存在）
        if !case.client_name.is_empty() {
            let _ = conn.execute(
                "INSERT OR IGNORE INTO clients (id, name) VALUES (?1, ?2)",
                rusqlite::params![db::new_id(), case.client_name],
            );
        }

        db::cases::insert_case(&conn, &case)?;

        // 自动创建案件文件夹（7 个子目录）
        match crate::files::ensure_case_folder(&case) {
            Ok(folder) => {
                let folder_str = folder.to_string_lossy().to_string();
                let _ = conn.execute(
                    "UPDATE cases SET folder_path = ?1 WHERE id = ?2",
                    rusqlite::params![folder_str, case.id],
                );
                case.folder_path = Some(folder_str);
                log::info!("案件文件夹已创建: {:?}", folder);
            }
            Err(e) => log::warn!("创建案件文件夹失败: {}", e),
        }

        // 触发飞书自动推送（5 秒防抖）
        crate::sync::feishu::get_auto_push_manager().notify_change();

        Ok(case)
    })
    .await
}

#[tauri::command]
pub async fn update_case(id: String, data: serde_json::Value) -> Result<db::cases::Case, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let case = db::cases::update_case(&conn, &id, &data)?;

        // 触发飞书自动推送（5 秒防抖）
        crate::sync::feishu::get_auto_push_manager().notify_change();

        Ok(case)
    })
    .await
}

#[tauri::command]
pub async fn delete_case(id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;

        // 删除前将案件文件夹移入系统回收站
        if let Ok(case) = db::cases::get_case(&conn, &id) {
            if let Some(ref folder_path) = case.folder_path {
                let path = std::path::Path::new(folder_path);
                if path.exists() {
                    let trash_result = trash::delete(path);
                    if let Err(e) = trash_result {
                        log::warn!("移入回收站失败，尝试直接删除: {}", e);
                        let _ = std::fs::remove_dir_all(path);
                    } else {
                        log::info!("案件文件夹已移入回收站: {}", folder_path);
                    }
                }
            }
        }

        db::cases::delete_case(&conn, &id)?;

        // 触发飞书自动推送（5 秒防抖）
        crate::sync::feishu::get_auto_push_manager().notify_change();

        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn search_cases(query: String) -> Result<Vec<db::cases::Case>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        db::cases::search_cases(&conn, &query)
    })
    .await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaseStats {
    pub total: i64,
    pub active: i64,
    pub closed: i64,
    pub by_track: Vec<(String, i64)>,
    pub by_client: Vec<(String, i64)>,
}

#[tauri::command]
pub async fn case_stats() -> Result<CaseStats, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let total: i64 = conn.query_row("SELECT COUNT(*) FROM cases", [], |r| r.get(0))?;
        let active: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cases WHERE case_status IS NULL OR case_status != '已完结'",
            [],
            |r| r.get(0),
        )?;
        let closed = total - active;
        let by_track = db::cases::case_counts_by_track(&conn)?;
        let by_client = db::cases::case_counts_by_client(&conn)?;

        Ok(CaseStats {
            total,
            active,
            closed,
            by_track,
            by_client,
        })
    })
    .await
}

/// 最近活动条目
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentActivity {
    pub event_type: String,
    pub title: String,
    pub detail: Option<String>,
    pub event_date: String,
    pub case_id: String,
    pub case_name: String,
}

/// 仪表盘聚合数据
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardStats {
    pub active_count: i64,
    pub total_count: i64,
    pub closed_count: i64,
    pub deadline_warnings: Vec<DeadlineResult>,
    pub recent_activities: Vec<RecentActivity>,
    pub by_track: Vec<(String, i64)>,
}

#[tauri::command]
pub async fn get_dashboard_stats() -> Result<DashboardStats, String> {
    run_blocking(move || {
        let conn = db::open_db()?;

        // 基本统计
        let total_count: i64 = conn.query_row("SELECT COUNT(*) FROM cases", [], |r| r.get(0))?;
        let active_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM cases WHERE case_status IS NULL OR case_status != '已完结'",
            [],
            |r| r.get(0),
        )?;
        let closed_count = total_count - active_count;

        // 按轨道统计
        let by_track = db::cases::case_counts_by_track(&conn)?;

        // 期限预警（取最近 10 个最紧急的）
        let engine = crate::formula::engine::DeadlineEngine::new(&conn)?;
        let all_warnings = engine.generate_all_warnings(&conn)?;
        let deadline_warnings: Vec<DeadlineResult> = all_warnings.into_iter().take(10).collect();

        // 最近 7 天活动
        let seven_days_ago = chrono::Local::now()
            .naive_local()
            .date()
            .checked_sub_signed(chrono::Duration::days(7))
            .unwrap()
            .format("%Y-%m-%d")
            .to_string();

        let recent_activities = query_recent_activities(&conn, &seven_days_ago)?;

        Ok(DashboardStats {
            active_count,
            total_count,
            closed_count,
            deadline_warnings,
            recent_activities,
            by_track,
        })
    })
    .await
}

fn query_recent_activities(conn: &rusqlite::Connection, since: &str) -> anyhow::Result<Vec<RecentActivity>> {
    let mut activities = Vec::new();

    // 最近日志
    let mut stmt = conn.prepare(
        "SELECT l.event_summary, l.content, l.event_date, l.case_id, c.case_name
         FROM case_logs l JOIN cases c ON c.id = l.case_id
         WHERE l.event_date >= ?1
         ORDER BY l.event_date DESC LIMIT 20",
    )?;
    for row in stmt.query_map(rusqlite::params![since], |r| {
        Ok(RecentActivity {
            event_type: "log".into(),
            title: r.get(0)?,
            detail: r.get::<_, Option<String>>(1)?,
            event_date: r.get(2)?,
            case_id: r.get(3)?,
            case_name: r.get(4)?,
        })
    })? {
        activities.push(row?);
    }

    // 最近庭审
    let mut stmt = conn.prepare(
        "SELECT h.hearing_name, h.venue, h.hearing_date, h.case_id, c.case_name
         FROM hearings h JOIN cases c ON c.id = h.case_id
         WHERE h.hearing_date >= ?1
         ORDER BY h.hearing_date DESC LIMIT 10",
    )?;
    for row in stmt.query_map(rusqlite::params![since], |r| {
        Ok(RecentActivity {
            event_type: "hearing".into(),
            title: r.get(0)?,
            detail: r.get::<_, Option<String>>(1)?,
            event_date: r.get(2)?,
            case_id: r.get(3)?,
            case_name: r.get(4)?,
        })
    })? {
        activities.push(row?);
    }

    // 最近任务
    let mut stmt = conn.prepare(
        "SELECT t.task_name, t.description, t.created_date, t.case_id, c.case_name
         FROM tasks t JOIN cases c ON c.id = t.case_id
         WHERE t.created_date >= ?1
         ORDER BY t.created_date DESC LIMIT 10",
    )?;
    for row in stmt.query_map(rusqlite::params![since], |r| {
        Ok(RecentActivity {
            event_type: "task".into(),
            title: r.get(0)?,
            detail: r.get::<_, Option<String>>(1)?,
            event_date: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
            case_id: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
            case_name: r.get(4)?,
        })
    })? {
        activities.push(row?);
    }

    // 按日期倒序，取前 15 条
    activities.sort_by(|a, b| b.event_date.cmp(&a.event_date));
    activities.truncate(15);
    Ok(activities)
}

/// 导出案件列表为 CSV
#[tauri::command]
pub async fn export_cases(format: String, filter: Option<CaseFilter>) -> Result<String, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let filter = filter.unwrap_or_default();

        // 查询案件（不分页，最多 10000 条）
        let mut export_filter = filter;
        export_filter.page = Some(1);
        export_filter.per_page = Some(10000);
        let result = db::cases::list_cases(&conn, &export_filter)?;

        if format == "csv" {
            let csv = cases_to_csv(&result.items)?;

            // 保存到下载目录
            let filename = format!(
                "casy_export_{}.csv",
                chrono::Local::now().format("%Y%m%d_%H%M%S")
            );
            let download_dir = dirs::download_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."));
            let path = download_dir.join(&filename);
            std::fs::write(&path, csv)?;

            log::info!("Exported {} cases to {:?}", result.items.len(), path);
            Ok(path.to_string_lossy().to_string())
        } else {
            Err(anyhow::anyhow!("Unsupported format: {}", format))
        }
    })
    .await
}

fn cases_to_csv(cases: &[db::cases::Case]) -> anyhow::Result<String> {
    let mut wtr = csv::Writer::from_writer(vec![]);

    // 写入表头
    wtr.write_record(&[
        "案件名称", "案号", "内部卷号", "轨道", "案由", "客户", "我方地位",
        "对方", "对方地位", "对方代理", "法院", "审级", "案件进展",
        "案件结果", "专利名称", "专利申请号", "立案日期", "开庭日期",
        "判决日期", "备注",
    ])?;

    for c in cases {
        let track_label = match c.track.as_str() {
            "patent_invalidation" => "专利无效",
            "admin_litigation" => "行政诉讼",
            "civil_tort" => "民事侵权",
            _ => "其他",
        };
        wtr.write_record(&[
            c.case_name.as_str(),
            c.case_no.as_deref().unwrap_or(""),
            c.internal_no.as_deref().unwrap_or(""),
            track_label,
            c.cause_action.as_deref().unwrap_or(""),
            c.client_name.as_str(),
            c.our_role.as_deref().unwrap_or(""),
            c.opponent_name.as_str(),
            c.opponent_role.as_deref().unwrap_or(""),
            c.opponent_firm.as_deref().unwrap_or(""),
            c.court.as_deref().unwrap_or(""),
            c.case_level.as_deref().unwrap_or(""),
            c.case_progress.as_deref().unwrap_or(""),
            c.case_result.as_deref().unwrap_or(""),
            c.patent_name.as_deref().unwrap_or(""),
            c.patent_app_no.as_deref().unwrap_or(""),
            c.filing_date.as_deref().unwrap_or(""),
            c.trial_date.as_deref().unwrap_or(""),
            c.verdict_date.as_deref().unwrap_or(""),
            c.notes.as_deref().unwrap_or(""),
        ])?;
    }

    let data = wtr.into_inner()?;
    String::from_utf8(data).map_err(|e| anyhow::anyhow!(e))
}
