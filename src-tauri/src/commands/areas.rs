use super::run_blocking;
use crate::db;

#[tauri::command]
pub async fn list_areas() -> Result<Vec<serde_json::Value>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, description, icon, sort_order, created_at, updated_at 
             FROM areas ORDER BY sort_order ASC, name ASC"
        )?;
        
        let areas: Vec<serde_json::Value> = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, String>("id")?,
                    "name": row.get::<_, String>("name")?,
                    "description": row.get::<_, Option<String>>("description")?,
                    "icon": row.get::<_, Option<String>>("icon")?,
                    "sortOrder": row.get::<_, i32>("sort_order")?,
                    "createdAt": row.get::<_, String>("created_at")?,
                    "updatedAt": row.get::<_, String>("updated_at")?,
                }))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        
        Ok(areas)
    })
    .await
}

#[tauri::command]
pub async fn get_area(id: String) -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let area = conn.query_row(
            "SELECT id, name, description, icon, sort_order, created_at, updated_at 
             FROM areas WHERE id = ?1",
            rusqlite::params![id],
            |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, String>("id")?,
                    "name": row.get::<_, String>("name")?,
                    "description": row.get::<_, Option<String>>("description")?,
                    "icon": row.get::<_, Option<String>>("icon")?,
                    "sortOrder": row.get::<_, i32>("sort_order")?,
                    "createdAt": row.get::<_, String>("created_at")?,
                    "updatedAt": row.get::<_, String>("updated_at")?,
                }))
            },
        ).map_err(|e| anyhow::anyhow!(e))?;
        
        Ok(area)
    })
    .await
}

#[tauri::command]
pub async fn create_area(data: serde_json::Value) -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let id = db::new_id();
        let now = db::now_local();
        
        conn.execute(
            "INSERT INTO areas (id, name, description, icon, sort_order, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                id,
                data["name"].as_str().ok_or_else(|| anyhow::anyhow!("Missing area name"))?,
                data["description"].as_str(),
                data["icon"].as_str(),
                data["sortOrder"].as_i64().unwrap_or(0),
                now,
                now,
            ],
        )?;
        
        Ok(serde_json::json!({ "id": id }))
    })
    .await
}

#[tauri::command]
pub async fn update_area(id: String, data: serde_json::Value) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let now = db::now_local();
        
        conn.execute(
            "UPDATE areas SET 
                name = COALESCE(?1, name),
                description = ?2,
                icon = ?3,
                sort_order = COALESCE(?4, sort_order),
                updated_at = ?5
             WHERE id = ?6",
            rusqlite::params![
                data["name"].as_str(),
                data["description"].as_str(),
                data["icon"].as_str(),
                data["sortOrder"].as_i64(),
                now,
                id,
            ],
        )?;
        
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn delete_area(id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        
        // 检查是否有任务关联到此领域
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE area_id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        )?;
        
        if count > 0 {
            return Err(anyhow::anyhow!("该领域下有 {} 个任务，无法删除", count));
        }
        
        conn.execute("DELETE FROM areas WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn get_area_stats(id: String) -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        
        // 获取领域信息
        let area_name: String = conn.query_row(
            "SELECT name FROM areas WHERE id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        ).map_err(|e| anyhow::anyhow!(e))?;
        let total_tasks: i32 = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE area_id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        )?;
        
        let completed_tasks: i32 = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE area_id = ?1 AND completed = 1",
            rusqlite::params![id],
            |row| row.get(0),
        )?;
        
        let pending_tasks: i32 = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE area_id = ?1 AND completed = 0",
            rusqlite::params![id],
            |row| row.get(0),
        )?;
        
        // 统计案件
        let total_cases: i32 = conn.query_row(
            "SELECT COUNT(*) FROM cases WHERE area_id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        )?;
        
        Ok(serde_json::json!({
            "areaId": id,
            "areaName": area_name,
            "totalTasks": total_tasks,
            "completedTasks": completed_tasks,
            "pendingTasks": pending_tasks,
            "totalCases": total_cases,
        }))
    })
    .await
}
