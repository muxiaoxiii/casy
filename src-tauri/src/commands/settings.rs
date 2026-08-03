use std::collections::HashMap;

use super::run_blocking;
use crate::db;

/// 获取所有设置（从 settings 表读取为 HashMap）
#[tauri::command]
pub async fn get_settings() -> Result<HashMap<String, serde_json::Value>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
        let rows = stmt.query_map([], |row| {
            let key: String = row.get(0)?;
            let value_str: String = row.get(1)?;
            Ok((key, value_str))
        })?;

        let mut map = HashMap::new();
        for row in rows {
            let (key, value_str) = row?;
            // 尝试解析为 JSON，失败则存为字符串
            let value: serde_json::Value = serde_json::from_str(&value_str)
                .unwrap_or(serde_json::Value::String(value_str));
            map.insert(key, value);
        }
        Ok(map)
    })
    .await
}

/// 保存设置（逐条 UPSERT 到 settings 表）
#[tauri::command]
pub async fn save_settings(settings: HashMap<String, serde_json::Value>) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        for (key, value) in &settings {
            let value_str = match value {
                serde_json::Value::String(s) => s.clone(),
                _ => serde_json::to_string(value).unwrap_or_default(),
            };
            conn.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                rusqlite::params![key, value_str],
            )?;
        }
        Ok(())
    })
    .await
}

/// 从 JSON 文件导入节假日数据，验证并保存到 settings 表
#[tauri::command]
pub async fn import_holidays_json(json_path: String) -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let path = std::path::PathBuf::from(&json_path);
        let cal = crate::deadline::holidays::HolidayCalendar::from_json(&path)
            .map_err(|e| anyhow::anyhow!(e))?;
        let json_str = cal.to_json();
        let conn = db::open_db()?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('holidays_json', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            rusqlite::params![json_str],
        )?;
        Ok(serde_json::json!({ "ok": true, "holidays_count": cal.holidays_count() }))
    })
    .await
}

/// 获取当前节假日数据摘要
#[tauri::command]
pub async fn get_holidays_summary() -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let cal = crate::deadline::holidays::HolidayCalendar::builtin();
        Ok(serde_json::json!({
            "holidaysCount": cal.holidays_count(),
            "workdaysCount": cal.workdays_count(),
            "yearRange": cal.year_range(),
        }))
    })
    .await
}

// ═══════════════════════════════════════════════════════════
// 案件文件夹模板命令
// ═══════════════════════════════════════════════════════════

/// 列出所有文件夹模板
#[tauri::command]
pub async fn list_folder_templates() -> Result<Vec<serde_json::Value>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, name, case_type, is_builtin, directories_json, file_naming_json, created_at
                 FROM case_folder_templates ORDER BY is_builtin DESC, name",
            )
            ?;
        let rows = stmt
            .query_map([], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, String>(0)?,
                    "name": row.get::<_, String>(1)?,
                    "caseType": row.get::<_, String>(2)?,
                    "isBuiltin": row.get::<_, i32>(3)?,
                    "directories": serde_json::from_str::<serde_json::Value>(&row.get::<_, String>(4).unwrap_or_default()).unwrap_or(serde_json::Value::Array(vec![])),
                    "fileNaming": row.get::<_, Option<String>>(5)?.and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()),
                    "createdAt": row.get::<_, Option<String>>(6)?,
                }))
            })
            ?;
        let mut templates = Vec::new();
        for row in rows {
            templates.push(row?);
        }
        Ok(templates)
    })
    .await
}

/// 获取单个模板
#[tauri::command]
pub async fn get_folder_template(template_id: String) -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, name, case_type, is_builtin, directories_json, file_naming_json, created_at
                 FROM case_folder_templates WHERE id = ?1",
            )
            ?;
        let result = stmt.query_row(rusqlite::params![template_id], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "name": row.get::<_, String>(1)?,
                "caseType": row.get::<_, String>(2)?,
                "isBuiltin": row.get::<_, i32>(3)?,
                "directories": serde_json::from_str::<serde_json::Value>(&row.get::<_, String>(4).unwrap_or_default()).unwrap_or(serde_json::Value::Array(vec![])),
                "fileNaming": row.get::<_, Option<String>>(5)?.and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()),
                "createdAt": row.get::<_, Option<String>>(6)?,
            }))
        })?;
        Ok(result)
    })
    .await
}

