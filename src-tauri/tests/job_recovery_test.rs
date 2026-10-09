//! R-06 故障注入：引擎产物已写全但任务被标记中断时，启动恢复必须不重跑 OCR 即完成。
use casy_lib::{background_jobs, db, document_pipeline};

#[test]
fn interrupted_job_with_complete_artifacts_recovers_without_reocr() {
    db::enable_test_mode();
    let profile = tempfile::tempdir().unwrap();
    let source = profile.path().join("scan.pdf");
    std::fs::write(&source, b"%PDF-recovery-source").unwrap();
    let hash = document_pipeline::sha256_file(&source).unwrap();

    let conn = db::open_db().unwrap();
    db::init_db(&conn).unwrap();
    conn.execute_batch(
        "INSERT INTO cases(id,case_name,client_name) VALUES('recovery-case','恢复案件','委托人');",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO case_files(id,case_id,file_name,file_path,category) VALUES('recovery-file','recovery-case','scan.pdf',?1,'evidence')",
        [source.to_string_lossy().to_string()],
    )
    .unwrap();
    // 伪造崩溃现场：任务失败且标记 INTERRUPTED，卷宗文件停留在 failed
    conn.execute(
        "INSERT INTO document_processing_jobs(id,file_id,source_sha256,status,error_code,error_message)
         VALUES('recovery-job','recovery-file',?1,'failed','INTERRUPTED','应用上次退出时任务仍在运行，请重试')",
        [&hash],
    )
    .unwrap();
    conn.execute(
        "UPDATE case_files SET ocr_status='failed' WHERE id='recovery-file'",
        [],
    )
    .unwrap();

    // 引擎在崩溃前已写全的产物：两页页 IR + Markdown + 可搜索 PDF（+ 来源映射）
    let dir = document_pipeline::artifact_dir("recovery-job", &hash).unwrap();
    let pages = vec![
        document_pipeline::DocumentPage {
            page_number: 1,
            width: Some(400.0),
            height: Some(600.0),
            plain_text: "第一页证据".into(),
            markdown: "第一页证据".into(),
            regions: vec![],
            confidence: None,
            layout: None,
            timing: None,
        },
        document_pipeline::DocumentPage {
            page_number: 2,
            width: Some(400.0),
            height: Some(600.0),
            plain_text: "第二页证据".into(),
            markdown: "第二页证据".into(),
            regions: vec![],
            confidence: None,
            layout: None,
            timing: None,
        },
    ];
    std::fs::write(dir.join("source.document.json"), serde_json::to_vec(&pages).unwrap()).unwrap();
    std::fs::write(dir.join("source.md"), "第一页证据\n\n---\n\n第二页证据").unwrap();
    std::fs::write(dir.join("source.searchable.pdf"), b"%PDF-derived").unwrap();
    drop(conn);

    background_jobs::recover_interrupted_jobs();

    let conn = db::open_db().unwrap();
    let (status, engine): (String, String) = conn
        .query_row(
            "SELECT status,engine FROM document_processing_jobs WHERE id='recovery-job'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(status, "completed");
    assert_eq!(engine, "recovered-after-interrupt");
    let page_rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM document_pages WHERE job_id='recovery-job'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(page_rows, 2, "恢复必须把落盘页 IR 写进 document_pages");
    let result_rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM document_job_results WHERE job_id='recovery-job'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(result_rows, 1, "R-06 结果清单必须随恢复落库");
    let ocr_status: String = conn
        .query_row("SELECT ocr_status FROM case_files WHERE id='recovery-file'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(ocr_status, "completed");

    // 第二个场景顺序执行（并行会争用进程级测试库）
    {
    let profile = tempfile::tempdir().unwrap();
    let source = profile.path().join("missing.pdf");
    std::fs::write(&source, b"%PDF-no-artifacts").unwrap();
    let hash = document_pipeline::sha256_file(&source).unwrap();
    let conn = db::open_db().unwrap();
    db::init_db(&conn).unwrap();
    conn.execute_batch(
        "INSERT INTO cases(id,case_name,client_name) VALUES('recovery-case-2','无产物案件','委托人');",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO case_files(id,case_id,file_name,file_path,category) VALUES('recovery-file-2','recovery-case-2','missing.pdf',?1,'evidence')",
        [source.to_string_lossy().to_string()],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO document_processing_jobs(id,file_id,source_sha256,status,error_code)
         VALUES('recovery-job-2','recovery-file-2',?1,'failed','INTERRUPTED')",
        [&hash],
    )
    .unwrap();
    drop(conn);

    background_jobs::recover_interrupted_jobs();

    let conn = db::open_db().unwrap();
    let status: String = conn
        .query_row("SELECT status FROM document_processing_jobs WHERE id='recovery-job-2'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(status, "failed", "没有产物可恢复时必须保持中断态（交由用户重试/续算）");
    }
}
