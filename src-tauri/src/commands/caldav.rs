//! CalDAV 日历同步命令（设计哲学 §11.2 M1）
//!
//! - test_caldav_connection:      测试 CalDAV 连接（OPTIONS + PROPFIND）
//! - sync_reminders_to_calendar:  手动补同步 pending/sync_failed/delivery_unknown 作业
//! - get_calendar_sync_status:    同步状态统计
//!
//! 作业执行（execute_calendar_job）由提醒引擎 dispatch 路径与手动补同步共用。

use super::run_blocking;
use crate::db;
use crate::sync::caldav::{self, CalDavClient};
use anyhow::Result;
use chrono::{NaiveDate, NaiveDateTime};
use rusqlite::params;
use serde::Serialize;

/// 单个日历同步作业的载荷
pub struct CalendarJobPayload {
    /// reminder_jobs.id
    pub job_id: String,
    /// ICS UID（= job_id，不可变）
    pub uid: String,
    /// 脱敏标题
    pub summary: String,
    pub description: String,
    pub dtstart: NaiveDateTime,
    pub alarm_minutes: i64,
    /// 同步失败时回退本地提醒的完整消息
    pub fallback_message: String,
}

/// R 等级 → VALARM 提前分钟数（事件定于截止日 09:00）
///
/// R1 温和：当天 09:00 → 0 分钟（事件开始时提醒）
/// R2 明确：提前 1 天 → 1440 分钟
/// R3 强提醒：当天 09:00 → 0 分钟
/// R4 逾期：立即 → 0 分钟（日历客户端收到即弹）
pub(crate) fn alarm_minutes_for_level(level: &str) -> i64 {
    match level {
        "R2" => 1440,
        _ => 0,
    }
}

/// 截止日期 → 当天 09:00（本地浮动时间）
pub(crate) fn parse_due_morning(due_date: &str) -> Option<NaiveDateTime> {
    NaiveDate::parse_from_str(due_date, "%Y-%m-%d")
        .ok()?
        .and_hms_opt(9, 0, 0)
}

/// 打开 CalDAV 客户端（配置缺失返回 Err）
fn open_client() -> Result<CalDavClient> {
    let conn = db::open_db()?;
    let config = caldav::load_caldav_config(&conn)?
        .ok_or_else(|| anyhow::anyhow!("未配置 CalDAV（caldav_url / caldav_user / caldav_pass）"))?;
    CalDavClient::from_config(&config)
}

/// 更新作业状态（失败只记日志，不 panic）
fn update_job_status(job_id: &str, status: &str, etag: Option<&str>, error: Option<&str>) {
    let apply = || -> Result<()> {
        let conn = db::open_db()?;
        if status == "synced" {
            conn.execute(
                "UPDATE reminder_jobs
                 SET status='synced', calendar_event_id=?2, calendar_etag=?3,
                     last_error=NULL, attempts=attempts+1
                 WHERE id=?1",
                params![job_id, job_id, etag],
            )?;
        } else {
            conn.execute(
                "UPDATE reminder_jobs
                 SET status=?2, last_error=?3, attempts=attempts+1
                 WHERE id=?1",
                params![job_id, status, error],
            )?;
        }
        Ok(())
    };
    if let Err(e) = apply() {
        log::error!("[日历同步] 更新作业 {} 状态失败: {}", job_id, e);
    }
}

