use super::run_blocking;
use crate::db;

// ============================================================
// B1 DomainCommand 样板：强类型输入/输出 + specta 导出
// （本域为全域类型化参考实现；空值语义见各结构体注释）
// ============================================================

/// 领域条目
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AreaDto {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
}

/// 领域统计
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AreaStatsDto {
    pub area_id: String,
    pub area_name: String,
    pub total_tasks: i32,
    pub completed_tasks: i32,
    pub pending_tasks: i32,
    pub total_cases: i32,
}

/// 新建领域输入
#[derive(Debug, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CreateAreaInput {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub sort_order: i32,
}

/// 更新领域输入（⚠️ A1-2 语义：description/icon 直接赋值——None 即清空；
/// name/sort_order 为 COALESCE 保留语义。前端须整组提交。）
#[derive(Debug, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", default)]
#[derive(Default)]
pub struct UpdateAreaInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub sort_order: Option<i32>,
}

/// 新建结果（保持原 {id} 形状，消费方零改动）
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CreateAreaOutput {
    pub id: String,
}

#[tauri::command]
pub async fn list_areas() -> Result<Vec<AreaDto>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, description, icon, sort_order, created_at, updated_at 
             FROM areas ORDER BY sort_order ASC, name ASC",
        )?;

        let areas: Vec<AreaDto> = stmt
            .query_map([], |row| {
                Ok(AreaDto {
                    id: row.get::<_, String>("id")?,
                    name: row.get::<_, String>("name")?,
                    description: row.get::<_, Option<String>>("description")?,
                    icon: row.get::<_, Option<String>>("icon")?,
                    sort_order: row.get::<_, i32>("sort_order")?,
                    created_at: row.get::<_, String>("created_at")?,
                    updated_at: row.get::<_, String>("updated_at")?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(areas)
    })
    .await
}

#[tauri::command]
pub async fn get_area(id: String) -> Result<AreaDto, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let area = conn
            .query_row(
                "SELECT id, name, description, icon, sort_order, created_at, updated_at 
             FROM areas WHERE id = ?1",
                rusqlite::params![id],
                |row| {
                    Ok(AreaDto {
                        id: row.get::<_, String>("id")?,
                        name: row.get::<_, String>("name")?,
                        description: row.get::<_, Option<String>>("description")?,
                        icon: row.get::<_, Option<String>>("icon")?,
                        sort_order: row.get::<_, i32>("sort_order")?,
                        created_at: row.get::<_, String>("created_at")?,
                        updated_at: row.get::<_, String>("updated_at")?,
                    })
                },
            )
            .map_err(|e| anyhow::anyhow!(e))?;

        Ok(area)
    })
    .await
}

#[tauri::command]
pub async fn create_area(data: CreateAreaInput) -> Result<CreateAreaOutput, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let id = db::new_id();
        let now = db::now_local();

        if data.name.is_empty() {
            return Err(anyhow::anyhow!(crate::error_code::err(
                crate::error_code::codes::AREA_NAME_REQUIRED,
                "领域名称不能为空",
            )));
        }

        conn.execute(
            "INSERT INTO areas (id, name, description, icon, sort_order, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                id,
                data.name.as_str(),
                data.description.as_deref(),
                data.icon.as_deref(),
                data.sort_order,
                now,
                now,
            ],
        )?;

        Ok(CreateAreaOutput { id })
    })
    .await
}

#[tauri::command]
pub async fn update_area(id: String, data: UpdateAreaInput) -> Result<(), String> {
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
                data.name.as_deref(),
                data.description.as_deref(),
                data.icon.as_deref(),
                data.sort_order,
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
pub async fn get_area_stats(id: String) -> Result<AreaStatsDto, String> {
    run_blocking(move || {
        let conn = db::open_db()?;

        // 获取领域信息
        let area_name: String = conn
            .query_row(
                "SELECT name FROM areas WHERE id = ?1",
                rusqlite::params![id],
                |row| row.get(0),
            )
            .map_err(|e| anyhow::anyhow!(e))?;
        let total_tasks: i32 = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE area_id = ?1 AND deleted_at IS NULL",
            rusqlite::params![id],
            |row| row.get(0),
        )?;

        let completed_tasks: i32 = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE area_id = ?1 AND completed = 1 AND deleted_at IS NULL",
            rusqlite::params![id],
            |row| row.get(0),
        )?;

        let pending_tasks: i32 = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE area_id = ?1 AND completed = 0 AND deleted_at IS NULL",
            rusqlite::params![id],
            |row| row.get(0),
        )?;

        // 统计案件
        let total_cases: i32 = conn.query_row(
            "SELECT COUNT(*) FROM cases WHERE area_id = ?1",
            rusqlite::params![id],
            |row| row.get(0),
        )?;

        Ok(AreaStatsDto {
            area_id: id,
            area_name,
            total_tasks,
            completed_tasks,
            pending_tasks,
            total_cases,
        })
    })
    .await
}
