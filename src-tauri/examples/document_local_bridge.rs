//! Isolated document QA bridge. Never connects to Feishu or starts recurring workers.
use anyhow::{bail, Result};
use casy_lib::{
    commands::{cases, document_intelligence as docs, files, knowledge, search, smart_rules},
    db,
};
use serde_json::{json, Value};
use std::io::Read;

#[tokio::main]
async fn main() -> Result<()> {
    if !cfg!(debug_assertions) || std::env::var_os("CASY_TEST_DATA_DIR").is_none() {
        bail!("Requires a debug build and CASY_TEST_DATA_DIR");
    }
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let request: Value = serde_json::from_str(&input)?;
    db::init_db(&db::open_db()?)?;
    let args = &request["args"];
    let file = args["fileId"].as_str().unwrap_or("").to_owned();
    let job = args["jobId"].as_str().unwrap_or("").to_owned();
    let result: Result<Value, String> = match request["command"].as_str().unwrap_or("") {
        "qa_seed" => {
            let root = db::get_db_path().parent().unwrap().join("source");
            std::fs::create_dir_all(&root)?;
            let source = std::path::Path::new(args["source"].as_str().unwrap_or(""));
            let destination = root.join(
                source
                    .file_name()
                    .ok_or_else(|| anyhow::anyhow!("Expected source file"))?,
            );
            std::fs::copy(source, &destination)?;
            let conn = db::open_db()?;
            let file_id = args["fileId"].as_str().unwrap_or("ocr-file");
            conn.execute("INSERT OR IGNORE INTO cases(id,case_name,client_name) VALUES('ocr-case','中文卷宗识别验收','本地测试')", [])?;
            conn.execute("INSERT INTO case_files(id,case_id,file_name,file_path,file_type,category) VALUES(?4,'ocr-case',?1,?2,?3,'evidence')",
                rusqlite::params![source.file_name().unwrap().to_string_lossy(),destination.display().to_string(),source.extension().unwrap().to_string_lossy(),file_id])?;
            conn.execute("INSERT OR IGNORE INTO smart_rules(id,name,match_field,match_pattern,action_type,action_payload,enabled) VALUES('ocr-rule','通知书归类','ocr_text','开庭通知书','set_category','summons',1)", [])?;
            Ok(json!({"fileId":file_id,"caseId":"ocr-case"}))
        }
        "qa_process_next" => casy_lib::background_jobs::process_next_document_job()
            .await
            .map(|v| json!(v))
            .map_err(|e| e.to_string()),
        "qa_audit" => {
            let conn = db::open_db()?;
            let state: (String, String, String) = conn.query_row(
                "SELECT ocr_status,index_status,category FROM case_files WHERE id='ocr-file'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;
            let pages: i64 =
                conn.query_row("SELECT count(*) FROM document_pages", [], |r| r.get(0))?;
            let nodes: i64 =
                conn.query_row("SELECT count(*) FROM page_index_nodes", [], |r| r.get(0))?;
            let integrity: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
            let fk: i64 =
                conn.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
                    r.get(0)
                })?;
            Ok(
                json!({"ocrStatus":state.0,"indexStatus":state.1,"category":state.2,"pages":pages,"nodes":nodes,"integrity":integrity,"foreignKeyErrors":fk}),
            )
        }
        "get_case" => cases::get_case(args["id"].as_str().unwrap_or("").into())
            .await
            .map(|v| json!(v)),
        "list_case_files" => {
            files::list_case_files(args["caseId"].as_str().unwrap_or("").into(), None)
                .await
                .map(|v| json!(v))
        }
        "get_document_engine_status" => docs::get_document_engine_status().await.map(|v| json!(v)),
        "search_document_passages" => search::search_document_passages(
            args["query"].as_str().unwrap_or("").into(),
            serde_json::from_value(args["scope"].clone())?,
        )
        .await
        .map(|v| json!(v)),
        "list_knowledge_document_sources" => knowledge::list_knowledge_document_sources()
            .await
            .map(|v| json!(v)),
        "import_pageindex_to_knowledge" => knowledge::import_pageindex_to_knowledge(file)
            .await
            .map(|v| json!(v)),
        "list_knowledge" => knowledge::list_knowledge(None).await.map(|v| json!(v)),
        "get_knowledge_with_blocks" => {
            knowledge::get_knowledge_with_blocks(args["id"].as_str().unwrap_or("").into())
                .await
                .map(|v| json!(v))
        }
        "queue_document_processing" => docs::queue_document_processing(file)
            .await
            .map(|v| json!(v)),
        "list_document_jobs" => docs::list_document_jobs(file).await.map(|v| json!(v)),
        "retry_document_job" => docs::retry_document_job(job).await.map(|v| json!(v)),
        "cancel_document_job" => docs::cancel_document_job(job).await.map(|v| json!(v)),
        "get_file_ocr_text" => smart_rules::get_file_ocr_text(file).await.map(|v| json!(v)),
        "list_case_ocr_states" => {
            smart_rules::list_case_ocr_states(args["caseId"].as_str().unwrap_or("").into())
                .await
                .map(|v| json!(v))
        }
        _ => Err("Command outside document QA allowlist".into()),
    };
    println!(
        "{}",
        match result {
            Ok(data) => json!({"ok":true,"data":data}),
            Err(error) => json!({"ok":false,"error":error}),
        }
    );
    Ok(())
}
