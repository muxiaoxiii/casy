use serde::Serialize;

use super::run_blocking;
use crate::db;
use crate::db::cases::CaseFilter;
use crate::deadline::engine::DeadlineResult;

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
        let engine = crate::deadline::engine::DeadlineEngine::new(&conn)?;
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

/// 动态字段分组查询命令
#[tauri::command]
pub async fn list_field_groups(case_type: Option<String>) -> Result<Vec<serde_json::Value>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;

        // 查询所有字段分组
        let mut stmt = conn
            .prepare(
                "SELECT id, name, description, case_types, court_levels, sort_order
                 FROM field_groups ORDER BY sort_order",
            )
            ?;

        let groups: Vec<serde_json::Value> = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let name: String = row.get(1)?;
                let description: Option<String> = row.get(2)?;
                let case_types_json: Option<String> = row.get(3)?;
                let court_levels_json: Option<String> = row.get(4)?;
                let sort_order: i32 = row.get(5)?;
                Ok((id, name, description, case_types_json, court_levels_json, sort_order))
            })
            ?
            .filter_map(|r| r.ok())
            .filter(|(_, _, _, case_types_json, _, _)| {
                // 如果指定了 case_type，过滤掉不适用的分组
                if let Some(ref ct) = case_type {
                    if let Some(ref json_str) = case_types_json {
                        // case_types 不为 null 表示仅适用于特定类型
                        if let Ok(types) = serde_json::from_str::<Vec<String>>(json_str) {
                            return types.iter().any(|t| ct.contains(t.as_str()));
                        }
                    }
                    // case_types 为 null 表示通用，始终包含
                    true
                } else {
                    true
                }
            })
            .map(|(id, name, description, case_types_json, court_levels_json, sort_order)| {
                // 查询该分组下的字段项
                let items = query_field_group_items(&conn, &id).unwrap_or_default();

                serde_json::json!({
                    "id": id,
                    "name": name,
                    "description": description,
                    "caseTypes": case_types_json.and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()),
                    "courtLevels": court_levels_json.and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()),
                    "sortOrder": sort_order,
                    "items": items,
                })
            })
            .collect();

        Ok(groups)
    })
    .await
}

fn query_field_group_items(conn: &rusqlite::Connection, group_id: &str) -> anyhow::Result<Vec<serde_json::Value>> {
    let mut stmt = conn.prepare(
        "SELECT id, column_name, label, field_type, options, required, sort_order
         FROM field_group_items WHERE group_id = ?1 ORDER BY sort_order"
    )?;

    let items = stmt.query_map([group_id], |row| {
        let id: String = row.get(0)?;
        let column_name: String = row.get(1)?;
        let label: String = row.get(2)?;
        let field_type: String = row.get(3)?;
        let options: Option<String> = row.get(4)?;
        let required: i32 = row.get(5)?;
        let sort_order: i32 = row.get(6)?;
        Ok(serde_json::json!({
            "id": id,
            "columnName": column_name,
            "label": label,
            "fieldType": field_type,
            "options": options.and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()),
            "required": required != 0,
            "sortOrder": sort_order,
        }))
    })?.filter_map(|r| r.ok()).collect();

    Ok(items)
}

