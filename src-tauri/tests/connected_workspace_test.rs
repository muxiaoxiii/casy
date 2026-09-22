use casy_lib::{commands::{calendar, calendar_events, drafts, editor_tasks, tasks}, db};

/// Uses the real command implementations and an isolated SQLite database, no browser mocks.
#[tokio::test]
async fn note_task_schedule_and_draft_share_identity_without_moving_deadlines() {
    let profile = tempfile::tempdir().unwrap();
    std::env::set_var("CASY_TEST_DATA_DIR", profile.path());
    db::enable_test_mode();
    let conn = db::open_db().unwrap();
    db::init_db(&conn).unwrap();
    conn.execute_batch("INSERT INTO cases(id,case_name,client_name) VALUES('connected-case','工作流验证','测试客户');
        INSERT INTO knowledge_items(id,title,content,category,linked_case_id) VALUES('connected-note','事实笔记','核对证据','reference','connected-case');").unwrap();
    let task_id = editor_tasks::bind_editor_task(
        "knowledge".into(), "connected-note".into(), "block-1".into(), "核对证据".into(), None, Some("connected-case".into()),
    ).await.unwrap();
    tasks::update_task(serde_json::json!({"id": task_id, "dueDate": "2026-09-30"})).await.unwrap();

    let event = calendar_events::create_calendar_event(serde_json::json!({
        "title": "核对证据", "eventDate": "2026-09-14", "startTime": "09:00", "endTime": "10:30",
        "allDay": false, "caseId": "connected-case", "taskId": task_id,
    })).await.unwrap();
    assert_eq!(event.task_id.as_deref(), Some(task_id.as_str()));
    calendar_events::move_calendar_event(event.id.clone(), "2026-09-15".into(), Some("13:00".into())).await.unwrap();
    let rows = calendar_events::list_calendar_events("2026-09-15".into(), "2026-09-15".into()).await.unwrap();
    assert_eq!(rows[0].start_time.as_deref(), Some("13:00"));
    assert_eq!(rows[0].end_time.as_deref(), Some("14:30"));
    assert_eq!(rows[0].task_id.as_deref(), Some(task_id.as_str()));
    let due: String = conn.query_row("SELECT due_date FROM tasks WHERE id=?1", [&task_id], |r| r.get(0)).unwrap();
    assert_eq!(due, "2026-09-30");
    assert!(calendar::get_calendar_events(2026, 9, None).await.unwrap().iter().any(|e| e.id == event.id && e.event_type == "event"));

    let draft = drafts::create_draft("代理意见".into(), Some("<p>原文</p>".into()), Some("connected-case".into()), None).await.unwrap();
    editor_tasks::bind_editor_task("doc".into(), draft.id.clone(), "draft-block".into(), "核对证据".into(), Some(task_id.clone()), Some("connected-case".into())).await.unwrap();
    editor_tasks::set_editor_task_completed("doc".into(), draft.id.clone(), task_id, true, false).await.unwrap();
    assert!(editor_tasks::get_editor_tasks("knowledge".into(), "connected-note".into()).await.unwrap()[0].completed);
    let updated = drafts::update_draft(draft.id.clone(), None, Some("<p>已核对</p>".into()), None, None, Some(draft.version), None).await.unwrap();
    assert!(drafts::update_draft(draft.id.clone(), None, Some("旧窗口内容".into()), None, None, Some(draft.version), None).await.is_err());
    assert_eq!(drafts::get_draft(draft.id).await.unwrap().version, updated.version);
}
