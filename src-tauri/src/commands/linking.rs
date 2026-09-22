//! 通用跨模块双链（W4 · Hookmark/Obsidian 式）
//!
//! 铁律：链接一律以内部 ID 引用目标（case_files.id / knowledge_items.id / tasks.id …），
//! 操作系统层重命名/移动文件不会断链。anchor 承载页码等定位信息（如 "page:12"）。
use rusqlite::params;
use serde::Serialize;

use super::run_blocking;
use crate::db;

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LinkDto {
    pub id: String,
    pub source_type: String,
    pub source_id: String,
    pub target_type: String,
    pub target_id: String,
    pub anchor: Option<String>,
    pub label: Option<String>,
    pub created_at: Option<String>,
    /// 目标展示名（JOIN 解析，尽力而为；解析不到为 None）
    pub target_title: Option<String>,
}

const VALID_TYPES: [&str; 5] = ["doc", "knowledge", "task", "case", "file"];

fn entity_exists(conn: &rusqlite::Connection, kind: &str, id: &str) -> anyhow::Result<bool> {
    let sql = match kind {
        "doc" => "SELECT EXISTS(SELECT 1 FROM drafts WHERE id=?1)",
        "knowledge" => "SELECT EXISTS(SELECT 1 FROM knowledge_items WHERE id=?1)",
        "task" => "SELECT EXISTS(SELECT 1 FROM tasks WHERE id=?1 AND deleted_at IS NULL)",
        "case" => "SELECT EXISTS(SELECT 1 FROM cases WHERE id=?1)",
        "file" => "SELECT EXISTS(SELECT 1 FROM case_files WHERE id=?1 AND deleted_at IS NULL)",
        _ => anyhow::bail!("无效的链接类型: {kind}"),
    };
    Ok(conn.query_row(sql, params![id], |r| r.get::<_, bool>(0))?)
}

fn row_to_link(row: &rusqlite::Row) -> rusqlite::Result<LinkDto> {
    Ok(LinkDto {
        id: row.get("id")?,
        source_type: row.get("source_type")?,
        source_id: row.get("source_id")?,
        target_type: row.get("target_type")?,
        target_id: row.get("target_id")?,
        anchor: row.get("anchor")?,
        label: row.get("label")?,
        created_at: row.get("created_at")?,
        target_title: None,
    })
}

/// 尽力解析目标展示名（按 target_type 查对应表）
fn resolve_title(conn: &rusqlite::Connection, link: &mut LinkDto) {
    let sql = match link.target_type.as_str() {
        "file" => Some("SELECT file_name FROM case_files WHERE id = ?1 AND deleted_at IS NULL"),
        "knowledge" => Some("SELECT title FROM knowledge_items WHERE id = ?1"),
        "task" => Some("SELECT task_name FROM tasks WHERE id = ?1 AND deleted_at IS NULL"),
        "case" => Some("SELECT case_name FROM cases WHERE id = ?1"),
        _ => None,
    };
    if let Some(sql) = sql {
        link.target_title = conn
            .query_row(sql, params![link.target_id], |r| r.get::<_, String>(0))
            .ok();
    }
}

#[tauri::command]
pub async fn create_link(
    source_type: String,
    source_id: String,
    target_type: String,
    target_id: String,
    anchor: Option<String>,
    label: Option<String>,
) -> Result<LinkDto, String> {
    run_blocking(move || {
        if !VALID_TYPES.contains(&source_type.as_str()) {
            return Err(anyhow::anyhow!("无效的 source_type: {source_type}"));
        }
        if !VALID_TYPES.contains(&target_type.as_str()) {
            return Err(anyhow::anyhow!("无效的 target_type: {target_type}"));
        }
        let mut raw = db::open_db()?;
        let conn = raw.transaction()?;
        anyhow::ensure!(entity_exists(&conn, &source_type, &source_id)?, "链接源不存在或已删除");
        anyhow::ensure!(entity_exists(&conn, &target_type, &target_id)?, "链接目标不存在或已删除");
        let id = db::new_id();
        conn.execute(
            "INSERT INTO links (id, source_type, source_id, target_type, target_id, anchor, label)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id,
                source_type,
                source_id,
                target_type,
                target_id,
                anchor,
                label
            ],
        )?;
        let mut stmt = conn.prepare(
            "SELECT id, source_type, source_id, target_type, target_id, anchor, label, created_at
             FROM links WHERE id = ?1",
        )?;
        let mut link = stmt.query_row(params![id], row_to_link)?;
        drop(stmt);
        resolve_title(&conn, &mut link);
        conn.commit()?;
        Ok(link)
    })
    .await
}

#[tauri::command]
pub async fn remove_link(id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        conn.execute("DELETE FROM links WHERE id = ?1", params![id])?;
        Ok(())
    })
    .await
}

/// 某实体的出链（它引用了谁）
#[tauri::command]
pub async fn list_links_for(
    source_type: String,
    source_id: String,
) -> Result<Vec<LinkDto>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT id, source_type, source_id, target_type, target_id, anchor, label, created_at
             FROM links WHERE source_type = ?1 AND source_id = ?2 ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map(params![source_type, source_id], row_to_link)?;
        let mut out: Vec<LinkDto> = rows.filter_map(|r| r.ok()).collect();
        for l in out.iter_mut() {
            resolve_title(&conn, l);
        }
        Ok(out)
    })
    .await
}

/// 某实体的反链（谁引用了它）——知识/文件详情的「被引用于」面板
#[tauri::command]
pub async fn get_backlinks(target_type: String, target_id: String) -> Result<Vec<LinkDto>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT id, source_type, source_id, target_type, target_id, anchor, label, created_at
             FROM links WHERE target_type = ?1 AND target_id = ?2 ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map(params![target_type, target_id], row_to_link)?;
        let mut out: Vec<LinkDto> = rows.filter_map(|r| r.ok()).collect();
        for l in out.iter_mut() {
            resolve_title(&conn, l);
        }
        Ok(out)
    })
    .await
}
