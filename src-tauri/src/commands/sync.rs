use super::run_blocking;
use crate::sync;

#[tauri::command]
pub async fn get_sync_status() -> Result<sync::SyncStatus, String> {
    Ok(sync::get_sync_status())
}

#[tauri::command]
pub async fn test_webdav_connection(
    url: String,
    username: String,
    password: String,
) -> Result<String, String> {
    run_blocking(move || {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let client = sync::webdav::WebDavClient::new(&url, &username, &password)?;
            match client.head("").await {
                Ok(_) => Ok("连接成功".into()),
                Err(e) => Ok(format!("连接失败: {}", e)),
            }
        })
    })
    .await
}

/// WebDAV 同步：启动时检查
#[tauri::command]
pub async fn webdav_startup_sync(
    url: String,
    username: String,
    password: String,
) -> Result<sync::SyncResult, String> {
    run_blocking(move || {
        let db_path = crate::db::get_db_path();
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            // 从 settings 读取上次同步的 ETag
            let conn = crate::db::open_db()?;
            let local_etag = crate::db::get_setting(&conn, "webdav_last_etag")
                .ok()
                .flatten();
            sync::startup_sync(&url, &username, &password, &db_path, local_etag.as_deref())
                .await
        })
    })
    .await
}

/// WebDAV 同步：手动推送
#[tauri::command]
pub async fn webdav_push(
    url: String,
    username: String,
    password: String,
) -> Result<sync::SyncResult, String> {
    run_blocking(move || {
        let db_path = crate::db::get_db_path();
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let result = sync::manual_sync_push(&url, &username, &password, &db_path)
                .await?;

            // 保存同步后的 ETag
            if let Some(etag) = &result.remote_etag {
                let conn = crate::db::open_db()?;
                crate::db::set_setting(&conn, "webdav_last_etag", etag)?;
                crate::db::set_setting(
                    &conn,
                    "webdav_last_sync_at",
                    &chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                )?;
            }

            Ok(result)
        })
    })
    .await
}

/// WebDAV 同步：手动拉取
#[tauri::command]
pub async fn webdav_pull(
    url: String,
    username: String,
    password: String,
) -> Result<sync::SyncResult, String> {
    run_blocking(move || {
        let db_path = crate::db::get_db_path();
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let result = sync::manual_sync_pull(&url, &username, &password, &db_path)
                .await?;

            // 保存同步后的 ETag
            if let Some(etag) = &result.remote_etag {
                let conn = crate::db::open_db()?;
                crate::db::set_setting(&conn, "webdav_last_etag", etag)?;
                crate::db::set_setting(
                    &conn,
                    "webdav_last_sync_at",
                    &chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                )?;
            }

            Ok(result)
        })
    })
    .await
}

/// 冲突解决：保留本地版本
#[tauri::command]
pub async fn webdav_resolve_keep_local(
    url: String,
    username: String,
    password: String,
) -> Result<sync::SyncResult, String> {
    run_blocking(move || {
        let db_path = crate::db::get_db_path();
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let result = sync::resolve_keep_local(&url, &username, &password, &db_path)
                .await?;

            if let Some(etag) = &result.remote_etag {
                let conn = crate::db::open_db()?;
                crate::db::set_setting(&conn, "webdav_last_etag", etag)?;
            }

            Ok(result)
        })
    })
    .await
}

/// 冲突解决：保留远程版本
#[tauri::command]
pub async fn webdav_resolve_keep_remote(
    url: String,
    username: String,
    password: String,
) -> Result<sync::SyncResult, String> {
    run_blocking(move || {
        let db_path = crate::db::get_db_path();
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let result = sync::resolve_keep_remote(&url, &username, &password, &db_path)
                .await?;

            if let Some(etag) = &result.remote_etag {
                let conn = crate::db::open_db()?;
                crate::db::set_setting(&conn, "webdav_last_etag", etag)?;
            }

            Ok(result)
        })
    })
    .await
}

// ============================================================
// 飞书同步命令
// ============================================================

/// 保存飞书凭证到 OS keychain
#[tauri::command]
pub async fn configure_feishu(app_id: String, app_secret: String) -> Result<String, String> {
    run_blocking(move || {
        sync::feishu::save_feishu_credentials(&app_id, &app_secret)?;
        Ok("飞书凭证已保存".to_string())
    })
    .await
}

/// 测试飞书连通性
#[tauri::command]
pub async fn test_feishu_connection() -> Result<String, String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(async {
        sync::feishu::test_feishu_connection_inner()
            .await
            .map_err(|e| e.to_string())
    })
}