/// 跨类型统一视图查询命令
#[tauri::command]
pub async fn get_case_unified_view(filters: Option<serde_json::Value>) -> Result<Vec<serde_json::Value>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;

        let mut sql = String::from(
            "SELECT id, case_name, case_no, client_name, cause_action, track,
                    status, court, case_level, operator, trial_date, filing_date,
                    next_deadline, next_hearing, updated_at
             FROM v_case_unified WHERE 1=1"
        );
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut param_idx = 1;

        if let Some(ref f) = filters {
            // 案件类型过滤
            if let Some(case_type) = f.get("caseType").and_then(|v| v.as_str()) {
                if !case_type.is_empty() {
                    sql.push_str(&format!(" AND cause_action LIKE '%{}%'", case_type));
                }
            }

            // 轨道过滤
            if let Some(track) = f.get("track").and_then(|v| v.as_str()) {
                if !track.is_empty() {
                    sql.push_str(&format!(" AND track = ?{}", param_idx));
                    params.push(Box::new(track.to_string()));
                    param_idx += 1;
                }
            }

            // 状态过滤
            if let Some(status) = f.get("status").and_then(|v| v.as_str()) {
                if !status.is_empty() {
                    sql.push_str(&format!(" AND status = ?{}", param_idx));
                    params.push(Box::new(status.to_string()));
                    param_idx += 1;
                }
            }

            // 审理机关过滤
            if let Some(court) = f.get("court").and_then(|v| v.as_str()) {
                if !court.is_empty() {
                    sql.push_str(&format!(" AND court LIKE ?{}", param_idx));
                    params.push(Box::new(format!("%{}%", court)));
                    param_idx += 1;
                }
            }

            // 办案人过滤
            if let Some(operator) = f.get("operator").and_then(|v| v.as_str()) {
                if !operator.is_empty() {
                    sql.push_str(&format!(" AND operator LIKE ?{}", param_idx));
                    params.push(Box::new(format!("%{}%", operator)));
                    param_idx += 1;
                }
            }

            // 期限范围过滤
            if let Some(deadline_from) = f.get("deadlineFrom").and_then(|v| v.as_str()) {
                if !deadline_from.is_empty() {
                    sql.push_str(&format!(" AND next_deadline >= ?{}", param_idx));
                    params.push(Box::new(deadline_from.to_string()));
                    param_idx += 1;
                }
            }
            if let Some(deadline_to) = f.get("deadlineTo").and_then(|v| v.as_str()) {
                if !deadline_to.is_empty() {
                    sql.push_str(&format!(" AND next_deadline <= ?{}", param_idx));
                    params.push(Box::new(deadline_to.to_string()));
                    param_idx += 1;
                }
            }

            // 开庭日期范围过滤
            if let Some(hearing_from) = f.get("hearingFrom").and_then(|v| v.as_str()) {
                if !hearing_from.is_empty() {
                    sql.push_str(&format!(" AND next_hearing >= ?{}", param_idx));
                    params.push(Box::new(hearing_from.to_string()));
                    param_idx += 1;
                }
            }
            if let Some(hearing_to) = f.get("hearingTo").and_then(|v| v.as_str()) {
                if !hearing_to.is_empty() {
                    sql.push_str(&format!(" AND next_hearing <= ?{}", param_idx));
                    params.push(Box::new(hearing_to.to_string()));
                    param_idx += 1;
                }
            }
        }

        sql.push_str(" ORDER BY next_deadline ASC NULLS LAST, updated_at DESC");

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "caseName": row.get::<_, Option<String>>(1)?,
                "caseNo": row.get::<_, Option<String>>(2)?,
                "clientName": row.get::<_, Option<String>>(3)?,
                "causeAction": row.get::<_, Option<String>>(4)?,
                "track": row.get::<_, Option<String>>(5)?,
                "status": row.get::<_, Option<String>>(6)?,
                "court": row.get::<_, Option<String>>(7)?,
                "caseLevel": row.get::<_, Option<String>>(8)?,
                "operator": row.get::<_, Option<String>>(9)?,
                "trialDate": row.get::<_, Option<String>>(10)?,
                "filingDate": row.get::<_, Option<String>>(11)?,
                "nextDeadline": row.get::<_, Option<String>>(12)?,
                "nextHearing": row.get::<_, Option<String>>(13)?,
                "updatedAt": row.get::<_, Option<String>>(14)?,
            }))
        })?;

        let results: Vec<serde_json::Value> = rows.filter_map(|r| r.ok()).collect();
        Ok(results)
    })
    .await
}

/// 重新计算单个案件的公式缓存列
#[tauri::command]
pub async fn recalculate_case_formulas(case_id: String) -> Result<usize, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let count = crate::formula::recalculate_case_formulas(&conn, &case_id)?;
        Ok(count)
    }).await
}

/// 重新计算所有案件的公式缓存列
#[tauri::command]
pub async fn recalculate_all_formulas() -> Result<usize, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let count = crate::formula::recalculate_all_formulas(&conn)?;
        Ok(count)
    }).await
}

