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
            if key == "ai_api_key" || key == "ai_profiles_v1" || key == "mcp_auth_token" {
                continue;
            }
            if crate::credentials::settings_secret_type(&key).is_some() {
                map.insert(format!("{key}_configured"), (!value_str.is_empty()).into());
                map.insert(format!("{key}_needs_migration"), (!value_str.is_empty() && !value_str.starts_with("keychain:v1:")).into());
                map.insert(key, "".into());
                continue;
            }
            // 尝试解析为 JSON，失败则存为字符串
            let value: serde_json::Value =
                serde_json::from_str(&value_str).unwrap_or(serde_json::Value::String(value_str));
            map.insert(key, value);
        }
        map.insert("caseFolderBase".into(), crate::files::case_folder_base().to_string_lossy().to_string().into());
        Ok(map)
    })
    .await
}

/// 保存设置（逐条 UPSERT 到 settings 表）
#[tauri::command]
pub async fn save_settings(settings: HashMap<String, serde_json::Value>) -> Result<(), String> {
    run_blocking(move || {
        let connection = db::open_db()?;
        let conn = connection.unchecked_transaction()?;
        for (key, value) in &settings {
            if key.starts_with("ai_") || key == "caseFolderBase" || key.ends_with("_configured") || key.ends_with("_needs_migration") {
                continue;
            }
            if key == "personal_calendar_days" { validate_personal_days(value)?; }
            if key == "workspace_sync" { serde_json::from_value::<crate::workspace_sync::Options>(value.clone())?; }
            if key == "quote_sources" {
                let names = value.as_array().filter(|v|v.len()==4).ok_or_else(||anyhow::anyhow!("引用来源须为四项"))?;
                if names.iter().any(|v|v.as_str().is_none_or(|s|s.chars().count()>24)) { anyhow::bail!("引用来源名称过长或格式错误"); }
            }
            if crate::credentials::settings_secret_type(key).is_some() {
                // null explicitly clears; empty password inputs preserve (and migrate) the existing value.
                let previous=db::get_setting(&conn,key)?.unwrap_or_default();
                let candidate=if value.is_null() { "" } else { value.as_str().filter(|s|!s.is_empty()).unwrap_or(&previous) };
                anyhow::ensure!(!value.as_str().is_some_and(|s|s.starts_with("keychain:v1:")), "不能从前端设置凭据引用");
                let secure=crate::credentials::secure_settings_secret(key,candidate)?;
                db::set_setting(&conn,key,&secure)?;
                continue;
            }
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
        conn.commit()?;
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
        let conn = db::open_db()?;
        let cal = db::get_setting(&conn, "holidays_json")
            .ok()
            .flatten()
            .and_then(|value| {
                crate::deadline::holidays::HolidayCalendar::from_json_str(&value).ok()
            })
            .unwrap_or_else(crate::deadline::holidays::HolidayCalendar::builtin);
        Ok(serde_json::json!({
            "holidaysCount": cal.holidays_count(),
            "workdaysCount": cal.workdays_count(),
            "yearRange": cal.year_range(),
        }))
    })
    .await
}

/// Personal availability is independent of the statutory deadline calendar.
fn validate_personal_days(value: &serde_json::Value) -> anyhow::Result<()> {
    let entries = value.as_array().filter(|entries| entries.len() <= 3000)
        .ok_or_else(|| anyhow::anyhow!("个人调休数据格式错误或超过 3000 天"))?;
    let mut dates = std::collections::HashSet::new();
    for entry in entries {
        let date = entry["date"].as_str().unwrap_or_default();
        let parsed = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")?;
        anyhow::ensure!(parsed.to_string() == date && date >= "1900-01-01" && date <= "2200-12-31", "个人调休日期无效");
        anyhow::ensure!(matches!(entry["kind"].as_str(), Some("holiday" | "workday")), "个人调休类型无效");
        anyhow::ensure!(dates.insert(date), "同一天只能设置一个个人调休安排");
        anyhow::ensure!(entry["name"].as_str().is_some_and(|name| name.chars().count() <= 80), "个人调休备注过长或格式错误");
    }
    Ok(())
}

/// 获取日历展示用的年度法定节假日与调休工作日。
#[tauri::command]
pub async fn get_holiday_calendar(year: i32) -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let cal = db::get_setting(&conn, "holidays_json")
            .ok()
            .flatten()
            .and_then(|value| {
                crate::deadline::holidays::HolidayCalendar::from_json_str(&value).ok()
            })
            .unwrap_or_else(crate::deadline::holidays::HolidayCalendar::builtin);
        let mut entries: Vec<serde_json::Value> = cal.entries_for_year(year).into_iter().map(|entry| {
            let mut value = serde_json::to_value(entry).expect("serializable holiday entry");
            value["source"] = serde_json::json!("official"); value
        }).collect();
        if let Some(raw) = db::get_setting(&conn, "personal_calendar_days")? {
            let personal: serde_json::Value = serde_json::from_str(&raw)?;
            validate_personal_days(&personal)?;
            for entry in personal.as_array().unwrap() {
                if entry["date"].as_str().is_some_and(|date| date.starts_with(&format!("{year}-"))) {
                    let mut entry = entry.clone(); entry["source"] = serde_json::json!("personal"); entries.push(entry);
                }
            }
        }
        Ok(serde_json::json!({ "year": year, "entries": entries }))
    })
    .await
}

