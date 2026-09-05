use casy_lib::{
    ai::retrieval,
    background_jobs,
    commands::{document_intelligence as docs, knowledge},
    db,
    document_pipeline::sha256_file,
};
use rusqlite::params;
use std::{path::Path, time::Instant};

#[tokio::test]
async fn native_text_queue_preserves_source_backup_latest_index_and_knowledge_import() {
    db::enable_test_mode();
    let mut conn = db::open_db().unwrap();
    db::init_db(&conn).unwrap();
    conn.execute(
        "INSERT INTO cases(id,case_name,client_name) VALUES('text-case','文本队列验证','测试客户')",
        [],
    )
    .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let md = format!(
        "# 案件研究\n\n{}\n\n# 特别约定\n第三人乙公司的赔偿金额为125000.25元。",
        "普通程序的材料已整理归档。\n".repeat(20000)
    );
    let md_path = temp.path().join("long.md");
    std::fs::write(&md_path, &md).unwrap();
    let docx_path = temp.path().join("evidence.docx");
    docx_rs::Docx::new()
        .add_paragraph(
            docx_rs::Paragraph::new()
                .add_run(docx_rs::Run::new().add_text("第三人为乙公司，标的额125000.25元。")),
        )
        .build()
        .pack(std::fs::File::create(&docx_path).unwrap())
        .unwrap();
    let invalid_path = temp.path().join("broken.docx");
    std::fs::write(&invalid_path, b"not a docx").unwrap();
    let start = Instant::now();
    let mut original_note = String::new();
    for (id, path) in [
        ("md", &md_path),
        ("docx", &docx_path),
        ("broken", &invalid_path),
    ] {
        conn.execute("INSERT INTO case_files(id,case_id,file_name,file_path,category) VALUES(?1,'text-case',?2,?3,'evidence')",params![id,path.file_name().unwrap().to_str().unwrap(),path.to_str().unwrap()]).unwrap();
        let hash = sha256_file(path).unwrap();
        let job = docs::queue_document_processing(id.into()).await.unwrap();
        assert!(background_jobs::process_next_document_job().await.unwrap());
        let done = docs::list_document_jobs(id.into()).await.unwrap().remove(0);
        if id == "broken" {
            assert_eq!(done.status, "failed");
            assert_eq!(
                conn.query_row(
                    "SELECT count(*) FROM document_pages WHERE file_id='broken'",
                    [],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
                0
            );
            continue;
        }
        assert_eq!(done.status, "completed", "{:?}", done.error_message);
        assert!(done.searchable_pdf_path.is_none());
        assert!(Path::new(done.markdown_path.as_ref().unwrap()).is_file());
        assert_eq!(sha256_file(path).unwrap(), hash);
        assert_eq!(
            docs::queue_document_processing(id.into()).await.unwrap().id,
            job.id
        );
        let backup = std::fs::read_to_string(done.markdown_path.as_ref().unwrap()).unwrap();
        if id == "md" {
            assert_eq!(backup, md);
            assert!(done.total_pages > 50);
        }
        assert!(backup.contains("125000.25"));
        let hits = retrieval::search(&conn, "第三人的赔偿金额是多少", &[id.into()], 10).unwrap();
        assert!(!hits.is_empty());
        assert!(hits[0].content.contains("125000.25"));
        assert_eq!(hits[0].location_kind, "segment");
        let imported = knowledge::import_pageindex_inner(&mut conn, id).unwrap();
        let note: String = conn
            .query_row(
                "SELECT content FROM knowledge_items WHERE id=?1",
                [&imported.knowledge_id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(note.contains("125000.25"));
        if id == "md" {
            original_note = imported.knowledge_id;
            conn.execute("UPDATE knowledge_items SET content=content || '\n人工批注：保留旧版金额' WHERE id=?1", [&original_note]).unwrap();
        }
    }
    std::fs::write(&md_path, "# 修订稿\n第三人的赔偿金额改为250000元。").unwrap();
    let new = docs::queue_document_processing("md".into()).await.unwrap();
    assert!(background_jobs::process_next_document_job().await.unwrap());
    let hits = retrieval::search(&conn, "第三人", &["md".into()], 10).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].job_id, new.id);
    assert!(hits[0].content.contains("250000"));
    let sources = knowledge::list_knowledge_document_sources().await.unwrap();
    assert!(sources
        .iter()
        .find(|s| s.file_id == "md")
        .unwrap()
        .imported_knowledge_id
        .is_none());
    let revision = knowledge::import_pageindex_inner(&mut conn, "md").unwrap();
    assert!(!revision.reused);
    assert_ne!(revision.knowledge_id, original_note);
    let old: String = conn
        .query_row(
            "SELECT content FROM knowledge_items WHERE id=?1",
            [&original_note],
            |r| r.get(0),
        )
        .unwrap();
    assert!(old.contains("125000.25") && old.contains("人工批注"));
    let revised: String = conn
        .query_row(
            "SELECT content FROM knowledge_items WHERE id=?1",
            [&revision.knowledge_id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(revised.contains("250000") && !revised.contains("125000.25"));
    let reused = knowledge::import_pageindex_inner(&mut conn, "md").unwrap();
    assert!(reused.reused);
    assert_eq!(reused.knowledge_id, revision.knowledge_id);
    assert_eq!(reused.child_count, revision.child_count);
    let sources = knowledge::list_knowledge_document_sources().await.unwrap();
    assert_eq!(
        sources
            .iter()
            .find(|s| s.file_id == "md")
            .unwrap()
            .imported_knowledge_id
            .as_deref(),
        Some(revision.knowledge_id.as_str())
    );
    assert_eq!(
        conn.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
            .get::<_, i64>(
            0
        ))
        .unwrap(),
        0
    );
    println!(
        "Native text queue: {:.2}s; markdown={} bytes",
        start.elapsed().as_secs_f64(),
        md.len()
    );
}
