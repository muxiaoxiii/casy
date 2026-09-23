mod rest_day_reminders;
pub mod editor_tasks;
pub mod ai_routes;
pub mod areas;
pub mod backup;
pub mod portable_backup;
pub mod editor_recovery;
pub mod caldav;
pub mod calendar;
pub mod calendar_events;
pub mod task_plans;
mod personal_availability;
pub mod cases;
pub mod dashboard;
pub mod deadline_rules;
pub mod decisions;
pub mod demo;
pub mod docs;
pub mod document_intelligence;
pub mod conversion;
mod markdown_export;
pub mod processing;
pub mod drafts;
pub mod feishu_snapshot;
pub mod files;
pub mod filters;
pub mod import_excel;
pub mod import_feishu;
pub mod inbox;
mod inbox_actions;
pub mod knowledge;
pub mod linking;
pub mod notifications;
pub mod persons;
pub mod procedure;
pub mod projects;
pub mod relations;
pub mod reminder;
pub mod search;
pub mod settings;
pub mod smart_rules;
pub mod sync;
pub mod tasks;
mod task_lifecycle;
pub mod timeline;
pub mod whiteboard;
pub mod whiteboard_scene;
pub mod whiteboard_document;

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
pub async fn get_deadline_warnings() -> Result<Vec<crate::deadline::engine::DeadlineResult>, String>
{
    run_blocking(move || {
        let conn = crate::db::open_db()?;
        let engine = crate::deadline::engine::DeadlineEngine::new(&conn)?;
        engine.generate_all_warnings(&conn)
    })
    .await
}

/// 前端崩溃/未捕获异常落盘（R-6；D-13 严格本地不上报）
#[tauri::command]
pub async fn append_fe_crash(
    message: String,
    stack: Option<String>,
    url: Option<String>,
) -> Result<(), String> {
    run_blocking(move || {
        crate::app_log::append_crash_line(
            "crash-fe",
            &serde_json::json!({
                "ts": chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f").to_string(),
                "version": env!("CARGO_PKG_VERSION"),
                "kind": "fe_exception",
                "message": message,
                "stack": stack,
                "url": url,
            })
            .to_string(),
        );
        Ok(())
    })
    .await
}

/// AI 工具调用审计（K-3 归因：ai_runs 记过程、audit_events 记结果，actor='ai'）
/// digest 仅参数键名列表（脱敏，§11.9：模型可见即记录、内容不入审计）
#[tauri::command]
pub async fn record_ai_tool_audit(
    tool: String,
    turn_id: String,
    outcome: String,
    digest: Option<String>,
    run_id: Option<String>,
) -> Result<(), String> {
    run_blocking(move || {
        let conn = crate::db::open_db()?;
        let payload = serde_json::json!({
            "outcome": outcome,
            "turn_id": turn_id,
            "digest": digest,
            "run_id": run_id,
        });
        conn.execute(
            "INSERT INTO audit_events (id, aggregate_type, aggregate_id, event_type, payload, actor)
             VALUES (?1, 'ai_tool_call', ?2, ?3, ?4, 'ai')",
            rusqlite::params![crate::db::new_id(), turn_id, tool, payload.to_string()],
        )?;
        Ok(())
    })
    .await
}

