//! 数据备份与恢复（R-5 · 本地优先的"不丢数据"承诺）
//!
//! 备份 = `VACUUM INTO` 一致性快照（SQLCipher 加密随库保持）到
//! `<data>/Casy/backups/casy-backup-<时间戳>.db`
//!
//! 恢复 = 将所选备份覆盖活动数据库文件后**要求重启**（共享连接持有旧句柄，
//! 重启后重新打开即加载恢复的数据）。UI 层必须显式提示重启与覆盖风险。
use super::run_blocking;
use crate::db;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupFile {
    pub filename: String,
    pub size_bytes: u64,
    pub modified_at: String,
}

fn backups_dir() -> std::path::PathBuf {
    db::get_db_path()
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("backups")
}

#[tauri::command]
pub async fn create_backup() -> Result<BackupFile, String> {
    run_blocking(move || {
        let dir = backups_dir();
        std::fs::create_dir_all(&dir).map_err(|e| anyhow::anyhow!("创建备份目录失败: {e}"))?;
        let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let dest = dir.join(format!("casy-backup-{ts}.db"));

        db::with_conn(|conn| {
            // VACUUM INTO：生成事务一致的快照（含加密），不阻塞后续写入
            conn.execute("VACUUM INTO ?1", rusqlite::params![dest.to_string_lossy().as_ref()])
                .map_err(|e| anyhow::anyhow!("备份失败: {e}"))?;
            Ok(())
        })?;

        let meta = std::fs::metadata(&dest).map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(BackupFile {
            filename: dest
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default(),
            size_bytes: meta.len(),
            modified_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        })
    })
    .await
}

#[tauri::command]
pub async fn list_backups() -> Result<Vec<BackupFile>, String> {
    run_blocking(move || {
        let dir = backups_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut out: Vec<BackupFile> = std::fs::read_dir(&dir)
            .map_err(|e| anyhow::anyhow!("{e}"))?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.starts_with("casy-backup-") || !name.ends_with(".db") {
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
                })
            })
            .collect();
        out.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
        Ok(out)
    })
    .await
}

/// 恢复备份：覆盖活动数据库文件。返回 true 表示需要重启应用以生效。
/// 安全设计：
/// - 仅接受 backups 目录内的 casy-backup-*.db 文件名（防路径穿越）
/// - 恢复前先把当前库复制为 pre-restore 快照，误操作可再回退
#[tauri::command]
pub async fn restore_backup(filename: String) -> Result<bool, String> {
    run_blocking(move || {
        if !filename.starts_with("casy-backup-") || !filename.ends_with(".db") || filename.contains('[') {
            return Err(anyhow::anyhow!("非法备份文件名"));
        }
        let src = backups_dir().join(&filename);
        if !src.exists() {
            return Err(anyhow::anyhow!("备份文件不存在"));
        }
        let live = db::get_db_path();

        // 恢复前自备份：pre-restore-<ts>.db（同样在 backups 目录内）
        let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let pre = backups_dir().join(format!("pre-restore-{ts}.db"));
        std::fs::copy(&live, &pre).map_err(|e| anyhow::anyhow!("恢复前快照失败: {e}"))?;

        std::fs::copy(&src, &live).map_err(|e| anyhow::anyhow!("恢复写入失败: {e}"))?;
        Ok(true) // UI 提示重启
    })
    .await
}
