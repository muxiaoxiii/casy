use serde::{Deserialize, Serialize};

use super::run_blocking;
use crate::db;

#[derive(Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CaseFile {
    pub id: String,
    pub case_id: String,
    pub file_name: String,
    pub file_path: String,
    pub file_size: Option<i64>,
    pub file_type: Option<String>,
    pub category: String,
    pub sub_category: Option<String>,
    pub created_at: Option<String>,
}

/// 列出案件的文件
#[tauri::command]
pub async fn list_case_files(
    case_id: String,
    category: Option<String>,
) -> Result<Vec<CaseFile>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let sql = if category.is_some() {
            "SELECT id, case_id, file_name, file_path, file_size, file_type, category, sub_category, created_at
             FROM case_files WHERE case_id = ?1 AND category = ?2 ORDER BY created_at DESC"
        } else {
            "SELECT id, case_id, file_name, file_path, file_size, file_type, category, sub_category, created_at
             FROM case_files WHERE case_id = ?1 ORDER BY created_at DESC"
        };

        let mut stmt = conn.prepare(sql)?;
        let rows = if let Some(ref cat) = category {
            stmt.query_map(rusqlite::params![case_id, cat], map_file_row)?
        } else {
            stmt.query_map(rusqlite::params![case_id], map_file_row)?
        };

        let mut files = Vec::new();
        for row in rows {
            files.push(row?);
        }
        Ok(files)
    })
    .await
}

/// 添加案件文件记录
#[tauri::command]
pub async fn add_case_file(
    case_id: String,
    file_name: String,
    file_path: String,
    category: String,
) -> Result<CaseFile, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let id = db::new_id();
        // 审计 P1#1：拒绝登记不存在的文件（防"有记录、没文件"假卷宗）
        if !std::path::Path::new(&file_path).is_file() {
            return Err(anyhow::anyhow!("文件不存在，拒绝登记: {file_path}"));
        }
        let file_size = std::fs::metadata(&file_path)
            .ok()
            .map(|m| m.len() as i64);
        let file_type = std::path::Path::new(&file_path)
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_string());

        conn.execute(
            "INSERT INTO case_files (id, case_id, file_name, file_path, file_size, file_type, category)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![id, case_id, file_name, file_path, file_size, file_type, category],
        )?;

        Ok(CaseFile {
            id,
            case_id,
            file_name,
            file_path,
            file_size,
            file_type,
            category,
            sub_category: None,
            created_at: Some(db::now_local()),
        })
    })
    .await
}

