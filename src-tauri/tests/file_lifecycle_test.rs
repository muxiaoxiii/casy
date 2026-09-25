use casy_lib::{
    ai::retrieval,
    background_jobs,
    commands::{document_intelligence as docs, files, knowledge, search},
    db,
};
use rusqlite::params;
use serde_json::json;
use std::path::{Path, PathBuf};

fn name(id: &str, value: &str) -> files::RenameItem {
    serde_json::from_value(json!({"id":id,"newName":value})).unwrap()
}

async fn by_id(id: &str) -> files::CaseFile {
    files::list_case_files("file-case".into(), None)
        .await
        .unwrap()
        .into_iter()
        .find(|f| f.id == id)
        .unwrap()
}

#[tokio::test]
async fn files_preserve_bytes_identity_and_relations_through_lifecycle() {
    let profile = tempfile::tempdir().unwrap();
    std::env::set_var("CASY_TEST_DATA_DIR", profile.path());
    db::enable_test_mode();
    let mut conn = db::open_db().unwrap();
    db::init_db(&conn).unwrap();
    conn.execute("INSERT INTO cases(id,case_name,case_no,client_name) VALUES('file-case','卷宗验收','旧案号','合成客户')",[]).unwrap();
    conn.execute(
        "INSERT INTO cases(id,case_name,client_name) VALUES('related-case','关联卷宗','合成客户')",
        [],
    )
    .unwrap();
    let inputs = tempfile::tempdir().unwrap();
    let source = inputs.path().join("claim.md");
    let text = "# 合成案件材料\n第三人的赔偿金额为125000.25元。";
    std::fs::write(&source, text).unwrap();
    let bad = inputs.path().join("missing.md");
    assert!(files::import_files_to_case(
        "file-case".into(),
        None,
        vec![source.display().to_string(), bad.display().to_string()],
        None
    )
    .await
    .is_err());
    assert!(files::list_case_files("file-case".into(), None)
        .await
        .unwrap()
        .is_empty());
    let imported = files::import_files_to_case(
        "file-case".into(),
        None,
        vec![source.display().to_string(), source.display().to_string()],
        Some("evidence".into()),
    )
    .await
    .unwrap();
    assert_eq!(imported.len(), 1);
    let id = imported[0].id.clone();
    let first = by_id(&id).await;
    assert_eq!(first.category, "evidence");
    assert!(casy_lib::commands::relations::add_relation(
        "related-case".into(),
        "file-case".into(),
        "same_party".into(),
        None,
        Some(true)
    )
    .await
    .is_err());
    assert!(
        casy_lib::commands::relations::get_relations("file-case".into())
            .await
            .unwrap()
            .is_empty()
    );
    casy_lib::commands::relations::add_relation(
        "related-case".into(),
        "file-case".into(),
        "same_party".into(),
        None,
        None,
    )
    .await
    .unwrap();
    assert_eq!(by_id(&id).await.case_id, "file-case");
    assert_eq!(
        casy_lib::commands::relations::get_relations("file-case".into())
            .await
            .unwrap()[0]
            .case_id,
        "related-case"
    );
    assert_eq!(std::fs::read_to_string(&first.file_path).unwrap(), text);
    assert_eq!(std::fs::read_to_string(&source).unwrap(), text);
    assert_ne!(first.file_path, source.display().to_string());
    let root = PathBuf::from(&first.file_path).parent().unwrap().to_owned();
    assert_eq!(
        files::register_existing_files(
            "file-case".into(),
            vec![first.file_path.clone(), first.file_path.clone()]
        )
        .await
        .unwrap(),
        0
    );
    let repeated = files::add_case_file(
        "file-case".into(),
        "claim.md".into(),
        first.file_path.clone(),
        "evidence".into(),
    )
    .await
    .unwrap();
    assert_eq!(repeated.id, id);
    let copied = files::import_files_to_case(
        "file-case".into(),
        None,
        vec![source.display().to_string()],
        None,
    )
    .await
    .unwrap();
    assert_eq!(copied[0].file_name, "claim-1.md");
    assert_eq!(std::fs::read_to_string(&first.file_path).unwrap(), text);
    let extra = inputs.path().join("extra.md");
    std::fs::write(&extra, "Extra source").unwrap();
    let fail = inputs.path().join("fail.md");
    std::fs::write(&fail, "Do not publish").unwrap();
    conn.execute_batch("CREATE TRIGGER reject_file_import BEFORE INSERT ON case_files WHEN NEW.file_name='fail.md' BEGIN SELECT RAISE(ABORT,'forced failure'); END;").unwrap();
    assert!(files::import_files_to_case(
        "file-case".into(),
        None,
        vec![extra.display().to_string(), fail.display().to_string()],
        None
    )
    .await
    .is_err());
    assert!(!root.join("extra.md").exists() && !root.join("fail.md").exists());
    assert!(files::scan_unregistered_files("file-case".into())
        .await
        .unwrap()
        .is_empty());
    conn.execute_batch("DROP TRIGGER reject_file_import")
        .unwrap();
    let added = files::add_case_file(
        "file-case".into(),
        "extra.md".into(),
        extra.display().to_string(),
        "internal".into(),
    )
    .await
    .unwrap();
    assert_eq!(added.category, "internal");
    assert!(Path::new(&added.file_path).starts_with(&root));
    conn.execute(
        "UPDATE cases SET case_no='变更后的案号' WHERE id='file-case'",
        [],
    )
    .unwrap();
    let dirs = files::list_case_dirs("file-case".into()).await.unwrap();
    assert_eq!(Path::new(&dirs[0].absolute_path), root);
    files::create_case_subdir("file-case".into(), None, "证据目录".into())
        .await
        .unwrap();
    files::create_case_subdir("file-case".into(), Some("证据目录".into()), "庭审".into())
        .await
        .unwrap();
    assert!(files::list_case_dirs("file-case".into())
        .await
        .unwrap()
        .iter()
        .any(|d| d.rel_path == "证据目录/庭审"));
    let job = docs::queue_document_processing(id.clone()).await.unwrap();
    background_jobs::process_next_document_job().await.unwrap();
    assert_eq!(
        docs::list_document_jobs(id.clone()).await.unwrap()[0].status,
        "completed"
    );
    let note = knowledge::import_pageindex_inner(&mut conn, &id).unwrap();
    let nodes: i64 = conn
        .query_row(
            "SELECT count(*) FROM page_index_nodes WHERE file_id=?1",
            [&id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(nodes > 0);
    let moved = files::move_case_files(
        "file-case".into(),
        vec![id.clone()],
        Some("证据目录/庭审".into()),
    )
    .await
    .unwrap();
    assert!(moved[0].warning.is_none());
    let current = by_id(&id).await;
    assert!(!Path::new(&first.file_path).exists());
    assert!(Path::new(&current.file_path).starts_with(root.join("证据目录/庭审")));
    std::fs::write(root.join("证据目录/庭审/renamed.md"), "Existing evidence").unwrap();
    let renamed = files::apply_case_file_renames("file-case".into(), vec![name(&id, "renamed")])
        .await
        .unwrap();
    assert_eq!(renamed[0].new_name, "renamed-1.md");
    assert_eq!(
        std::fs::read_to_string(root.join("证据目录/庭审/renamed.md")).unwrap(),
        "Existing evidence"
    );
    assert!(!Path::new(&current.file_path).exists());
    let current = by_id(&id).await;
    let hits = retrieval::search(&conn, "赔偿金额", std::slice::from_ref(&id), 10).unwrap();
    assert!(hits[0].content.contains("125000.25"));
    assert_eq!(hits[0].source_path, current.file_path);
    assert_eq!(
        docs::queue_document_processing(id.clone())
            .await
            .unwrap()
            .id,
        job.id
    );
    conn.execute_batch("CREATE TRIGGER reject_rename BEFORE UPDATE OF file_name ON case_files WHEN NEW.file_name='reject.md' BEGIN SELECT RAISE(ABORT,'forced rename failure'); END;").unwrap();
    assert!(files::apply_case_file_renames(
        "file-case".into(),
        vec![name(&id, "rollback"), name(&added.id, "reject")]
    )
    .await
    .is_err());
    assert_eq!(by_id(&id).await.file_path, current.file_path);
    assert_eq!(by_id(&added.id).await.file_path, added.file_path);
    assert!(!root.join("证据目录/庭审/rollback.md").exists());
    assert!(!root.join("reject.md").exists());
    conn.execute_batch("DROP TRIGGER reject_rename").unwrap();
    for invalid in ["", "../escape", "bad\\name", "wrong.pdf", ".", "a/b"] {
        assert!(
            files::apply_case_file_renames("file-case".into(), vec![name(&id, invalid)])
                .await
                .is_err(),
            "{invalid}"
        );
    }
    files::set_case_file_category(id.clone(), "submitted".into())
        .await
        .unwrap();
    assert_eq!(by_id(&id).await.category, "submitted");
    files::delete_case_file(id.clone()).await.unwrap();
    assert!(Path::new(&current.file_path).is_file());
    assert!(!files::list_case_files("file-case".into(), None)
        .await
        .unwrap()
        .iter()
        .any(|f| f.id == id));
    assert_eq!(
        files::list_removed_case_files("file-case".into())
            .await
            .unwrap()[0]
            .id,
        id
    );
    assert!(retrieval::search(&conn, "赔偿金额", std::slice::from_ref(&id), 10)
        .unwrap()
        .is_empty());
    assert!(!search::global_search("renamed".into())
        .await
        .unwrap()
        .iter()
        .any(|r| r.id == id));
    assert!(knowledge::list_knowledge_document_sources()
        .await
        .unwrap()
        .is_empty());
    assert!(docs::queue_document_processing(id.clone()).await.is_err());
    assert!(
        files::register_existing_files("file-case".into(), vec![current.file_path.clone()])
            .await
            .is_err()
    );
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM page_index_nodes WHERE file_id=?1",
            [&id],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        nodes
    );
    files::restore_case_file(id.clone()).await.unwrap();
    assert_eq!(by_id(&id).await.id, id);
    assert_eq!(
        knowledge::list_knowledge_document_sources().await.unwrap()[0]
            .imported_knowledge_id
            .as_deref(),
        Some(note.knowledge_id.as_str())
    );
    // The queued version differs from the last completed source; removal must cancel it.
    std::fs::write(&current.file_path, "# 新版本\n赔偿金额改为500元。").unwrap();
    let newer = docs::queue_document_processing(id.clone()).await.unwrap();
    conn.execute(
        "UPDATE document_processing_jobs SET status='running' WHERE id=?1",
        [&newer.id],
    )
    .unwrap();
    assert!(
        files::move_case_files("file-case".into(), vec![id.clone()], None)
            .await
            .is_err()
    );
    files::delete_case_file(id.clone()).await.unwrap();
    assert_eq!(
        docs::list_document_jobs(id.clone()).await.unwrap()[0].status,
        "cancelled"
    );
    assert!(docs::retry_document_job(newer.id).await.is_err());
    files::restore_case_file(id.clone()).await.unwrap();
    assert!(retrieval::search(&conn, "赔偿金额", std::slice::from_ref(&id), 10)
        .unwrap()
        .is_empty());
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM document_pages WHERE file_id=?1",
            [&id],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert!(knowledge::list_knowledge_document_sources()
        .await
        .unwrap()
        .is_empty());
    let historical: String = conn
        .query_row(
            "SELECT content FROM knowledge_items WHERE id=?1",
            [&note.knowledge_id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(historical.contains("125000.25"));
    docs::queue_document_processing(id.clone()).await.unwrap();
    background_jobs::process_next_document_job().await.unwrap();
    assert!(
        retrieval::search(&conn, "赔偿金额", std::slice::from_ref(&id), 10).unwrap()[0]
            .content
            .contains("500")
    );
    files::delete_case_file(id.clone()).await.unwrap();
    std::fs::remove_file(&current.file_path).unwrap();
    assert!(files::restore_case_file(id.clone()).await.is_err());
    assert_eq!(
        files::list_removed_case_files("file-case".into())
            .await
            .unwrap()
            .len(),
        1
    );
    let base = root.parent().unwrap();
    let escaped = base.parent().unwrap().join("unexpected-folder");
    conn.execute("UPDATE cases SET folder_path=?1 WHERE id='related-case'", [base.join("../unexpected-folder").display().to_string()]).unwrap();
    assert!(files::list_case_dirs("related-case".into()).await.is_err());
    assert!(!escaped.exists());
    #[cfg(unix)]
    {
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), base.join("redirect")).unwrap();
        conn.execute("UPDATE cases SET folder_path=?1 WHERE id='related-case'", [base.join("redirect/new-folder").display().to_string()]).unwrap();
        assert!(files::list_case_dirs("related-case".into()).await.is_err());
        assert!(!outside.path().join("new-folder").exists());
    }
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
    assert!(
        conn.query_row(
            "SELECT count(*) FROM links WHERE source_id=?1 AND target_id=?2",
            params![note.knowledge_id, id],
            |r| r.get::<_, i64>(0)
        )
        .unwrap()
            > 0
    );
}
