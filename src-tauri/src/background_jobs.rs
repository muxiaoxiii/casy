//! Durable single-flight document processing worker.
use log::{error, info};
use rusqlite::OptionalExtension;
use std::time::Duration;
use tauri::AppHandle;

#[derive(Debug)]
pub(crate) struct ClaimedJob {
    pub(crate) id: String,
    pub(crate) file_id: String,
    pub(crate) source_path: String,
    pub(crate) source_sha256: String,
}

fn claim_next_job() -> anyhow::Result<Option<ClaimedJob>> {
    let mut conn = crate::db::open_db()?;
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let candidate:Option<(String,String,String,String)>=tx.query_row(
        "SELECT j.id,j.file_id,f.file_path,j.source_sha256 FROM document_processing_jobs j JOIN case_files f ON f.id=j.file_id WHERE j.status='queued' AND f.deleted_at IS NULL ORDER BY j.created_at LIMIT 1",
        [], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?)),
    ).optional()?;
    let Some((id, file_id, source_path, source_sha256)) = candidate else {
        tx.commit()?;
        return Ok(None);
    };
    let changed=tx.execute("UPDATE document_processing_jobs SET status='running',progress=0.01,started_at=datetime('now','localtime'),updated_at=datetime('now','localtime') WHERE id=?1 AND status='queued'",[&id])?;
    if changed != 1 {
        tx.commit()?;
        return Ok(None);
    }
    tx.execute(
        "UPDATE case_files SET ocr_status='processing',ocr_error=NULL WHERE id=?1",
        [&file_id],
    )?;
    tx.commit()?;
    Ok(Some(ClaimedJob {
        id,
        file_id,
        source_path,
        source_sha256,
    }))
}

pub(crate) fn persist_success(
    job: &ClaimedJob,
    result: &crate::document_pipeline::ProcessResult,
) -> anyhow::Result<()> {
    let mut conn = crate::db::open_db()?;
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let running: bool = tx.query_row(
        "SELECT status='running' FROM document_processing_jobs WHERE id=?1",
        [&job.id],
        |row| row.get(0),
    )?;
    if !running {
        anyhow::bail!("CANCELLED: 任务已取消");
    }
    tx.execute("DELETE FROM document_pages WHERE job_id=?1", [&job.id])?;
    for page in &result.pages {
        tx.execute("INSERT INTO document_pages(job_id,file_id,page_number,width,height,plain_text,markdown,regions_json,confidence) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",rusqlite::params![job.id,job.file_id,page.page_number,page.width,page.height,page.plain_text,page.markdown,serde_json::to_string(&page.regions)?,page.confidence])?;
    }
    tx.execute("UPDATE document_processing_jobs SET status='completed',engine=?1,model_version=?2,current_page=?3,total_pages=?3,progress=1,searchable_pdf_path=?4,page_ir_path=?5,markdown_path=?6,error_code=NULL,error_message=NULL,completed_at=datetime('now','localtime'),updated_at=datetime('now','localtime') WHERE id=?7",rusqlite::params![result.engine,result.model_version,result.pages.len() as i64,result.searchable_pdf_path,result.page_ir_path,result.markdown_path,job.id])?;
    tx.execute("UPDATE case_files SET source_sha256=?1,searchable_pdf_path=?2,document_ir_path=?3,ocr_markdown_path=?4,ocr_engine=?5,ocr_error=NULL,ocr_status='completed',index_status='processing' WHERE id=?6",rusqlite::params![job.source_sha256,result.searchable_pdf_path,result.page_ir_path,result.markdown_path,result.engine,job.file_id])?;
    let text = result
        .pages
        .iter()
        .map(|page| page.plain_text.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    tx.execute(
        "UPDATE case_files SET ocr_text=?1,updated_at=datetime('now','localtime') WHERE id=?2",
        rusqlite::params![text, job.file_id],
    )?;
    tx.commit()?;
    Ok(())
}

pub(crate) fn persist_failure(job: &ClaimedJob, error: &anyhow::Error) {
    if let Ok(mut conn) = crate::db::open_db() {
        let message = error.to_string();
        let code = message.split(':').next().unwrap_or("DOC_PROCESSING_FAILED");
        if let Ok(tx) = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate) {
            if tx.execute("UPDATE document_processing_jobs SET status='failed',error_code=?1,error_message=?2,updated_at=datetime('now','localtime') WHERE id=?3 AND status='running'",rusqlite::params![code,message,job.id]).ok() == Some(1) {
                let _=tx.execute("UPDATE case_files SET ocr_status='failed',index_status='failed',ocr_error=?1 WHERE id=?2",rusqlite::params![message,job.file_id]);
            }
            let _ = tx.commit();
        }
    }
}