/// 从飞书拉取数据到本地
#[tauri::command]
pub async fn sync_feishu_pull(
    app_token: String,
    table_id: String,
) -> Result<sync::feishu::FeishuSyncReport, String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(async {
        sync::feishu::sync_feishu_pull_inner(&app_token, &table_id)
            .await
            .map_err(|e| e.to_string())
    })
}

/// 本地数据推送到飞书
#[tauri::command]
pub async fn sync_feishu_push(
    app_token: String,
    table_id: String,
) -> Result<sync::feishu::FeishuSyncReport, String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(async {
        sync::feishu::sync_feishu_push_inner(&app_token, &table_id)
            .await
            .map_err(|e| e.to_string())
    })
}

/// 获取飞书同步状态信息
#[tauri::command]
pub async fn get_feishu_sync_info() -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = crate::db::open_db()?;
        let configured = sync::feishu::is_feishu_configured();
        let last_pull_at = sync::feishu::get_sync_metadata(&conn, "feishu_last_pull_at")
            .ok()
            .flatten();
        let last_push_at = sync::feishu::get_sync_metadata(&conn, "feishu_last_push_at")
            .ok()
            .flatten();
        let last_pull_count = sync::feishu::get_sync_metadata(&conn, "feishu_last_pull_count")
            .ok()
            .flatten();
        let last_push_count = sync::feishu::get_sync_metadata(&conn, "feishu_last_push_count")
            .ok()
            .flatten();
        let app_token = sync::feishu::get_sync_metadata(&conn, "feishu_app_token")
            .ok()
            .flatten();
        let table_id = sync::feishu::get_sync_metadata(&conn, "feishu_table_id")
            .ok()
            .flatten();

        Ok(serde_json::json!({
            "configured": configured,
            "lastPullAt": last_pull_at,
            "lastPushAt": last_push_at,
            "lastPullCount": last_pull_count,
            "lastPushCount": last_push_count,
            "appToken": app_token,
            "tableId": table_id,
        }))
    })
    .await
}

/// 保存飞书表格配置（app_token 和 table_id）
#[tauri::command]
pub async fn configure_feishu_table(
    app_token: String,
    table_id: String,
) -> Result<String, String> {
    run_blocking(move || {
        let conn = crate::db::open_db()?;
        sync::feishu::update_sync_metadata(&conn, "feishu_app_token", &app_token)?;
        sync::feishu::update_sync_metadata(&conn, "feishu_table_id", &table_id)?;
        Ok("飞书表格配置已保存".to_string())
    })
    .await
}

// ============================================================
// 飞书自动推送命令
// ============================================================

/// 设置飞书自动推送开关
#[tauri::command]
pub async fn set_feishu_auto_push(enabled: bool) -> Result<String, String> {
    let manager = sync::feishu::get_auto_push_manager();
    manager.set_enabled(enabled);

    // 保存设置
    let conn = crate::db::open_db().map_err(|e| e.to_string())?;
    sync::feishu::update_sync_metadata(
        &conn,
        "feishu_auto_push_enabled",
        &enabled.to_string(),
    )
    .map_err(|e| e.to_string())?;

    Ok(format!(
        "飞书自动推送已{}",
        if enabled { "启用" } else { "禁用" }
    ))
}

/// 获取飞书自动推送状态
#[tauri::command]
pub async fn get_feishu_auto_push_status() -> Result<serde_json::Value, String> {
    let manager = sync::feishu::get_auto_push_manager();
    Ok(manager.get_status_sync())
}

/// 手动触发飞书推送（用于测试）
#[tauri::command]
pub async fn trigger_feishu_push() -> Result<String, String> {
    let manager = sync::feishu::get_auto_push_manager();
    manager.notify_change();
    Ok("已触发飞书推送（5 秒后执行）".to_string())
}

// ============================================================
// Phase 1: 表结构发现命令
// ============================================================

/// 获取飞书多维表格中所有表的列表
#[tauri::command]
pub async fn feishu_list_tables(app_token: String) -> Result<serde_json::Value, String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(async {
        let tables = sync::feishu::list_bitable_tables(&app_token)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::to_value(&tables).map_err(|e| e.to_string())
    })
}

/// 获取指定表的所有字段定义
#[tauri::command]
pub async fn feishu_list_fields(
    app_token: String,
    table_id: String,
) -> Result<serde_json::Value, String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(async {
        let fields = sync::feishu::list_bitable_fields(&app_token, &table_id)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::to_value(&fields).map_err(|e| e.to_string())
    })
}