pub fn build_handler() -> impl Fn(tauri::ipc::Invoke) -> bool {
    tauri::generate_handler![
        notifications::list_notifications,
        notifications::unread_notification_count,
        notifications::create_notification,
        notifications::mark_notification_read,
        notifications::dismiss_notification,
        notifications::dismiss_all_notifications,
        tasks::defer_task,
        tasks::clear_task_defer,
        ai_routes::get_proposal_preview,
        processing::get_processing_center,
        processing::register_conversion_batch,
        processing::cancel_queued_conversions,
        processing::cancel_conversion,
        document_intelligence::get_document_engine_status,
        document_intelligence::get_document_page,
        document_intelligence::correct_document_region,
        document_intelligence::queue_document_processing,
        document_intelligence::list_document_jobs,
        document_intelligence::list_case_document_jobs,
        document_intelligence::retry_document_job,
        document_intelligence::cancel_document_job,
        deadline_rules::list_deadline_rules,
        deadline_rules::upsert_deadline_rule,
        deadline_rules::toggle_deadline_rule,
        deadline_rules::delete_deadline_rule,
        deadline_rules::list_deadline_rule_audit,
        crate::deadline::recalc::recalculate_deadlines_for_track,
        linking::create_link,
        linking::remove_link,
        linking::list_links_for,
        linking::get_backlinks,
        persons::list_persons,
        persons::upsert_person,
        persons::delete_person,
        persons::attach_person_to_case,
        persons::detach_person_from_case,
        persons::list_case_persons,
        persons::list_person_cases,
        smart_rules::list_smart_rules,
        smart_rules::upsert_smart_rule,
        smart_rules::delete_smart_rule,
        smart_rules::apply_smart_rules,
        smart_rules::run_smart_rules_for_all,
        smart_rules::list_pending_ocr_files,
        smart_rules::ocr_case_file,
        smart_rules::ocr_all_pending,
        smart_rules::list_case_ocr_states,
        smart_rules::get_file_ocr_text,
        whiteboard_document::get_whiteboard_document,
            editor_tasks::get_editor_tasks,
            editor_tasks::bind_editor_task,
            editor_tasks::set_editor_task_completed,
        whiteboard_document::save_whiteboard_document,
        whiteboard_document::list_whiteboard_sources,
        whiteboard_document::write_whiteboard_export,
        whiteboard_scene::get_whiteboard_scene,
        whiteboard_scene::save_whiteboard_scene,
        whiteboard_scene::list_whiteboard_scene_history,
        whiteboard::list_whiteboards,
        whiteboard::create_whiteboard,
        whiteboard::rename_whiteboard,
        whiteboard::delete_whiteboard,
        whiteboard::list_fact_nodes,
        whiteboard::list_fact_history,
            procedure::get_procedure_board,
            procedure::save_procedure_event,
            procedure::set_procedure_item_state,
            procedure::get_procedure_history,
        whiteboard::create_fact_node,
        whiteboard::update_fact_node,
        whiteboard::delete_fact_node,
        whiteboard::list_whiteboard_edges,
        whiteboard::create_whiteboard_edge,
        whiteboard::delete_whiteboard_edge,
        search::global_search,
        search::reasoning_search,
        search::search_document_passages,
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
        cases::list_case_hearings,
        cases::create_case_hearing,
        cases::update_case_hearing,
        cases::delete_case_hearing,
        cases::get_today_stats,
        cases::get_case_type_metrics,
        cases::get_all_case_type_metrics,
        import_feishu_data,
        import_feishu::feishu_check_config,
        import_feishu::feishu_inspect_bitable,
        feishu_snapshot::feishu_download_snapshot,
        feishu_snapshot::feishu_import_snapshot,
        feishu_snapshot::get_case_source_records,
        feishu_snapshot::get_imported_asset,
        feishu_snapshot::export_imported_asset,
        feishu_snapshot::save_feishu_snapshot,
        import_feishu::feishu_import_bitable_cases,
        import_feishu::feishu_import_bitable_subtable,
        import_excel::excel_get_sheets,
        import_excel::excel_inspect_sheet,
        import_excel::excel_import_cases,
        import_excel::excel_import_subtable,
        get_deadline_warnings,
        record_ai_tool_audit,
        append_fe_crash,
        tasks::list_tasks,
        tasks::create_task,
        tasks::toggle_task,
        tasks::delete_task,
        tasks::restore_task,
        tasks::snooze_task,
        tasks::update_task,
        tasks::search_tasks,
        tasks::generate_hearing_prep_tasks,
        calendar::get_calendar_events,
        calendar_events::list_calendar_events,
        calendar_events::create_calendar_event,
        calendar_events::update_calendar_event,
        task_plans::list_task_plans,
        task_plans::save_task_plan,
        calendar_events::move_calendar_event,
        calendar_events::delete_calendar_event,
        projects::list_projects,
        projects::create_personal_project,
        projects::update_personal_project,
        projects::delete_project,
        backup::create_backup,
        backup::list_backups,
        backup::restore_backup,
        backup::verify_backup_integrity_cmd,
        portable_backup::export_full_backup,
        portable_backup::import_full_backup,
        editor_recovery::save_editor_recovery,
        editor_recovery::recover_editor_drafts,
        dashboard::get_project_status_distribution,
        dashboard::get_track_distribution,
        dashboard::get_monthly_task_trend,
        dashboard::get_upcoming_hearings,
        dashboard::get_today_kpis,
        demo::seed_demo_data,
        timeline::get_case_timeline,
        timeline::add_case_log,
        timeline::delete_case_log,
        sync::get_sync_status,
        sync::test_webdav_connection,
        sync::webdav_startup_sync,
        sync::webdav_push,
        sync::webdav_backup_full,
        sync::webdav_restore_full,
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
        sync::sync_export_persons_to_vcard,
        sync::sync_export_whiteboards_to_blob,
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
        knowledge::get_knowledge_graph,
        // 知识块级化（设计哲学 §8.2）
        knowledge::list_knowledge_blocks,
        knowledge::get_knowledge_with_blocks,
        knowledge::list_knowledge_document_sources,
        knowledge::import_pageindex_to_knowledge,
        knowledge::restore_knowledge_version,
        knowledge::export_knowledge_markdown,
        // 混合检索命令
        crate::db::search::hybrid_search_knowledge,
        crate::db::search::search_knowledge_index,
        crate::db::knowledge_index::embed_knowledge,
        crate::db::knowledge_index::embed_all_knowledge,
        crate::db::knowledge_index::get_knowledge_index_status,
        crate::db::knowledge_index::cancel_knowledge_index_job,
        crate::ai::embeddings::test_embedding_connection,
        inbox::add_inbox_item,
        inbox::list_inbox_items,
        inbox::process_inbox_item,
        inbox::file_inbox_item,
        inbox::dismiss_inbox_item,
        inbox::parse_holiday_notice,
        inbox::get_inbox_action_result,
        inbox::quick_judge_inbox_item,
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
        docs::export_edited_docx,
        docs::export_editor_document,
        docs::preview_editor_document,
        conversion::convert_file_to_markdown,
        // 邮件监听命令
        crate::email::configure_imap,
        crate::email::start_email_monitor,
        crate::email::stop_email_monitor,
        crate::email::get_email_monitor_status,
        crate::email::list_imap_accounts,
        crate::email::delete_imap_account,
        // SMTP ICS 邀请发送
        crate::email::smtp::send_ics_invitation_cmd,
        // AI 后端命令
        crate::ai::configure_ai,
        crate::ai::test_ai_connection,
        crate::ai::get_ai_config,
        crate::ai::get_ai_usage,
        crate::ai::generate_writing_suggestion,
        crate::ai::ai_chat,
        crate::ai::profiles::get_ai_profiles,
        crate::ai::profiles::save_ai_profiles,
        crate::ai::profiles::test_ai_profile,
        // 设置命令
        settings::get_settings,
        settings::backup_database_key_to_keychain,
        settings::save_settings,
        settings::import_holidays_json,
        settings::get_holidays_summary,
        settings::get_holiday_calendar,
        settings::list_folder_templates,
        settings::get_folder_template,
        settings::save_folder_template,
        settings::delete_folder_template,
        settings::get_folder_naming_settings,
        settings::save_folder_naming_settings,
        settings::get_lawyer_profile,
        settings::save_lawyer_profile,
        // 文件管理命令
        files::list_case_files,
        files::add_case_file,
        files::delete_case_file,
        files::list_removed_case_files,
        files::restore_case_file,
        files::move_case_files,
        files::set_case_file_category,
        files::list_case_dirs,
        files::create_case_subdir,
        files::import_files_to_case,
        files::scan_unregistered_files,
        files::register_existing_files,
        crate::workspace_sync::get_workspace_sync_status,
        crate::workspace_sync::list_workspace_sources,
        crate::workspace_sync::get_workspace_document,
        files::reveal_path,
        files::open_file_with_default,
        files::apply_case_file_renames,
        // 提醒规则命令
        reminder::list_reminder_rules,
        reminder::create_reminder_rule,
        reminder::update_reminder_rule,
        reminder::delete_reminder_rule,
        reminder::test_reminder,
        reminder::start_reminder_engine,
        reminder::reminder_recompute_now,
        reminder::record_reminder_feedback,
        reminder::get_reminder_log,
        // 分级预警 R1-R4（设计哲学 §11.2）
        reminder::get_deadline_warnings_with_levels,
        // CalDAV 日历同步（设计哲学 §11.2 M1）
        caldav::test_caldav_connection,
        caldav::sync_reminders_to_calendar,
        caldav::get_calendar_sync_status,
        caldav::cancel_reminder_jobs_for,
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
        // 多通道捕获（设计哲学 §10）
        inbox::capture_screenshot,
        inbox::capture_clipboard,
        inbox::start_clipboard_monitor,
        inbox::save_voice_note,
        inbox::transcribe_voice_note,
        inbox::reject_inbox_recommendation,
        // 日志调试命令
        get_log_dir,
        get_recent_logs,
        search_logs,
        // AI 路由与确认命令
        ai_routes::get_command_route_info,
        ai_routes::get_ai_run_history,
        ai_routes::check_confirmation_required,
        ai_routes::calculate_effective_policy_cmd,
        ai_routes::create_ai_proposal,
        ai_routes::get_ai_proposal,
        ai_routes::approve_ai_proposal,
        ai_routes::renew_ai_proposal,
        ai_routes::reject_ai_proposal,
        // MCP Server 命令（设计哲学 §11.11）
        mcp_list_tools,
        mcp_execute_tool,
        list_mcp_pending_writes,
        approve_mcp_write,
        reject_mcp_write,
        crate::mcp::server::get_mcp_server_info,
        // 凭据安全存储命令
        migrate_credentials_to_keychain,
        check_keychain_status,
        // AI 推荐引擎命令（设计哲学 §11.6）
        get_today_recommendations,
        // 自动报表命令（设计哲学 §11.3）
        generate_daily_brief_cmd,
        generate_weekly_summary_cmd,
        get_today_brief,
        get_latest_weekly_summary,
        list_summaries,
        // 行为学习分析命令（设计哲学 §11.9）
        get_learning_analysis,
        apply_learning_calibration,
        // 数据蒸馏命令（设计哲学 §11.10）
        run_distillation_cmd,
        list_pending_memories,
        confirm_memory,
        dismiss_memory,
        // 隐性关联学习命令（设计哲学 §3.2 通道 B）
        generate_insights_cmd,
        list_pending_insights,
        confirm_insight,
        dismiss_insight,
        // Saved Filters 命令（设计哲学 §9）
        filters::list_saved_filters,
        filters::save_filter,
        filters::delete_filter,
        // 决策记录命令（设计哲学 §11.6）
        decisions::record_decision,
        decisions::list_decisions,
        // 决策复核命令（设计哲学 §11.7）
        decisions::get_pending_decision_reviews,
        decisions::mark_decision_reviewed,
        // L3 递归确认命令（设计哲学 §11.5）
        crate::ai::recursive_check::run_recursive_check,
    ]
}

