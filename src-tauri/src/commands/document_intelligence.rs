use super::run_blocking;
use crate::{db, document_pipeline};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DocumentJobDto {
    pub id: String,
    pub file_id: String,
    pub source_sha256: String,
    pub status: String,
    pub engine: String,
    pub model_version: Option<String>,
    pub current_page: i64,
    pub total_pages: i64,
    pub progress: f64,
    pub searchable_pdf_path: Option<String>,
    pub page_ir_path: Option<String>,
    pub markdown_path: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
fn map_job(row: &rusqlite::Row<'_>) -> rusqlite::Result<DocumentJobDto> {
    Ok(DocumentJobDto {
        id: row.get(0)?,
        file_id: row.get(1)?,
        source_sha256: row.get(2)?,
        status: row.get(3)?,
        engine: row.get(4)?,
        model_version: row.get(5)?,
        current_page: row.get(6)?,
        total_pages: row.get(7)?,
        progress: row.get(8)?,
        searchable_pdf_path: row.get(9)?,
        page_ir_path: row.get(10)?,
        markdown_path: row.get(11)?,
        error_code: row.get(12)?,
        error_message: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
    })
}
const JOB_COLUMNS:&str="id,file_id,source_sha256,status,engine,model_version,current_page,total_pages,progress,searchable_pdf_path,page_ir_path,markdown_path,error_code,error_message,created_at,updated_at";

#[tauri::command]
pub async fn get_document_engine_status() -> Result<document_pipeline::DocumentEngineStatus, String>
{
    Ok(document_pipeline::probe_engine().await)
}

#[tauri::command]
pub async fn queue_document_processing(file_id: String) -> Result<DocumentJobDto, String> {
    run_blocking(move||{
    let mut conn=db::open_db()?; let tx=conn.transaction()?;
    let source_path:String=tx.query_row("SELECT file_path FROM case_files WHERE id=?1",[&file_id],|row|row.get(0))?;
    let path=std::path::Path::new(&source_path);
    if path.extension().and_then(|v|v.to_str()).map(|v|v.eq_ignore_ascii_case("pdf"))!=Some(true){ anyhow::bail!("当前文档流水线仅接受 PDF"); }
    let source_sha256=document_pipeline::sha256_file(path)?;
    let existing=tx.query_row(&format!("SELECT {JOB_COLUMNS} FROM document_processing_jobs WHERE file_id=?1 AND source_sha256=?2 AND status IN ('queued','running','completed') ORDER BY created_at DESC LIMIT 1"),rusqlite::params![file_id,source_sha256],map_job).optional()?;
    if let Some(job)=existing{tx.commit()?;return Ok(job)}
    let id=db::new_id(); tx.execute("INSERT INTO document_processing_jobs(id,file_id,source_sha256) VALUES(?1,?2,?3)",rusqlite::params![id,file_id,source_sha256])?;
    tx.execute("UPDATE case_files SET source_sha256=?1,ocr_status='pending',index_status='pending',ocr_error=NULL WHERE id=?2",rusqlite::params![source_sha256,file_id])?;
    let job=tx.query_row(&format!("SELECT {JOB_COLUMNS} FROM document_processing_jobs WHERE id=?1"),[&id],map_job)?; tx.commit()?; Ok(job)
}).await
}

#[tauri::command]
pub async fn list_document_jobs(file_id: String) -> Result<Vec<DocumentJobDto>, String> {
    run_blocking(move||{let conn=db::open_db()?;let mut stmt=conn.prepare(&format!("SELECT {JOB_COLUMNS} FROM document_processing_jobs WHERE file_id=?1 ORDER BY created_at DESC"))?;let jobs=stmt.query_map([file_id],map_job)?.collect::<Result<Vec<_>,_>>()?;Ok(jobs)}).await
}

#[tauri::command]
pub async fn retry_document_job(job_id: String) -> Result<(), String> {
    run_blocking(move||{let conn=db::open_db()?;let changed=conn.execute("UPDATE document_processing_jobs SET status='queued',progress=0,current_page=0,error_code=NULL,error_message=NULL,updated_at=datetime('now','localtime') WHERE id=?1 AND status IN ('failed','cancelled')",[&job_id])?;if changed!=1{anyhow::bail!("只有失败或已取消的任务可以重试")};Ok(())}).await
}

#[tauri::command]
pub async fn cancel_document_job(job_id: String) -> Result<(), String> {
    run_blocking(move||{let conn=db::open_db()?;let changed=conn.execute("UPDATE document_processing_jobs SET status='cancelled',updated_at=datetime('now','localtime') WHERE id=?1 AND status='queued'",[&job_id])?;if changed!=1{anyhow::bail!("当前仅能取消尚未启动的任务")};Ok(())}).await
}