/// 手动切换案件状态（双轨状态机）
///
/// 更新指定轨道的状态，并自动记录到 case_track_history。
/// track: "civil_status" | "invalidation_status" | "admin_status"
/// new_status: 目标状态值
#[tauri::command]
pub async fn update_case_status(
    case_id: String,
    track: String,
    new_status: String,
    note: Option<String>,
) -> Result<db::cases::Case, String> {
    run_blocking(move || {
        let conn = db::open_db()?;

        // 1. 获取当前状态
        let old_status: Option<String> = conn.query_row(
            &format!("SELECT {} FROM cases WHERE id = ?1", track),
            rusqlite::params![case_id],
            |r| r.get(0),
        ).ok().flatten();

        // 2. 更新状态
        conn.execute(
            &format!("UPDATE cases SET {} = ?1, updated_at = ?2 WHERE id = ?3", track),
            rusqlite::params![new_status, db::now_local(), case_id],
        )?;

        // 3. 同步更新聚合状态 case_status
        let civil: Option<String> = conn.query_row(
            "SELECT civil_status FROM cases WHERE id = ?1",
            rusqlite::params![case_id],
            |r| r.get(0),
        ).unwrap_or(None);
        let invalidation: Option<String> = conn.query_row(
            "SELECT invalidation_status FROM cases WHERE id = ?1",
            rusqlite::params![case_id],
            |r| r.get(0),
        ).unwrap_or(None);
        let admin: Option<String> = conn.query_row(
            "SELECT admin_status FROM cases WHERE id = ?1",
            rusqlite::params![case_id],
            |r| r.get(0),
        ).unwrap_or(None);

        let aggregate = compute_aggregate_status(civil.as_deref(), invalidation.as_deref(), admin.as_deref());
        conn.execute(
            "UPDATE cases SET case_status = ?1 WHERE id = ?2",
            rusqlite::params![aggregate, case_id],
        )?;

        // 4. 记录到 case_track_history
        let track_name = match track.as_str() {
            "civil_status" => "民事诉讼",
            "invalidation_status" => "专利无效",
            "admin_status" => "行政诉讼",
            _ => "其他",
        };
        conn.execute(
            "INSERT INTO case_track_history (id, case_id, track, from_status, to_status, changed_at, source, note)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'manual', ?7)",
            rusqlite::params![
                db::new_id(),
                case_id,
                track_name,
                old_status,
                new_status,
                db::now_local(),
                note,
            ],
        )?;

        // 5. 返回更新后的案件
        db::cases::get_case(&conn, &case_id)
    })
    .await
}

/// 从三轨状态推导聚合 case_status
fn compute_aggregate_status(
    civil: Option<&str>,
    invalidation: Option<&str>,
    admin: Option<&str>,
) -> &'static str {
    let civil_closed = civil == Some("closed");
    let inv_done = invalidation.is_none() || invalidation == Some("decision_issued");
    let admin_closed = admin.is_none() || admin == Some("closed");

    if civil_closed && inv_done && admin_closed {
        "已完结"
    } else if civil.is_some() || invalidation.is_some() || admin.is_some() {
        "进行中"
    } else {
        "未知"
    }
}

/// 获取今日概览统计
#[tauri::command]
pub async fn get_today_stats() -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();

        // 硬性日程（开庭/口审）
        let hard_schedule: i32 = conn.query_row(
            "SELECT COUNT(*) FROM hearings WHERE hearing_date = ?1",
            rusqlite::params![today],
            |row| row.get(0),
        ).unwrap_or(0);

        // 今日到期任务
        let due_today: i32 = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE (deadline = ?1 OR due_date = ?1) AND completed = 0",
            rusqlite::params![today],
            |row| row.get(0),
        ).unwrap_or(0);

        // 等待超3天
        let waiting_overdue: i32 = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE task_type = 'waiting' AND follow_up_date < date(?1, '-3 days') AND completed = 0",
            rusqlite::params![today],
            |row| row.get(0),
        ).unwrap_or(0);

        // 需回顾案件
        let need_review: i32 = conn.query_row(
            "SELECT COUNT(*) FROM cases WHERE next_review_date <= ?1",
            rusqlite::params![today],
            |row| row.get(0),
        ).unwrap_or(0);

        Ok(serde_json::json!({
            "hardSchedule": hard_schedule,
            "dueToday": due_today,
            "waitingOverdue": waiting_overdue,
            "needReview": need_review,
        }))
    })
    .await
}

// ═══════════════════════════════════════════════════════════
// 案件类型差异化评估（设计哲学 §原则五 / P3）
//
// 按 cases.case_type 输出不同评估指标，纯 SQL 确定性计算，不调 AI：
// - computational（计算型）：期限内按时完成率 + 当前逾期数
// - exploratory（探索型）：阶段推进（近 90 天状态变迁）+ blocked 任务解锁进度
// - growth（成长型）：近 30 天活跃天数 + 连续无活动天数
// - 未设 case_type（NULL）：通用指标（任务完成率 + 逾期数）
// ═══════════════════════════════════════════════════════════

