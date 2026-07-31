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