/// 获取指定表的记录（分页）
#[tauri::command]
pub async fn feishu_list_records(
    app_token: String,
    table_id: String,
    page_token: String,
) -> Result<serde_json::Value, String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(async {
        let page = sync::feishu::list_bitable_records(&app_token, &table_id, &page_token)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::to_value(&page).map_err(|e| e.to_string())
    })
}

// ============================================================
// Phase 2: 字段映射 + 比较命令
// ============================================================

/// Schema 比较结果
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaDiff {
    feishu_only: Vec<FieldDiffItem>,
    local_only: Vec<FieldDiffItem>,
    type_conflict: Vec<FieldDiffItem>,
    mapped: Vec<FieldDiffItem>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldDiffItem {
    feishu_field: Option<String>,
    feishu_type: Option<String>,
    feishu_type_code: Option<i32>,
    local_column: Option<String>,
    local_type: Option<String>,
    status: String, // "new", "local_only", "type_conflict", "mapped"
}

/// 比较飞书表结构 vs 本地表结构
#[tauri::command]
pub async fn feishu_compare_table(
    app_token: String,
    table_id: String,
    local_table: String,
) -> Result<SchemaDiff, String> {
    // 1. 获取飞书字段
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    let feishu_fields = rt
        .block_on(async {
            sync::feishu::list_bitable_fields(&app_token, &table_id)
                .await
                .map_err(|e| e.to_string())
        })?;

    // 2. 获取本地表列
    let conn = crate::db::open_db().map_err(|e| e.to_string())?;
    let local_columns = get_local_table_columns(&conn, &local_table)
        .map_err(|e| e.to_string())?;

    // 3. 建立列名到类型的映射（忽略 id, created_at, updated_at 等系统列）
    let system_columns = ["id", "feishu_record_id", "created_at", "updated_at"];
    let local_col_map: std::collections::HashMap<String, String> = local_columns
        .iter()
        .filter(|(col, _)| !system_columns.contains(&col.as_str()))
        .cloned()
        .collect();

    let mut diff = SchemaDiff {
        feishu_only: Vec::new(),
        local_only: Vec::new(),
        type_conflict: Vec::new(),
        mapped: Vec::new(),
    };

    let mut matched_local: std::collections::HashSet<String> = std::collections::HashSet::new();

    // 4. 检查每个飞书字段
    for field in &feishu_fields {
        let type_name = sync::feishu::feishu_type_name(field.field_type);
        let sqlite_type = sync::feishu::feishu_type_to_sqlite(field.field_type);

        // 尝试名称匹配
        let match_col = find_matching_local_column(&field.field_name, &local_col_map);

        if let Some(local_col) = match_col {
            matched_local.insert(local_col.clone());
            let local_type = local_col_map.get(&local_col).cloned().unwrap_or_default();

            if types_compatible(sqlite_type, &local_type) {
                diff.mapped.push(FieldDiffItem {
                    feishu_field: Some(field.field_name.clone()),
                    feishu_type: Some(type_name.to_string()),
                    feishu_type_code: Some(field.field_type),
                    local_column: Some(local_col),
                    local_type: Some(local_type),
                    status: "mapped".to_string(),
                });
            } else {
                diff.type_conflict.push(FieldDiffItem {
                    feishu_field: Some(field.field_name.clone()),
                    feishu_type: Some(type_name.to_string()),
                    feishu_type_code: Some(field.field_type),
                    local_column: Some(local_col),
                    local_type: Some(local_type),
                    status: "type_conflict".to_string(),
                });
            }
        } else {
            diff.feishu_only.push(FieldDiffItem {
                feishu_field: Some(field.field_name.clone()),
                feishu_type: Some(type_name.to_string()),
                feishu_type_code: Some(field.field_type),
                local_column: None,
                local_type: None,
                status: "new".to_string(),
            });
        }
    }

    // 5. 本地独有的列
    for (col, col_type) in &local_col_map {
        if !matched_local.contains(col) {
            diff.local_only.push(FieldDiffItem {
                feishu_field: None,
                feishu_type: None,
                feishu_type_code: None,
                local_column: Some(col.clone()),
                local_type: Some(col_type.clone()),
                status: "local_only".to_string(),
            });
        }
    }

    Ok(diff)
}

/// 获取本地表的列名和类型
fn get_local_table_columns(
    conn: &rusqlite::Connection,
    table_name: &str,
) -> anyhow::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table_name))?;
    let columns = stmt
        .query_map([], |row| {
            let name: String = row.get(1)?;
            let col_type: String = row.get(2)?;
            Ok((name, col_type))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(columns)
}

/// 尝试将飞书字段名匹配到本地列名
fn find_matching_local_column(
    feishu_name: &str,
    local_columns: &std::collections::HashMap<String, String>,
) -> Option<String> {
    // 1. 精确匹配（飞书字段名 == 本地列名）
    if local_columns.contains_key(feishu_name) {
        return Some(feishu_name.to_string());
    }

    // 2. 从设计文档中的 FIELD_MAP 查找
    let name_lower = feishu_name.to_lowercase();
    for &(db_col, feishu_key) in sync::feishu::FIELD_MAP {
        if feishu_key == feishu_name && local_columns.contains_key(db_col) {
            return Some(db_col.to_string());
        }
    }

    // 3. 模糊匹配
    let fuzzy_map = [
        ("案件信息", vec!["case_name", "case_title"]),
        ("案号", vec!["case_no"]),
        ("客户名称", vec!["client_name"]),
        ("备注", vec!["notes"]),
    ];
    for (feishu_key, candidates) in &fuzzy_map {
        if feishu_name == *feishu_key {
            for c in candidates {
                if local_columns.contains_key(*c) {
                    return Some(c.to_string());
                }
            }
        }
    }

    // 4. 大小写不敏感匹配
    for col in local_columns.keys() {
        if col.to_lowercase() == name_lower {
            return Some(col.clone());
        }
    }

    None
}

/// 检查飞书类型和本地类型是否兼容
fn types_compatible(feishu_sqlite_type: &str, local_type: &str) -> bool {
    let feishu = feishu_sqlite_type.to_uppercase();
    let local = local_type.to_uppercase();

    // 完全匹配
    if feishu == local {
        return true;
    }

    // TEXT 兼容一切
    if local == "TEXT" || feishu == "TEXT" {
        return true;
    }

    // REAL / INTEGER 兼容
    if (feishu == "REAL" && local == "INTEGER") || (feishu == "INTEGER" && local == "REAL") {
        return true;
    }

    false
}

/// 比较飞书记录 vs 本地记录
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordDiff {
    same: Vec<RecordDiffItem>,
    feishu_only: Vec<RecordDiffItem>,
    local_only: Vec<RecordDiffItem>,
    conflict: Vec<RecordDiffItem>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordDiffItem {
    record_id: Option<String>,
    local_id: Option<String>,
    match_value: Option<String>,
    feishu_fields: Option<serde_json::Value>,
    local_fields: Option<serde_json::Value>,
}

#[tauri::command]
pub async fn feishu_compare_records(
    app_token: String,
    table_id: String,
    local_table: String,
    match_field: String,
) -> Result<RecordDiff, String> {
    // 1. 获取飞书记录
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    let feishu_records = rt
        .block_on(async {
            sync::feishu::list_all_bitable_records(&app_token, &table_id, 20)
                .await
                .map_err(|e| e.to_string())
        })?;

    // 2. 获取本地记录
    let conn = crate::db::open_db().map_err(|e| e.to_string())?;
    let local_records = get_local_records(&conn, &local_table, &match_field)
        .map_err(|e| e.to_string())?;

    // 3. 以 match_field 为键比较
    let mut local_map: std::collections::HashMap<String, serde_json::Value> =
        std::collections::HashMap::new();
    for (id, match_val, row_json) in &local_records {
        if !match_val.is_empty() {
            local_map.insert(match_val.clone(), serde_json::json!({"id": id, "fields": row_json}));
        }
    }

    let mut diff = RecordDiff {
        same: Vec::new(),
        feishu_only: Vec::new(),
        local_only: Vec::new(),
        conflict: Vec::new(),
    };

    let mut matched_keys: std::collections::HashSet<String> = std::collections::HashSet::new();

    for record in &feishu_records {
        let match_val = record
            .fields
            .get(&match_field)
            .and_then(|v| {
                if v.is_string() {
                    v.as_str().map(|s| s.to_string())
                } else {
                    Some(v.to_string())
                }
            })
            .unwrap_or_default();

        if match_val.is_empty() {
            diff.feishu_only.push(RecordDiffItem {
                record_id: Some(record.record_id.clone()),
                local_id: None,
                match_value: None,
                feishu_fields: Some(record.fields.clone()),
                local_fields: None,
            });
            continue;
        }

        matched_keys.insert(match_val.clone());

        if let Some(local_info) = local_map.get(&match_val) {
            // Both exist — check if same or conflict
            let local_id = local_info["id"].as_str().unwrap_or("").to_string();
            diff.same.push(RecordDiffItem {
                record_id: Some(record.record_id.clone()),
                local_id: Some(local_id),
                match_value: Some(match_val),
                feishu_fields: Some(record.fields.clone()),
                local_fields: Some(local_info["fields"].clone()),
            });
        } else {
            diff.feishu_only.push(RecordDiffItem {
                record_id: Some(record.record_id.clone()),
                local_id: None,
                match_value: Some(match_val),
                feishu_fields: Some(record.fields.clone()),
                local_fields: None,
            });
        }
    }

    // 本地独有的记录
    for (id, match_val, row_json) in &local_records {
        if !matched_keys.contains(match_val) && !match_val.is_empty() {
            diff.local_only.push(RecordDiffItem {
                record_id: None,
                local_id: Some(id.clone()),
                match_value: Some(match_val.clone()),
                feishu_fields: None,
                local_fields: Some(row_json.clone()),
            });
        }
    }

    Ok(diff)
}

/// 获取本地表的记录（id, match_field值, row_json）
fn get_local_records(
    conn: &rusqlite::Connection,
    table_name: &str,
    match_field: &str,
) -> anyhow::Result<Vec<(String, String, serde_json::Value)>> {
    // 验证 match_field 存在
    let columns = get_local_table_columns(conn, table_name)?;
    let col_exists = columns.iter().any(|(name, _)| name == match_field);
    if !col_exists {
        anyhow::bail!("本地表 {} 中不存在列 {}", table_name, match_field);
    }

    let sql = format!(
        "SELECT id, COALESCE({}, '') as match_val FROM {}",
        match_field, table_name
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let match_val: String = row.get(1)?;
            Ok((id, match_val))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut results = Vec::new();
    for (id, match_val) in rows {
        results.push((
            id,
            match_val,
            serde_json::Value::Null, // Simplified — don't load full row for comparison
        ));
    }
    Ok(results)
}

/// 保存字段映射
#[tauri::command]
pub async fn feishu_save_mappings(mappings_json: serde_json::Value) -> Result<String, String> {
    run_blocking(move || {
        let conn = crate::db::open_db()?;
        let mappings = mappings_json
            .as_array()
            .ok_or(anyhow::anyhow!("mappings_json 应为数组"))?;

        for mapping in mappings {
            let id = mapping["id"]
                .as_str()
                .map(|s| s.to_string())
                .unwrap_or_else(|| crate::db::new_id());
            let connection_id = mapping["connectionId"]
                .as_str()
                .unwrap_or("default");
            let feishu_table_id = mapping["feishuTableId"]
                .as_str()
                .unwrap_or("");
            let feishu_field_id = mapping["feishuFieldId"]
                .as_str()
                .unwrap_or("");
            let feishu_field_name = mapping["feishuFieldName"]
                .as_str()
                .unwrap_or("");
            let feishu_field_type = mapping["feishuFieldType"]
                .as_i64()
                .unwrap_or(1) as i32;
            let local_table = mapping["localTable"]
                .as_str()
                .unwrap_or("");
            let local_column = mapping["localColumn"]
                .as_str()
                .unwrap_or("");
            let transform_rule = mapping["transformRule"]
                .as_str()
                .map(|s| s.to_string());
            let sync_direction = mapping["syncDirection"]
                .as_str()
                .unwrap_or("bidirectional");
            let is_formula = mapping["isFormula"].as_i64().unwrap_or(0);
            let is_link = mapping["isLink"].as_i64().unwrap_or(0);
            let is_lookup = mapping["isLookup"].as_i64().unwrap_or(0);

            conn.execute(
                "INSERT OR REPLACE INTO feishu_field_mappings
                 (id, connection_id, feishu_table_id, feishu_field_id, feishu_field_name,
                  feishu_field_type, local_table, local_column, transform_rule, sync_direction,
                  is_formula, is_link, is_lookup)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                rusqlite::params![
                    id,
                    connection_id,
                    feishu_table_id,
                    feishu_field_id,
                    feishu_field_name,
                    feishu_field_type,
                    local_table,
                    local_column,
                    transform_rule,
                    sync_direction,
                    is_formula,
                    is_link,
                    is_lookup,
                ],
            )?;
        }

        Ok(format!("已保存 {} 条映射", mappings.len()))
    })
    .await
}

/// 获取已保存的字段映射
#[tauri::command]
pub async fn feishu_get_mappings(
    connection_id: String,
    table_id: String,
) -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = crate::db::open_db()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, connection_id, feishu_table_id, feishu_field_id, feishu_field_name,
                 feishu_field_type, local_table, local_column, transform_rule, sync_direction,
                 is_formula, is_link, is_lookup
                 FROM feishu_field_mappings
                 WHERE connection_id = ?1 AND feishu_table_id = ?2",
            )?;

        let rows = stmt
            .query_map(rusqlite::params![connection_id, table_id], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, String>(0)?,
                    "connectionId": row.get::<_, String>(1)?,
                    "feishuTableId": row.get::<_, String>(2)?,
                    "feishuFieldId": row.get::<_, String>(3)?,
                    "feishuFieldName": row.get::<_, String>(4)?,
                    "feishuFieldType": row.get::<_, i32>(5)?,
                    "localTable": row.get::<_, String>(6)?,
                    "localColumn": row.get::<_, String>(7)?,
                    "transformRule": row.get::<_, Option<String>>(8)?,
                    "syncDirection": row.get::<_, String>(9)?,
                    "isFormula": row.get::<_, i32>(10)?,
                    "isLink": row.get::<_, i32>(11)?,
                    "isLookup": row.get::<_, i32>(12)?,
                }))
            })?;

        let mappings: Vec<serde_json::Value> = rows
            .filter_map(|r| r.ok())
            .collect();

        Ok(serde_json::to_value(&mappings)?)
    })
    .await
}

