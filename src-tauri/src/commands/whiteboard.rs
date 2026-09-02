//! 事实白板（W7 · LiquidText 式事实节点网络）
//! 节点以 file_id + page 锚定出处；excerpt 为摘录原文；x/y 持久化自由布局。
use rusqlite::params;
use serde::Serialize;

use super::run_blocking;
use crate::db;

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WhiteboardDto {
    pub id: String,
    pub case_id: String,
    pub name: String,
    pub node_count: i64,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FactNodeDto {
    pub id: String,
    pub whiteboard_id: String,
    pub file_id: Option<String>,
    pub file_name: Option<String>,
    pub page: Option<i64>,
    pub excerpt: String,
    pub note: Option<String>,
    pub x: f64,
    pub y: f64,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[tauri::command]
pub async fn list_whiteboards(case_id: String) -> Result<Vec<WhiteboardDto>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT w.id, w.case_id, w.name, w.created_at, w.updated_at,
                    (SELECT COUNT(*) FROM fact_nodes fn WHERE fn.whiteboard_id = w.id) AS node_count
             FROM whiteboards w WHERE w.case_id = ?1 ORDER BY w.updated_at DESC",
        )?;
        let rows = stmt.query_map(params![case_id], |row| {
            Ok(WhiteboardDto {
                id: row.get("id")?,
                case_id: row.get("case_id")?,
                name: row.get("name")?,
                node_count: row.get("node_count")?,
                created_at: row.get("created_at")?,
                updated_at: row.get("updated_at")?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect::<Vec<_>>())
    })
    .await
}

#[tauri::command]
pub async fn create_whiteboard(case_id: String, name: String) -> Result<String, String> {
    run_blocking(move || {
        if name.trim().is_empty() {
            return Err(anyhow::anyhow!("白板名称不能为空"));
        }
        let conn = db::open_db()?;
        let id = db::new_id();
        conn.execute(
            "INSERT INTO whiteboards (id, case_id, name) VALUES (?1, ?2, ?3)",
            params![id, case_id, name],
        )?;
        Ok(id)
    })
    .await
}

#[tauri::command]
pub async fn rename_whiteboard(id: String, name: String) -> Result<(), String> {
    run_blocking(move || {
        if name.trim().is_empty() {
            return Err(anyhow::anyhow!("白板名称不能为空"));
        }
        let conn = db::open_db()?;
        let n = conn.execute(
            "UPDATE whiteboards SET name = ?2, updated_at = datetime('now','localtime') WHERE id = ?1",
            params![id, name],
        )?;
        if n == 0 {
            return Err(anyhow::anyhow!("白板不存在: {id}"));
        }
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn delete_whiteboard(id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        conn.execute("DELETE FROM whiteboards WHERE id = ?1", params![id])?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn list_fact_nodes(whiteboard_id: String) -> Result<Vec<FactNodeDto>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT fn.id, fn.whiteboard_id, fn.file_id, cf.file_name, fn.page, fn.excerpt,
                    fn.note, fn.x, fn.y, fn.created_at, fn.updated_at
             FROM fact_nodes fn
             LEFT JOIN case_files cf ON cf.id = fn.file_id
             WHERE fn.whiteboard_id = ?1
             ORDER BY fn.created_at ASC",
        )?;
        let rows = stmt.query_map(params![whiteboard_id], |row| {
            Ok(FactNodeDto {
                id: row.get("id")?,
                whiteboard_id: row.get("whiteboard_id")?,
                file_id: row.get("file_id")?,
                file_name: row.get("file_name")?,
                page: row.get("page")?,
                excerpt: row.get("excerpt")?,
                note: row.get("note")?,
                x: row.get("x")?,
                y: row.get("y")?,
                created_at: row.get("created_at")?,
                updated_at: row.get("updated_at")?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect::<Vec<_>>())
    })
    .await
}

#[tauri::command]
pub async fn create_fact_node(
    whiteboard_id: String,
    file_id: Option<String>,
    page: Option<i64>,
    excerpt: String,
    note: Option<String>,
    x: f64,
    y: f64,
) -> Result<String, String> {
    run_blocking(move || {
        if excerpt.trim().is_empty() {
            return Err(anyhow::anyhow!("摘录内容不能为空"));
        }
        let conn = db::open_db()?;
        let id = db::new_id();
        conn.execute(
            "INSERT INTO fact_nodes (id, whiteboard_id, file_id, page, excerpt, note, x, y)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, whiteboard_id, file_id, page, excerpt, note, x, y],
        )?;
        // 触碰白板更新时间用于排序
        conn.execute(
            "UPDATE whiteboards SET updated_at = datetime('now','localtime') WHERE id = ?1",
            params![whiteboard_id],
        )?;
        Ok(id)
    })
    .await
}

/// 更新节点（坐标拖拽 / 批注 / 页码）；None 字段保持不变
#[tauri::command]
pub async fn update_fact_node(
    id: String,
    page: Option<i64>,
    excerpt: Option<String>,
    note: Option<String>,
    x: Option<f64>,
    y: Option<f64>,
) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        // 位置与内容分开更新，避免三态复杂度：坐标总是成对出现
        if let (Some(nx), Some(ny)) = (x, y) {
            conn.execute(
                "UPDATE fact_nodes SET x = ?2, y = ?3, updated_at = datetime('now','localtime') WHERE id = ?1",
                params![id, nx, ny],
            )?;
        }
        if let Some(e) = excerpt {
            conn.execute(
                "UPDATE fact_nodes SET excerpt = ?2, updated_at = datetime('now','localtime') WHERE id = ?1",
                params![id, e],
            )?;
        }
        if let Some(n) = note {
            conn.execute(
                "UPDATE fact_nodes SET note = ?2, updated_at = datetime('now','localtime') WHERE id = ?1",
                params![id, n],
            )?;
        }
        if let Some(p) = page {
            conn.execute(
                "UPDATE fact_nodes SET page = ?2, updated_at = datetime('now','localtime') WHERE id = ?1",
                params![id, p],
            )?;
        }
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn delete_fact_node(id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        conn.execute("DELETE FROM fact_nodes WHERE id = ?1", params![id])?;
        Ok(())
    })
    .await
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WhiteboardEdgeDto {
    pub id: String,
    pub whiteboard_id: String,
    pub source_node_id: String,
    pub target_node_id: String,
    pub created_at: Option<String>,
}

#[tauri::command]
pub async fn list_whiteboard_edges(whiteboard_id: String) -> Result<Vec<WhiteboardEdgeDto>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT id, whiteboard_id, source_node_id, target_node_id, created_at
             FROM whiteboard_edges WHERE whiteboard_id = ?1 ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(params![whiteboard_id], |row| {
            Ok(WhiteboardEdgeDto {
                id: row.get("id")?,
                whiteboard_id: row.get("whiteboard_id")?,
                source_node_id: row.get("source_node_id")?,
                target_node_id: row.get("target_node_id")?,
                created_at: row.get("created_at")?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect::<Vec<_>>())
    })
    .await
}

#[tauri::command]
pub async fn create_whiteboard_edge(
    whiteboard_id: String,
    source_node_id: String,
    target_node_id: String,
) -> Result<String, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let id = db::new_id();
        conn.execute(
            "INSERT INTO whiteboard_edges (id, whiteboard_id, source_node_id, target_node_id)
             VALUES (?1, ?2, ?3, ?4)",
            params![id, whiteboard_id, source_node_id, target_node_id],
        )?;
        // touch whiteboard
        conn.execute(
            "UPDATE whiteboards SET updated_at = datetime('now','localtime') WHERE id = ?1",
            params![whiteboard_id],
        )?;
        Ok(id)
    })
    .await
}

#[tauri::command]
pub async fn delete_whiteboard_edge(id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        conn.execute("DELETE FROM whiteboard_edges WHERE id = ?1", params![id])?;
        Ok(())
    })
    .await
}
