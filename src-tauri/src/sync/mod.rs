pub mod caldav;
pub mod feishu;
pub mod webdav;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// WebDAV 冲突提示：远程文件在比对之后被其他设备修改（HTTP 412）
pub const WEBDAV_CONFLICT_MESSAGE: &str = "远程文件已被其他设备修改，请重新比对后再决定";

/// If-Match 基线：比对时观察到的远程 ETag（settings.webdav_observed_etag）
///
/// 启动同步 HEAD 到远程 ETag 时写入；手动推送与冲突解决据此做条件上传，
/// 成功后再刷新为新的 ETag，避免基线过期导致误报冲突（S-4 / P0-2）。
const OBSERVED_ETAG_KEY: &str = "webdav_observed_etag";

/// 记录比对时观察到的远程 ETag（None = 远程不存在，清除基线）
pub fn record_observed_remote_etag(etag: Option<&str>) -> Result<()> {
    let mut conn = crate::db::open_db()?;
    let tx = conn.transaction()?;
    match etag {
        Some(etag) => crate::db::set_setting(&tx, OBSERVED_ETAG_KEY, etag)?,
        None => {
            tx.execute("DELETE FROM settings WHERE key = ?1", [OBSERVED_ETAG_KEY])?;
        }
    }
    tx.commit()?;
    Ok(())
}

/// 读取比对时观察到的远程 ETag（条件上传的 If-Match 基线）
pub fn observed_remote_etag(conn: &rusqlite::Connection) -> Option<String> {
    crate::db::get_setting(conn, OBSERVED_ETAG_KEY)
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
}

#[derive(Debug, Serialize, Deserialize, Clone, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub configured: bool,
    pub connection_state: String,
    pub last_checked_at: Option<String>,
    pub last_error: Option<String>,
    pub webdav_connected: bool,
    pub webdav_url: String,
    pub last_sync_at: Option<String>,
    pub device_version: Option<u64>,
    pub remote_etag: Option<String>,
    pub pending_changes: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct SyncResult {
    pub direction: String,
    pub success: bool,
    pub message: String,
    pub conflict: bool,
    pub local_etag: Option<String>,
    pub remote_etag: Option<String>,
}

/// 获取当前同步状态
pub fn get_sync_status(conn: &rusqlite::Connection) -> Result<SyncStatus> {
    let get = |name: &str| crate::db::get_setting(conn, name);
    let url = get("webdavUrl")?.filter(|s| !s.is_empty()).or(get("webdav_url")?).unwrap_or_default();
    let username = get("webdavUsername")?.filter(|s| !s.is_empty()).or(get("webdav_username")?).unwrap_or_default();
    let configured = !url.is_empty() && !username.is_empty();
    let matches_config = configured && get("webdav_status_url")?.as_deref() == Some(url.as_str())
        && get("webdav_status_username")?.as_deref() == Some(username.as_str());
    let state = if matches_config { get("webdav_connection_state")?.unwrap_or_else(|| "unknown".into()) }
        else if configured { "unknown".into() } else { "unconfigured".into() };
    Ok(SyncStatus {
        configured,
        webdav_connected: state == "connected",
        connection_state: state,
        last_checked_at: if matches_config { get("webdav_last_checked_at")? } else { None },
        last_error: if matches_config { get("webdav_last_error")?.filter(|s| !s.is_empty()) } else { None },
        webdav_url: url,
        last_sync_at: if matches_config { get("webdav_last_sync_at")? } else { None },
        device_version: None, // No device revision protocol exists yet.
        remote_etag: if matches_config { get("webdav_last_etag")? } else { None },
        pending_changes: None, // Unknown is not the same as a clean, synchronized database.
    })
}

pub fn record_webdav_status(conn: &mut rusqlite::Connection, url: &str, username: &str, error: Option<&str>, synced: bool, etag: Option<&str>) -> Result<()> {
    let tx = conn.transaction()?;
    let now = crate::db::now_local();
    if crate::db::get_setting(&tx, "webdav_status_url")?.as_deref() != Some(url)
        || crate::db::get_setting(&tx, "webdav_status_username")?.as_deref() != Some(username) {
        tx.execute("DELETE FROM settings WHERE key IN ('webdav_last_sync_at','webdav_last_etag','webdav_observed_etag')", [])?;
    }
    for (name, value) in [("webdav_status_url", url), ("webdav_status_username", username),
        ("webdav_connection_state", if error.is_some() { "failed" } else { "connected" }),
        ("webdav_last_checked_at", now.as_str()), ("webdav_last_error", error.unwrap_or(""))] {
        crate::db::set_setting(&tx, name, value)?;
    }
    if synced { crate::db::set_setting(&tx, "webdav_last_sync_at", &now)?; }
    if let Some(etag) = etag { crate::db::set_setting(&tx, "webdav_last_etag", etag)?; }
    tx.commit()?;
    Ok(())
}