// ═══════════════════════════════════════════════════════════
// MCP Server 命令（设计哲学 §11.11）
// ═══════════════════════════════════════════════════════════

/// 获取 MCP 工具列表
#[tauri::command]
pub fn mcp_list_tools() -> Vec<serde_json::Value> {
    crate::mcp::get_tools()
        .into_iter()
        .map(|t| serde_json::to_value(t).unwrap_or_default())
        .collect()
}

/// 执行 MCP 工具调用（只读操作）
#[tauri::command]
pub async fn mcp_execute_tool(
    tool: String,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let call = crate::mcp::McpToolCall { tool, arguments };
    let result = crate::mcp::execute_tool(call).await;
    serde_json::to_value(result).map_err(|e| e.to_string())
}

/// 列出 MCP 待确认写操作（status='pending'，设计哲学 §11.11）
#[tauri::command]
pub async fn list_mcp_pending_writes() -> Result<Vec<crate::mcp::McpPendingWrite>, String> {
    run_blocking(move || {
        let conn = crate::db::open_db()?;
        crate::mcp::list_pending_writes(&conn)
    })
    .await
}

/// 批准并执行一条 MCP 待确认写（用户在应用内完成 L3 确认）
///
/// 通过 MCP 内部 allow_write 路径执行真实写；结果回写 mcp_pending_writes
/// （executed / failed），并留痕 audit_events（actor='mcp'）。
#[tauri::command]
pub async fn approve_mcp_write(id: String) -> Result<serde_json::Value, String> {
    let pending = {
        let id = id.clone();
        run_blocking(move || {
            let conn = crate::db::open_db()?;
            match crate::mcp::get_pending_write(&conn, &id)? {
                Some(w) if w.status == "pending" => Ok(w),
                Some(w) => Err(anyhow::anyhow!("该写操作已处理（status={}）", w.status)),
                None => Err(anyhow::anyhow!("待确认写不存在: {}", id)),
            }
        })
        .await?
    };

    // 执行真实写（MCP 内部 allow_write 路径）
    let call = crate::mcp::McpToolCall {
        tool: pending.tool.clone(),
        arguments: serde_json::from_str(&pending.arguments).unwrap_or(serde_json::Value::Null),
    };
    let exec_result = crate::mcp::execute_tool(call).await;

    let (status, result_text) = match &exec_result {
        Ok(v) => ("executed", serde_json::to_string(v).unwrap_or_default()),
        Err(e) => ("failed", e.clone()),
    };

    let write_id = pending.id.clone();
    let tool = pending.tool.clone();
    let audit_status = status;
    let audit_result = result_text.clone();
    run_blocking(move || {
        let conn = crate::db::open_db()?;
        crate::mcp::resolve_pending_write(&conn, &write_id, audit_status, Some(&audit_result))?;
        crate::mcp::write_mcp_audit(
            &conn,
            &write_id,
            "mcp_write_approved",
            &serde_json::json!({ "tool": tool, "outcome": audit_status }),
        )?;
        Ok(())
    })
    .await?;

    match exec_result {
        Ok(v) => Ok(serde_json::json!({ "status": "executed", "result": v })),
        Err(e) => Err(format!("写操作执行失败（已记录为 failed）: {}", e)),
    }
}