// ═══════════════════════════════════════════════════════════
// 案件文件夹模板命令
// ═══════════════════════════════════════════════════════════

/// 列出所有文件夹模板
#[tauri::command]
pub async fn list_folder_templates() -> Result<Vec<FolderTemplateOutput>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, case_type, is_builtin, directories_json, file_naming_json, created_at
                 FROM case_folder_templates ORDER BY is_builtin DESC, name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(FolderTemplateOutput {
                id: row.get::<_, String>(0)?,
                name: row.get::<_, String>(1)?,
                case_type: row.get::<_, String>(2)?,
                is_builtin: row.get::<_, i32>(3)?,
                directories: serde_json::from_str::<serde_json::Value>(
                    &row.get::<_, String>(4).unwrap_or_default(),
                )
                .unwrap_or(serde_json::Value::Array(vec![])),
                file_naming: row
                    .get::<_, Option<String>>(5)?
                    .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()),
                created_at: row.get::<_, Option<String>>(6)?,
            })
        })?;
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
pub async fn get_folder_template(template_id: String) -> Result<FolderTemplateOutput, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, case_type, is_builtin, directories_json, file_naming_json, created_at
                 FROM case_folder_templates WHERE id = ?1",
        )?;
        let result = stmt.query_row(rusqlite::params![template_id], |row| {
            Ok(FolderTemplateOutput {
                id: row.get::<_, String>(0)?,
                name: row.get::<_, String>(1)?,
                case_type: row.get::<_, String>(2)?,
                is_builtin: row.get::<_, i32>(3)?,
                directories: serde_json::from_str::<serde_json::Value>(
                    &row.get::<_, String>(4).unwrap_or_default(),
                )
                .unwrap_or(serde_json::Value::Array(vec![])),
                file_naming: row
                    .get::<_, Option<String>>(5)?
                    .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()),
                created_at: row.get::<_, Option<String>>(6)?,
            })
        })?;
        Ok(result)
    })
    .await
}

/// 保存自定义模板（创建或更新），禁止编辑内置模板
/// 文件夹模板输出（B1 类型化）
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FolderTemplateOutput {
    pub id: String,
    pub name: String,
    pub case_type: String,
    pub is_builtin: i32,
    pub directories: serde_json::Value,
    pub file_naming: Option<serde_json::Value>,
    pub created_at: Option<String>,
}

