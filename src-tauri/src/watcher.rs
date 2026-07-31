use crate::{db, get_app_handle};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use tauri::Emitter;

/// 启动文件夹监听，监控 ~/Documents/Casy/inbox/ 目录
pub fn start_inbox_watcher() -> notify::Result<()> {
    let inbox_dir = inbox_path();

    // 确保目录存在
    if !inbox_dir.exists() {
        std::fs::create_dir_all(&inbox_dir)?;
    }

    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();

    let mut watcher = RecommendedWatcher::new(tx, Config::default())?;
    watcher.watch(&inbox_dir, RecursiveMode::NonRecursive)?;

    // 后台线程处理文件事件
    let inbox_dir_clone = inbox_dir.clone();
    thread::spawn(move || {
        // 记录启动时已存在的文件，避免重复导入
        let existing_files: std::collections::HashSet<PathBuf> = scan_existing_files(&inbox_dir_clone);

        for res in rx {
            match res {
                Ok(event) => {
                    if let EventKind::Create(_) = event.kind {
                        for path in &event.paths {
                            // 跳过已存在的文件（启动前就有）
                            if existing_files.contains(path) {
                                continue;
                            }
                            // 跳过隐藏文件和临时文件
                            if is_temp_file(path) {
                                continue;
                            }
                            import_file_to_inbox(path);
                        }
                    }
                }
                Err(e) => {
                    log::error!("文件监听错误: {}", e);
                }
            }
        }
    });

    // 防止 watcher 被 drop
    std::mem::forget(watcher);

    log::info!("收件箱文件夹监听已启动: {}", inbox_dir.display());
    Ok(())
}

/// 获取收件箱目录路径
fn inbox_path() -> PathBuf {
    dirs::document_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Casy")
        .join("inbox")
}

/// 扫描目录中已存在的文件
fn scan_existing_files(dir: &PathBuf) -> std::collections::HashSet<PathBuf> {
    let mut files = std::collections::HashSet::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && !is_temp_file(&path) {
                files.insert(path);
            }
        }
    }
    files
}

/// 判断是否为临时文件
fn is_temp_file(path: &PathBuf) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    name.starts_with('.') || name.ends_with(".tmp") || name.ends_with(".crdownload")
}

/// 将文件导入收件箱
fn import_file_to_inbox(path: &PathBuf) {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("未知文件")
        .to_string();
    let source_path = path.to_string_lossy().to_string();

    // 读取文件内容用于分类（仅文本文件）
    let content_text = read_file_preview(path);

    match db::open_db() {
        Ok(conn) => {
            let id = db::new_id();
            let text = content_text.clone().unwrap_or_default();
            let parsed = crate::parse::classify_document(&text);

            match conn.execute(
                "INSERT INTO inbox_items (id, source_type, title, content_text, source_path,
                 ai_category, ai_confidence, status, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending', ?8)",
                rusqlite::params![
                    id,
                    "file",
                    file_name,
                    text,
                    source_path,
                    parsed.doc_type,
                    parsed.confidence,
                    db::now_local(),
                ],
            ) {
                Ok(_) => {
                    log::info!("自动导入收件箱: {}", file_name);
                    // 通知前端刷新
                    if let Some(handle) = get_app_handle() {
                        let _ = handle.emit("inbox:new_item", &id);
                    }
                }
                Err(e) => {
                    log::error!("导入收件箱失败 {}: {}", file_name, e);
                }
            }
        }
        Err(e) => {
            log::error!("数据库连接失败: {}", e);
        }
    }
}

/// 读取文件预览（仅文本类文件）
fn read_file_preview(path: &PathBuf) -> Option<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "txt" | "md" | "csv" | "json" | "xml" | "html" | "eml" => {
            std::fs::read_to_string(path).ok()
        }
        _ => None,
    }
}
