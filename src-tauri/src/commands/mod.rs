pub mod areas;
pub mod calendar;
pub mod cases;
pub mod docs;
pub mod drafts;
pub mod files;
pub mod import_feishu;
pub mod inbox;
pub mod knowledge;
pub mod relations;
pub mod reminder;
pub mod settings;
pub mod sync;
pub mod tasks;
pub mod timeline;
pub mod ai_routes;

// AI 和邮件命令直接在对应模块中定义

/// 将阻塞任务放入线程池执行
pub async fn run_blocking<T, F>(task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> anyhow::Result<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(task)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn import_feishu_data(json_path: String) -> Result<import_feishu::ImportReport, String> {
    run_blocking(move || {
        let mut conn = crate::db::open_db()?;
        let path = std::path::PathBuf::from(&json_path);
        import_feishu::import_feishu_dump(&mut conn, &path)
    })
    .await
}

#[tauri::command]
pub async fn get_deadline_warnings() -> Result<Vec<crate::deadline::engine::DeadlineResult>, String> {
    run_blocking(move || {
        let conn = crate::db::open_db()?;
        let engine = crate::deadline::engine::DeadlineEngine::new(&conn)?;
        engine.generate_all_warnings(&conn)
    })
    .await
}

pub fn build_handler() -> impl Fn(tauri::ipc::Invoke) -> bool {
    tauri::generate_handler![
        cases::list_cases,
        cases::get_case,
        cases::create_case,
        cases::update_case,
        cases::delete_case,
        cases::search_cases,
        cases::case_stats,
        cases::get_dashboard_stats,
        cases::list_field_groups,
        cases::get_case_unified_view,
        cases::recalculate_case_formulas,
        cases::recalculate_all_formulas,
        cases::export_cases,
        cases::update_case_status,
        cases::get_today_stats,
        import_feishu_data,
        get_deadline_warnings,
        tasks::list_tasks,
        tasks::create_task,
        tasks::toggle_task,
        tasks::delete_task,
        tasks::update_task,
        tasks::generate_hearing_prep_tasks,
        calendar::get_calendar_events,
        timeline::get_case_timeline,
        timeline::add_case_log,
        timeline::delete_case_log,
        sync::get_sync_status,
        sync::test_webdav_connection,
        sync::webdav_startup_sync,
        sync::webdav_push,
        sync::webdav_pull,
        sync::webdav_resolve_keep_local,
        sync::webdav_resolve_keep_remote,
        sync::configure_feishu,
        sync::test_feishu_connection,
        sync::sync_feishu_pull,
        sync::sync_feishu_push,
        sync::get_feishu_sync_info,
        sync::configure_feishu_table,
        sync::set_feishu_auto_push,
        sync::get_feishu_auto_push_status,
        sync::trigger_feishu_push,
        // 飞书通用同步 v3.0 命令
        sync::feishu_list_tables,
        sync::feishu_list_fields,
        sync::feishu_list_records,
        sync::feishu_compare_table,
        sync::feishu_compare_records,
        sync::feishu_save_mappings,
        sync::feishu_get_mappings,
        sync::feishu_import_all,
        sync::feishu_import_selected,
        sync::feishu_import_incremental,
        sync::feishu_sync_pull,
        sync::feishu_sync_push,
        knowledge::list_knowledge,
        knowledge::create_knowledge,
        knowledge::update_knowledge,
        knowledge::delete_knowledge,
        knowledge::search_knowledge,
        knowledge::knowledge_stats,
        knowledge::list_knowledge_versions,
        knowledge::diff_knowledge_versions,
        knowledge::diff_knowledge_with_current,
        knowledge::create_knowledge_from_selection,
        knowledge::link_knowledge_to_case,
        knowledge::link_knowledge_to_law,
        // 混合检索命令
        crate::db::search::hybrid_search_knowledge,
        crate::db::search::embed_knowledge,
        crate::db::search::embed_all_knowledge,
        inbox::add_inbox_item,
        inbox::list_inbox_items,
        inbox::process_inbox_item,
        inbox::file_inbox_item,
        inbox::dismiss_inbox_item,
        inbox::parse_holiday_notice,
        inbox::quick_judge_inbox_item,
        inbox::copy_file_with_progress,
        inbox::confirm_inbox_action,
        inbox::ai_analyze_inbox_item,
        relations::add_relation,
        relations::get_relations,
        relations::remove_relation,
        relations::detect_relations,
        drafts::create_draft,
        drafts::list_drafts,
        drafts::get_draft,
        drafts::update_draft,
        drafts::delete_draft,
        // Docsy 模板命令
        docs::list_docsy_templates,
        docs::render_docsy_template,
        docs::export_docx,
        // 邮件监听命令
        crate::email::configure_imap,
        crate::email::start_email_monitor,
        crate::email::stop_email_monitor,
        crate::email::get_email_monitor_status,
        crate::email::list_imap_accounts,
        crate::email::delete_imap_account,
        // AI 后端命令
        crate::ai::configure_ai,
        crate::ai::test_ai_connection,
        crate::ai::get_ai_config,
        crate::ai::get_ai_usage,
        crate::ai::generate_writing_suggestion,
        // 设置命令
        settings::get_settings,
        settings::save_settings,
        settings::import_holidays_json,
        settings::get_holidays_summary,
        settings::list_folder_templates,
        settings::get_folder_template,
        settings::save_folder_template,
        settings::delete_folder_template,
        settings::get_folder_naming_settings,
        settings::save_folder_naming_settings,
        // 文件管理命令
        files::list_case_files,
        files::add_case_file,
        files::delete_case_file,
        // 提醒规则命令
        reminder::list_reminder_rules,
        reminder::create_reminder_rule,
        reminder::update_reminder_rule,
        reminder::delete_reminder_rule,
        reminder::test_reminder,
        reminder::start_reminder_engine,
        reminder::get_reminder_log,
        // 任务模板命令
        tasks::list_task_templates,
        tasks::create_task_template,
        tasks::apply_task_template,
        // 领域命令
        areas::list_areas,
        areas::get_area,
        areas::create_area,
        areas::update_area,
        areas::delete_area,
        areas::get_area_stats,
        // 送达文书命令
        inbox::download_service_delivery,
        inbox::process_service_delivery,
        // 批量处理队列
        inbox::start_inbox_batch,
        inbox::pause_inbox_batch,
        inbox::resume_inbox_batch,
        inbox::cancel_inbox_batch,
        inbox::get_inbox_progress,
        inbox::retry_inbox_item,
        inbox::retry_inbox_case,
        // 日志调试命令
        get_log_dir,
        get_recent_logs,
        search_logs,
        // AI 路由与确认命令
        ai_routes::get_command_route_info,
        ai_routes::get_ai_run_history,
        ai_routes::check_confirmation_required,
        ai_routes::calculate_effective_policy_cmd,
    ]
}

// ═══════════════════════════════════════════════════════════
// 日志调试命令
// ═══════════════════════════════════════════════════════════

/// 获取日志目录路径
#[tauri::command]
pub fn get_log_dir() -> String {
    crate::app_log::log_dir_path()
}

/// 读取最近 N 行日志（默认 200 行）
#[tauri::command]
pub async fn get_recent_logs(lines: Option<usize>) -> Result<Vec<String>, String> {
    let log_dir = crate::app_log::log_dir_path();
    let dir_path = std::path::PathBuf::from(&log_dir);

    // 找最新的日志文件
    let mut entries: Vec<_> = std::fs::read_dir(&dir_path)
        .map_err(|e| format!("读取日志目录失败: {}", e))?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("casy.")
                && e.file_name().to_string_lossy().ends_with(".log")
        })
        .collect();

    entries.sort_by(|a, b| {
        b.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            .cmp(&a.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH))
    });

    let latest = entries.first()
        .ok_or("没有找到日志文件")?
        .path();

    let content = std::fs::read_to_string(&latest)
        .map_err(|e| format!("读取日志文件失败: {}", e))?;

    let max_lines = lines.unwrap_or(200);
    let all_lines: Vec<&str> = content.lines().collect();
    let start = all_lines.len().saturating_sub(max_lines);

    Ok(all_lines[start..].iter().map(|s| s.to_string()).collect())
}

/// 按关键词搜索日志
#[tauri::command]
pub async fn search_logs(keyword: String, limit: Option<usize>) -> Result<Vec<String>, String> {
    let log_dir = crate::app_log::log_dir_path();
    let dir_path = std::path::PathBuf::from(&log_dir);
    let max_results = limit.unwrap_or(100);

    let mut results = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(&dir_path)
        .map_err(|e| format!("读取日志目录失败: {}", e))?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("casy.")
                && e.file_name().to_string_lossy().ends_with(".log")
        })
        .collect();

    entries.sort_by(|a, b| {
        b.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            .cmp(&a.metadata().and_then(|m| m.modified()).unwrap_or(std::time::SystemTime::UNIX_EPOCH))
    });

    for entry in &entries {
        if results.len() >= max_results { break; }
        if let Ok(content) = std::fs::read_to_string(entry.path()) {
            for line in content.lines() {
                if line.to_lowercase().contains(&keyword.to_lowercase()) {
                    results.push(line.to_string());
                    if results.len() >= max_results { break; }
                }
            }
        }
    }

    Ok(results)
}
