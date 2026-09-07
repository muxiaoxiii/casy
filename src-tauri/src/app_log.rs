use std::path::PathBuf;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// 日志文件目录：~/Library/Logs/Casy/
fn log_dir() -> PathBuf {
    let dir = crate::runtime_paths::log_root();
    std::fs::create_dir_all(&dir).ok();
    dir
}

/// 获取日志文件路径（供前端展示）
pub fn log_dir_path() -> String {
    log_dir().to_string_lossy().to_string()
}

/// 追加一行崩溃记录（R-6：Rust panic 与前端异常共用目录，JSON 行，严格本地 D-13）
pub fn append_crash_line(file_stem: &str, line: &str) {
    use std::io::Write;
    let path = log_dir().join(format!("{file_stem}.jsonl"));
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{line}");
    }
}

/// 初始化日志系统
///
/// 输出到：
/// 1. 控制台（stderr）— 开发时可见
/// 2. 日志文件（~/Library/Logs/Casy/casy.log.YYYY-MM-DD）— 按天轮转，保留 7 天
///
/// 日志级别通过 CASY_LOG 环境变量控制，默认 info。
/// 示例：CASY_LOG=debug cargo tauri dev
pub fn init() -> tracing_appender::non_blocking::WorkerGuard {
    let log_dir = log_dir();

    // 文件 appender：按天轮转
    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("casy")
        .filename_suffix("log")
        .max_log_files(7)
        .build(&log_dir)
        .unwrap_or_else(|e| {
            eprintln!("无法创建日志文件: {}，回退到 stderr", e);
            // 回退：创建一个临时的 appender
            RollingFileAppender::builder()
                .rotation(Rotation::DAILY)
                .filename_prefix("casy")
                .filename_suffix("log")
                .build(std::env::temp_dir())
                .expect("无法创建临时日志文件")
        });

    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // 日志级别过滤
    let env_filter = EnvFilter::try_from_env("CASY_LOG").unwrap_or_else(|_| {
        if cfg!(debug_assertions) {
            EnvFilter::new("casy=debug,tauri=info,hyper=warn,reqwest=warn")
        } else {
            EnvFilter::new("casy=info,tauri=warn,hyper=warn,reqwest=warn")
        }
    });

    // 控制台层（stderr）
    let console_layer = fmt::layer()
        .with_writer(std::io::stderr)
        .with_ansi(true)
        .with_target(true)
        .with_file(true)
        .with_line_number(true);

    // 文件层（带时间戳、文件名、行号）
    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true)
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true);

    // 注册全局 subscriber
    tracing_subscriber::registry()
        .with(env_filter)
        .with(console_layer)
        .with(file_layer)
        .init();

    // 安装 panic hook
    install_panic_hook();

    tracing::info!(
        log_dir = %log_dir.display(),
        debug_mode = cfg!(debug_assertions),
        "日志系统已初始化"
    );
    guard
}

/// 安装 panic hook，将 panic 信息写入日志
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic".to_string());

        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".to_string());

        tracing::error!(location = %location, payload = %payload, "PANIC");

        // R-6 崩溃基线：结构化摘要写入 crash-rust.jsonl（版本号 + 时间戳；D-13 严格本地）
        let line = serde_json::json!({
            "ts": chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f").to_string(),
            "version": env!("CARGO_PKG_VERSION"),
            "kind": "rust_panic",
            "thread": std::thread::current().name().unwrap_or("<unnamed>"),
            "message": payload,
            "location": location,
        });
        append_crash_line("crash-rust", &line.to_string());

        default_hook(info);
    }));
}

#[cfg(test)]
mod tests {
    #[test]
    fn file_logger_survives_initialization_and_flushes_on_shutdown() {
        if std::env::var_os("CASY_LOG_GUARD_TEST_CHILD").is_some() {
            let guard = super::init();
            tracing::info!("conversion-log-after-init");
            drop(guard);
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "app_log::tests::file_logger_survives_initialization_and_flushes_on_shutdown", "--nocapture"])
            .env("CASY_LOG_GUARD_TEST_CHILD", "1")
            .env("CASY_TEST_DATA_DIR", root.path())
            .env("CASY_LOG", "casy_lib=info")
            .status().unwrap();
        assert!(status.success());
        let content = std::fs::read_dir(root.path().join("logs")).unwrap()
            .map(|entry| std::fs::read_to_string(entry.unwrap().path()).unwrap()).collect::<String>();
        assert!(content.contains("conversion-log-after-init"), "log writer stopped after init");
    }
}