/// 执行单个日历同步作业
///
/// 幂等：PUT 同 UID 覆盖更新。
/// delivery_unknown 语义：结果不明（超时/网络错误）时不盲目重 PUT，
/// 先 get_event_etag 对账确认存在性。
/// 返回最终状态（"synced" / "sync_failed" / "delivery_unknown"）。
pub async fn execute_calendar_job(p: CalendarJobPayload) -> &'static str {
    let result = match open_client() {
        Ok(client) => {
            client
                .upsert_event(&p.uid, &p.summary, &p.description, p.dtstart, 30, p.alarm_minutes)
                .await
        }
        Err(e) => Err(caldav::CalDavError::Client(0, e.to_string())),
    };

    match result {
        Ok(etag) => {
            update_job_status(&p.job_id, "synced", etag.as_deref(), None);
            log::info!("[日历同步] 作业 {} 已同步（etag: {:?}）", p.job_id, etag);
            "synced"
        }
        Err(e) if e.is_delivery_unknown() => {
            // 结果不明：先对账，确认事件是否已写入
            let found = match open_client() {
                Ok(client) => client.get_event_etag(&p.uid).await.ok().flatten(),
                Err(_) => None,
            };
            match found {
                Some(etag) => {
                    update_job_status(&p.job_id, "synced", Some(&etag), None);
                    log::info!("[日历同步] 作业 {} 对账确认已写入（etag: {}）", p.job_id, etag);
                    "synced"
                }
                None => {
                    update_job_status(&p.job_id, "delivery_unknown", None, Some(&e.to_string()));
                    log::warn!("[日历同步] 作业 {} 投递结果不明: {}", p.job_id, e);
                    let _ = super::reminder::send_local_notification(&p.fallback_message);
                    "delivery_unknown"
                }
            }
        }
        Err(e) => {
            update_job_status(&p.job_id, "sync_failed", None, Some(&e.to_string()));
            log::error!("[日历同步] 作业 {} 同步失败: {}", p.job_id, e);
            // 同步失败不阻塞提醒：回退本地通知（执行方互斥：仅在未 synced 时发）
            let _ = super::reminder::send_local_notification(&p.fallback_message);
            "sync_failed"
        }
    }
}

// ============================================================
// Tauri Commands
// ============================================================

/// 测试 CalDAV 连接
#[tauri::command]
pub async fn test_caldav_connection() -> Result<String, String> {
    let client = run_blocking(|| open_client()).await?;
    client.test_connection().await.map_err(|e| e.to_string())
}

/// 补同步结果
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarSyncReport {
    /// 待处理作业总数
    pub total: usize,
    /// 本轮成功同步
    pub synced: usize,
    /// 本轮失败（sync_failed / delivery_unknown）
    pub failed: usize,
    /// 超过重试上限（3 次）跳过
    pub skipped: usize,
}

