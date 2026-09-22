use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

use super::run_blocking;
use crate::db;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
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
             FROM case_files WHERE case_id = ?1 AND deleted_at IS NULL AND category = ?2 ORDER BY created_at DESC"
        } else {
            "SELECT id, case_id, file_name, file_path, file_size, file_type, category, sub_category, created_at
             FROM case_files WHERE case_id = ?1 AND deleted_at IS NULL ORDER BY created_at DESC"
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
        validate_category(&category)?;
        validate_leaf_name(&file_name)?;
        let (root, _) = case_root(&case_id)?;
        let root = std::fs::canonicalize(root)?;
        let source = std::fs::canonicalize(&file_path)?;
        if !source.starts_with(&root) {
            return Ok(import_batch(&case_id, None, &[file_path], &category)?
                .remove(0)
                .0);
        }
        let safe_path = canonical_file_in_case(&root, &source)?;
        let mut conn = db::open_db()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let file = insert_file(&tx, &case_id, &safe_path, &category, Some(&file_name))?;
        tx.commit()?;
        if category == "other" {
            crate::commands::smart_rules::auto_rules_on_register(
                &file.id,
                &file.file_name,
                file.file_type.as_deref(),
            );
        }
        Ok(file)
    })
    .await
}

/// 可恢复地移除登记；保留磁盘文件及原有引用。
#[tauri::command]
pub async fn delete_case_file(id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        if conn.execute(
            "UPDATE case_files SET deleted_at=datetime('now','localtime') WHERE id=?1 AND deleted_at IS NULL",
            rusqlite::params![id],
        )? == 0 { anyhow::bail!("文件不存在或已移除"); }
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn list_removed_case_files(case_id: String) -> Result<Vec<CaseFile>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let mut stmt = conn.prepare("SELECT id,case_id,file_name,file_path,file_size,file_type,category,sub_category,created_at
            FROM case_files WHERE case_id=?1 AND deleted_at IS NOT NULL ORDER BY deleted_at DESC")?;
        let rows = stmt.query_map([case_id], map_file_row)?.collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }).await
}