// ============================================================
// Phase 3: 导入引擎
// ============================================================

/// 全量导入：从飞书导入所有记录到本地表
#[tauri::command]
pub async fn feishu_import_all(
    app_token: String,
    table_id: String,
    local_table: String,
    mappings_json: serde_json::Value,
) -> Result<ImportResult, String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(async {
        let mappings = parse_mappings(&mappings_json)?;

        // 获取所有飞书记录
        let records = sync::feishu::list_all_bitable_records(&app_token, &table_id, 100)
            .await
            .map_err(|e| e.to_string())?;

        let conn = crate::db::open_db().map_err(|e| e.to_string())?;
        let mut result = ImportResult {
            total: records.len(),
            created: 0,
            updated: 0,
            skipped: 0,
            errors: Vec::new(),
        };

        for record in &records {
            match import_record(&conn, &local_table, record, &mappings) {
                Ok(action) => match action.as_str() {
                    "created" => result.created += 1,
                    "updated" => result.updated += 1,
                    _ => result.skipped += 1,
                },
                Err(e) => {
                    result.errors.push(format!("{}: {}", record.record_id, e));
                }
            }
        }

        Ok(result)
    })
}

/// 选择性导入：只导入指定的记录
#[tauri::command]
pub async fn feishu_import_selected(
    app_token: String,
    table_id: String,
    local_table: String,
    record_ids: Vec<String>,
    mappings_json: serde_json::Value,
) -> Result<ImportResult, String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(async {
        let mappings = parse_mappings(&mappings_json)?;

        // 获取所有记录（飞书 API 不支持按 record_id 过滤）
        let all_records = sync::feishu::list_all_bitable_records(&app_token, &table_id, 100)
            .await
            .map_err(|e| e.to_string())?;

        let selected: Vec<_> = all_records
            .iter()
            .filter(|r| record_ids.contains(&r.record_id))
            .collect();

        let conn = crate::db::open_db().map_err(|e| e.to_string())?;
        let mut result = ImportResult {
            total: selected.len(),
            created: 0,
            updated: 0,
            skipped: 0,
            errors: Vec::new(),
        };

        for record in &selected {
            match import_record(&conn, &local_table, record, &mappings) {
                Ok(action) => match action.as_str() {
                    "created" => result.created += 1,
                    "updated" => result.updated += 1,
                    _ => result.skipped += 1,
                },
                Err(e) => {
                    result.errors.push(format!("{}: {}", record.record_id, e));
                }
            }
        }

        Ok(result)
    })
}