/// 删除案件文件记录
#[tauri::command]
pub async fn delete_case_file(id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        conn.execute("DELETE FROM case_files WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    })
    .await
}

fn map_file_row(row: &rusqlite::Row) -> rusqlite::Result<CaseFile> {
    Ok(CaseFile {
        id: row.get(0)?,
        case_id: row.get(1)?,
        file_name: row.get(2)?,
        file_path: row.get(3)?,
        file_size: row.get(4)?,
        file_type: row.get(5)?,
        category: row.get(6)?,
        sub_category: row.get(7)?,
        created_at: row.get(8)?,
    })
}

// ============================================================
// 案卷管理 · 本地文件夹同步（index-v2 精装版规格 · 可交付目标③）
// ============================================================

use std::path::PathBuf;

/// 案件卷宗根目录（复用 files::ensure_case_folder 的命名规则）
fn case_root(case_id: &str) -> anyhow::Result<(PathBuf, crate::db::cases::Case)> {
    let conn = crate::db::open_db()?;
    let case = crate::db::cases::get_case(&conn, case_id)
        .map_err(|e| anyhow::anyhow!("案件不存在: {e}"))?;
    let root = crate::files::ensure_case_folder(&case)?;
    Ok((root, case))
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaseDirEntry {
    pub name: String,
    pub rel_path: String,
    pub file_count: u64,
    /// ok=有文件 · empty=空目录（warn 预留给"阶段应备未备"规则）
    pub state: String,
}

/// 列出案件卷宗的子目录与统计（确保目录树已按模板创建）
#[tauri::command]
pub async fn list_case_dirs(case_id: String) -> Result<Vec<CaseDirEntry>, String> {
    run_blocking(move || {
        let (root, _case) = case_root(&case_id)?;
        let mut out = Vec::new();
        let mut subs: Vec<PathBuf> = std::fs::read_dir(&root)
            ?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        subs.sort();
        for sub in subs {
            let name = sub.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let count = std::fs::read_dir(&sub)
                .map(|rd| rd.filter_map(|e| e.ok()).filter(|e| e.path().is_file()).count())
                .unwrap_or(0);
            out.push(CaseDirEntry {
                rel_path: name.clone(),
                name,
                file_count: count as u64,
                state: if count > 0 { "ok".into() } else { "empty".into() },
            });
        }
        Ok(out)
    })
    .await
}

/// 新建子文件夹（在卷宗根或指定子目录内）
#[tauri::command]
pub async fn create_case_subdir(case_id: String, parent_rel: Option<String>, name: String) -> Result<String, String> {
    run_blocking(move || {
        let name = name.trim();
        if name.is_empty() || name.contains('/') || name.contains("..") {
            return Err(anyhow::anyhow!("非法文件夹名"));
        }
        let (root, _) = case_root(&case_id)?;
        let target = match parent_rel.as_deref() {
            Some(p) if !p.is_empty() => root.join(p).join(name),
            _ => root.join(name),
        };
        std::fs::create_dir_all(&target)?;
        Ok(target.to_string_lossy().to_string())
    })
    .await
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedFile {
    pub id: String,
    pub file_name: String,
    pub archived_path: String,
    pub original_path: String,
}

/// 把系统拖入/选择的文件复制进指定子目录并登记（source_type='imported'）
#[tauri::command]
pub async fn import_files_to_case(
    case_id: String,
    dir_rel: Option<String>,
    paths: Vec<String>,
) -> Result<Vec<ImportedFile>, String> {
    run_blocking(move || {
        let (root, _case) = case_root(&case_id)?;
        let dir = match dir_rel.as_deref() {
            Some(d) if !d.is_empty() => root.join(d),
            _ => root.clone(),
        };
        std::fs::create_dir_all(&dir)?;

        let conn_guard = crate::db::open_db()?;
        let conn = &conn_guard;
        let mut out = Vec::new();

        for p in &paths {
            let src = PathBuf::from(p);
            if !src.is_file() {
                continue;
            }
            let orig_name = src.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            // 重名去重：name-1.ext / name-2.ext
            let stem = PathBuf::from(&orig_name)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| orig_name.clone());
            let ext = PathBuf::from(&orig_name)
                .extension()
                .map(|s| format!(".{}", s.to_string_lossy()))
                .unwrap_or_default();
            let mut final_name = orig_name.clone();
            let mut i = 1;
            while dir.join(&final_name).exists() {
                final_name = format!("{stem}-{i}{ext}");
                i += 1;
            }
            let dest = dir.join(&final_name);
            // 审计 P1#1：复制必须成功且字节数一致才允许登记
            let copied = std::fs::copy(&src, &dest)
                .map_err(|e| anyhow::anyhow!("复制 {orig_name} 失败: {e}"))?;
            let src_len = std::fs::metadata(&src).map(|m| m.len()).unwrap_or(0);
            if copied != src_len {
                let _ = std::fs::remove_file(&dest); // 清除残缺副本
                return Err(anyhow::anyhow!("复制不完整({orig_name}): {copied}/{src_len} 字节"));
            }

            // 登记（沿用 category='other'，前端可在详情里改）
            let fid = db::new_id();
            let size = std::fs::metadata(&dest).ok().map(|m| m.len() as i64);
            let ftype = PathBuf::new()
                .join(&final_name)
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.trim_start_matches('.').to_string());
            conn.execute(
                "INSERT INTO case_files (id, case_id, file_name, file_path, file_size, file_type, category, source_type)
                 VALUES (?1,?2,?3,?4,?5,?6,'other','imported')",
                rusqlite::params![fid, case_id, final_name, dest.to_string_lossy(), size, ftype],
            )
            ?;

            out.push(ImportedFile {
                id: fid,
                file_name: final_name,
                archived_path: dest.to_string_lossy().to_string(),
                original_path: p.clone(),
            });
        }
        Ok(out)
    })
    .await
}

/// 扫描磁盘上存在但未登记的文件（整理既有卷宗：批量入库入口）
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnregisteredFile {
    pub file_name: String,
    pub path: String,
    pub size_bytes: u64,
}

#[tauri::command]
pub async fn scan_unregistered_files(case_id: String) -> Result<Vec<UnregisteredFile>, String> {
    run_blocking(move || {
        let (root, _) = case_root(&case_id)?;
        let conn = crate::db::open_db()?;
        let known: std::collections::HashSet<String> = {
            let mut stmt = conn.prepare("SELECT file_path FROM case_files WHERE case_id = ?1")?;
            let rows = stmt.query_map(rusqlite::params![case_id], |r| r.get::<_, String>(0))?;
            rows.filter_map(|r| r.ok()).collect()
        };
        let mut out = Vec::new();
        fn walk(dir: &PathBuf, out: &mut Vec<UnregisteredFile>, known: &std::collections::HashSet<String>) {
            let Ok(rd) = std::fs::read_dir(dir) else { return };
            for e in rd.filter_map(|e| e.ok()) {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out, known);
                } else if let Some(ps) = p.to_str() {
                    if !known.contains(ps) {
                        if let Ok(meta) = e.metadata() {
                            out.push(UnregisteredFile {
                                file_name: p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
                                path: ps.to_string(),
                                size_bytes: meta.len(),
                            });
                        }
                    }
                }
            }
        }
        walk(&root, &mut out, &known);
        Ok(out)
    })
    .await
}

