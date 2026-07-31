pub mod calendar;
pub mod cases;
pub mod docs;
pub mod drafts;
pub mod files;
pub mod import_feishu;
pub mod inbox;
pub mod knowledge;
pub mod relations;
pub mod settings;
pub mod sync;
pub mod tasks;
pub mod timeline;

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
pub async fn get_deadline_warnings() -> Result<Vec<crate::formula::engine::DeadlineResult>, String> {
    run_blocking(move || {
        let conn = crate::db::open_db()?;
        let engine = crate::formula::engine::DeadlineEngine::new(&conn)?;
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
        cases::export_cases,
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
        // 文件管理命令
        files::list_case_files,
        files::add_case_file,
        files::delete_case_file,
    ]
}