/// 拒绝一条 MCP 待确认写（不执行，仅标记 rejected 并留痕）
#[tauri::command]
pub async fn reject_mcp_write(id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = crate::db::open_db()?;
        let pending = match crate::mcp::get_pending_write(&conn, &id)? {
            Some(w) if w.status == "pending" => w,
            Some(w) => return Err(anyhow::anyhow!("该写操作已处理（status={}）", w.status)),
            None => return Err(anyhow::anyhow!("待确认写不存在: {}", id)),
        };
        crate::mcp::resolve_pending_write(&conn, &id, "rejected", None)?;
        crate::mcp::write_mcp_audit(
            &conn,
            &id,
            "mcp_write_rejected",
            &serde_json::json!({ "tool": pending.tool }),
        )?;
        Ok(())
    })
    .await
}

// ═══════════════════════════════════════════════════════════
// 凭据安全存储命令
// ═══════════════════════════════════════════════════════════

/// 迁移 IMAP 密码从 base64 到 Keychain
#[tauri::command]
pub async fn migrate_credentials_to_keychain() -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let result = crate::credentials::migrate_imap_passwords_to_keychain()?;
        serde_json::to_value(result).map_err(anyhow::Error::msg)
    })
    .await
}

/// 检查 Keychain 状态
#[tauri::command]
pub async fn check_keychain_status() -> Result<serde_json::Value, String> {
    run_blocking(move || {
        // 检查 keyring 是否可用
        let test_entry = keyring::Entry::new("casy-test", "test")
            .map_err(|e| anyhow::anyhow!("Keychain 不可用: {}", e))?;

        // 尝试写入测试值
        test_entry
            .set_password("test")
            .map_err(|e| anyhow::anyhow!("Keychain 写入失败: {}", e))?;

        // 读取测试值
        let _ = test_entry
            .get_password()
            .map_err(|e| anyhow::anyhow!("Keychain 读取失败: {}", e))?;

        // 清理
        let _ = test_entry.delete_credential();

        // 检查已迁移的账号
        let conn = crate::db::open_db()?;
        let mut stmt = conn.prepare("SELECT id, email_address, password_enc FROM imap_accounts")?;

        let accounts: Vec<serde_json::Value> = stmt
            .query_map([], |row| {
                let email: String = row.get(1)?;
                let has_keychain = crate::credentials::has_credential(
                    crate::credentials::CredentialType::ImapPassword,
                    &email,
                );
                Ok(serde_json::json!({
                    "id": row.get::<_, String>(0)?,
                    "email": email,
                    "hasLegacyPassword": !row.get::<_, String>(2)?.is_empty(),
                    "hasKeychainPassword": has_keychain,
                }))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(serde_json::json!({
            "keychainAvailable": true,
            "accounts": accounts,
        }))
    })
    .await
}

// ═══════════════════════════════════════════════════════════
// AI 推荐引擎命令（设计哲学 §11.6）
// ═══════════════════════════════════════════════════════════

/// 获取今日推荐
#[tauri::command]
pub async fn get_today_recommendations() -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = crate::db::open_db()?;
        let result = crate::ai::recommender::generate_today_recommendations(&conn)?;
        serde_json::to_value(result).map_err(anyhow::Error::msg)
    })
    .await
}