/// 批量登记已存在的文件（不移动，仅入库）
#[tauri::command]
pub async fn register_existing_files(case_id: String, paths: Vec<String>) -> Result<u32, String> {
    run_blocking(move || {
        let conn = crate::db::open_db()?;
        let mut n = 0u32;
        for p in &paths {
            let meta = std::fs::metadata(p).ok();
            let name = PathBuf::from(p)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let ftype = PathBuf::from(p)
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_string());
            conn.execute(
                "INSERT INTO case_files (id, case_id, file_name, file_path, file_size, file_type, category, source_type)
                 VALUES (?1,?2,?3,?4,?5,?6,'other','imported')",
                rusqlite::params![db::new_id(), case_id, name, p, meta.as_ref().map(|m| m.len() as i64), ftype],
            )
            ?;
            n += 1;
        }
        Ok(n)
    })
    .await
}

/// Finder/资源管理器中显示文件（定位）
#[tauri::command]
pub async fn reveal_path(path: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").args(["-R", &path]).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer").args(["/select,", &path]).spawn().map_err(|e| anyhow::anyhow!("{e}"))?;
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(parent) = std::path::Path::new(&path).parent() {
            std::process::Command::new("xdg-open").arg(parent).spawn().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// 用系统默认应用打开文件
#[tauri::command]
pub async fn open_file_with_default(path: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(&path).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd").args(["/C", "start", "", &path]).spawn().map_err(|e| anyhow::anyhow!("{e}"))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(&path).spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ============================================================
// 智能重命名工作台（index-v2 · 规则版 v1）
// ============================================================

#[derive(serde::Deserialize)]
pub struct RenameItem {
    pub id: String,
    pub new_name: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameOutcome {
    pub id: String,
    pub old_name: String,
    pub new_name: String,
}

/// 批量应用重命名：磁盘同名目录内改名 + DB file_name/file_path 同步
/// - 自动补扩展名（新名缺 ext 时保留原 ext）
/// - 目标名冲突时自动追加 -1/-2 序号，不覆盖他人文件
#[tauri::command]
pub async fn apply_case_file_renames(
    case_id: String,
    renames: Vec<RenameItem>,
) -> Result<Vec<RenameOutcome>, String> {
    run_blocking(move || {
        let conn = crate::db::open_db()?;
        let mut out = Vec::new();
        for r in &renames {
            let row: (String, String) = conn
                .query_row(
                    "SELECT file_name, file_path FROM case_files WHERE id = ?1 AND case_id = ?2",
                    rusqlite::params![r.id, case_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(|_| anyhow::anyhow!("文件不存在: {}", r.id))?;
            let (old_name, old_path_str) = row;
            let old_path = PathBuf::from(&old_path_str);
            let dir = old_path
                .parent()
                .map(|p| p.to_path_buf())
                .ok_or_else(|| anyhow::anyhow!("无法解析原路径"))?;

            // 扩展名保护：新名无 ext 时继承旧 ext
            let old_ext = old_path
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default();
            let mut target_name = r.new_name.trim().to_string();
            if !target_name.is_empty() && !target_name.contains('.') && !old_ext.is_empty() {
                target_name += &old_ext;
            }
            if target_name == old_name || target_name.contains('/') || target_name.contains("..") {
                return Err(anyhow::anyhow!("非法新名称: {target_name}"));
            }

            // 冲突去重
            let stem = PathBuf::from(&target_name)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| target_name.clone());
            let ext = PathBuf::from(&target_name)
                .extension()
                .map(|s| format!(".{}", s.to_string_lossy()))
                .unwrap_or_default();
            let mut final_name = target_name.clone();
            let mut i = 1;
            while dir.join(&final_name).exists() && final_name != old_name {
                final_name = format!("{stem}-{i}{ext}");
                i += 1;
            }

            let new_path = dir.join(&final_name);
            std::fs::rename(&old_path, &new_path)
                .map_err(|e| anyhow::anyhow!("改名失败({old_name} → {final_name}): {e}"))?;

            let rel_dir = dir
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            // 审计 P1#1：DB 更新失败时回滚磁盘，保证文件与登记一致
            if let Err(db_err) = conn.execute(
                "UPDATE case_files SET file_name = ?1, file_path = ?2 WHERE id = ?3",
                rusqlite::params![final_name, new_path.to_string_lossy(), r.id],
            ) {
                let _ = std::fs::rename(&new_path, &old_path); // 回滚磁盘
                return Err(anyhow::anyhow!("登记更新失败已回滚: {db_err}"));
            }

            out.push(RenameOutcome {
                id: r.id.clone(),
                old_name,
                new_name: if rel_dir.is_empty() {
                    final_name
                } else {
                    format!("{rel_dir}/{final_name}")
                },
            });
        }
        Ok(out)
    })
    .await
}
