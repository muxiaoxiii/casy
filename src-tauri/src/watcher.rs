use crate::{db, get_app_handle};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use tauri::Emitter;

/// 收件箱文件夹监听器状态（Tauri managed state）。
///
/// 由 [`start_inbox_watcher`] 启动成功后，把拿到所有权的 [`RecommendedWatcher`]
/// 交给 [`tauri::Manager::manage`] 托管。这样一来：
///
/// - 监听器的生命周期与整个应用一致，`setup` 里启动、应用退出时随状态一起被 drop。
/// - 在 macOS 上 [`RecommendedWatcher`] 即 [`notify::FsEventWatcher`]，其 `Drop` 实现会
///   停掉底层的 FSEvents 监听线程并等待其退出，因此不再需要用原来的
///   [`std::mem::forget`] 把监听器泄漏掉。
/// - 监听器（`FsEventWatcher`）通过 `unsafe impl Send + Sync` 满足
///   `Send + Sync + 'static`，可以直接放进 Tauri managed state。
#[derive(Debug)]
pub struct InboxWatcherState {
    /// 正在运行的文件夹监听器；只有 `start_inbox_watcher` 成功后才被托管。
    ///
    /// 该字段不会被读取，其作用是用作 Drop guard：随 managed state 一起被 drop 时，
    /// 触发 `FsEventWatcher` 的 Drop 以停掉 OS 监听线程并清理，因此不需要读取。
    #[allow(dead_code)]
    watcher: RecommendedWatcher,
}

impl InboxWatcherState {
    /// 用启动成功的监听器构造监听状态。
    pub fn new(watcher: RecommendedWatcher) -> Self {
        Self { watcher }
    }
}

/// 启动文件夹监听，监控 `~/Documents/Casy/inbox/` 目录。
///
/// 返回持有的 [`RecommendedWatcher`]，由调用方（通常是 `setup`）交给
/// [`tauri::Manager::manage`] 保管所有权；监听失败只返回错误，由调用方记录日志。
///
/// ## 回调的 Send/Sync 约束
///
/// - notify 的事件回调必须实现 [`notify::EventHandler`]，即
///   `FnMut(notify::Result<Event>) + Send + 'static`。这里传给
///   [`RecommendedWatcher::new`] 的是 `std::sync::mpsc::Sender<notify::Result<Event>>`，
///   它本身是 `Send + 'static`，满足约束。
/// - 后台事件处理线程拥有 `Receiver` 端；监听器 owned 的 `Sender` 在监听器被 drop
///   时随之释放并断开通道，事件处理线程读到通道关闭后自然退出。
pub fn start_inbox_watcher() -> notify::Result<RecommendedWatcher> {
    let inbox_dir = inbox_path();

    // 确保目录存在
    if !inbox_dir.exists() {
        std::fs::create_dir_all(&inbox_dir)?;
    }

    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();

    let mut watcher = RecommendedWatcher::new(tx, Config::default())?;
    watcher.watch(&inbox_dir, RecursiveMode::NonRecursive)?;

    // 后台线程处理文件事件
    spawn_event_processing_thread(rx, inbox_dir.clone());

    log::info!("收件箱文件夹监听已启动: {}", inbox_dir.display());
    Ok(watcher)
}

/// 启动后台线程消费 watcher 事件并导入新文件。
///
/// 事件处理线程只依赖 `rx` 与收件目录，监听器本身与线程解耦：监听器 drop 时断开
/// `rx`，线程读到通道关闭即退出，无需显式 join。
fn spawn_event_processing_thread(rx: mpsc::Receiver<notify::Result<Event>>, inbox_dir: PathBuf) {
    thread::spawn(move || {
        // 记录启动时已存在的文件，避免重复导入
        let existing_files = scan_existing_files(&inbox_dir);

        for res in rx {
            match res {
                Ok(event) => {
                    for path in files_to_import(&event, &existing_files) {
                        import_file_to_inbox(&path);
                    }
                }
                Err(e) => {
                    crate::processing::service("inbox","收件箱目录监听","failed","监听异常",Some(&e.to_string()));
                    log::error!("文件监听错误: {}", e);
                }
            }
        }
    });
}

/// 从 watcher 事件中筛选出本次需要导入的文件路径。
///
/// 规则：
/// - 只处理 `Create` 事件；
/// - 只处理真实文件（目录/元数据事件不进导入）；
/// - 跳过启动前就已存在的文件（去重）；
/// - 跳过隐藏/临时文件。
fn files_to_import(
    event: &Event,
    existing_files: &std::collections::HashSet<PathBuf>,
) -> Vec<PathBuf> {
    if !matches!(event.kind, EventKind::Create(_)) {
        return Vec::new();
    }
    event
        .paths
        .iter()
        .filter(|p| p.is_file())
        .filter(|p| !existing_files.contains(*p))
        .filter(|p| !is_temp_file(p))
        .cloned()
        .collect()
}

