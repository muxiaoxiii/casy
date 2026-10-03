use casy_lib::{
    commands::{calendar_events, editor_recovery, knowledge, portable_backup},
    db,
};
use serde_json::json;


/// 恢复后的附件链接可能以 `file://` URL 形式写入笔记（含空格/反斜杠的路径会被转义）。
/// 统一分隔符并解码百分号转义后再比对，既跨平台又不会因编码差异误判。
fn normalize_path_text(value: &str) -> String {
    let mut decoded = Vec::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(byte) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                decoded.push(byte);
                i += 3;
                continue;
            }
        }
        decoded.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&decoded).replace('\\', "/")
}

#[tokio::test]
async fn full_backup_calendar_and_editor_recovery_survive_reopening() {
    let profile = tempfile::tempdir().unwrap();
    std::env::set_var("CASY_TEST_DATA_DIR", profile.path());
    let conn = db::open_db().unwrap();
    db::init_db(&conn).unwrap();
    let id = db::new_id();
    let root = db::get_db_path()
        .parent()
        .unwrap()
        .join("documents")
        .join(&id);
    std::fs::create_dir_all(&root).unwrap();
    let evidence = root.join("evidence.md");
    std::fs::write(&evidence, "第三人提交的原始证据\nSchadensersatz").unwrap();
    conn.execute("INSERT INTO cases(id,case_name,client_name,folder_path) VALUES(?1,'Delivery lifecycle','Client',?2)",rusqlite::params![id,root.to_str().unwrap()]).unwrap();
    let file_id = db::new_id();
    conn.execute("INSERT INTO case_files(id,case_id,file_name,file_path,category) VALUES(?1,?2,'evidence.md',?3,'evidence')",rusqlite::params![file_id,id,evidence.to_str().unwrap()]).unwrap();
    drop(conn);

    let event = calendar_events::create_calendar_event(json!({"title":"Meeting","eventDate":"2026-09-30","startTime":"09:30","endTime":"11:45","location":"Court 3","notes":"Retain","color":"#123456"})).await.unwrap();
    calendar_events::update_calendar_event(
        event.id.clone(),
        json!({"title":"Renamed","eventDate":"2026-10-01"}),
    )
    .await
    .unwrap();
    let events = calendar_events::list_calendar_events("2026-10-01".into(), "2026-10-01".into())
        .await
        .unwrap();
    let edited = events.iter().find(|e| e.id == event.id).unwrap();
    assert_eq!(edited.end_time.as_deref(), Some("11:45"));
    assert_eq!(edited.location.as_deref(), Some("Court 3"));
    assert_eq!(edited.color.as_deref(), Some("#123456"));
    calendar_events::move_calendar_event(
        event.id.clone(),
        "2026-10-02".into(),
        Some("13:00".into()),
    )
    .await
    .unwrap();
    let moved = calendar_events::list_calendar_events("2026-10-02".into(), "2026-10-02".into())
        .await
        .unwrap();
    assert_eq!(
        moved
            .iter()
            .find(|e| e.id == event.id)
            .unwrap()
            .end_time
            .as_deref(),
        Some("15:15")
    );
    let draft = casy_lib::commands::drafts::create_draft(
        "Draft".into(),
        Some("original".into()),
        None,
        None,
    )
    .await
    .unwrap();
    casy_lib::commands::drafts::update_draft(
        draft.id.clone(),
        None,
        Some("new".into()),
        None,
        None,
        Some(draft.version),
        None,
    )
    .await
    .unwrap();
    assert!(casy_lib::commands::drafts::update_draft(
        draft.id.clone(),
        None,
        Some("stale".into()),
        None,
        None,
        Some(draft.version),
        None
    )
    .await
    .unwrap_err()
    .contains("EDIT_CONFLICT"));

    let conn = db::open_db().unwrap();
    let note = db::new_id();
    conn.execute("INSERT INTO knowledge_items(id,title,content,category,status) VALUES(?1,'Note','original','reference','current')",[&note]).unwrap();
    drop(conn);
    knowledge::update_knowledge(
        note.clone(),
        json!({"content":"first window","expectedContent":"original"}),
    )
    .await
    .unwrap();
    let conflict = knowledge::update_knowledge(
        note.clone(),
        json!({"content":"stale window","expectedContent":"original"}),
    )
    .await
    .unwrap_err();
    assert!(conflict.contains("EDIT_CONFLICT"));
    let session = db::new_id();
    editor_recovery::save_editor_recovery(
        session.clone(),
        Some(json!({"id":note,"title":"Note","content":"unsaved text"})),
    )
    .await
    .unwrap();
    let conn = db::open_db().unwrap();
    conn.execute(
        "UPDATE settings SET value=json_set(value,'$.processSession','previous-run') WHERE key=?1",
        [format!("editor_recovery:{session}")],
    )
    .unwrap();
    drop(conn);
    assert_eq!(editor_recovery::recover_editor_drafts().await.unwrap(), 1);
    assert_eq!(editor_recovery::recover_editor_drafts().await.unwrap(), 0);
    let attachment_note = db::new_id();
    {
        let conn = db::open_db().unwrap();
        conn.execute("INSERT INTO knowledge_items(id,title,content,category,status) VALUES(?1,'Attachment note',?2,'reference','current')",rusqlite::params![attachment_note,format!("[证据原件](<{}>)\n",evidence.display())]).unwrap();
    }

    let outside = tempfile::tempdir().unwrap();
    let archive = outside.path().join("full.casy");
    portable_backup::export_full_backup(
        archive.to_string_lossy().into_owned(),
        "recovery-test-password".into(),
    )
    .await
    .unwrap();
    std::fs::write(&evidence, "newer original retained").unwrap();
    portable_backup::import_full_backup(
        archive.to_string_lossy().into_owned(),
        "recovery-test-password".into(),
    )
    .await
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(&evidence).unwrap(),
        "newer original retained"
    );
    let conn = db::open_db().unwrap();
    let recovered_path: String = conn
        .query_row(
            "SELECT file_path FROM case_files WHERE id=?1",
            [&file_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(&recovered_path).unwrap(),
        "第三人提交的原始证据\nSchadensersatz"
    );
    let restored_note: String = conn
        .query_row(
            "SELECT content FROM knowledge_items WHERE id=?1",
            [&attachment_note],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        normalize_path_text(&restored_note).contains(&normalize_path_text(&recovered_path)),
        "附件链接应指向恢复后的路径\n note: {restored_note}\n target: {recovered_path}"
    );
    let cases_path: String = conn
        .query_row("SELECT folder_path FROM cases WHERE id=?1", [&id], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(std::path::Path::new(&recovered_path).starts_with(cases_path));
    assert_eq!(
        conn.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
    assert!(conn
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .query([])
        .unwrap()
        .next()
        .unwrap()
        .is_none());
    assert!(std::fs::read_dir(casy_lib::commands::backup::backups_dir())
        .unwrap()
        .any(|e| e.unwrap().path().extension().is_some_and(|e| e == "casy")));
}
