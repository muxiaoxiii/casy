//! projects CRUD（A1-1/D-9 垂直拆表 · 阶段一绞杀式）
//!
//! - kind='personal'：直写本表（个人项目，无法律列）
//! - kind='legal'：由 cases 经触发器镜像；本阶段法律流仍以 cases 为事实源，
//!   因此 legal 行禁止在此删除/改名（走案件管理路径），只读透出
use super::run_blocking;
use crate::db;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRow {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub description: Option<String>,
    pub status: String,
    pub area_id: Option<String>,
    pub color: Option<String>,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[tauri::command]
pub async fn list_projects(query: Option<String>) -> Result<Vec<ProjectRow>, String> {
    run_blocking(move || {
        db::with_conn(|conn| {
            let like = query
                .as_deref()
                .map(|q| format!("%{}%", q.trim().replace('%', "")));
            let mut sql = String::from(
                "SELECT id, name, kind, description, status, area_id, color, sort_order, created_at, updated_at
                 FROM projects",
            );
            if like.is_some() {
                sql.push_str(" WHERE name LIKE ?1");
            }
            sql.push_str(" ORDER BY kind ASC, sort_order ASC, updated_at DESC");
            let mut stmt = conn.prepare(&sql)?;
            let map = |r: &rusqlite::Row| -> rusqlite::Result<ProjectRow> {
                Ok(ProjectRow {
                    id: r.get("id")?,
                    name: r.get("name")?,
                    kind: r.get("kind")?,
                    description: r.get("description")?,
                    status: r.get("status")?,
                    area_id: r.get("area_id")?,
                    color: r.get("color")?,
                    sort_order: r.get("sort_order")?,
                    created_at: r.get("created_at")?,
                    updated_at: r.get("updated_at")?,
                })
            };
            let rows = match &like {
                Some(l) => stmt.query_map(rusqlite::params![l], map)?.collect::<std::result::Result<Vec<_>, _>>()?,
                None => stmt.query_map([], map)?.collect::<std::result::Result<Vec<_>, _>>()?,
            };
            Ok(rows)
        })
    })
    .await
}

/// 创建个人项目。kind 恒为 'personal'——legal 项目只能经案件创建流程产生
/// （触发器自动镜像到本表），避免双写分歧。
#[tauri::command]
pub async fn create_personal_project(data: serde_json::Value) -> Result<ProjectRow, String> {
    run_blocking(move || {
        db::with_conn(|conn| {
            let name = data["name"]
                .as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| anyhow::anyhow!(crate::error_code::err(
                    crate::error_code::codes::PROJECT_NAME_REQUIRED,
                    "项目名称不能为空",
                )))?;
            let id = db::new_id();
            let now = db::now_local();

            conn.execute(
                "INSERT INTO projects (id, name, kind, description, status, area_id, color, created_at, updated_at)
                 VALUES (?1, ?2, 'personal', ?3, 'active', ?4, ?5, ?6, ?6)",
                rusqlite::params![
                    id,
                    name,
                    data["description"].as_str(),
                    data["areaId"].as_str(),
                    data["color"].as_str(),
                    now,
                ],
            )?;

            let p = conn.query_row(
                "SELECT id, name, kind, description, status, area_id, color, sort_order, created_at, updated_at
                 FROM projects WHERE id = ?1",
                rusqlite::params![id],
                |r| {
                    Ok(ProjectRow {
                        id: r.get("id")?,
                        name: r.get("name")?,
                        kind: r.get("kind")?,
                        description: r.get("description")?,
                        status: r.get("status")?,
                        area_id: r.get("area_id")?,
                        color: r.get("color")?,
                        sort_order: r.get("sort_order")?,
                        created_at: r.get("created_at")?,
                        updated_at: r.get("updated_at")?,
                    })
                },
            )?;
            Ok(p)
        })
    })
    .await
}

/// 更新个人项目的通用字段。legal 行拒绝在此修改（名称/状态归案件管理管）
#[tauri::command]
pub async fn update_personal_project(id: String, data: serde_json::Value) -> Result<(), String> {
    run_blocking(move || {
        db::with_conn(|conn| {
            let kind: String = conn
                .query_row("SELECT kind FROM projects WHERE id = ?1", rusqlite::params![id], |r| r.get(0))
                .map_err(|_| {
                    anyhow::anyhow!(crate::error_code::err(
                        crate::error_code::codes::PROJECT_NOT_FOUND,
                        "项目不存在",
                    ))
                })?;
            if kind != "personal" {
                return Err(anyhow::anyhow!(crate::error_code::err(
                    crate::error_code::codes::PROJECT_LEGAL_READONLY,
                    "法律项目请在案件管理中维护",
                )));
            }
            let changed = conn.execute(
                "UPDATE projects SET
                    name = COALESCE(?1, name),
                    description = ?2,
                    status = COALESCE(?3, status),
                    area_id = ?4,
                    color = COALESCE(?5, color),
                    updated_at = ?6
                 WHERE id = ?7",
                rusqlite::params![
                    data["name"].as_str(),
                    data["description"].as_str(),
                    data["status"].as_str(),
                    data["areaId"].as_str(),
                    data["color"].as_str(),
                    db::now_local(),
                    id,
                ],
            )?;
            if changed == 0 {
                return Err(anyhow::anyhow!(crate::error_code::err(
                    crate::error_code::codes::PROJECT_NOT_FOUND,
                    "项目不存在",
                )));
            }
            Ok(())
        })
    })
    .await
}

/// 删除个人项目。legal 行拒绝（删除必须走案件管理，保证级联与审计完整）
#[tauri::command]
pub async fn delete_project(id: String) -> Result<(), String> {
    run_blocking(move || {
        db::with_conn(|conn| {
            let (kind, name): (String, String) = conn
                .query_row(
                    "SELECT kind, name FROM projects WHERE id = ?1",
                    rusqlite::params![id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .map_err(|_| {
                    anyhow::anyhow!(crate::error_code::err(
                        crate::error_code::codes::PROJECT_NOT_FOUND,
                        "项目不存在",
                    ))
                })?;
            if kind != "personal" {
                return Err(anyhow::anyhow!(crate::error_code::err(
                    crate::error_code::codes::PROJECT_LEGAL_READONLY,
                    "法律项目请在案件管理中删除",
                )));
            }
            let linked: i64 = conn.query_row(
                "SELECT COUNT(*) FROM tasks WHERE case_id = ?1 AND completed = 0",
                rusqlite::params![id],
                |r| r.get(0),
            )?;
            if linked > 0 {
                return Err(anyhow::anyhow!(crate::error_code::err(
                    crate::error_code::codes::PROJECT_HAS_ACTIVE_TASKS,
                    format!("「{name}」下仍有 {linked} 个未完成任务，请先处理"),
                )));
            }
            conn.execute("DELETE FROM projects WHERE id = ?1", rusqlite::params![id])?;
            Ok(())
        })
    })
    .await
}