/// 获取收件箱目录路径
fn inbox_path() -> PathBuf {
    crate::runtime_paths::documents_root().join("inbox")
}

/// 扫描目录中已存在的文件（跳过临时文件），用于启动时去重
fn scan_existing_files(dir: &Path) -> std::collections::HashSet<PathBuf> {
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
fn is_temp_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    name.starts_with('.') || name.ends_with(".tmp") || name.ends_with(".crdownload")
}

/// 将文件导入收件箱
fn import_file_to_inbox(path: &Path) {
    // 双保险：目录/不存在路径直接拒绝（防 watcher 事件把 inbox 目录本身导入）
    if !path.is_file() {
        return;
    }
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("未知文件")
        .to_string();
    let source_path = path.to_string_lossy().to_string();

    let activity=crate::processing::Activity::start("inbox",&format!("收件箱导入：{file_name}"));
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
                    activity.finish(&Ok::<(),String>(()));
                    log::info!("自动导入收件箱: {}", file_name);
                    // 通知前端刷新
                    if let Some(handle) = get_app_handle() {
                        let _ = handle.emit("inbox:new_item", &id);
                    }
                }
                Err(e) => {
                    activity.finish(&Err::<(),_>(&e));
                    log::error!("导入收件箱失败 {}: {}", file_name, e);
                }
            }
        }
        Err(e) => {
            activity.finish(&Err::<(),_>(&e));
            log::error!("数据库连接失败: {}", e);
        }
    }
}

/// 读取文件预览（仅文本类文件）
fn read_file_preview(path: &Path) -> Option<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "txt" | "md" | "csv" | "json" | "xml" | "html" | "eml" => {
            std::fs::read_to_string(path).ok()
        }
        "pdf" => {
            pdf_extract::extract_text(path).ok().map(|text| {
                text.chars().take(2000).collect::<String>()
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{CreateKind, DataChange, ModifyKind};
    use notify::EventKind;
    use std::fs;

    fn tmp_file(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn is_temp_file_flags_hidden_and_download_tails() {
        assert!(is_temp_file(Path::new("/tmp/.DS_Store")));
        assert!(is_temp_file(Path::new("/tmp/a.tmp")));
        assert!(is_temp_file(Path::new("/tmp/a.crdownload")));
        assert!(!is_temp_file(Path::new("/tmp/ordinary.txt")));
        assert!(!is_temp_file(Path::new("/tmp/no_extension")));
    }

    #[test]
    fn scan_existing_files_skips_temp_files() {
        let dir = tempfile::tempdir().unwrap();
        tmp_file(dir.path(), "keep.txt", "x");
        tmp_file(dir.path(), ".hidden", "x");
        tmp_file(dir.path(), "later.tmp", "x");

        let files = scan_existing_files(dir.path());

        assert!(files.contains(&dir.path().join("keep.txt")));
        assert!(!files.contains(&dir.path().join(".hidden")));
        assert!(!files.contains(&dir.path().join("later.tmp")));
    }

    #[test]
    fn files_to_import_only_keeps_new_real_files() {
        let dir = tempfile::tempdir().unwrap();
        let new_file = tmp_file(dir.path(), "new.txt", "x");
        let existing_file = tmp_file(dir.path(), "existing.txt", "x");
        let temp_file = tmp_file(dir.path(), ".swp.tmp", "x");
        let subdir = dir.path().join("sub");
        fs::create_dir(&subdir).unwrap();

        let mut existing_files = std::collections::HashSet::new();
        existing_files.insert(existing_file.clone());

        // Create 事件：新真实文件 → 导入
        let ev = Event::new(EventKind::Create(CreateKind::File)).add_path(new_file.clone());
        assert_eq!(
            files_to_import(&ev, &existing_files),
            vec![new_file.clone()]
        );

        // Create 事件：已存在文件 → 去重
        let ev = Event::new(EventKind::Create(CreateKind::File)).add_path(existing_file.clone());
        assert!(files_to_import(&ev, &existing_files).is_empty());

        // Create 事件：临时文件 → 跳过
        let ev = Event::new(EventKind::Create(CreateKind::File)).add_path(temp_file.clone());
        assert!(files_to_import(&ev, &existing_files).is_empty());

        // Create 事件：目录元数据 → 跳过
        let ev = Event::new(EventKind::Create(CreateKind::Folder)).add_path(subdir.clone());
        assert!(files_to_import(&ev, &existing_files).is_empty());

        // 非 Create 事件（如 Modify/Remove/Other）→ 不导入
        let ev = Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Content)))
            .add_path(new_file.clone());
        assert!(files_to_import(&ev, &existing_files).is_empty());

        let ev = Event::new(EventKind::Other).add_path(new_file.clone());
        assert!(files_to_import(&ev, &existing_files).is_empty());
    }
}