/// 文件夹命名设置输出（缺省键以 null 呈现，前端按 ?? 默认值消费）
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FolderNamingSettingsOutput {
    pub folder_naming_date_format: Option<String>,
    pub folder_naming_case_no_format: Option<String>,
    pub folder_naming_file_format: Option<String>,
}

/// 文件夹模板输入（directories 保持自由 JSON 结构）
#[derive(Debug, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FolderTemplateInput {
    pub id: Option<String>,
    pub name: Option<String>,
    pub case_type: Option<String>,
    pub directories: Option<serde_json::Value>,
    pub file_naming: Option<serde_json::Value>,
}

/// 文件夹命名设置（三项均可选，缺省项跳过不写库——与原 Value 版语义一致）
#[derive(Debug, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FolderNamingSettingsInput {
    pub folder_naming_date_format: Option<String>,
    pub folder_naming_case_no_format: Option<String>,
    pub folder_naming_file_format: Option<String>,
}

#[tauri::command]
pub async fn save_folder_template(data: FolderTemplateInput) -> Result<String, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let id = data
            .id
            .unwrap_or_else(|| format!("tpl-custom-{}", uuid::Uuid::new_v4()));
        let name = data.name.ok_or_else(|| {
            anyhow::anyhow!(crate::error_code::err(
                crate::error_code::codes::SETTINGS_MISSING_FIELD,
                "缺少模板名称",
            ))
        })?;
        let case_type = data.case_type.ok_or_else(|| {
            anyhow::anyhow!(crate::error_code::err(
                crate::error_code::codes::SETTINGS_MISSING_FIELD,
                "缺少案件类型",
            ))
        })?;
        let directories = data.directories.ok_or_else(|| {
            anyhow::anyhow!(crate::error_code::err(
                crate::error_code::codes::SETTINGS_MISSING_FIELD,
                "缺少目录结构",
            ))
        })?;
        let directories_json = serde_json::to_string(&directories)?;

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
            .file_naming
            .as_ref()
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
        let is_builtin: i32 = conn.query_row(
            "SELECT is_builtin FROM case_folder_templates WHERE id = ?1",
            rusqlite::params![template_id],
            |row| row.get(0),
        )?;
        if is_builtin == 1 {
            return Err(anyhow::anyhow!("不能删除内置模板"));
        }
        conn.execute(
            "DELETE FROM case_folder_templates WHERE id = ?1",
            rusqlite::params![template_id],
        )?;
        Ok(())
    })
    .await
}

/// 获取文件夹命名设置
#[tauri::command]
pub async fn get_folder_naming_settings() -> Result<FolderNamingSettingsOutput, String> {
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
        // 转强类型输出：缺省键 → None（序列化为 null，前端 ?? 默认值消费）
        let get = |k: &str| map.get(k).and_then(|v| v.as_str()).map(|s| s.to_string());
        Ok(FolderNamingSettingsOutput {
            folder_naming_date_format: get("folder_naming_date_format"),
            folder_naming_case_no_format: get("folder_naming_case_no_format"),
            folder_naming_file_format: get("folder_naming_file_format"),
        })
    })
    .await
}

/// 保存文件夹命名设置
#[tauri::command]
pub async fn save_folder_naming_settings(data: FolderNamingSettingsInput) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        for (key, val) in [
            ("folder_naming_date_format", data.folder_naming_date_format),
            (
                "folder_naming_case_no_format",
                data.folder_naming_case_no_format,
            ),
            ("folder_naming_file_format", data.folder_naming_file_format),
        ] {
            if let Some(val_str) = val {
                conn.execute(
                    "INSERT INTO settings (key, value) VALUES (?1, ?2)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    rusqlite::params![key, val_str],
                )?;
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

#[tauri::command]
pub async fn backup_database_key_to_keychain() -> Result<(), String> {
    run_blocking(crate::db::backup_database_key_to_keychain).await
}
