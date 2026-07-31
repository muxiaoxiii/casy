pub mod feishu;
pub mod webdav;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub webdav_connected: bool,
    pub webdav_url: String,
    pub last_sync_at: Option<String>,
    pub device_version: u64,
    pub remote_etag: Option<String>,
    pub pending_changes: bool,
}

#[derive(Debug, Serialize, Deserialize)]
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
pub fn get_sync_status() -> SyncStatus {
    // TODO: 从 settings 表读取配置
    SyncStatus {
        webdav_connected: false,
        webdav_url: String::new(),
        last_sync_at: None,
        device_version: 0,
        remote_etag: None,
        pending_changes: false,
    }
}

/// VACUUM INTO 安全拷贝数据库
/// 创建数据库的一致性快照，避免并发读写问题
fn vacuum_into(db_path: &std::path::Path, output_path: &std::path::Path) -> Result<()> {
    let conn = rusqlite::Connection::open(db_path)?;
    let output_str = output_path.to_string_lossy().replace('\'', "''");
    conn.execute_batch(&format!("VACUUM INTO '{}'", output_str))?;
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
    let client = webdav::WebDavClient::new(webdav_url, username, password)?;

    // 检查远程文件
    let remote_etag = client.head("casy.db").await?;

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
                message: "数据库已是最新".into(),
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
}

/// 手动同步：PUSH 本地到远程
/// 流程：
/// 1. VACUUM INTO 创建安全拷贝
/// 2. PUT 到临时路径
/// 3. MOVE 到正式路径（原子操作）
#[allow(dead_code)]
pub async fn manual_sync_push(
    webdav_url: &str,
    username: &str,
    password: &str,
    db_path: &std::path::Path,
) -> Result<SyncResult> {
    // VACUUM INTO 安全拷贝
    let temp_local = db_path.with_extension("db.upload");
    vacuum_into(db_path, &temp_local)?;

    let client = webdav::WebDavClient::new(webdav_url, username, password)?;
    let data = std::fs::read(&temp_local)?;

    // PUT 到临时路径
    let etag = client.put("casy.db.uploading", &data).await?;

    // MOVE 到正式路径（原子操作）
    client.move_resource("casy.db.uploading", "casy.db").await?;

    // 清理本地临时文件
    let _ = std::fs::remove_file(&temp_local);

    Ok(SyncResult {
        direction: "push".into(),
        success: true,
        message: "同步完成".into(),
        conflict: false,
        local_etag: Some(etag.clone()),
        remote_etag: Some(etag),
    })
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
    let client = webdav::WebDavClient::new(webdav_url, username, password)?;

    // GET 下载远程数据库
    let (data, etag) = client.get("casy.db").await?;

    // 写入临时文件
    let temp_local = db_path.with_extension("db.download");
    std::fs::write(&temp_local, &data)?;

    // 验证数据库完整性
    {
        let conn = rusqlite::Connection::open(&temp_local)?;
        let integrity: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        if integrity != "ok" {
            let _ = std::fs::remove_file(&temp_local);
            anyhow::bail!("Downloaded database failed integrity check: {}", integrity);
        }
    }

    // 备份原数据库
    let backup_path = db_path.with_extension("db.bak");
    let _ = std::fs::copy(db_path, &backup_path);

    // 替换本地数据库
    std::fs::rename(&temp_local, db_path)?;

    Ok(SyncResult {
        direction: "pull".into(),
        success: true,
        message: "拉取完成".into(),
        conflict: false,
        local_etag: Some(etag.clone()),
        remote_etag: Some(etag),
    })
}

/// 冲突解决：保留本地版本（上传覆盖远程）
#[allow(dead_code)]
pub async fn resolve_keep_local(
    webdav_url: &str,
    username: &str,
    password: &str,
    db_path: &std::path::Path,
) -> Result<SyncResult> {
    manual_sync_push(webdav_url, username, password, db_path).await
}

/// 冲突解决：保留远程版本（下载覆盖本地）
#[allow(dead_code)]
pub async fn resolve_keep_remote(
    webdav_url: &str,
    username: &str,
    password: &str,
    db_path: &std::path::Path,
) -> Result<SyncResult> {
    manual_sync_pull(webdav_url, username, password, db_path).await
}