/// 生成每日早报（手动触发，落库 daily_stats + smart_summaries；随后尝试叙事层覆盖）
#[tauri::command]
pub async fn generate_daily_brief_cmd() -> Result<serde_json::Value, String> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let date = today.clone();
    let result = run_blocking(move || {
        let conn = crate::db::open_db()?;
        let brief = crate::ai::reports::generate_daily_brief(&conn, &date)?;
        serde_json::to_value(brief).map_err(anyhow::Error::msg)
    })
    .await?;

    // 叙事层（§11.3）：规则版已落库，AI 可用时覆盖 content；失败静默回退（§12.5）
    let _ = crate::ai::reports::try_narrative_layer("daily", &today, "daily_brief_narrative").await;

    Ok(result)
}

/// 获取今日早报（smart_summaries 有则取之，没有则现算）
#[tauri::command]
pub async fn get_today_brief() -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = crate::db::open_db()?;
        crate::ai::reports::get_today_brief(&conn)
    })
    .await
}

/// 生成每周总结（手动触发，落库 smart_summaries；随后尝试叙事层覆盖）
#[tauri::command]
pub async fn generate_weekly_summary_cmd() -> Result<serde_json::Value, String> {
    let result = run_blocking(move || {
        let conn = crate::db::open_db()?;
        let summary = crate::ai::reports::generate_weekly_summary(&conn)?;
        serde_json::to_value(summary).map_err(anyhow::Error::msg)
    })
    .await?;

    // 叙事层（§11.3）：规则版已落库，AI 可用时覆盖 content；失败静默回退（§12.5）
    if let Some(week_start) = result["weekStart"].as_str() {
        let _ =
            crate::ai::reports::try_narrative_layer("weekly", week_start, "weekly_brief_narrative")
                .await;
    }

    Ok(result)
}

