use casy_lib::db::schema::{run_migrations, CURRENT_SCHEMA_VERSION};
use rusqlite::Connection;

fn database() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    run_migrations(&conn, 0).unwrap();
    conn
}

#[test]
fn v21_has_durable_document_jobs_and_page_ir() {
    let conn = database();
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, CURRENT_SCHEMA_VERSION);
    // 版本号随迁移演进，与 CURRENT_SCHEMA_VERSION 保持一致即可（上文已断言）
    for table in [
        "document_processing_jobs",
        "document_pages",
        "document_pages_fts",
    ] {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                [table],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "missing {table}");
    }
    let columns: Vec<String> = conn
        .prepare("PRAGMA table_info(case_files)")
        .unwrap()
        .query_map([], |row| row.get(1))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    for column in [
        "source_sha256",
        "searchable_pdf_path",
        "document_ir_path",
        "ocr_markdown_path",
        "ocr_engine",
        "ocr_error",
    ] {
        assert!(
            columns.iter().any(|existing| existing == column),
            "missing {column}"
        );
    }
}

#[test]
fn document_pages_fts_tracks_page_text() {
    let conn = database();
    conn.execute(
        "INSERT INTO cases(id,case_name,client_name) VALUES('c1','测试案','客户')",
        [],
    )
    .unwrap();
    conn.execute("INSERT INTO case_files(id,case_id,file_name,file_path,category) VALUES('f1','c1','scan.pdf','/tmp/scan.pdf','evidence')", []).unwrap();
    conn.execute(
        "INSERT INTO document_processing_jobs(id,file_id,source_sha256,status)
         VALUES('j1','f1','abc','completed')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO document_pages(job_id,file_id,page_number,plain_text,markdown)
         VALUES('j1','f1',1,'行政裁决与民事侵权并行事实','')",
        [],
    )
    .unwrap();

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM document_pages_fts WHERE document_pages_fts MATCH '\"行政裁决\"'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn document_status_constraints_reject_false_completion_state() {
    let conn = database();
    conn.execute(
        "INSERT INTO cases(id,case_name,client_name) VALUES('c1','测试案','客户')",
        [],
    )
    .unwrap();
    conn.execute("INSERT INTO case_files(id,case_id,file_name,file_path,category) VALUES('f1','c1','scan.pdf','/tmp/scan.pdf','evidence')", []).unwrap();
    let invalid = conn.execute("INSERT INTO document_processing_jobs(id,file_id,source_sha256,status) VALUES('j1','f1','abc','processing')", []);
    assert!(invalid.is_err());
    conn.execute("INSERT INTO document_processing_jobs(id,file_id,source_sha256,status) VALUES('j1','f1','abc','queued')", []).unwrap();
    let invalid_page = conn.execute(
        "INSERT INTO document_pages(job_id,file_id,page_number) VALUES('j1','f1',0)",
        [],
    );
    assert!(invalid_page.is_err());
}
