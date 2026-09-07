//! 数据备份与恢复（R-5 · 本地优先的"不丢数据"承诺）
//!
//! 备份 = `VACUUM INTO` 一致性快照（SQLCipher 加密随库保持）到
//! `<data>/Casy/backups/casy-backup-<时间戳>.db`
//!
//! 恢复 = 独占维护模式锁 + 预检备份完整性 + 自动 pre-restore 快照 + 覆盖后自愈校验

use super::run_blocking;
use crate::db;
use anyhow::{anyhow, Result};
use rusqlite::Connection;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupFile {
    pub filename: String,
    pub size_bytes: u64,
    pub modified_at: String,
    pub sha256: Option<String>,
    pub verified: bool,
}

pub fn backups_dir() -> PathBuf {
    db::get_db_path()
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("backups")
}

/// 计算文件的 SHA-256 校验和
pub fn calculate_file_sha256(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// 验证 SQLite / SQLCipher 数据库文件完整性与解密可用性
pub fn verify_db_file_integrity(path: &Path, key: &str) -> Result<()> {
    if !path.exists() {
        return Err(anyhow!("数据库文件不存在: {:?}", path));
    }

    let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    if key.len() != 64 || !key.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(anyhow!("数据库密钥格式无效"));
    }
    conn.execute_batch(&format!("PRAGMA key = \"x'{}'\";", key))?;
    conn.execute_batch("PRAGMA busy_timeout=5000;")?;

    // 1. 测试解密：查询 sqlite_master
    let table_count: Result<i64, _> =
        conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| r.get(0));

    if table_count.is_err() {
        return Err(anyhow!("SQLCipher 密钥不匹配或库损坏，无法解密"));
    }

    // 2. 快速完整性校验
    let check_res: String = conn
        .query_row("PRAGMA quick_check;", [], |r| r.get(0))
        .map_err(|e| anyhow!("PRAGMA quick_check 执行失败: {e}"))?;

    if check_res != "ok" {
        return Err(anyhow!("数据库完整性检查未通过: {check_res}"));
    }

    Ok(())
}

/// 创建全量加密快照备份
#[tauri::command]
pub async fn create_backup() -> Result<BackupFile, String> {
    run_blocking(move || {
        let dir = backups_dir();
        std::fs::create_dir_all(&dir).map_err(|e| anyhow!("创建备份目录失败: {e}"))?;
        let ts = format!("{}-{}", chrono::Local::now().format("%Y%m%d-%H%M%S"), uuid::Uuid::new_v4());
        let dest = dir.join(format!("casy-backup-{ts}.db"));

        let key = db::get_or_create_encryption_key()?;

        db::with_conn(|conn| {
            // VACUUM INTO：生成事务一致的快照（含加密），不阻塞后续写入
            conn.execute(
                "VACUUM INTO ?1",
                rusqlite::params![dest.to_string_lossy().as_ref()],
            )
            .map_err(|e| anyhow!("备份执行失败: {e}"))?;
            Ok(())
        })?;

        // 备份落盘后立即执行解密与完整性校验（P1-4）
        if let Err(e) = verify_db_file_integrity(&dest, &key) {
            let _ = std::fs::remove_file(&dest);
            return Err(anyhow!("备份生成后自检失败，已自动销毁损坏文件: {e}"));
        }

        let checksum = calculate_file_sha256(&dest).ok();
        let meta = std::fs::metadata(&dest).map_err(|e| anyhow!("{e}"))?;

        Ok(BackupFile {
            filename: dest
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default(),
            size_bytes: meta.len(),
            modified_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            sha256: checksum,
            verified: true,
        })
    })
    .await
}

/// 列出所有备份文件
#[tauri::command]
pub async fn list_backups() -> Result<Vec<BackupFile>, String> {
    run_blocking(move || {
        let dir = backups_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut out: Vec<BackupFile> = std::fs::read_dir(&dir)
            .map_err(|e| anyhow!("{e}"))?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                if (!name.starts_with("casy-backup-") && !name.starts_with("pre-restore-"))
                    || !name.ends_with(".db")
                {
                    return None;
                }
                let meta = entry.metadata().ok()?;
                let modified = meta
                    .modified()
                    .ok()
                    .map(chrono::DateTime::<chrono::Local>::from)
                    .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_default();
                Some(BackupFile {
                    filename: name,
                    size_bytes: meta.len(),
                    modified_at: modified,
                    sha256: None, // 列表展示时按需轻量获取
                    verified: false,
                })
            })
            .collect();
        out.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
        Ok(out)
    })
    .await
}