/// 获取最新一期周报（没有则现算）
#[tauri::command]
pub async fn get_latest_weekly_summary() -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = crate::db::open_db()?;
        crate::ai::reports::get_latest_weekly_summary(&conn)
    })
    .await
}

/// 获取行为学习分析
#[tauri::command]
pub async fn get_learning_analysis() -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = crate::db::open_db()?;
        let result = crate::ai::learning::generate_learning_analysis(&conn)?;
        serde_json::to_value(result).map_err(anyhow::Error::msg)
    })
    .await
}

/// 执行数据蒸馏
#[tauri::command]
pub async fn run_distillation_cmd() -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = crate::db::open_db()?;
        let result = crate::ai::distillation::run_distillation(&conn)?;
        serde_json::to_value(result).map_err(anyhow::Error::msg)
    })
    .await
}

/// 应用预估校准（把偏差 >50% 或未设预估的未完成任务更新为历史均值）
#[tauri::command]
pub async fn apply_learning_calibration() -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = crate::db::open_db()?;
        let result = crate::ai::learning::apply_estimation_calibration(&conn)?;
        serde_json::to_value(result).map_err(anyhow::Error::msg)
    })
    .await
}

/// 列出待确认候选记忆
#[tauri::command]
pub async fn list_pending_memories() -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = crate::db::open_db()?;
        let result = crate::ai::distillation::list_pending_memories(&conn)?;
        serde_json::to_value(result).map_err(anyhow::Error::msg)
    })
    .await
}