#[tauri::command]
pub async fn restore_case_file(id: String) -> Result<(), String> {
    run_blocking(move || {
        let mut conn = db::open_db()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (path,hash): (String,Option<String>) = tx.query_row("SELECT file_path,COALESCE((SELECT j.source_sha256 FROM document_processing_jobs j WHERE j.file_id=f.id AND j.status='completed' ORDER BY j.rowid DESC LIMIT 1),source_sha256) FROM case_files f WHERE id=?1 AND deleted_at IS NOT NULL", [&id], |r|Ok((r.get(0)?,r.get(1)?)))?;
        let path = canonical_file_in_case(&crate::files::case_folder_base().canonicalize()?,Path::new(&path))?;
        let path = path.as_path();
        if !path.is_file() { anyhow::bail!("原文件不存在，请先恢复磁盘文件"); }
        if let Some(hash) = hash {
            if crate::document_pipeline::sha256_file(path)? != hash {
                tx.execute("DELETE FROM document_pages WHERE file_id=?1", [&id])?;
                tx.execute("DELETE FROM page_index_nodes WHERE file_id=?1", [&id])?;
                tx.execute("UPDATE document_processing_jobs SET status='failed',error_code='SOURCE_CHANGED',error_message='移除后原文件已变化，请重新提取正文' WHERE file_id=?1 AND status='completed'", [&id])?;
                tx.execute("UPDATE case_files SET source_sha256=NULL,ocr_text=NULL,searchable_pdf_path=NULL,document_ir_path=NULL,ocr_markdown_path=NULL,ocr_status='pending',index_status='pending',ocr_error=NULL WHERE id=?1", [&id])?;
            }
        }
        tx.execute("UPDATE case_files SET deleted_at=NULL,file_size=?2 WHERE id=?1", params![id,std::fs::metadata(path)?.len() as i64])?;
        tx.commit()?;
        Ok(())
    }).await
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

const FILE_COLUMNS: &str =
    "id,case_id,file_name,file_path,file_size,file_type,category,sub_category,created_at";

fn validate_category(category: &str) -> anyhow::Result<()> {
    if ![
        "summons",
        "evidence",
        "submitted",
        "received",
        "internal",
        "correspondence",
        "other",
    ]
    .contains(&category)
    {
        anyhow::bail!("无效文件分类");
    }
    Ok(())
}

fn validate_leaf_name(name: &str) -> anyhow::Result<()> {
    if name.trim().is_empty()
        || name.len() > 240
        || name == "."
        || name == ".."
        || name.ends_with(['.', ' '])
        || name
            .chars()
            .any(|c| c.is_control() || "/\\:*?\"<>|".contains(c))
    {
        anyhow::bail!("文件名为空、过长或包含非法字符");
    }
    Ok(())
}

fn available_link(source: &Path, dir: &Path, name: &str) -> anyhow::Result<PathBuf> {
    validate_leaf_name(name)?;
    let path = Path::new(name);
    let stem = path.file_stem().and_then(|p| p.to_str()).unwrap_or(name);
    let ext = path
        .extension()
        .and_then(|p| p.to_str())
        .map(|p| format!(".{p}"))
        .unwrap_or_default();
    for number in 0..10000 {
        let target = dir.join(if number == 0 {
            name.to_owned()
        } else {
            format!("{stem}-{number}{ext}")
        });
        match std::fs::hard_link(source, &target) {
            Ok(()) => return Ok(target),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    anyhow::bail!("目标目录中重名文件过多");
}

// Until the DB commits, source links remain authoritative and destinations can be undone.
struct PendingLinks {
    paths: Vec<(PathBuf, PathBuf)>,
    committed: bool,
}
impl PendingLinks {
    fn new() -> Self {
        Self {
            paths: vec![],
            committed: false,
        }
    }
}
impl Drop for PendingLinks {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        for (source, target) in self.paths.iter().rev() {
            if same_file::is_same_file(source, target).unwrap_or(false) {
                if let Err(error) = std::fs::remove_file(target) {
                    log::error!("文件操作回滚副本清理失败: {}: {}", target.display(), error);
                }
            }
        }
    }
}

fn find_registered(
    conn: &rusqlite::Connection,
    case_id: &str,
    path: &Path,
) -> anyhow::Result<Option<CaseFile>> {
    let existing: Option<(String,Option<String>)> = conn.query_row("SELECT id,deleted_at FROM case_files WHERE case_id=?1 AND file_path=?2 ORDER BY deleted_at IS NOT NULL,rowid LIMIT 1",params![case_id,path.to_string_lossy()],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
    match existing {
        Some((_, Some(_))) => anyhow::bail!("文件已移除登记，请在已移除列表恢复"),
        Some((id, None)) => Ok(Some(conn.query_row(
            &format!("SELECT {FILE_COLUMNS} FROM case_files WHERE id=?1"),
            [id],
            map_file_row,
        )?)),
        None => Ok(None),
    }
}

pub(crate) fn insert_file(
    conn: &rusqlite::Connection,
    case_id: &str,
    path: &Path,
    category: &str,
    display_name: Option<&str>,
) -> anyhow::Result<CaseFile> {
    if let Some(existing) = find_registered(conn, case_id, path)? {
        return Ok(existing);
    }
    let id = db::new_id();
    let name = match display_name.map(str::to_owned) {
        Some(name) => name,
        None => path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("路径缺少有效文件名: {}", path.display()))?
            .to_string(),
    };
    validate_leaf_name(&name)?;
    let size = std::fs::metadata(path)?.len() as i64;
    let kind = path
        .extension()
        .and_then(|p| p.to_str())
        .unwrap_or("")
        .to_lowercase();
    conn.execute("INSERT INTO case_files(id,case_id,file_name,file_path,file_size,file_type,category,source_type)
        VALUES(?1,?2,?3,?4,?5,?6,?7,'imported')",params![id,case_id,name,path.to_string_lossy(),size,kind,category])?;
    Ok(conn.query_row(
        &format!("SELECT {FILE_COLUMNS} FROM case_files WHERE id=?1"),
        [id],
        map_file_row,
    )?)
}

// ============================================================
// 案卷管理 · 本地文件夹同步（index-v2 精装版规格 · 可交付目标③）
// ============================================================

/// 案件卷宗根目录（复用 files::ensure_case_folder 的命名规则）
fn case_root(case_id: &str) -> anyhow::Result<(PathBuf, crate::db::cases::Case)> {
    let conn = crate::db::open_db()?;
    let case = crate::db::cases::get_case(&conn, case_id)
        .map_err(|e| anyhow::anyhow!("案件不存在: {e}"))?;
    let root = crate::files::ensure_case_folder(&case)?;
    conn.execute(
        "UPDATE cases SET folder_path=?1 WHERE id=?2 AND (folder_path IS NULL OR folder_path='')",
        params![root.to_string_lossy(), case_id],
    )?;
    Ok((root, case))
}

fn validate_case_relative_path(value: &str) -> anyhow::Result<()> {
    let path = Path::new(value);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        anyhow::bail!("非法卷宗相对路径: {value}");
    }
    Ok(())
}

fn canonical_dir_in_case(root: &Path, relative: Option<&str>) -> anyhow::Result<PathBuf> {
    let target = match relative.filter(|value| !value.is_empty()) {
        Some(value) => {
            validate_case_relative_path(value)?;
            root.join(value)
        }
        None => root.to_path_buf(),
    };
    let target = std::fs::canonicalize(target)?;
    if !target.is_dir() || !target.starts_with(root) {
        anyhow::bail!("目标目录不在案件卷宗内");
    }
    Ok(target)
}

fn canonical_file_in_case(root: &Path, path: &Path) -> anyhow::Result<PathBuf> {
    let path = std::fs::canonicalize(path)
        .map_err(|_| anyhow::anyhow!("文件不存在，拒绝登记: {}", path.display()))?;
    if !path.is_file() || !path.starts_with(root) {
        anyhow::bail!("仅允许登记案件卷宗根目录内的文件");
    }
    Ok(path)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaseDirEntry {
    pub name: String,
    pub rel_path: String,
    pub absolute_path: String,
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
        fn walk(
            root: &Path,
            sub: &Path,
            depth: usize,
            out: &mut Vec<CaseDirEntry>,
        ) -> anyhow::Result<()> {
            if depth > 20 || out.len() >= 2000 {
                anyhow::bail!("卷宗目录过深或超过 2,000 个目录");
            }
            let entries = std::fs::read_dir(sub)?.collect::<std::io::Result<Vec<_>>>()?;
            let mut children = Vec::new();
            let mut count = 0;
            for entry in entries {
                if entry.file_name().to_string_lossy().starts_with('.') {
                    continue;
                }
                let kind = entry.file_type()?;
                if kind.is_file() {
                    count += 1;
                }
                if kind.is_dir() {
                    children.push(entry.path());
                }
            }
            let relative = sub.strip_prefix(root)?.to_string_lossy().replace('\\', "/");
            out.push(CaseDirEntry {
                name: if relative.is_empty() {
                    "卷宗根目录".into()
                } else {
                    relative.clone()
                },
                rel_path: relative,
                absolute_path: sub.to_string_lossy().to_string(),
                file_count: count as u64,
                state: if count > 0 {
                    "ok".into()
                } else {
                    "empty".into()
                },
            });
            children.sort();
            for child in children {
                walk(root, &child, depth + 1, out)?;
            }
            Ok(())
        }
        walk(&root, &root, 0, &mut out)?;
        Ok(out)
    })
    .await
}

/// 新建子文件夹（在卷宗根或指定子目录内）
#[tauri::command]
pub async fn create_case_subdir(
    case_id: String,
    parent_rel: Option<String>,
    name: String,
) -> Result<String, String> {
    run_blocking(move || {
        let name = name.trim();
        validate_leaf_name(name)?;
        let (root, _) = case_root(&case_id)?;
        let root = std::fs::canonicalize(root)?;
        let parent = canonical_dir_in_case(&root, parent_rel.as_deref())?;
        let target = parent.join(name);
        std::fs::create_dir(&target)?;
        let target = std::fs::canonicalize(target)?;
        if !target.starts_with(&root) {
            anyhow::bail!("新建目录越出案件卷宗范围");
        }
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
    category: Option<String>,
) -> Result<Vec<ImportedFile>, String> {
    run_blocking(move || {
        Ok(import_batch(
            &case_id,
            dir_rel.as_deref(),
            &paths,
            category.as_deref().unwrap_or("other"),
        )?
        .into_iter()
        .map(|(file, original_path)| ImportedFile {
            id: file.id,
            file_name: file.file_name,
            archived_path: file.file_path,
            original_path,
        })
        .collect())
    })
    .await
}

fn import_batch(
    case_id: &str,
    dir_rel: Option<&str>,
    paths: &[String],
    category: &str,
) -> anyhow::Result<Vec<(CaseFile, String)>> {
    validate_category(category)?;
    if paths.is_empty() || paths.len() > 500 {
        anyhow::bail!("每次请选择 1 至 500 个文件");
    }
    let (root, _) = case_root(case_id)?;
    let root = root.canonicalize()?;
    let dir = canonical_dir_in_case(&root, dir_rel)?;
    let mut sources = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for input in paths {
        let source =
            std::fs::canonicalize(input).map_err(|e| anyhow::anyhow!("无法读取 {input}: {e}"))?;
        if !source.is_file() {
            anyhow::bail!("不是普通文件: {input}");
        }
        validate_leaf_name(
            source
                .file_name()
                .and_then(|p| p.to_str())
                .ok_or_else(|| anyhow::anyhow!("文件名不是有效 Unicode"))?,
        )?;
        if seen.insert(source.clone()) {
            sources.push((source, input.clone()));
        }
    }
    let mut staged = Vec::new();
    for (source, _) in &sources {
        if source.starts_with(&root) {
            staged.push(None);
            continue;
        }
        let mut temp = tempfile::Builder::new()
            .prefix(".casy-import-")
            .tempfile_in(&dir)?;
        let mut input = std::fs::File::open(source)?;
        std::io::copy(&mut input, temp.as_file_mut())?;
        temp.as_file().sync_all()?;
        if crate::document_pipeline::sha256_file(source)?
            != crate::document_pipeline::sha256_file(temp.path())?
        {
            anyhow::bail!("复制期间原文件变化，请重试: {}", source.display());
        }
        staged.push(Some(temp));
    }
    let mut links = PendingLinks::new();
    let mut conn = db::open_db()?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut out = Vec::new();
    for ((source, original), staged) in sources.iter().zip(&staged) {
        let destination = if let Some(staged) = staged {
            let dest = available_link(
                staged.path(),
                &dir,
                source
                    .file_name()
                    .and_then(|n| n.to_str())
                    .ok_or_else(|| anyhow::anyhow!("来源路径缺少有效文件名: {}", source.display()))?,
            )?;
            links
                .paths
                .push((staged.path().to_path_buf(), dest.clone()));
            dest
        } else {
            canonical_file_in_case(&root, source)?
        };
        out.push((
            insert_file(&tx, case_id, &destination, category, None)?,
            original.clone(),
        ));
    }
    tx.commit()?;
    links.committed = true;
    for (file, _) in &out {
        if category == "other" {
            crate::commands::smart_rules::auto_rules_on_register(
                &file.id,
                &file.file_name,
                file.file_type.as_deref(),
            );
        }
    }
    Ok(out)
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
        fn walk(
            dir: &PathBuf,
            out: &mut Vec<UnregisteredFile>,
            known: &std::collections::HashSet<String>,
        ) {
            let Ok(rd) = std::fs::read_dir(dir) else {
                return;
            };
            for e in rd.filter_map(|e| e.ok()) {
                let p = e.path();
                if e.file_name().to_string_lossy().starts_with('.') {
                    continue;
                }
                let Ok(metadata) = std::fs::symlink_metadata(&p) else {
                    continue;
                };
                if metadata.file_type().is_symlink() {
                    continue;
                }
                if metadata.is_dir() {
                    walk(&p, out, known);
                } else if let Some(ps) = p.to_str() {
                    if !known.contains(ps) {
                        if let Ok(meta) = e.metadata() {
                            out.push(UnregisteredFile {
                                file_name: p
                                    .file_name()
                                    .map(|s| s.to_string_lossy().to_string())
                                    .unwrap_or_default(),
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
        let (root, _) = case_root(&case_id)?;
        let root = std::fs::canonicalize(root)?;
        let mut safe_files = Vec::with_capacity(paths.len());
        for value in &paths {
            safe_files.push(canonical_file_in_case(&root, Path::new(value))?);
        }

        let mut conn = crate::db::open_db()?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let mut n = 0u32;
        let mut registered: Vec<(String, String)> = Vec::new();
        for p in &safe_files {
            if find_registered(&tx, &case_id, p)?.is_some() {
                continue;
            }
            let file = insert_file(&tx, &case_id, p, "other", None)?;
            registered.push((file.id, file.file_name));
            n += 1;
        }
        tx.commit()?;
        // W5 自动接线：事务提交后对 pdf/图片套用 Smart Rules（失败仅记日志，不影响登记结果）
        for (fid, name) in &registered {
            crate::commands::smart_rules::auto_rules_on_register(fid, name, None);
        }
        Ok(n)
    })
    .await
}

/// Finder/资源管理器中显示文件（定位）
///
/// 安全（审查 P0-2）：目标必须是某个已登记案件卷宗内的文件。
/// 先 canonicalize 消除 `../` 与符号链接穿越，再反查所属案件卷宗根做 starts_with 校验。
/// 现有 4 处前端调用点均传 `file.filePath`（已登记案件文件），故此校验零误伤。
#[tauri::command]
pub async fn reveal_path(path: String) -> Result<(), String> {
    // 校验与规范化：后续平台分支一律使用校验后的路径，不再信任原始入参
    let verified = run_blocking(move || {
        let requested = std::path::Path::new(&path);
        let target = std::fs::canonicalize(requested)
            .map_err(|_| anyhow::anyhow!("文件不存在，拒绝定位: {}", requested.display()))?;

        let conn = db::open_db()?;
        let case_id: String = conn
            .query_row(
                "SELECT case_id FROM case_files WHERE file_path = ?1 LIMIT 1",
                rusqlite::params![path],
                |r| r.get(0),
            )
            .map_err(|_| {
                anyhow::anyhow!(
                    "目标文件不属于任何已登记案件，拒绝定位: {}",
                    requested.display()
                )
            })?;

        let (root, _case) = case_root(&case_id)?;
        let root = std::fs::canonicalize(&root).unwrap_or(root);
        if !target.starts_with(&root) {
            anyhow::bail!("目标文件不在案件卷宗内，拒绝定位: {}", target.display());
        }
        Ok(target)
    })
    .await?;

    let verified = verified.to_string_lossy().to_string();

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-R", &verified])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .args(["/select,", &verified])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(parent) = std::path::Path::new(&verified).parent() {
            std::process::Command::new("xdg-open")
                .arg(parent)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// 识别已知可执行 / 脚本扩展名。macOS 的 `open` 会直接执行其中的 `.command`、
/// `.app` 等；其余 shell/解释脚本在「终端/脚本编辑器打开」的默认处理下同样会执行。
/// 无论目标位于何处，命中即拒绝打开，阻断「任意文件处置」升级为代码执行。
fn is_executable_extension(extension: &str) -> bool {
    matches!(
        extension,
        // macOS 直接执行
        "command" | "app" | "workflow" | "osx" | "scpt" | "scptd"
        // 可执行 / 安装 / 宏 / 脚本
        | "exe" | "com" | "cmd" | "bat" | "msi" | "scr" | "pif" | "hta" | "cpl"
        | "gadget" | "jar" | "vbs" | "ps1" | "psm1" | "psd1"
        // shell / 解释脚本
        | "sh" | "bash" | "zsh" | "csh" | "ksh" | "fish" | "py" | "rb" | "pl"
        | "pm" | "php" | "lua" | "ahk"
    )
}

/// 应用目录之外允许打开的安全导出类型（与 `resolve_explicit_output_path` 的
/// 「保存对话框允许导出到任意已存在目录」语义保持一致）。
fn is_safe_open_extension(extension: &str) -> bool {
    matches!(extension, "docx" | "md" | "markdown" | "pdf")
}

/// 打开路径判定（`target` 传入 canonicalize 后的绝对路径；`documents_root` 传逻辑根，内部 canonicalize 配对）。
///
/// 安全（审查 P0-3 修订）——把信任边界与普通导出流程对齐，而非一刀切禁止导出目录外的文件：
///   1. 始终拒绝可执行 / 脚本扩展名——即使位于应用托管目录内也不放行（macOS `open` 会执行 `.command` 等）；
///   2. 应用托管目录（`documents_root`：卷宗 `cases/`、导出 `exports/`、模板 `templates/`）内允许常规文件；
///   3. 目录之外仅允许用户经系统保存对话框出口导出的安全类型（docx / md / markdown / pdf）；
///   4. **不再信任 `data_root`**——数据库与密钥文件禁止被默认应用打开。
fn check_openable(target: &Path, documents_root: &Path) -> anyhow::Result<()> {
    let extension = target
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if is_executable_extension(&extension) {
        anyhow::bail!("拒绝打开可执行文件: {}", target.display());
    }

    // canonicalize 配对：macOS 上 tempdir/逻辑路径常为 /var，real path 为 /private/var
    let canon_root =
        std::fs::canonicalize(documents_root).unwrap_or_else(|_| documents_root.to_path_buf());
    if target.starts_with(&canon_root) {
        return Ok(());
    }

    if is_safe_open_extension(&extension) {
        return Ok(());
    }

    anyhow::bail!(
        "目标不在应用托管目录内，且不是安全导出类型，拒绝打开: {}",
        target.display()
    );
}

/// 校验待打开路径：canonicalize 消除 `../` 与符号链接后，按 `check_openable` 策略校验。
fn verify_openable_path(path: &str) -> anyhow::Result<PathBuf> {
    let requested = Path::new(path);
    let target = std::fs::canonicalize(requested)
        .map_err(|_| anyhow::anyhow!("文件不存在，拒绝打开: {}", requested.display()))?;

    check_openable(&target, &crate::runtime_paths::documents_root())?;
    Ok(target)
}

/// 用系统默认应用打开文件
///
/// 安全（审查 P0-3）：与 `reveal_path` 同源的任意文件处置原语，上一轮只修了 `reveal_path`。
/// macOS 的 `open` 会直接执行 `.command` 等可执行文件，故未校验的透传等同代码执行原语。
/// 现有 3 处前端调用点（导出结果打开 ×2、案件卷宗文件打开 ×1）走 `verify_openable_path`：
/// 始终拒绝可执行/脚本扩展；应用托管目录（卷宗/导出/模板）内允许常规文件；
/// 目录之外仅放行用户经保存对话框导出的安全类型（docx/md/markdown/pdf），故为零误伤。
#[tauri::command]
pub async fn open_file_with_default(path: String) -> Result<(), String> {
    // 校验与规范化：后续平台分支一律使用校验后的路径，不再信任原始入参
    let verified = verify_openable_path(&path)
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .to_string();

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&verified)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &verified])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&verified)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ============================================================
// 智能重命名工作台（index-v2 · 规则版 v1）
// ============================================================

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameItem {
    pub id: String,
    #[serde(alias = "new_name")]
    pub new_name: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameOutcome {
    pub id: String,
    pub old_name: String,
    pub new_name: String,
    pub warning: Option<String>,
}

/// 批量应用重命名：磁盘同名目录内改名 + DB file_name/file_path 同步
/// - 自动补扩展名（新名缺 ext 时保留原 ext）
/// - 目标名冲突时自动追加 -1/-2 序号，不覆盖他人文件
#[tauri::command]
pub async fn apply_case_file_renames(
    case_id: String,
    renames: Vec<RenameItem>,
) -> Result<Vec<RenameOutcome>, String> {
    run_blocking(move || relocate_files(&case_id, &renames, None)).await
}

pub(crate) fn relocate_files(
    case_id: &str,
    renames: &[RenameItem],
    target: Option<&Path>,
) -> anyhow::Result<Vec<RenameOutcome>> {
    if renames.is_empty() || renames.len() > 500 {
        anyhow::bail!("每次请选择 1 至 500 个文件");
    }
    let (root, _) = case_root(case_id)?;
    let root = root.canonicalize()?;
    let mut conn = db::open_db()?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut sources = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for rename in renames {
        let row = tx.query_row(&format!("SELECT {FILE_COLUMNS} FROM case_files WHERE id=?1 AND case_id=?2 AND deleted_at IS NULL"),params![rename.id,case_id],map_file_row)?;
        let path = canonical_file_in_case(&root, Path::new(&row.file_path))?;
        if !seen.insert(path.clone()) {
            anyhow::bail!("同一磁盘文件不能重复提交");
        }
        let processing: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM document_processing_jobs j JOIN case_files f ON f.id=j.file_id WHERE f.file_path=?1 AND j.status='running')",[&row.file_path],|r|r.get(0))?;
        if processing {
            anyhow::bail!("文件正在处理，请先取消处理任务");
        }
        let mut name = rename.new_name.trim().to_owned();
        validate_leaf_name(&name)?;
        let old_ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        if !name.is_empty() && Path::new(&name).extension().is_none() && !old_ext.is_empty() {
            name.push('.');
            name.push_str(old_ext);
        }
        validate_leaf_name(&name)?;
        if !old_ext.is_empty()
            && !Path::new(&name)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .eq_ignore_ascii_case(old_ext)
        {
            anyhow::bail!("请保留原文件扩展名 .{old_ext}");
        }
        sources.push((row, path, name));
    }
    let mut links = PendingLinks::new();
    let mut out = Vec::new();
    for (row, source, name) in &sources {
        let dir = match target {
            Some(dir) => dir,
            None => source
                .parent()
                .ok_or_else(|| anyhow::anyhow!("路径缺少父目录: {}", source.display()))?,
        };
        if (target.is_none() && *name == row.file_name) || dir.join(name) == *source {
            out.push(RenameOutcome {
                id: row.id.clone(),
                old_name: row.file_name.clone(),
                new_name: row.file_name.clone(),
                warning: None,
            });
            continue;
        }
        let dest = available_link(source, dir, name)?;
        links.paths.push((source.clone(), dest.clone()));
        let final_name = dest
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("目标路径缺少有效文件名: {}", dest.display()))?
            .to_string();
        tx.execute(
            "UPDATE case_files SET file_name=?1,file_path=?2 WHERE file_path=?3",
            params![final_name, dest.to_string_lossy(), row.file_path],
        )?;

        out.push(RenameOutcome {
            id: row.id.clone(),
            old_name: row.file_name.clone(),
            new_name: final_name,
            warning: None,
        });
    }
    relocate_knowledge_reference_batch(&tx, &links.paths)?;
    tx.commit()?;
    links.committed = true;
    for (source, dest) in &links.paths {
        let cleanup = if same_file::is_same_file(source, dest).unwrap_or(false) {
            std::fs::remove_file(source)
        } else {
            Err(std::io::Error::other("源路径已变化"))
        };
        if let Err(error) = cleanup {
            for item in &mut out {
                item.warning = Some(format!(
                    "登记已更新，旧副本未清理：{} ({error})",
                    source.display()
                ));
            }
        }
    }
    Ok(out)
}

pub(crate) fn relocate_knowledge_references(conn: &rusqlite::Connection, source: &Path, destination: &Path) -> anyhow::Result<()> {
    relocate_knowledge_reference_batch(conn, &[(source.to_owned(), destination.to_owned())])
}
fn relocate_knowledge_reference_batch(conn: &rusqlite::Connection, mappings: &[(PathBuf,PathBuf)]) -> anyhow::Result<()> {
    if mappings.is_empty() { return Ok(()); }
    let items = {
        let mut stmt = conn.prepare("SELECT id,content FROM knowledge_items")?;
        let items = stmt.query_map([], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        items
    };
    for (id, content) in items {
        let relocated = super::portable_backup::relocate_markdown(&content, mappings);
        if relocated != content {
            conn.execute("UPDATE knowledge_items SET content=?2,updated_at=datetime('now','localtime') WHERE id=?1",params![id,relocated])?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn move_case_files(
    case_id: String,
    ids: Vec<String>,
    dir_rel: Option<String>,
) -> Result<Vec<RenameOutcome>, String> {
    run_blocking(move || {
        let (root,_) = case_root(&case_id)?;
        let root = root.canonicalize()?;
        let target = canonical_dir_in_case(&root,dir_rel.as_deref())?;
        let conn = db::open_db()?;
        let renames = ids.into_iter().map(|id| {
            let name = conn.query_row("SELECT file_name FROM case_files WHERE id=?1 AND case_id=?2 AND deleted_at IS NULL",params![id,case_id],|r|r.get::<_,String>(0))?;
            Ok(RenameItem {id,new_name:name})
        }).collect::<rusqlite::Result<Vec<_>>>()?;
        relocate_files(&case_id,&renames,Some(&target))
    }).await
}

#[tauri::command]
pub async fn set_case_file_category(id: String, category: String) -> Result<(), String> {
    run_blocking(move || {
        validate_category(&category)?;
        if db::open_db()?.execute(
            "UPDATE case_files SET category=?1 WHERE id=?2 AND deleted_at IS NULL",
            params![category, id],
        )? == 0
        {
            anyhow::bail!("文件不存在或已移除");
        }
        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::{
        check_openable, is_executable_extension, is_safe_open_extension,
        validate_case_relative_path,
    };
    use std::path::Path;

    /// 在临时目录内生成一个真实文件并返回其 canonicalize 后的绝对路径。
    fn make_file(dir: &Path, name: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, b"x").unwrap();
        std::fs::canonicalize(&path).unwrap()
    }

    #[test]
    fn accepts_nested_case_relative_path() {
        assert!(validate_case_relative_path("03_证据/对方证据").is_ok());
    }

    #[test]
    fn rejects_case_path_escape_and_absolute_path() {
        assert!(validate_case_relative_path("../其他案件").is_err());
        assert!(validate_case_relative_path("03_证据/../../其他案件").is_err());
        assert!(validate_case_relative_path("/tmp/其他案件").is_err());
    }

    #[test]
    fn executable_extension_classification() {
        for ext in [
            "command", "app", "sh", "bash", "py", "rb", "exe", "bat", "ps1", "jar", "vbs", "com",
        ] {
            assert!(is_executable_extension(ext), "应为可执行: {ext}");
        }
        // 证明安全导出类型未被误判为可执行
        for ext in ["docx", "md", "markdown", "pdf", "txt", "png"] {
            assert!(!is_executable_extension(ext), "不应为可执行: {ext}");
        }
    }

    #[test]
    fn safe_open_extension_classification() {
        for ext in ["docx", "md", "markdown", "pdf"] {
            assert!(is_safe_open_extension(ext), "应为安全导出类型: {ext}");
        }
        for ext in ["txt", "command", "sh", "png", "db", "key"] {
            assert!(!is_safe_open_extension(ext), "不应为安全导出类型: {ext}");
        }
    }

    #[test]
    fn manages_regular_file_in_app_dir_allowed() {
        let doc_root = tempfile::tempdir().unwrap();
        let target = make_file(doc_root.path(), "判决书.docx");
        assert!(check_openable(&target, doc_root.path()).is_ok());
    }

    #[test]
    fn executable_is_rejected_even_inside_app_dir() {
        // 审查 P0-3 修订：即便位于 documents_root 内，`.command` 也要拒绝（macOS `open` 会执行）
        let doc_root = tempfile::tempdir().unwrap();
        let target = make_file(doc_root.path(), "payload.command");
        let err = check_openable(&target, doc_root.path()).unwrap_err();
        assert!(err.to_string().contains("可执行"));
    }

    #[test]
    fn safe_export_outside_app_dir_allowed() {
        let doc_root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        // 用户经保存对话框导出的 docx / md / pdf 落在应用目录之外，应可打开
        for (name, _ext) in [
            ("报告.docx", "docx"),
            ("笔记.md", "md"),
            ("卷宗.pdf", "pdf"),
        ] {
            let target = make_file(outside.path(), name);
            assert!(
                check_openable(&target, doc_root.path()).is_ok(),
                "目录外安全导出应放行: {name}"
            );
        }
    }

    #[test]
    fn non_safe_type_outside_app_dir_rejected() {
        let doc_root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        for (name, _) in [
            ("notes.txt", "txt"),
            ("db.bin", "db"),
            ("secret.key", "key"),
        ] {
            let target = make_file(outside.path(), name);
            assert!(
                check_openable(&target, doc_root.path()).is_err(),
                "目录外的非安全类型应拒绝: {name}"
            );
        }
    }

    #[test]
    fn executable_outside_app_dir_rejected() {
        let doc_root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = make_file(outside.path(), "payload.sh");
        assert!(check_openable(&target, doc_root.path()).is_err());
    }

    #[test]
    fn data_root_db_and_key_not_openable() {
        // 审查 P0-3 修订：data_root 不再进入允许集，数据库/密钥文件（无扩展名或非安全类型）必须拒绝。
        let doc_root = tempfile::tempdir().unwrap();
        let data_root = tempfile::tempdir().unwrap();
        // 模拟 SQLite 库（无扩展名）与密钥文件
        let db_file = make_file(data_root.path(), "casy.db");
        let key_file = make_file(data_root.path(), "system.key");
        assert!(check_openable(&db_file, doc_root.path()).is_err());
        assert!(check_openable(&key_file, doc_root.path()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_to_external_executable_rejected_after_canonicalize() {
        // 审查 P0-3 修订：`verify_openable_path` 先 canonicalize，符号链接被解析到真实目标。
        // 若托管目录内一个「无关扩展（如 .docx）」的链接指向外部的可执行文件，解析后
        // 目标扩展名是可执行扩展 → 仍须拒绝，不能借链接绕过扩展名白名单。
        let doc_root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let evil = std::fs::canonicalize(make_file(outside.path(), "nasty.command")).unwrap();
        let link = doc_root.path().join("report.docx");
        std::os::unix::fs::symlink(&evil, &link).unwrap();

        // 模拟 verify_openable_path：先 canonicalize 解析符号链接，再走 check_openable
        let resolved = std::fs::canonicalize(&link).unwrap();
        assert_eq!(resolved, evil, "canonicalize 应解析到外部真实文件");
        let err = check_openable(&resolved, doc_root.path()).unwrap_err();
        assert!(
            err.to_string().contains("可执行"),
            "外部可执行必须被拒绝: {err}"
        );
    }
}