async fn process_one(job: ClaimedJob) {
    let current_hash =
        match crate::document_pipeline::sha256_file(std::path::Path::new(&job.source_path)) {
            Ok(value) => value,
            Err(error) => {
                persist_failure(&job, &error);
                return;
            }
        };
    if current_hash != job.source_sha256 {
        persist_failure(&job, &anyhow::anyhow!("SOURCE_CHANGED: 排队后源文件已变化"));
        return;
    }
    let output_dir = match crate::document_pipeline::artifact_dir(&job.id, &job.source_sha256) {
        Ok(path) => path,
        Err(error) => {
            persist_failure(&job, &error);
            return;
        }
    };
    let request = crate::document_pipeline::process_request(
        &job.id,
        &job.source_path,
        &job.source_sha256,
        &output_dir,
    );
    match crate::document_pipeline::run_engine(request).await {
        Ok(result) => {
            if let Err(error) = persist_success(&job, &result) {
                persist_failure(&job, &error);
                return;
            }
            if let Err(error) = crate::commands::smart_rules::apply_rules_inner(&job.file_id) {
                log::warn!("OCR smart rules failed for {}: {}", job.file_id, error);
            }
            match crate::ai::page_index::build_page_index_tree(&job.file_id, &job.source_path).await
            {
                Ok(()) => {
                    if let Ok(conn) = crate::db::open_db() {
                        let _ = conn.execute(
                            "UPDATE case_files SET index_status='completed' WHERE id=?1",
                            [&job.file_id],
                        );
                    }
                }
                Err(message) => {
                    error!("PageIndex build failed {}: {}", job.id, message);
                    if let Ok(conn) = crate::db::open_db() {
                        let _ = conn.execute(
                            "UPDATE case_files SET index_status='failed',ocr_error=?1 WHERE id=?2",
                            rusqlite::params![format!("PAGE_INDEX_FAILED: {message}"), job.file_id],
                        );
                    }
                }
            }
            info!("document processing completed: {}", job.id);
        }
        Err(process_error) => {
            error!("document processing failed {}: {}", job.id, process_error);
            persist_failure(&job, &process_error)
        }
    }
}

/// Process one durable job; also used by the isolated native integration harness.
pub async fn process_next_document_job() -> anyhow::Result<bool> {
    match claim_next_job()? {
        Some(job) => {
            process_one(job).await;
            Ok(true)
        }
        None => Ok(false),
    }
}

pub fn start_background_worker(_app: AppHandle) {
    crate::workspace_sync::start();
    tauri::async_runtime::spawn(async move {
        if let Ok(conn) = crate::db::open_db() {
            if let Err(error) = crate::db::knowledge_index::recover_interrupted(&conn) {
                error!("knowledge index recovery failed: {}", error);
                return;
            }
        }
        loop {
            match crate::db::knowledge_index::process_next().await {
                Ok(true) => {}
                Ok(false) => tokio::time::sleep(Duration::from_secs(3)).await,
                Err(error) => {
                    error!("knowledge index worker failed: {}", error);
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            }
        }
    });
    tauri::async_runtime::spawn(async move {
        if let Ok(conn) = crate::db::open_db() {
            let _ = conn.execute(
                "UPDATE case_files SET ocr_status='failed',index_status='failed',ocr_error='INTERRUPTED: 应用上次退出时任务仍在运行，请重试' WHERE id IN (SELECT file_id FROM document_processing_jobs WHERE status='running')",
                [],
            );
            let _=conn.execute("UPDATE document_processing_jobs SET status='failed',error_code='INTERRUPTED',error_message='应用上次退出时任务仍在运行，请重试',updated_at=datetime('now','localtime') WHERE status='running'",[]);
        }
        info!("durable document intelligence worker started");
        loop {
            match process_next_document_job().await {
                Ok(true) => {}
                Ok(false) => tokio::time::sleep(Duration::from_secs(5)).await,
                Err(error) => {
                    error!("document worker claim failed: {}", error);
                    tokio::time::sleep(Duration::from_secs(10)).await
                }
            }
        }
    });

}
