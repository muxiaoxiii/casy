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

// ═══════════════════════════════════════════════════════════
// 律师画像（LawyerProfile 持久化 + 首次使用引导数据）
//
// 存 settings 表 key='lawyer_profile'（JSON）。工作时段偏好（work_hours）
// 已被提醒模块用于时段外延迟（见 commands/reminder.rs 的 work_hours/next_work_start）。
// ═══════════════════════════════════════════════════════════

/// settings 表中的画像键
const LAWYER_PROFILE_KEY: &str = "lawyer_profile";

/// 默认画像（未完成首次引导）
fn default_lawyer_profile() -> serde_json::Value {
    serde_json::json!({
        "name": "",
        "practice_areas": [],
        "common_case_types": [],
        "work_hours": { "start_hour": 9, "end_hour": 18 },
        "reminder_channels": ["local"],
        "onboarding_completed": false,
    })
}

/// 读取律师画像：无记录或解析失败时返回默认画像（onboarding_completed=false）
pub fn load_lawyer_profile(conn: &rusqlite::Connection) -> serde_json::Value {
    let default = default_lawyer_profile();
    let Ok(Some(raw)) = db::get_setting(conn, LAWYER_PROFILE_KEY) else {
        return default;
    };
    let Ok(stored) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return default;
    };
    merge_profile(default, &stored)
}

/// 把已存画像浅合并到默认画像上（保证字段齐全；非法类型回退默认值）
fn merge_profile(mut base: serde_json::Value, stored: &serde_json::Value) -> serde_json::Value {
    if let (Some(base_map), Some(stored_map)) = (base.as_object_mut(), stored.as_object()) {
        for (key, value) in stored_map {
            if base_map.contains_key(key) {
                base_map.insert(key.clone(), value.clone());
            }
        }
    }
    base
}

/// 保存律师画像（与默认画像合并后落库，缺失字段自动补默认）
pub fn store_lawyer_profile(
    conn: &rusqlite::Connection,
    profile: &serde_json::Value,
) -> anyhow::Result<()> {
    if !profile.is_object() {
        anyhow::bail!("律师画像必须是 JSON 对象");
    }
    let merged = merge_profile(default_lawyer_profile(), profile);
    db::set_setting(conn, LAWYER_PROFILE_KEY, &serde_json::to_string(&merged)?)?;
    Ok(())
}

/// 获取律师画像（无则返回默认画像 + onboarding_completed=false）
#[tauri::command]
pub async fn get_lawyer_profile() -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        Ok(load_lawyer_profile(&conn))
    })
    .await
}

/// 保存律师画像（首次使用引导完成时传 onboarding_completed=true）
#[tauri::command]
pub async fn save_lawyer_profile(profile: serde_json::Value) -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        store_lawyer_profile(&conn, &profile)?;
        Ok(load_lawyer_profile(&conn))
    })
    .await
}

#[cfg(test)]
mod lawyer_profile_tests {
    use super::*;

    fn setup_test_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);")
            .unwrap();
        conn
    }

    #[test]
    fn test_default_profile_when_missing() {
        let conn = setup_test_db();
        let p = load_lawyer_profile(&conn);
        assert_eq!(p["onboarding_completed"], false);
        assert_eq!(p["work_hours"]["start_hour"], 9);
        assert_eq!(p["work_hours"]["end_hour"], 18);
        assert_eq!(p["reminder_channels"][0], "local");
    }

    #[test]
    fn test_save_and_load_profile() {
        let conn = setup_test_db();
        store_lawyer_profile(
            &conn,
            &serde_json::json!({
                "name": "张三",
                "practice_areas": ["专利无效"],
                "work_hours": { "start_hour": 8, "end_hour": 20 },
                "onboarding_completed": true,
            }),
        )
        .unwrap();

        let p = load_lawyer_profile(&conn);
        assert_eq!(p["name"], "张三");
        assert_eq!(p["practice_areas"][0], "专利无效");
        assert_eq!(p["work_hours"]["start_hour"], 8);
        assert_eq!(p["onboarding_completed"], true);
        // 未提供的字段保留默认
        assert_eq!(p["common_case_types"], serde_json::json!([]));
    }

    #[test]
    fn test_corrupt_value_falls_back_to_default() {
        let conn = setup_test_db();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('lawyer_profile', 'not-json')",
            [],
        )
        .unwrap();
        let p = load_lawyer_profile(&conn);
        assert_eq!(p["onboarding_completed"], false);
    }

    #[test]
    fn test_reject_non_object() {
        let conn = setup_test_db();
        assert!(store_lawyer_profile(&conn, &serde_json::json!("str")).is_err());
        assert!(store_lawyer_profile(&conn, &serde_json::json!(42)).is_err());
    }
}