/// 任务的实际截止日期（due_date 优先，回退 deadline）
const TASK_DUE_EXPR: &str = "COALESCE(t.due_date, t.deadline)";

/// 当前逾期未完成任务数
fn count_overdue_tasks(conn: &rusqlite::Connection, case_id: &str) -> anyhow::Result<i64> {
    let sql = format!(
        "SELECT COUNT(*) FROM tasks t
         WHERE t.case_id=?1 AND t.completed=0
           AND {due} IS NOT NULL AND {due} != ''
           AND {due} < date('now','localtime')",
        due = TASK_DUE_EXPR
    );
    let n = conn.query_row(&sql, rusqlite::params![case_id], |row| row.get(0))?;
    Ok(n)
}

/// 计算单个案件的类型差异化指标
pub fn compute_case_type_metrics(
    conn: &rusqlite::Connection,
    case_id: &str,
) -> anyhow::Result<serde_json::Value> {
    let case_type: Option<String> = conn
        .query_row(
            "SELECT case_type FROM cases WHERE id=?1",
            rusqlite::params![case_id],
            |row| row.get(0),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other),
        })?
        .ok_or_else(|| anyhow::anyhow!("案件不存在: {}", case_id))?;

    let overdue_count = count_overdue_tasks(conn, case_id)?;

    let metrics = match case_type.as_deref() {
        Some("computational") => {
            // 计算型：已完成任务中在截止前完成的比例（完成时间取 task_events 的 completed 事件）
            let sql = format!(
                "SELECT COUNT(*),
                        SUM(CASE WHEN due IS NOT NULL THEN 1 ELSE 0 END),
                        SUM(CASE WHEN due IS NOT NULL AND completed_at IS NOT NULL
                                  AND date(completed_at) <= due THEN 1 ELSE 0 END)
                 FROM (
                   SELECT t.id, {due} AS due,
                          (SELECT MIN(te.occurred_at) FROM task_events te
                            WHERE te.task_id = t.id AND te.event_type='completed') AS completed_at
                   FROM tasks t
                   WHERE t.case_id=?1 AND t.completed=1
                 )",
                due = TASK_DUE_EXPR
            );
            let (completed_total, with_due, on_time): (i64, Option<i64>, Option<i64>) =
                conn.query_row(&sql, rusqlite::params![case_id], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })?;
            let with_due = with_due.unwrap_or(0);
            let on_time = on_time.unwrap_or(0);
            let on_time_rate = if with_due > 0 {
                serde_json::Value::from(on_time as f64 / with_due as f64)
            } else {
                serde_json::Value::Null
            };
            serde_json::json!({
                "completedTotal": completed_total,
                "completedWithDue": with_due,
                "onTimeCompleted": on_time,
                "onTimeRate": on_time_rate,
                "overdueCount": overdue_count,
            })
        }
        Some("exploratory") => {
            // 探索型：阶段推进（近 90 天轨道状态变迁次数）+ blocked 任务解锁进度
            let transitions_90d: i64 = conn.query_row(
                "SELECT COUNT(*) FROM case_track_history
                 WHERE case_id=?1 AND changed_at >= datetime('now','localtime','-90 days')",
                rusqlite::params![case_id],
                |row| row.get(0),
            )?;
            let (total_tasks, blocked_total, blocked_remaining): (i64, Option<i64>, Option<i64>) =
                conn.query_row(
                    "SELECT COUNT(*),
                            SUM(CASE WHEN blocked=1 THEN 1 ELSE 0 END),
                            SUM(CASE WHEN blocked=1 AND completed=0 THEN 1 ELSE 0 END)
                     FROM tasks WHERE case_id=?1",
                    rusqlite::params![case_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )?;
            let blocked_total = blocked_total.unwrap_or(0);
            let blocked_remaining = blocked_remaining.unwrap_or(0);
            serde_json::json!({
                "trackTransitions90d": transitions_90d,
                "totalTasks": total_tasks,
                "blockedTotal": blocked_total,
                "blockedRemaining": blocked_remaining,
                "blockedResolved": blocked_total - blocked_remaining,
                "overdueCount": overdue_count,
            })
        }
        Some("growth") => {
            // 成长型：近 30 天活跃天数（task_events distinct date）+ 连续无活动天数
            let active_days_30d: i64 = conn.query_row(
                "SELECT COUNT(DISTINCT date(te.occurred_at))
                 FROM task_events te JOIN tasks t ON t.id = te.task_id
                 WHERE t.case_id=?1 AND te.occurred_at >= datetime('now','localtime','-30 days')",
                rusqlite::params![case_id],
                |row| row.get(0),
            )?;
            let inactive_days: Option<f64> = conn.query_row(
                "SELECT julianday(date('now','localtime')) - julianday(date(MAX(te.occurred_at)))
                 FROM task_events te JOIN tasks t ON t.id = te.task_id
                 WHERE t.case_id=?1",
                rusqlite::params![case_id],
                |row| row.get(0),
            )?;
            serde_json::json!({
                "activeDays30d": active_days_30d,
                "inactiveStreakDays": inactive_days.map(|d| d as i64),
                "overdueCount": overdue_count,
            })
        }
        _ => {
            // 未设 case_type：通用指标（任务完成率 + 逾期数）
            let (total, completed): (i64, Option<i64>) = conn.query_row(
                "SELECT COUNT(*), SUM(CASE WHEN completed=1 THEN 1 ELSE 0 END)
                 FROM tasks WHERE case_id=?1",
                rusqlite::params![case_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            let completed = completed.unwrap_or(0);
            let completion_rate = if total > 0 {
                serde_json::Value::from(completed as f64 / total as f64)
            } else {
                serde_json::Value::Null
            };
            serde_json::json!({
                "totalTasks": total,
                "completedTasks": completed,
                "completionRate": completion_rate,
                "overdueCount": overdue_count,
            })
        }
    };

    Ok(serde_json::json!({
        "caseId": case_id,
        "caseType": case_type.as_deref().unwrap_or("generic"),
        "metrics": metrics,
    }))
}

/// 获取单个案件的类型差异化评估指标
#[tauri::command]
pub async fn get_case_type_metrics(case_id: String) -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        compute_case_type_metrics(&conn, &case_id)
    })
    .await
}