/// 增量导入：只导入指定时间之后修改的记录
#[tauri::command]
pub async fn feishu_import_incremental(
    app_token: String,
    table_id: String,
    local_table: String,
    since_timestamp: String,
    mappings_json: serde_json::Value,
) -> Result<ImportResult, String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(async {
        let mappings = parse_mappings(&mappings_json)?;

        // 解析时间
        let since_dt = chrono::NaiveDateTime::parse_from_str(
            &since_timestamp,
            "%Y-%m-%d %H:%M:%S",
        )
        .map_err(|e| format!("时间格式错误: {}", e))?;
        let since_ms = since_dt.and_utc().timestamp_millis();

        // 获取所有记录
        let all_records = sync::feishu::list_all_bitable_records(&app_token, &table_id, 100)
            .await
            .map_err(|e| e.to_string())?;

        // 过滤增量
        let incremental: Vec<_> = all_records
            .iter()
            .filter(|r| {
                r.last_modified_time
                    .map(|t| t > since_ms)
                    .unwrap_or(false)
            })
            .collect();

        let conn = crate::db::open_db().map_err(|e| e.to_string())?;
        let mut result = ImportResult {
            total: incremental.len(),
            created: 0,
            updated: 0,
            skipped: 0,
            errors: Vec::new(),
        };

        for record in &incremental {
            match import_record(&conn, &local_table, record, &mappings) {
                Ok(action) => match action.as_str() {
                    "created" => result.created += 1,
                    "updated" => result.updated += 1,
                    _ => result.skipped += 1,
                },
                Err(e) => {
                    result.errors.push(format!("{}: {}", record.record_id, e));
                }
            }
        }

        Ok(result)
    })
}