/// WebDAV 同步：启动时检查
/// 流程：
/// 1. HEAD 检查远程文件 ETag
/// 2. 比较本地 ETag（从 sync_map 读取）
/// 3. 决定 push/pull/冲突
#[allow(dead_code)]
pub async fn startup_sync(
    webdav_url: &str,
    username: &str,
    password: &str,
    _db_path: &std::path::Path,
    local_etag: Option<&str>,
) -> Result<SyncResult> {
    crate::processing::tracked("sync","WebDAV 启动同步",async {
    let client = webdav::WebDavClient::new(webdav_url, username, password)?;

    // 检查远程文件
    let remote_etag = client.head("casy.db").await?;

    // 记录比对时观察到的远程 ETag：后续手动推送 / 冲突解决用它做 If-Match
    // 基线，远程在用户决策前又被其他设备修改时拒绝覆盖（S-4 / P0-2）
    if let Err(e) = record_observed_remote_etag(remote_etag.as_deref()) {
        log::warn!("记录远程 ETag 基线失败: {}", e);
    }

    if remote_etag.is_none() {
        // 远程不存在，首次上传
        return Ok(SyncResult {
            direction: "first_push".into(),
            success: true,
            message: "远程数据库不存在，需要首次上传".into(),
            conflict: false,
            local_etag: local_etag.map(|s| s.to_string()),
            remote_etag: None,
        });
    }

    let remote_etag = remote_etag.unwrap();

    // 比较 ETag
    match local_etag {
        Some(local) if local == remote_etag => {
            // ETag 相同，无需同步
            Ok(SyncResult {
                direction: "none".into(),
                success: true,
                message: "远程版本未变化；本地是否有新修改尚未判断".into(),
                conflict: false,
                local_etag: Some(local.to_string()),
                remote_etag: Some(remote_etag),
            })
        }
        Some(local) => {
            // ETag 不同，存在冲突
            Ok(SyncResult {
                direction: "conflict".into(),
                success: false,
                message: "数据库版本冲突，需要手动解决".into(),
                conflict: true,
                local_etag: Some(local.to_string()),
                remote_etag: Some(remote_etag),
            })
        }
        None => {
            // 本地无 ETag 记录，需要拉取
            Ok(SyncResult {
                direction: "pull".into(),
                success: true,
                message: "需要从远程拉取数据库".into(),
                conflict: false,
                local_etag: None,
                remote_etag: Some(remote_etag),
            })
        }
    }

    }).await
}

/// 手动同步：PUSH 本地到远程
/// 流程：
/// 1. VACUUM INTO 创建安全拷贝（加密快照，绝不上传数据库密钥）
/// 2. 有远程 ETag 基线时直接带 `If-Match` 条件 PUT
/// 3. 无基线（首次上传/服务器不返回 ETag）才走 临时文件 + MOVE 原子路径
///
/// `expected_etag` 为比对时观察到的远程 ETag（settings.webdav_observed_etag）。
/// 远程已被其他设备修改则报冲突而不是无条件覆盖（S-4 / P0-2）。
#[allow(dead_code)]
pub async fn manual_sync_push(
    webdav_url: &str,
    username: &str,
    password: &str,
    db_path: &std::path::Path,
    expected_etag: Option<&str>,
) -> Result<SyncResult> {
    crate::processing::tracked("sync","WebDAV 上传",async {
    anyhow::ensure!(db_path == crate::db::get_db_path(), "同步仅支持当前资料库");
    // Use the encrypted, verified snapshot path. Never upload the database key.
    let snapshot = crate::commands::backup::create_backup().await.map_err(anyhow::Error::msg)?;
    let temp_local = crate::commands::backup::backups_dir().join(&snapshot.filename);
    let client = webdav::WebDavClient::new(webdav_url, username, password)?;
    let data = std::fs::read(&temp_local)?;
    // 条件上传基线：优先用比对时观察到的远程 ETag；没有（从未比对/远程刚被
    // 其他设备清掉）时现场 HEAD 取当前 ETag。两者都没有（首次上传、服务器不
    // 返回 ETag）才回退到原有的 临时文件 + MOVE 原子路径。
    let baseline = match expected_etag {
        Some(expected) if !expected.is_empty() => Some(expected.to_string()),
        _ => match client.head("casy.db").await {
            Ok(etag) => etag.filter(|etag| !etag.is_empty()),
            // 服务器不支持 HEAD 等异常：退回原有 临时文件 + MOVE 路径，不阻断推送
            Err(e) => {
                log::warn!("读取远程 ETag 失败，退回非条件上传: {}", e);
                None
            }
        },
    };
    let etag = match baseline {
        // 已知远程基线：条件上传，远程已变则 412 → 冲突提示（不覆盖）
        Some(expected) => client.put_if_match("casy.db", &data, &expected).await?,
        None => {
            let remote_temp = format!("casy.db.{}.uploading", uuid::Uuid::new_v4());
            client.put(&remote_temp, &data).await?;
            client.move_resource(&remote_temp, "casy.db").await?;
            client.head("casy.db").await?.unwrap_or_default()
        }
    };

    // 刷新基线为本次上传后的 ETag，避免下次推送用过期的 If-Match 误报冲突
    if let Err(e) = record_observed_remote_etag(Some(&etag)) {
        log::warn!("刷新远程 ETag 基线失败: {}", e);
    }

    Ok(SyncResult {
        direction: "push".into(),
        success: true,
        message: "同步完成".into(),
        conflict: false,
        local_etag: Some(etag.clone()),
        remote_etag: Some(etag),
    })

    }).await
}