/// 批量获取所有案件的类型差异化评估指标（单个案件失败不影响其余）
#[tauri::command]
pub async fn get_all_case_type_metrics() -> Result<Vec<serde_json::Value>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare("SELECT id FROM cases ORDER BY created_at")?;
        let ids: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            match compute_case_type_metrics(&conn, &id) {
                Ok(m) => out.push(m),
                Err(e) => log::warn!("计算案件 {} 类型指标失败: {}", id, e),
            }
        }
        Ok(out)
    })
    .await
}

#[cfg(test)]
mod case_type_metrics_tests {
    use super::*;

    /// 最小内存库：cases / tasks / task_events / case_track_history
    fn setup_test_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE cases (id TEXT PRIMARY KEY, case_type TEXT);
             CREATE TABLE tasks (
               id TEXT PRIMARY KEY, case_id TEXT, completed INTEGER DEFAULT 0,
               due_date TEXT, deadline TEXT, blocked INTEGER DEFAULT 0
             );
             CREATE TABLE task_events (
               id TEXT PRIMARY KEY, task_id TEXT, event_type TEXT, occurred_at TEXT
             );
             CREATE TABLE case_track_history (
               id TEXT PRIMARY KEY, case_id TEXT, changed_at TEXT
             );",
        )
        .unwrap();
        conn
    }

    fn add_task(conn: &rusqlite::Connection, id: &str, case_id: &str, completed: i32, due: Option<&str>, blocked: i32) {
        conn.execute(
            "INSERT INTO tasks (id, case_id, completed, due_date, blocked) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![id, case_id, completed, due, blocked],
        )
        .unwrap();
    }

    #[test]
    fn test_computational_metrics() {
        let conn = setup_test_db();
        conn.execute("INSERT INTO cases (id, case_type) VALUES ('c1', 'computational')", []).unwrap();
        let today = db::today();
        // 按时完成：due 今天，completed 事件今天
        add_task(&conn, "t1", "c1", 1, Some(&today), 0);
        conn.execute(
            "INSERT INTO task_events (id, task_id, event_type, occurred_at) VALUES ('e1', 't1', 'completed', datetime('now','localtime'))",
            [],
        ).unwrap();
        // 逾期完成：due 昨天，completed 事件今天
        add_task(&conn, "t2", "c1", 1, Some("2000-01-01"), 0);
        conn.execute(
            "INSERT INTO task_events (id, task_id, event_type, occurred_at) VALUES ('e2', 't2', 'completed', datetime('now','localtime'))",
            [],
        ).unwrap();
        // 当前逾期未完成
        add_task(&conn, "t3", "c1", 0, Some("2000-01-01"), 0);

        let m = compute_case_type_metrics(&conn, "c1").unwrap();
        assert_eq!(m["caseType"], "computational");
        assert_eq!(m["metrics"]["completedTotal"], 2);
        assert_eq!(m["metrics"]["completedWithDue"], 2);
        assert_eq!(m["metrics"]["onTimeCompleted"], 1);
        assert_eq!(m["metrics"]["onTimeRate"], 0.5);
        assert_eq!(m["metrics"]["overdueCount"], 1);
    }

    #[test]
    fn test_exploratory_metrics() {
        let conn = setup_test_db();
        conn.execute("INSERT INTO cases (id, case_type) VALUES ('c2', 'exploratory')", []).unwrap();
        add_task(&conn, "t1", "c2", 1, None, 1); // blocked 已完成
        add_task(&conn, "t2", "c2", 0, None, 1); // blocked 未完成
        add_task(&conn, "t3", "c2", 0, None, 0); // 非 blocked
        // 90 天内 2 次变迁，90 天外 1 次
        conn.execute("INSERT INTO case_track_history (id, case_id, changed_at) VALUES ('h1', 'c2', datetime('now','localtime','-10 days'))", []).unwrap();
        conn.execute("INSERT INTO case_track_history (id, case_id, changed_at) VALUES ('h2', 'c2', datetime('now','localtime','-80 days'))", []).unwrap();
        conn.execute("INSERT INTO case_track_history (id, case_id, changed_at) VALUES ('h3', 'c2', datetime('now','localtime','-120 days'))", []).unwrap();

        let m = compute_case_type_metrics(&conn, "c2").unwrap();
        assert_eq!(m["caseType"], "exploratory");
        assert_eq!(m["metrics"]["trackTransitions90d"], 2);
        assert_eq!(m["metrics"]["totalTasks"], 3);
        assert_eq!(m["metrics"]["blockedTotal"], 2);
        assert_eq!(m["metrics"]["blockedRemaining"], 1);
        assert_eq!(m["metrics"]["blockedResolved"], 1);
    }

    #[test]
    fn test_growth_metrics() {
        let conn = setup_test_db();
        conn.execute("INSERT INTO cases (id, case_type) VALUES ('c3', 'growth')", []).unwrap();
        add_task(&conn, "t1", "c3", 0, None, 0);
        // 今天、5 天前、40 天前（超出 30 天窗口）各一条事件
        conn.execute("INSERT INTO task_events (id, task_id, event_type, occurred_at) VALUES ('e1', 't1', 'created', datetime('now','localtime'))", []).unwrap();
        conn.execute("INSERT INTO task_events (id, task_id, event_type, occurred_at) VALUES ('e2', 't1', 'moved', datetime('now','localtime','-5 days'))", []).unwrap();
        conn.execute("INSERT INTO task_events (id, task_id, event_type, occurred_at) VALUES ('e3', 't1', 'moved', datetime('now','localtime','-40 days'))", []).unwrap();

        let m = compute_case_type_metrics(&conn, "c3").unwrap();
        assert_eq!(m["caseType"], "growth");
        assert_eq!(m["metrics"]["activeDays30d"], 2);
        assert_eq!(m["metrics"]["inactiveStreakDays"], 0); // 今天有活动
    }

    #[test]
    fn test_generic_metrics_when_case_type_null() {
        let conn = setup_test_db();
        conn.execute("INSERT INTO cases (id, case_type) VALUES ('c4', NULL)", []).unwrap();
        add_task(&conn, "t1", "c4", 1, None, 0);
        add_task(&conn, "t2", "c4", 0, Some("2000-01-01"), 0);
        add_task(&conn, "t3", "c4", 0, None, 0);

        let m = compute_case_type_metrics(&conn, "c4").unwrap();
        assert_eq!(m["caseType"], "generic");
        assert_eq!(m["metrics"]["totalTasks"], 3);
        assert_eq!(m["metrics"]["completedTasks"], 1);
        assert_eq!(m["metrics"]["overdueCount"], 1);
        let rate = m["metrics"]["completionRate"].as_f64().unwrap();
        assert!((rate - 1.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn test_case_not_found() {
        let conn = setup_test_db();
        assert!(compute_case_type_metrics(&conn, "no-such").is_err());
    }
}