/// 导入结果
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    total: usize,
    created: usize,
    updated: usize,
    skipped: usize,
    errors: Vec<String>,
}

/// 映射条目
struct MappingEntry {
    feishu_field_name: String,
    feishu_field_type: i32,
    local_column: String,
    sync_direction: String,
    is_formula: bool,
    is_link: bool,
}

/// 从 JSON 解析映射配置
fn parse_mappings(json: &serde_json::Value) -> Result<Vec<MappingEntry>, String> {
    let arr = json.as_array().ok_or("mappings 应为数组")?;
    let mut mappings = Vec::new();

    for m in arr {
        mappings.push(MappingEntry {
            feishu_field_name: m["feishuFieldName"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            feishu_field_type: m["feishuFieldType"].as_i64().unwrap_or(1) as i32,
            local_column: m["localColumn"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            sync_direction: m["syncDirection"]
                .as_str()
                .unwrap_or("bidirectional")
                .to_string(),
            is_formula: m["isFormula"].as_bool().unwrap_or(false)
                || m["isFormula"].as_i64().unwrap_or(0) != 0,
            is_link: m["isLink"].as_bool().unwrap_or(false)
                || m["isLink"].as_i64().unwrap_or(0) != 0,
        });
    }

    Ok(mappings)
}

/// 导入单条记录到本地表
fn import_record(
    conn: &rusqlite::Connection,
    local_table: &str,
    record: &sync::feishu::BitableRecordInfo,
    mappings: &[MappingEntry],
) -> anyhow::Result<String> {
    // 检查 sync_map 是否已有映射
    let existing: Option<String> = conn
        .query_row(
            "SELECT local_id FROM sync_map WHERE remote_id = ?1 AND remote_source = 'feishu'",
            rusqlite::params![record.record_id],
            |row| row.get(0),
        )
        .ok();

    let mut col_values: Vec<(String, String)> = Vec::new();

    for mapping in mappings {
        // 跳过不拉取的方向
        if mapping.sync_direction == "push_only" || mapping.sync_direction == "none" {
            continue;
        }

        // 跳过链接字段
        if mapping.is_link {
            continue;
        }

        let feishu_val = record.fields.get(&mapping.feishu_field_name);
        if let Some(val) = feishu_val {
            if !val.is_null() {
                let str_val = sync::feishu::extract_field_value_as_string(
                    val,
                    mapping.feishu_field_type,
                );
                if let Some(sv) = str_val {
                    col_values.push((mapping.local_column.clone(), sv));
                }
            }
        }
    }

    if let Some(local_id) = existing {
        // 更新已有记录
        if !col_values.is_empty() {
            let set_clause: Vec<String> = col_values
                .iter()
                .enumerate()
                .map(|(i, (col, _))| format!("{} = ?{}", col, i + 1))
                .collect();
            let sql = format!(
                "UPDATE {} SET {}, updated_at = datetime('now','localtime') WHERE id = ?{}",
                local_table,
                set_clause.join(", "),
                col_values.len() + 1
            );
            let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = col_values
                .iter()
                .map(|(_, v)| Box::new(v.clone()) as Box<dyn rusqlite::types::ToSql>)
                .collect();
            params.push(Box::new(local_id.clone()));
            let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
            conn.execute(&sql, param_refs.as_slice())?;

            // 更新 sync_map
            conn.execute(
                "UPDATE sync_map SET sync_status = 'synced', last_synced_at = ?1,
                 remote_updated = ?2 WHERE remote_id = ?3 AND remote_source = 'feishu'",
                rusqlite::params![
                    crate::db::now_local(),
                    record.last_modified_time.map(|t| t.to_string()).unwrap_or_default(),
                    record.record_id,
                ],
            )?;
        }
        Ok("updated".to_string())
    } else {
        // 创建新记录
        let local_id = crate::db::new_id();

        // 确保有 id 和 created_at/updated_at
        let all_cols: Vec<String> = col_values.iter().map(|(c, _)| c.clone()).collect();
        let all_vals: Vec<String> = col_values.iter().map(|(_, v)| v.clone()).collect();

        // Add id, created_at, updated_at, feishu_record_id
        let all_col_str = format!(
            "id, feishu_record_id, {}, created_at, updated_at",
            all_cols.join(", ")
        );
        let placeholders: Vec<String> = (1..=all_vals.len() + 4)
            .map(|i| format!("?{}", i))
            .collect();

        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            local_table,
            all_col_str,
            placeholders.join(", ")
        );

        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        params.push(Box::new(local_id.clone()));
        params.push(Box::new(record.record_id.clone()));
        for v in &all_vals {
            params.push(Box::new(v.clone()));
        }
        params.push(Box::new(crate::db::now_local()));
        params.push(Box::new(crate::db::now_local()));
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        // 先检查 feishu_record_id 列是否存在
        let has_feishu_col = get_local_table_columns(conn, local_table)
            .map(|cols| cols.iter().any(|(name, _)| name == "feishu_record_id"))
            .unwrap_or(false);

        if !has_feishu_col {
            // 如果没有 feishu_record_id 列，使用简化插入
            let simple_cols: Vec<String> = ["id"]
                .iter()
                .map(|s| s.to_string())
                .chain(all_cols.iter().cloned())
                .chain(["created_at".to_string(), "updated_at".to_string()].iter().cloned())
                .collect();
            let simple_placeholders: Vec<String> = (1..=simple_cols.len())
                .map(|i| format!("?{}", i))
                .collect();
            let simple_sql = format!(
                "INSERT INTO {} ({}) VALUES ({})",
                local_table,
                simple_cols.join(", "),
                simple_placeholders.join(", ")
            );
            let mut simple_params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
            simple_params.push(Box::new(local_id.clone()));
            for v in &all_vals {
                simple_params.push(Box::new(v.clone()));
            }
            simple_params.push(Box::new(crate::db::now_local()));
            simple_params.push(Box::new(crate::db::now_local()));
            let simple_refs: Vec<&dyn rusqlite::types::ToSql> =
                simple_params.iter().map(|p| p.as_ref()).collect();
            conn.execute(&simple_sql, simple_refs.as_slice())?;
        } else {
            conn.execute(&sql, param_refs.as_slice())?;
        }

        // 记录到 sync_map
        conn.execute(
            "INSERT INTO sync_map (id, local_table, local_id, remote_id, remote_source,
             remote_updated, sync_status, last_synced_at)
             VALUES (?1, ?2, ?3, ?4, 'feishu', ?5, 'synced', ?6)",
            rusqlite::params![
                crate::db::new_id(),
                local_table,
                local_id,
                record.record_id,
                record.last_modified_time.map(|t| t.to_string()).unwrap_or_default(),
                crate::db::now_local(),
            ],
        )?;

        Ok("created".to_string())
    }
}

