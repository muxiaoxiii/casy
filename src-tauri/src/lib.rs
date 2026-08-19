mod app_log;
mod ai;
mod commands;
mod credentials;
mod db;
mod docsy_engine;
mod email;
mod files;
mod formula;
mod deadline;
mod mcp;
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

            // 全局热键: Cmd+I → 速记入袋（设计哲学 §10）
            {
                use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
                let shortcut = Shortcut::new(Some(Modifiers::SUPER), Code::KeyI);
                let handle = app.handle().clone();
                app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, event| {
                    if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        let _ = tauri::Emitter::emit(&handle, "global:quick_capture", "note");
                    }
                });
            }

            // 全局热键: Cmd+E → 新日程入袋
            {
                use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
                let shortcut = Shortcut::new(Some(Modifiers::SUPER), Code::KeyE);
                let handle = app.handle().clone();
                app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, event| {
                    if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        let _ = tauri::Emitter::emit(&handle, "global:quick_capture", "event");
                    }
                });
            }

            // 全局热键: Cmd+N → 新笔记入袋
            {
                use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
                let shortcut = Shortcut::new(Some(Modifiers::SUPER), Code::KeyN);
                let handle = app.handle().clone();
                app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, event| {
                    if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        let _ = tauri::Emitter::emit(&handle, "global:quick_capture", "note");
                    }
                });
            }

            // 文件夹监听: ~/Documents/Casy/inbox/
            if let Err(e) = watcher::start_inbox_watcher() {
                log::warn!("收件箱文件夹监听启动失败: {}", e);
            }

            // MCP Server（设计哲学 §11.11）：只读数据接口，绑定 127.0.0.1:37877
            // settings 表 mcp_server_enabled（默认 'true'），绑定失败只记日志不 panic
            {
                let mcp_enabled = db::get_setting(&conn, "mcp_server_enabled")
                    .ok()
                    .flatten()
                    .map(|v| v != "false")
                    .unwrap_or(true);
                if mcp_enabled {
                    tauri::async_runtime::spawn(async {
                        if let Err(e) = mcp::server::run().await {
                            log::error!("MCP server 未启动: {}", e);
                        }
                    });
                } else {
                    log::info!("MCP server 已禁用（settings.mcp_server_enabled=false）");
                }
            }

            // IMAP 凭据迁移：base64 → OS keychain（best-effort，失败只记日志）
            tauri::async_runtime::spawn(async {
                let result = tauri::async_runtime::spawn_blocking(
                    credentials::migrate_imap_passwords_to_keychain,
                )
                .await;
                match result {
                    Ok(Ok(r)) => {
                        if r.migrated > 0 || r.failed > 0 {
                            log::info!(
                                "IMAP 凭据迁移完成：迁移 {}，跳过 {}，失败 {}",
                                r.migrated,
                                r.skipped,
                                r.failed
                            );
                        }
                    }
                    Ok(Err(e)) => log::warn!("IMAP 凭据迁移失败: {}", e),
                    Err(e) => log::warn!("IMAP 凭据迁移任务异常: {}", e),
                }
            });

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
            tauri::async_runtime::spawn(async {
                deadline_recalc_scheduler().await;
            });

            // 自动报表调度（设计哲学 §11.3）
            // 每天 08:00 生成早报；每周日 21:00 生成周报
            tauri::async_runtime::spawn(async {
                daily_brief_scheduler().await;
            });
            tauri::async_runtime::spawn(async {
                weekly_report_scheduler().await;
            });

            // 决策复核调度（设计哲学 §11.7）：每日 08:30（早报之后）
            tauri::async_runtime::spawn(async {
                decision_review_scheduler().await;
            });

            // 数据蒸馏调度（设计哲学 §11.10）：每周日 23:00 清理 + 提炼
            tauri::async_runtime::spawn(async {
                distillation_scheduler().await;
            });

            // 隐性关联学习调度（设计哲学 §3.2 通道 B）：每周六 22:00
            // （错开周日 21:00 周报 / 23:00 蒸馏）
            tauri::async_runtime::spawn(async {
                insights_scheduler().await;
            });

            // 启动提醒引擎（每 5 分钟检查期限/开庭/任务规则）
            // 幂等：引擎内部 running 标志保证单实例
            tauri::async_runtime::spawn(async move {
                match commands::reminder::start_reminder_engine(Some(300)).await {
                    Ok(_) => log::info!("提醒引擎已在启动时拉起"),
                    Err(e) => log::warn!("提醒引擎启动失败: {}", e),
                }
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

/// 计算到下一个触发点的等待时长
/// weekday 为 None 表示每天触发，Some 表示每周固定星期几触发
fn next_trigger_delay(weekday: Option<chrono::Weekday>, hour: u32, minute: u32) -> std::time::Duration {
    use chrono::Datelike;
    use tokio::time::Duration;

    let now = chrono::Local::now();
    let days_ahead: i64 = match weekday {
        Some(wd) => {
            (wd.num_days_from_monday() as i64 - now.weekday().num_days_from_monday() as i64 + 7) % 7
        }
        None => 0,
    };

    let target_date = now.date_naive() + chrono::Duration::days(days_ahead);
    let mut target = target_date
        .and_hms_opt(hour, minute, 0)
        .unwrap_or_else(|| target_date.and_hms_opt(0, 0, 0).unwrap());

    // 今天的触发点已过 → 推到下一周期
    if target <= now.naive_local() {
        let step = if weekday.is_some() { 7 } else { 1 };
        let next_date = target_date + chrono::Duration::days(step);
        target = next_date
            .and_hms_opt(hour, minute, 0)
            .unwrap_or_else(|| next_date.and_hms_opt(0, 0, 0).unwrap());
    }

    (target - now.naive_local())
        .to_std()
        .unwrap_or(Duration::from_secs(3600))
}

/// 每日早报调度：每天 08:00 自动生成（失败只记日志不 panic）
async fn daily_brief_scheduler() {
    use tokio::time::sleep;

    loop {
        let wait = next_trigger_delay(None, 8, 0);
        log::info!("每日早报定时器已启动，下次运行: {:?}", wait);
        sleep(wait).await;

        // 先规则版落库（确定性数据一定在），再尝试叙事层覆盖 content（§11.3 / §12.5）
        let result = db::open_db().and_then(|conn| {
            let today = chrono::Local::now().format("%Y-%m-%d").to_string();
            ai::reports::generate_daily_brief(&conn, &today)
        });

        match result {
            Ok(_) => {
                log::info!("每日早报自动生成完成");
                let today = chrono::Local::now().format("%Y-%m-%d").to_string();
                let _ = ai::reports::try_narrative_layer("daily", &today, "daily_brief_narrative").await;
            }
            Err(e) => log::error!("每日早报自动生成失败: {}", e),
        }
    }
}

/// 每周总结调度：每周日 21:00 自动生成（失败只记日志不 panic）
async fn weekly_report_scheduler() {
    use tokio::time::sleep;

    loop {
        let wait = next_trigger_delay(Some(chrono::Weekday::Sun), 21, 0);
        log::info!("每周总结定时器已启动，下次运行: {:?}", wait);
        sleep(wait).await;

        let result = db::open_db().and_then(|conn| ai::reports::generate_weekly_summary(&conn));

        match result {
            Ok(summary) => {
                log::info!("每周总结自动生成完成");
                // 先规则版落库，再尝试叙事层覆盖（§11.3 / §12.5）
                let _ = ai::reports::try_narrative_layer("weekly", &summary.week_start, "weekly_brief_narrative").await;
            }
            Err(e) => log::error!("每周总结自动生成失败: {}", e),
        }
    }
}

/// 决策复核调度（设计哲学 §11.7）：每日 08:30（早报之后）检查到期决策，
/// 有待复核决策时向前端 emit 'decision:review-due' 事件（照 reminder 模块的通知模式）
async fn decision_review_scheduler() {
    use tokio::time::sleep;

    loop {
        let wait = next_trigger_delay(None, 8, 30);
        log::info!("决策复核定时器已启动，下次运行: {:?}", wait);
        sleep(wait).await;

        let result = db::open_db().and_then(|conn| commands::decisions::pending_decision_reviews(&conn));

        match result {
            Ok(pending) if !pending.is_empty() => {
                log::info!("有 {} 条决策到期待复核", pending.len());
                if let Some(handle) = get_app_handle() {
                    let _ = tauri::Emitter::emit(
                        handle,
                        "decision:review-due",
                        serde_json::json!({
                            "count": pending.len(),
                            "decisions": pending,
                            "at": db::now_local(),
                        }),
                    );
                }
            }
            Ok(_) => {}
            Err(e) => log::error!("决策复核检查失败: {}", e),
        }
    }
}

/// 数据蒸馏调度：每周日 23:00 自动清理 + 提炼（失败只记日志不 panic）
async fn distillation_scheduler() {
    use tokio::time::sleep;

    loop {
        let wait = next_trigger_delay(Some(chrono::Weekday::Sun), 23, 0);
        log::info!("数据蒸馏定时器已启动，下次运行: {:?}", wait);
        sleep(wait).await;

        let result = db::open_db().and_then(|conn| ai::distillation::run_distillation(&conn));

        match result {
            Ok(r) => log::info!(
                "数据蒸馏完成：清理 {} 条，新增候选 {} 条，合并 {} 条，陈旧 {} 条，归档 {} 条",
                r.cleaned_count, r.inserted_count, r.merged_count, r.stale_count, r.archived_count
            ),
            Err(e) => log::error!("数据蒸馏失败: {}", e),
        }
    }
}

/// 隐性关联学习调度（设计哲学 §3.2 通道 B）：每周六 22:00 生成关联洞察
/// （失败只记日志不 panic；AI 未配置时静默跳过）
async fn insights_scheduler() {
    use tokio::time::sleep;

    loop {
        let wait = next_trigger_delay(Some(chrono::Weekday::Sat), 22, 0);
        log::info!("隐性关联学习定时器已启动，下次运行: {:?}", wait);
        sleep(wait).await;

        // generate_relation_insights 内部会起独立 runtime 调 async AI，
        // 必须放到阻塞线程执行（照 inbox.rs 的 Runtime::new().block_on 模式）
        let result = tauri::async_runtime::spawn_blocking(|| {
            db::open_db().and_then(|conn| ai::insights::generate_relation_insights(&conn))
        })
        .await;

        match result {
            Ok(Ok(n)) => log::info!("隐性关联学习完成：新增洞察 {} 条", n),
            Ok(Err(e)) => log::error!("隐性关联学习失败: {}", e),
            Err(e) => log::error!("隐性关联学习任务异常: {}", e),
        }
    }
}