/// 显式校验某个备份文件的完整性与解密性
#[tauri::command]
pub async fn verify_backup_integrity_cmd(filename: String) -> Result<bool, String> {
    run_blocking(move || {
        validate_backup_filename(&filename)?;
        let bdir = backups_dir();
        let src = bdir.join(&filename);
        let key = db::get_or_create_encryption_key()?;
        verify_db_file_integrity(&src, &key)?;
        Ok(true)
    })
    .await
}

/// 严格备份文件名校验（防路径穿越与非法注入）
pub fn validate_backup_filename(filename: &str) -> Result<()> {
    let stem = if let Some(s) = filename.strip_prefix("casy-backup-") {
        s.strip_suffix(".db")
    } else if let Some(s) = filename.strip_prefix("pre-restore-") {
        s.strip_suffix(".db")
    } else {
        None
    };

    let stem = match stem {
        Some(s) if !s.is_empty() => s,
        _ => {
            return Err(anyhow!(
                "非法备份文件名: 必须以 casy-backup- 或 pre-restore- 开头并以 .db 结尾，且时间戳不可为空"
            ))
        }
    };

    if filename.contains('/')
        || filename.contains('\\')
        || filename.contains("..")
        || filename.contains('[')
        || filename.contains(']')
        || filename.contains('\0')
    {
        return Err(anyhow!("非法备份文件名: 包含非法字符或路径遍历特征"));
    }

    // 仅允许字母、数字、短横线与下划线
    if !stem
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(anyhow!("非法备份文件名: 包含未允许的字符"));
    }

    Ok(())
}

/// 恢复备份：独占维护模式 + 完整性前置校验 + pre-restore 快照 + 失败自愈回滚
#[tauri::command]
pub async fn restore_backup(filename: String) -> Result<bool, String> {
    run_blocking(move || {
        // 1. 严格防路径穿越校验
        validate_backup_filename(&filename)?;

        let bdir = backups_dir();
        let src = bdir.join(&filename);
        if !src.exists() {
            return Err(anyhow!("备份文件不存在: {}", filename));
        }

        // 验证规范化路径在 backups_dir 内
        let can_src = src
            .canonicalize()
            .map_err(|e| anyhow!("备份源路径校验失败: {e}"))?;
        let can_dir = bdir
            .canonicalize()
            .map_err(|e| anyhow!("备份目录校验失败: {e}"))?;
        if !can_src.starts_with(&can_dir) {
            return Err(anyhow!("备份路径非法越界"));
        }

        let key = db::get_or_create_encryption_key()?;

        // 2. 恢复前置校验：在动活动库前，先对源备份做解密与完整性检验
        verify_db_file_integrity(&can_src, &key)
            .map_err(|e| anyhow!("目标备份文件损坏或密码不匹配，已中止恢复: {e}"))?;

        // 3. 申请系统独占维护锁（阻止所有并发 IPC 读写并释放共享连接文件句柄）
        let _guard = db::enter_maintenance()?;

        let live = db::get_db_path();
        let conn = db::open_db_encrypted()?;
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;

        // 4. 恢复前自保快照：pre-restore-<ts>.db
        let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let pre_restore_path = bdir.join(format!("pre-restore-{ts}.db"));
        if live.exists() {
            conn.execute("VACUUM INTO ?1", [pre_restore_path.to_string_lossy().as_ref()])?;
        }
        drop(conn);

        // 5. 覆盖活动数据库文件
        let next = live.with_extension("restore-next");
        let previous = live.with_extension("restore-previous");
        std::fs::copy(&can_src, &next)?;
        std::fs::File::open(&next)?.sync_all()?;
        if previous.exists() { std::fs::remove_file(&previous)?; }
        std::fs::rename(&live, &previous)?;
        if let Err(error) = std::fs::rename(&next, &live) {
            std::fs::rename(&previous, &live)?;
            return Err(error.into());
        }

        // 6. 覆盖后活动库一致性再确认
        if let Err(e) = verify_db_file_integrity(&live, &key) {
            log::error!("恢复后活动库自检未通过: {e}，正在执行自动回滚...");
            if pre_restore_path.exists() {
                let _ = std::fs::copy(&pre_restore_path, &live);
            }
            return Err(anyhow!("恢复后数据库自检失败，已自动回滚至恢复前状态: {e}"));
        }

        log::info!("Database restore completed successfully from {}", filename);
        Ok(true) // UI 提示重启生效
    })
    .await
}
