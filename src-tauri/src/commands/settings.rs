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
        let cal = crate::formula::holidays::HolidayCalendar::from_json(&path)
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
        let cal = crate::formula::holidays::HolidayCalendar::builtin();
        Ok(serde_json::json!({
            "holidaysCount": cal.holidays_count(),
            "workdaysCount": cal.workdays_count(),
            "yearRange": cal.year_range(),
        }))
    })
    .await
}
