use casy_lib::{
    background_jobs,
    commands::{files, knowledge},
    db,
    workspace_sync::{self, Reconciler},
};
use serde_json::{json, Value};

fn flags(conn: &rusqlite::Connection, value: Value) {
    conn.execute("INSERT INTO settings(key,value) VALUES('workspace_sync',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[value.to_string()]).unwrap();
}

#[tokio::test]
async fn local_folder_lifecycle_preserves_originals_ids_notes_and_opt_in() {
    let profile = tempfile::tempdir().unwrap();
    std::env::set_var("CASY_TEST_DATA_DIR", profile.path());
    db::enable_test_mode();
    let conn = db::open_db().unwrap();
    db::init_db(&conn).unwrap();
    conn.execute("INSERT INTO cases(id,case_name,client_name,case_no) VALUES('sync-case','同步验收','本地客户','2026-test')",[]).unwrap();
    let dirs = files::list_case_dirs("sync-case".into()).await.unwrap();
    let root = std::path::PathBuf::from(&dirs[0].absolute_path);
    let original = root.join("scan.md");
    let content = "# Patent Evidence\n\nPrüfung français 日本語，跨页证据与赔偿责任。";
    std::fs::write(&original, content).unwrap();
    std::fs::write(root.join("ignore.tmp"), "temporary").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&original, root.join("link.md")).unwrap();
    let mut sync = Reconciler::default();
    sync.tick().unwrap();
    sync.tick().unwrap();
    assert_eq!(
        conn.query_row("SELECT count(*) FROM case_files", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    flags(&conn, json!({"register":true}));
    sync.tick().unwrap();
    sync.tick().unwrap();
    let id: String = conn
        .query_row("SELECT id FROM case_files", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        conn.query_row("SELECT count(*) FROM document_processing_jobs", [], |r| r
            .get::<_, i64>(
            0
        ))
        .unwrap(),
        0
    );
    flags(&conn, json!({"register":true,"ocr":true,"knowledge":true}));
    sync.tick().unwrap();
    assert!(background_jobs::process_next_document_job().await.unwrap());
    sync.tick().unwrap();
    let snapshot: String = conn
        .query_row(
            "SELECT id FROM knowledge_items WHERE source_id=?1 AND parent_id IS NULL",
            [&id],
            |r| r.get(0),
        )
        .unwrap();
    knowledge::update_knowledge(
        snapshot.clone(),
        json!({"content":"人工研究意见，不能被自动覆盖"}),
    )
    .await
    .unwrap();
    let preview = workspace_sync::get_workspace_document(id.clone())
        .await
        .unwrap();
    assert!(preview["markdown"]
        .as_str()
        .unwrap()
        .contains("Patent Evidence"));
    conn.execute("INSERT INTO document_processing_jobs(id,file_id,source_sha256,status) SELECT 'failed-retry',file_id,source_sha256,'failed' FROM document_processing_jobs WHERE file_id=?1 AND status='completed' LIMIT 1", [&id]).unwrap();
    assert_eq!(workspace_sync::get_workspace_document(id.clone()).await.unwrap()["markdown"], preview["markdown"]);
    let sources = workspace_sync::list_workspace_sources(vec!["sync-case".into()]).await.unwrap();
    assert_eq!(sources.iter().find(|s|s["fileId"]==id).unwrap()["status"], "completed");
    conn.execute("DELETE FROM document_processing_jobs WHERE id='failed-retry'", []).unwrap();
    let second = root.join("renamed.md");
    std::fs::rename(&original, &second).unwrap();
    sync.tick().unwrap();
    sync.tick().unwrap();
    assert_eq!(
        conn.query_row("SELECT count(*) FROM case_files", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row("SELECT file_name FROM case_files WHERE id=?1", [&id], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "renamed.md"
    );
    let changed = "# Updated Patent Evidence\n\nNeue Beweise, preuve, 新しい証拠。";
    std::fs::write(&second, changed).unwrap();
    assert!(workspace_sync::get_workspace_document(id.clone())
        .await
        .unwrap_err()
        .contains("原文件已变化"));
    sync.tick().unwrap();
    sync.tick().unwrap();
    assert!(background_jobs::process_next_document_job().await.unwrap());
    sync.tick().unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT content FROM knowledge_items WHERE id=?1",
            [&snapshot],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "人工研究意见，不能被自动覆盖"
    );
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM knowledge_items WHERE source_id=?1 AND parent_id IS NULL",
            [&id],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        2
    );
    let note_content = format!("[原件](<{}>)", second.display());
    conn.execute("INSERT INTO knowledge_items(id,title,content,category) VALUES('file-link','来源链接',?1,'reference')",[note_content]).unwrap();
    flags(
        &conn,
        json!({"register":true,"ocr":true,"knowledge":true,"name":true,"content_name":true}),
    );
    sync.tick().unwrap();
    let renamed: String = conn
        .query_row("SELECT file_path FROM case_files WHERE id=?1", [&id], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(renamed.ends_with("Updated Patent Evidence.md"));
    assert_eq!(std::fs::read_to_string(&renamed).unwrap(), changed);
    let link: String = conn
        .query_row(
            "SELECT content FROM knowledge_items WHERE id='file-link'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let destination = pulldown_cmark::Parser::new(&link).find_map(|event| {
        if let pulldown_cmark::Event::Start(pulldown_cmark::Tag::Link { dest_url, .. }) = event {
            Some(dest_url.to_string())
        } else { None }
    }).expect("renamed note must retain a valid Markdown link");
    let linked_path = reqwest::Url::parse(&destination).unwrap().to_file_path().unwrap();
    assert_eq!(linked_path, std::path::PathBuf::from(&renamed));
    assert_eq!(std::fs::read_to_string(linked_path).unwrap(), changed);
    assert!(!link.contains("renamed.md"));
    std::fs::write(&renamed, "# Another Revision\n\nUpdated evidence.").unwrap();
    sync.tick().unwrap();
    sync.tick().unwrap();
    let cancelled: String = conn
        .query_row(
            "SELECT id FROM document_processing_jobs ORDER BY rowid DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    casy_lib::commands::document_intelligence::cancel_document_job(cancelled.clone())
        .await
        .unwrap();
    sync.tick().unwrap();
    sync.tick().unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT id FROM document_processing_jobs ORDER BY rowid DESC LIMIT 1",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        cancelled
    );
    // Unregistering never causes automatic resurrection.
    files::delete_case_file(id.clone()).await.unwrap();
    sync.tick().unwrap();
    sync.tick().unwrap();
    assert!(
        workspace_sync::list_workspace_sources(vec!["sync-case".into()])
            .await
            .unwrap()
            .is_empty()
    );
    assert!(std::path::Path::new(&renamed).exists());
    assert_eq!(
        conn.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
            .get::<_, i64>(
            0
        ))
        .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
}
