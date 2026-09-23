//! 应用内通知中心（W2 · Linear 式 Inbox-Zero：处理即消失， dismissed_at 非空即隐藏）
use rusqlite::params;
use serde::Serialize;

use super::run_blocking;
use crate::db;

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppNotification {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    pub body: Option<String>,
    pub payload_json: Option<String>,
    pub created_at: Option<String>,
    pub read_at: Option<String>,
}

fn row_to_notification(row: &rusqlite::Row) -> rusqlite::Result<AppNotification> {
    Ok(AppNotification {
        id: row.get("id")?,
        kind: row.get("type")?,
        title: row.get("title")?,
        body: row.get("body")?,
        payload_json: row.get("payload_json")?,
        created_at: row.get("created_at")?,
        read_at: row.get("read_at")?,
    })
}

const SELECT_ACTIVE: &str = "SELECT id, type, title, body, payload_json, created_at, read_at
     FROM notifications WHERE dismissed_at IS NULL";

/// 列出通知中心活跃通知（未 dismissed），新的在前
#[tauri::command]
pub async fn list_notifications() -> Result<Vec<AppNotification>, String> {
    run_blocking(|| {
        let conn = db::open_db()?;
        if let Err(error) = super::rest_day_reminders::refresh(&conn, chrono::Local::now().date_naive()) {
            log::warn!("休息日提醒检查失败: {error}");
        }
        let mut stmt = conn.prepare(&format!("{SELECT_ACTIVE} ORDER BY created_at DESC"))?;
        let rows = stmt.query_map([], row_to_notification)?;
        Ok(rows.filter_map(|r| r.ok()).collect::<Vec<_>>())
    })
    .await
}

/// 未读数量（顶栏铃铛角标）
#[tauri::command]
pub async fn unread_notification_count() -> Result<i64, String> {
    run_blocking(|| {
        let conn = db::open_db()?;
        if let Err(error) = super::rest_day_reminders::refresh(&conn, chrono::Local::now().date_naive()) {
            log::warn!("休息日提醒检查失败: {error}");
        }
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM notifications WHERE dismissed_at IS NULL AND read_at IS NULL",
            [],
            |r| r.get(0),
        )?;
        Ok(n)
    })
    .await
}

/// 写入一条通知（供 reminder / OCR / smart rules 等内部复用，也暴露给前端测试）
#[tauri::command]
pub async fn create_notification(
    kind: String,
    title: String,
    body: Option<String>,
    payload_json: Option<String>,
) -> Result<String, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let id = db::new_id();
        conn.execute(
            "INSERT INTO notifications (id, type, title, body, payload_json) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, kind, title, body, payload_json],
        )?;
        Ok(id)
    })
    .await
}

#[tauri::command]
pub async fn mark_notification_read(id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        conn.execute(
            "UPDATE notifications SET read_at = datetime('now','localtime') WHERE id = ?1 AND read_at IS NULL",
            params![id],
        )?;
        Ok(())
    })
    .await
}

/// Inbox-Zero 核心：处理一条即消失一条
#[tauri::command]
pub async fn dismiss_notification(id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        conn.execute(
            "UPDATE notifications SET dismissed_at = datetime('now','localtime') WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn dismiss_all_notifications() -> Result<i64, String> {
    run_blocking(|| {
        let conn = db::open_db()?;
        let n = conn.execute(
            "UPDATE notifications SET dismissed_at = datetime('now','localtime') WHERE dismissed_at IS NULL",
            [],
        )?;
        Ok(n as i64)
    })
    .await
}