// ============================================================
// Phase 4: 双向同步引擎
// ============================================================

/// 通用 Pull：从飞书拉取变更到本地（基于映射配置）
#[tauri::command]
pub async fn feishu_sync_pull(
    app_token: String,
    table_id: String,
    local_table: String,
    mappings_json: serde_json::Value,
) -> Result<sync::feishu::FeishuSyncReport, String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(async {
        let mappings = parse_mappings(&mappings_json)?;
        let sync_mappings: Vec<sync::feishu::SyncMappingEntry> = mappings
            .into_iter()
            .map(|m| sync::feishu::SyncMappingEntry {
                feishu_field_name: m.feishu_field_name,
                feishu_field_type: m.feishu_field_type,
                local_column: m.local_column,
                sync_direction: m.sync_direction,
                is_formula: m.is_formula,
                is_link: m.is_link,
            })
            .collect();
        sync::feishu::sync_table_pull(&app_token, &table_id, &local_table, &sync_mappings)
            .await
            .map_err(|e| e.to_string())
    })
}

/// 通用 Push：将本地变更推送到飞书（基于映射配置）
#[tauri::command]
pub async fn feishu_sync_push(
    app_token: String,
    table_id: String,
    local_table: String,
    mappings_json: serde_json::Value,
) -> Result<sync::feishu::FeishuSyncReport, String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(async {
        let mappings = parse_mappings(&mappings_json)?;
        let sync_mappings: Vec<sync::feishu::SyncMappingEntry> = mappings
            .into_iter()
            .map(|m| sync::feishu::SyncMappingEntry {
                feishu_field_name: m.feishu_field_name,
                feishu_field_type: m.feishu_field_type,
                local_column: m.local_column,
                sync_direction: m.sync_direction,
                is_formula: m.is_formula,
                is_link: m.is_link,
            })
            .collect();
        sync::feishu::sync_table_push(&app_token, &table_id, &local_table, &sync_mappings)
            .await
            .map_err(|e| e.to_string())
    })
}