/// 手动同步：PULL 远程到本地
/// 流程：
/// 1. GET 下载远程数据库
/// 2. 写入本地临时文件
/// 3. 验证完整性
/// 4. 替换本地数据库
#[allow(dead_code)]
pub async fn manual_sync_pull(
    webdav_url: &str,
    username: &str,
    password: &str,
    db_path: &std::path::Path,
) -> Result<SyncResult> {
    crate::processing::tracked("sync","WebDAV 下载",async {
    anyhow::ensure!(db_path == crate::db::get_db_path(), "同步仅支持当前资料库");
    let client = webdav::WebDavClient::new(webdav_url, username, password)?;
    let (data, etag) = client.get("casy.db").await?;
    let dir = crate::commands::backup::backups_dir();
    std::fs::create_dir_all(&dir)?;
    let mut downloaded = tempfile::Builder::new().prefix("casy-backup-webdav-").suffix(".db").tempfile_in(&dir)?;
    use std::io::Write;
    downloaded.write_all(&data)?;
    downloaded.as_file().sync_all()?;
    let key = crate::db::get_or_create_encryption_key()?;
    crate::commands::backup::verify_db_file_integrity(downloaded.path(), &key)
        .map_err(|_| anyhow::anyhow!("远程数据库损坏或密钥不匹配；本地数据和密钥未更改。新设备请先通过完整加密备份迁移。"))?;
    let filename = downloaded.path().file_name().unwrap().to_string_lossy().into_owned();
    crate::commands::backup::restore_backup(filename).await.map_err(anyhow::Error::msg)?;

    // 刷新基线为拉取到的 ETag，避免后续推送用过期的 If-Match 误报冲突
    if let Err(e) = record_observed_remote_etag(Some(&etag)) {
        log::warn!("刷新远程 ETag 基线失败: {}", e);
    }

    Ok(SyncResult {
        direction: "pull".into(),
        success: true,
        message: "拉取完成，请重启应用刷新资料库".into(),
        conflict: false,
        local_etag: Some(etag.clone()),
        remote_etag: Some(etag),
    })

    }).await
}

/// 冲突解决：保留本地版本（上传覆盖远程）
///
/// `expected_etag` 为比对时观察到的远程 ETag：上传改为带 If-Match 的条件
/// PUT，远程已被其他设备修改时报冲突而不是无条件覆盖（S-4 / P0-2）。
#[allow(dead_code)]
pub async fn resolve_keep_local(
    webdav_url: &str,
    username: &str,
    password: &str,
    db_path: &std::path::Path,
    expected_etag: Option<&str>,
) -> Result<SyncResult> {
    manual_sync_push(webdav_url, username, password, db_path, expected_etag).await
}

/// 冲突解决：保留远程版本（下载覆盖本地）
///
/// 先核对远程 ETag：若与比对时观察到的 ETag 不一致，说明用户在查看冲突后
/// 远程又被其他设备修改，此时中止下载并提示重新比对（S-4 / P0-2）。
#[allow(dead_code)]
pub async fn resolve_keep_remote(
    webdav_url: &str,
    username: &str,
    password: &str,
    db_path: &std::path::Path,
    expected_etag: Option<&str>,
) -> Result<SyncResult> {
    let client = webdav::WebDavClient::new(webdav_url, username, password)?;
    if let Some(expected) = expected_etag {
        let current = client.head("casy.db").await?;
        if current.as_deref() != Some(expected) {
            anyhow::bail!(WEBDAV_CONFLICT_MESSAGE);
        }
    }
    manual_sync_pull(webdav_url, username, password, db_path).await
}