/// 手动把 pending/sync_failed/delivery_unknown 的 reminder_jobs 补同步到日历
/// 重试上限 3 次/作业（attempts >= 3 跳过）
#[tauri::command]
pub async fn sync_reminders_to_calendar() -> Result<CalendarSyncReport, String> {
    let jobs = run_blocking(|| {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT id, masked_content, due_snapshot, offset_snapshot, content, attempts
             FROM reminder_jobs
             WHERE channel = 'calendar'
               AND status IN ('pending', 'sync_failed', 'delivery_unknown')
             ORDER BY scheduled_at",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    })
    .await?;

    let mut report = CalendarSyncReport {
        total: jobs.len(),
        synced: 0,
        failed: 0,
        skipped: 0,
    };

    for (job_id, masked, due, level, content, attempts) in jobs {
        if attempts >= 3 {
            report.skipped += 1;
            continue;
        }
        let Some(dtstart) = due.as_deref().and_then(parse_due_morning) else {
            update_job_status(&job_id, "sync_failed", None, Some("缺少/无法解析截止日期"));
            report.failed += 1;
            continue;
        };

        let payload = CalendarJobPayload {
            uid: job_id.clone(),
            job_id,
            summary: masked.unwrap_or_else(|| "案件提醒".to_string()),
            description: format!("截止：{}\n由 Casy 补同步", due.as_deref().unwrap_or("")),
            dtstart,
            alarm_minutes: alarm_minutes_for_level(level.as_deref().unwrap_or("R1")),
            fallback_message: content.unwrap_or_default(),
        };

        match execute_calendar_job(payload).await {
            "synced" => report.synced += 1,
            _ => report.failed += 1,
        }
    }

    log::info!(
        "[日历同步] 补同步完成：共 {}，成功 {}，失败 {}，跳过 {}",
        report.total, report.synced, report.failed, report.skipped
    );
    Ok(report)
}

// ============================================================
// 提醒作业撤销（任务完成/删除后清理日历事件）
// ============================================================

/// 按 UID 删除 CalDAV 事件（提醒撤销清理用）
pub(crate) async fn delete_calendar_event_by_uid(uid: &str) -> Result<()> {
    let client = open_client()?;
    client
        .delete_event(uid)
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    log::info!("[日历同步] 已删除日历事件 {}", uid);
    Ok(())
}

/// 撤销实体关联的提醒作业：
/// - status IN ('pending','synced','delivery_unknown') 的作业置为 'cancelled'
///   （local executor 的作业仅置 cancelled，不涉及外部副作用）
/// - 其中 executor='calendar' 且已 synced 的作业异步调用 CalDAV delete_event
///   （失败只记日志，不阻塞主流程）
///
/// 返回被撤销的作业数。供 toggle_task / delete_task 及前端显式调用复用。
pub async fn cancel_jobs_for_entity(entity_type: &str, entity_id: &str) -> Result<usize, String> {
    let et = entity_type.to_string();
    let eid = entity_id.to_string();

    let (synced_uids, cancelled) = run_blocking(move || {
        let conn = db::open_db()?;
        // 已同步到日历的作业（ICS UID = job id），撤销后需删除远端事件
        let mut stmt = conn.prepare(
            "SELECT id FROM reminder_jobs
             WHERE entity_type=?1 AND entity_id=?2 AND executor='calendar' AND status='synced'",
        )?;
        let uids: Vec<String> = stmt
            .query_map(params![et, eid], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let n = conn.execute(
            "UPDATE reminder_jobs SET status='cancelled'
             WHERE entity_type=?1 AND entity_id=?2
               AND status IN ('pending','synced','delivery_unknown')",
            params![et, eid],
        )?;
        Ok((uids, n))
    })
    .await?;

    for uid in synced_uids {
        tauri::async_runtime::spawn(async move {
            if let Err(e) = delete_calendar_event_by_uid(&uid).await {
                log::error!("[日历同步] 撤销作业时删除日历事件 {} 失败: {}", uid, e);
            }
        });
    }

    Ok(cancelled)
}

/// 撤销某实体（task/case/hearing/deadline）关联的全部未完成提醒作业
#[tauri::command]
pub async fn cancel_reminder_jobs_for(
    entity_type: String,
    entity_id: String,
) -> Result<serde_json::Value, String> {
    let cancelled = cancel_jobs_for_entity(&entity_type, &entity_id).await?;
    Ok(serde_json::json!({ "cancelledCount": cancelled }))
}

/// 获取日历同步状态
#[tauri::command]
pub async fn get_calendar_sync_status() -> Result<serde_json::Value, String> {
    run_blocking(|| {
        let conn = db::open_db()?;
        let enabled = caldav::calendar_sync_enabled(&conn);
        let configured = caldav::load_caldav_config(&conn)?.is_some();

        let count = |statuses: &[&str]| -> Result<i64> {
            let placeholders = statuses.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!(
                "SELECT COUNT(*) FROM reminder_jobs WHERE channel='calendar' AND status IN ({})",
                placeholders
            );
            let mut stmt = conn.prepare(&sql)?;
            let params: Vec<&dyn rusqlite::ToSql> =
                statuses.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
            Ok(stmt.query_row(params.as_slice(), |row| row.get(0))?)
        };

        let synced = count(&["synced"])?;
        let pending = count(&["pending"])?;
        let failed = count(&["sync_failed", "delivery_unknown"])?;

        let last_sync_at: Option<String> = conn
            .query_row(
                "SELECT MAX(updated_at) FROM reminder_jobs WHERE channel='calendar' AND status='synced'",
                [],
                |row| row.get(0),
            )
            .ok()
            .flatten();

        Ok(serde_json::json!({
            "enabled": enabled,
            "configured": configured,
            "syncedCount": synced,
            "pendingCount": pending,
            "failedCount": failed,
            "lastSyncAt": last_sync_at,
        }))
    })
    .await
}