/// 采纳候选记忆（可选同时沉淀进 knowledge_items 经验类）
#[tauri::command]
pub async fn confirm_memory(
    id: String,
    sink_to_knowledge: Option<bool>,
) -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = crate::db::open_db()?;
        let result = crate::ai::distillation::confirm_memory(
            &conn,
            &id,
            sink_to_knowledge.unwrap_or(false),
        )?;
        serde_json::to_value(result).map_err(anyhow::Error::msg)
    })
    .await
}

/// 丢弃候选记忆
#[tauri::command]
pub async fn dismiss_memory(id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = crate::db::open_db()?;
        crate::ai::distillation::dismiss_memory(&conn, &id)
    })
    .await
}

/// 列出报表历史（smart_summaries，summary_type 可选过滤，§11.3 报表浏览）
#[tauri::command]
pub async fn list_summaries(
    summary_type: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<serde_json::Value>, String> {
    run_blocking(move || {
        let conn = crate::db::open_db()?;
        crate::ai::reports::list_summaries(&conn, summary_type.as_deref(), limit)
    })
    .await
}

// ═══════════════════════════════════════════════════════════
// 隐性关联学习命令（设计哲学 §3.2 通道 B）
// ═══════════════════════════════════════════════════════════

/// 手动触发隐性关联洞察生成（AI 未配置时静默返回 0）
#[tauri::command]
pub async fn generate_insights_cmd() -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = crate::db::open_db()?;
        let inserted = crate::ai::insights::generate_relation_insights(&conn)?;
        Ok(serde_json::json!({ "inserted": inserted }))
    })
    .await
}

/// 列出待确认关联洞察
#[tauri::command]
pub async fn list_pending_insights() -> Result<Vec<serde_json::Value>, String> {
    run_blocking(move || {
        let conn = crate::db::open_db()?;
        crate::ai::insights::list_pending_insights(&conn)
    })
    .await
}

/// 确认关联洞察（status → confirmed，可选沉淀 knowledge_items 经验类）
#[tauri::command]
pub async fn confirm_insight(
    id: String,
    sink_to_knowledge: Option<bool>,
) -> Result<serde_json::Value, String> {
    run_blocking(move || {
        let conn = crate::db::open_db()?;
        crate::ai::insights::confirm_insight(&conn, &id, sink_to_knowledge.unwrap_or(false))
    })
    .await
}

/// 丢弃关联洞察（status → rejected）
#[tauri::command]
pub async fn dismiss_insight(id: String) -> Result<(), String> {
    run_blocking(move || {
        let conn = crate::db::open_db()?;
        crate::ai::insights::dismiss_insight(&conn, &id)
    })
    .await
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
            e.file_name().to_string_lossy().starts_with("casy.")
                && e.file_name().to_string_lossy().ends_with(".log")
        })
        .collect();

    entries.sort_by(|a, b| {
        b.metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            .cmp(
                &a.metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
            )
    });

    let latest = entries.first().ok_or("没有找到日志文件")?.path();

    let content =
        std::fs::read_to_string(&latest).map_err(|e| format!("读取日志文件失败: {}", e))?;

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
            e.file_name().to_string_lossy().starts_with("casy.")
                && e.file_name().to_string_lossy().ends_with(".log")
        })
        .collect();

    entries.sort_by(|a, b| {
        b.metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            .cmp(
                &a.metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
            )
    });

    for entry in &entries {
        if results.len() >= max_results {
            break;
        }
        if let Ok(content) = std::fs::read_to_string(entry.path()) {
            for line in content.lines() {
                if line.to_lowercase().contains(&keyword.to_lowercase()) {
                    results.push(line.to_string());
                    if results.len() >= max_results {
                        break;
                    }
                }
            }
        }
    }

    Ok(results)
}