/// 保存自定义模板（创建或更新），禁止编辑内置模板
#[tauri::command]
pub async fn save_folder_template(data: serde_json::Value) -> Result<String, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let id = data
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("tpl-custom-{}", uuid::Uuid::new_v4()));
        let name = data
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("缺少模板名称"))?;
        let case_type = data
            .get("caseType")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("缺少案件类型"))?;
        let directories = data
            .get("directories")
            .ok_or_else(|| anyhow::anyhow!("缺少目录结构"))?;
        let directories_json = serde_json::to_string(directories)?;

        // 检查是否为内置模板
        let is_builtin: i32 = conn
            .query_row(
                "SELECT is_builtin FROM case_folder_templates WHERE id = ?1",
                rusqlite::params![id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        if is_builtin == 1 {
            return Err(anyhow::anyhow!("不能编辑内置模板"));
        }

        let file_naming_json = data
            .get("fileNaming")
            .map(|v| serde_json::to_string(v).unwrap_or_default());

        conn.execute(
            "INSERT INTO case_folder_templates (id, name, case_type, is_builtin, directories_json, file_naming_json)
             VALUES (?1, ?2, ?3, 0, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET name=excluded.name, case_type=excluded.case_type,
             directories_json=excluded.directories_json, file_naming_json=excluded.file_naming_json",
            rusqlite::params![id, name, case_type, directories_json, file_naming_json],
        )
        ?;
        Ok(id)
    })
    .await
}

/// 删除自定义模板，禁止删除内置模板
#[tauri::command]
pub async fn delete_folder_template(template_id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let is_builtin: i32 = conn
            .query_row(
                "SELECT is_builtin FROM case_folder_templates WHERE id = ?1",
                rusqlite::params![template_id],
                |row| row.get(0),
            )
            ?;
        if is_builtin == 1 {
            return Err(anyhow::anyhow!("不能删除内置模板"));
        }
        conn.execute(
            "DELETE FROM case_folder_templates WHERE id = ?1",
            rusqlite::params![template_id],
        )
        ?;
        Ok(())
    })
    .await
}

/// 获取文件夹命名设置
#[tauri::command]
pub async fn get_folder_naming_settings() -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut map = serde_json::Map::new();
        for key in &[
            "folder_naming_date_format",
            "folder_naming_case_no_format",
            "folder_naming_file_format",
        ] {
            let val: Option<String> = conn
                .query_row(
                    "SELECT value FROM settings WHERE key = ?1",
                    rusqlite::params![key],
                    |row| row.get(0),
                )
                .ok();
            if let Some(v) = val {
                let parsed: serde_json::Value =
                    serde_json::from_str(&v).unwrap_or(serde_json::Value::String(v));
                map.insert(key.to_string(), parsed);
            }
        }
        Ok(serde_json::Value::Object(map))
    })
    .await
}

/// 保存文件夹命名设置
#[tauri::command]
pub async fn save_folder_naming_settings(data: serde_json::Value) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        for key in &[
            "folder_naming_date_format",
            "folder_naming_case_no_format",
            "folder_naming_file_format",
        ] {
            if let Some(val) = data.get(*key) {
                let val_str = match val {
                    serde_json::Value::String(s) => s.clone(),
                    _ => serde_json::to_string(val).unwrap_or_default(),
                };
                conn.execute(
                    "INSERT INTO settings (key, value) VALUES (?1, ?2)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    rusqlite::params![key, val_str],
                )
                ?;
            }
        }
        Ok(())
    })
    .await
}
