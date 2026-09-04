use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

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
        let (root, _) = case_root(&case_id)?;
        let root = std::fs::canonicalize(root)?;
        let safe_path = canonical_file_in_case(&root, Path::new(&file_path))?;
        let safe_path_string = safe_path.to_string_lossy().to_string();
        let file_size = std::fs::metadata(&safe_path)
            .ok()
            .map(|m| m.len() as i64);
        let file_type = safe_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_string());

        conn.execute(
            "INSERT INTO case_files (id, case_id, file_name, file_path, file_size, file_type, category)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![id, case_id, file_name, safe_path_string, file_size, file_type, category],
        )?;

        // W5 自动接线：PDF/图片登记成功后立即套用 Smart Rules（filename 类规则即时生效；失败不影响登记）
        crate::commands::smart_rules::auto_rules_on_register(&id, &file_name, file_type.as_deref());

        Ok(CaseFile {
            id,
            case_id,
            file_name,
            file_path: safe_path_string,
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
        conn.execute(
            "DELETE FROM case_files WHERE id = ?1",
            rusqlite::params![id],
        )?;
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

/// 案件卷宗根目录（复用 files::ensure_case_folder 的命名规则）
fn case_root(case_id: &str) -> anyhow::Result<(PathBuf, crate::db::cases::Case)> {
    let conn = crate::db::open_db()?;
    let case = crate::db::cases::get_case(&conn, case_id)
        .map_err(|e| anyhow::anyhow!("案件不存在: {e}"))?;
    let root = crate::files::ensure_case_folder(&case)?;
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
        let mut subs: Vec<PathBuf> = std::fs::read_dir(&root)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        subs.sort();
        for sub in subs {
            let name = sub
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let count = std::fs::read_dir(&sub)
                .map(|rd| {
                    rd.filter_map(|e| e.ok())
                        .filter(|e| e.path().is_file())
                        .count()
                })
                .unwrap_or(0);
            out.push(CaseDirEntry {
                rel_path: name.clone(),
                name,
                file_count: count as u64,
                state: if count > 0 {
                    "ok".into()
                } else {
                    "empty".into()
                },
            });
        }
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
        if name.is_empty() || Path::new(name).components().count() != 1 {
            return Err(anyhow::anyhow!("非法文件夹名"));
        }
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
) -> Result<Vec<ImportedFile>, String> {
    run_blocking(move || {
        let (root, _case) = case_root(&case_id)?;
        let root = std::fs::canonicalize(root)?;
        let dir = canonical_dir_in_case(&root, dir_rel.as_deref())?;

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
            if let Err(db_error) = conn.execute(
                "INSERT INTO case_files (id, case_id, file_name, file_path, file_size, file_type, category, source_type)
                 VALUES (?1,?2,?3,?4,?5,?6,'other','imported')",
                rusqlite::params![fid, case_id, final_name, dest.to_string_lossy(), size, ftype],
            ) {
                let _ = std::fs::remove_file(&dest);
                return Err(anyhow::anyhow!("登记失败，已移除复制文件: {db_error}"));
            }

            out.push(ImportedFile {
                id: fid,
                file_name: final_name,
                archived_path: dest.to_string_lossy().to_string(),
                original_path: p.clone(),
            });
        }

        // W5 自动接线：对 pdf/图片套用 Smart Rules（失败仅记日志，不影响导入结果）
        for f in &out {
            crate::commands::smart_rules::auto_rules_on_register(&f.id, &f.file_name, None);
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
        let tx = conn.transaction()?;
        let mut n = 0u32;
        let mut registered: Vec<(String, String)> = Vec::new();
        for p in &safe_files {
            let meta = std::fs::metadata(p).ok();
            let name = p
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let ftype = p
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_string());
            let fid = db::new_id();
            tx.execute(
                "INSERT INTO case_files (id, case_id, file_name, file_path, file_size, file_type, category, source_type)
                 VALUES (?1,?2,?3,?4,?5,?6,'other','imported')",
                rusqlite::params![fid, case_id, name, p.to_string_lossy(), meta.as_ref().map(|m| m.len() as i64), ftype],
            )?;
            registered.push((fid, name));
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
                anyhow::anyhow!("目标文件不属于任何已登记案件，拒绝定位: {}", requested.display())
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
        let (root, _) = case_root(&case_id)?;
        let root = std::fs::canonicalize(root)?;
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
            let old_path = canonical_file_in_case(&root, Path::new(&old_path_str))?;
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

#[cfg(test)]
mod tests {
    use super::{
        check_openable, is_executable_extension, is_safe_open_extension, validate_case_relative_path,
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
        for (name, _ext) in [("报告.docx", "docx"), ("笔记.md", "md"), ("卷宗.pdf", "pdf")] {
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
        for (name, _) in [("notes.txt", "txt"), ("db.bin", "db"), ("secret.key", "key")] {
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
        assert!(err.to_string().contains("可执行"), "外部可执行必须被拒绝: {err}");
    }
}
