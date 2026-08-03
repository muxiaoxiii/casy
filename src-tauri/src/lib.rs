mod app_log;
mod ai;
mod commands;
mod db;
mod docsy_engine;
mod email;
mod files;
mod formula;
mod deadline;
mod parse;
mod sync;
mod tray;
mod watcher;

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8};
use std::sync::Arc;

/// Global app handle for emitting events from non-command contexts.
static APP_HANDLE: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();

pub fn get_app_handle() -> Option<&'static tauri::AppHandle> {
    APP_HANDLE.get()
}

/// Shared state for long-running conversion operations.
pub struct ConversionState {
    pub timed_out: AtomicBool,
    pub response: AtomicU8,
    pub pid: AtomicU64,
}

impl ConversionState {
    pub fn new() -> Self {
        Self {
            timed_out: AtomicBool::new(false),
            response: AtomicU8::new(0),
            pid: AtomicU64::new(0),
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    app_log::init();
    let conversion_state = Arc::new(ConversionState::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|_app, _shortcut, event| {
                    if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        if let Some(handle) = crate::get_app_handle() {
                            let _ = tauri::Emitter::emit(handle, "tray:clipboard_to_inbox", ());
                        }
                    }
                })
                .build(),
        )
        .manage(conversion_state)
        .setup(|app| {
            let _ = APP_HANDLE.set(app.handle().clone());
            // 初始化数据库
            let conn = db::open_db()?;
            db::init_db(&conn)?;
            // 系统托盘
            tray::setup_tray(app.handle())?;

            // 全局热键: Cmd+Shift+V → 剪贴板到收件箱
            {
                use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
                let shortcut = Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyV);
                app.global_shortcut().register(shortcut)?;
            }

            // 文件夹监听: ~/Documents/Casy/inbox/
            if let Err(e) = watcher::start_inbox_watcher() {
                log::warn!("收件箱文件夹监听启动失败: {}", e);
            }

            // 恢复飞书自动推送状态并启动后台 watcher
            {
                let conn = db::open_db()?;
                if let Ok(Some(enabled_str)) = db::get_setting(&conn, "feishu_auto_push_enabled") {
                    if enabled_str == "true" {
                        sync::feishu::get_auto_push_manager().set_enabled(true);
                        log::info!("飞书自动推送已恢复启用");
                    }
                }
                // 启动 watch 通道监听（无论是否启用，task 始终运行等待信号）
                sync::feishu::start_auto_push_watcher();
            }

            // 启动每日期限重算定时器（每天 00:01）
            tokio::spawn(async {
                deadline_recalc_scheduler().await;
            });

            Ok(())
        })
        .invoke_handler(commands::build_handler())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 每日期限重算定时器
/// 每天 00:01 自动重算所有活跃案件的期限
async fn deadline_recalc_scheduler() {
    use tokio::time::{sleep, Duration};

    loop {
        // 计算距离下一个 00:01 的等待时间
        let now = chrono::Local::now();
        let today_target = now.date_naive().and_hms_opt(0, 1, 0).unwrap();

        let wait_duration = if now.naive_local().time() < chrono::NaiveTime::from_hms_opt(0, 1, 0).unwrap() {
            // 还没到今天的 00:01，等到今天
            (today_target - now.naive_local())
                .to_std()
                .unwrap_or(Duration::from_secs(3600))
        } else {
            // 已经过了今天的 00:01，等到明天
            let tomorrow = now.date_naive().succ_opt().unwrap_or(now.date_naive());
            let tomorrow_run = tomorrow.and_hms_opt(0, 1, 0).unwrap();
            (tomorrow_run - now.naive_local())
                .to_std()
                .unwrap_or(Duration::from_secs(86400))
        };

        log::info!(
            "期限重算定时器已启动，下次运行: {}",
            now + chrono::Duration::from_std(wait_duration).unwrap_or_default()
        );

        sleep(wait_duration).await;

        // 执行期限重算
        match recalc_all_deadlines() {
            Ok(count) => {
                log::info!("每日期限重算完成，处理 {} 个案件", count);
            }
            Err(e) => {
                log::error!("每日期限重算失败: {}", e);
            }
        }
    }
}

/// 重算所有活跃案件的期限
fn recalc_all_deadlines() -> anyhow::Result<usize> {
    let conn = db::open_db()?;
    let engine = deadline::engine::DeadlineEngine::new(&conn)?;

    // 计算所有活跃案件的期限预警
    let warnings = engine.generate_all_warnings(&conn)?;
    let count = warnings.len();

    log::debug!("期限重算完成，共 {} 条预警", count);

    Ok(count)
}
