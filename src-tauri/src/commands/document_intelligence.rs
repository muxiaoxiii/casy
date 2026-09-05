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
    run_blocking(move || queue_file(&mut db::open_db()?, &file_id)).await
}

pub(crate) fn queue_file(
    conn: &mut rusqlite::Connection,
    file_id: &str,
) -> anyhow::Result<DocumentJobDto> {
    let source_path: String = conn.query_row(
        "SELECT file_path FROM case_files WHERE id=?1",
        [file_id],
        |row| row.get(0),
    )?;
    let path = std::path::Path::new(&source_path);
    if !document_pipeline::supports_path(path) {
        anyhow::bail!("支持 PDF、图片、Markdown、TXT、Word、RTF 和 ODT");
    }
    let source_sha256 = document_pipeline::sha256_file(path)?;
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let active = tx.query_row(
        &format!("SELECT {JOB_COLUMNS} FROM document_processing_jobs WHERE file_id=?1 AND status IN ('queued','running') ORDER BY rowid DESC LIMIT 1"),
        [file_id], map_job,
    ).optional()?;
    if let Some(job) = active {
        if job.source_sha256 != source_sha256 {
            anyhow::bail!("SOURCE_CHANGED: 请先取消此文件的旧任务再处理新版本");
        }
        return Ok(job);
    }
    let completed = tx.query_row(
        &format!("SELECT {JOB_COLUMNS} FROM document_processing_jobs WHERE file_id=?1 ORDER BY rowid DESC LIMIT 1"),
        [file_id], map_job,
    ).optional()?;
    if let Some(job) = completed {
        if job.status == "completed"
            && job.source_sha256 == source_sha256
            && (crate::parse::text_document::supports(path)
                || job
                    .searchable_pdf_path
                    .as_ref()
                    .is_some_and(|p| std::path::Path::new(p).is_file()))
            && [&job.page_ir_path, &job.markdown_path].iter().all(|p| {
                p.as_ref()
                    .is_some_and(|p| std::path::Path::new(p).is_file())
            })
        {
            return Ok(job);
        }
    }
    let id = db::new_id();
    tx.execute(
        "INSERT INTO document_processing_jobs(id,file_id,source_sha256) VALUES(?1,?2,?3)",
        rusqlite::params![id, file_id, source_sha256],
    )?;
    tx.execute("UPDATE case_files SET source_sha256=?1,ocr_status='pending',index_status='pending',ocr_error=NULL WHERE id=?2",
        rusqlite::params![source_sha256,file_id])?;
    let job = tx.query_row(
        &format!("SELECT {JOB_COLUMNS} FROM document_processing_jobs WHERE id=?1"),
        [&id],
        map_job,
    )?;
    tx.commit()?;
    Ok(job)
}

#[tauri::command]
pub async fn list_document_jobs(file_id: String) -> Result<Vec<DocumentJobDto>, String> {
    run_blocking(move||{let conn=db::open_db()?;let mut stmt=conn.prepare(&format!("SELECT {JOB_COLUMNS} FROM document_processing_jobs WHERE file_id=?1 ORDER BY rowid DESC"))?;let jobs=stmt.query_map([file_id],map_job)?.collect::<Result<Vec<_>,_>>()?;Ok(jobs)}).await
}

#[tauri::command]
pub async fn retry_document_job(job_id: String) -> Result<(), String> {
    run_blocking(move || retry_job(&mut db::open_db()?, &job_id)).await
}

pub(crate) fn retry_job(conn: &mut rusqlite::Connection, job_id: &str) -> anyhow::Result<()> {
    let (path, expected): (String, String) = conn.query_row(
        "SELECT f.file_path,j.source_sha256 FROM document_processing_jobs j JOIN case_files f ON f.id=j.file_id WHERE j.id=?1",
        [job_id], |row| Ok((row.get(0)?,row.get(1)?)),
    )?;
    if document_pipeline::sha256_file(std::path::Path::new(&path))? != expected {
        anyhow::bail!("SOURCE_CHANGED: 文件已变化，请创建新处理任务");
    }
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    // A retry gets its own identity so a cancelled process cannot publish into it.
    let changed = tx.execute(
        "INSERT INTO document_processing_jobs(id,file_id,source_sha256)
         SELECT ?2,file_id,source_sha256 FROM document_processing_jobs
         WHERE id=?1 AND status IN ('failed','cancelled') AND NOT EXISTS
         (SELECT 1 FROM document_processing_jobs newer WHERE newer.file_id=document_processing_jobs.file_id
          AND (newer.rowid>document_processing_jobs.rowid OR newer.status IN ('queued','running')))",
        rusqlite::params![job_id,db::new_id()],
    )?;
    if changed != 1 {
        anyhow::bail!("仅可重试最新的失败或已取消任务");
    }
    tx.execute("UPDATE case_files SET ocr_status='pending',index_status='pending',ocr_error=NULL WHERE id=(SELECT file_id FROM document_processing_jobs WHERE id=?1)", [job_id])?;
    tx.commit()?;
    Ok(())
}

#[tauri::command]
pub async fn cancel_document_job(job_id: String) -> Result<(), String> {
    run_blocking(move || cancel_job(&mut db::open_db()?, &job_id)).await
}

pub(crate) fn cancel_job(conn: &mut rusqlite::Connection, job_id: &str) -> anyhow::Result<()> {
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let changed = tx.execute("UPDATE document_processing_jobs SET status='cancelled',updated_at=datetime('now','localtime') WHERE id=?1 AND status IN ('queued','running')", [job_id])?;
    if changed != 1 {
        anyhow::bail!("仅可取消排队或运行中的任务");
    }
    tx.execute("UPDATE case_files SET ocr_status='failed',index_status='failed',ocr_error='CANCELLED: 已取消处理' WHERE id=(SELECT file_id FROM document_processing_jobs WHERE id=?1)", [job_id])?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (rusqlite::Connection, tempfile::TempDir, std::path::PathBuf) {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON").unwrap();
        db::schema::run_migrations(&conn, 0).unwrap();
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("scan.png");
        std::fs::write(&path, b"synthetic queued source").unwrap();
        conn.execute(
            "INSERT INTO cases(id,case_name,client_name) VALUES('c','Queue test','Test')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO case_files(id,case_id,file_name,file_path,category) VALUES('f','c','scan.png',?1,'evidence')", [path.to_str().unwrap()]).unwrap();
        (conn, temp, path)
    }

    #[test]
    fn queue_deduplicates_and_retry_isolates_cancelled_processes() {
        let (mut conn, _temp, _path) = fixture();
        let first = queue_file(&mut conn, "f").unwrap();
        assert_eq!(queue_file(&mut conn, "f").unwrap().id, first.id);
        conn.execute(
            "UPDATE document_processing_jobs SET status='running' WHERE id=?1",
            [&first.id],
        )
        .unwrap();
        cancel_job(&mut conn, &first.id).unwrap();
        retry_job(&mut conn, &first.id).unwrap();
        let retry = queue_file(&mut conn, "f").unwrap();
        assert_ne!(retry.id, first.id);
        assert_eq!(retry.status, "queued");
        assert!(retry_job(&mut conn, &first.id).is_err());
        assert_eq!(
            conn.query_row("SELECT ocr_status FROM case_files WHERE id='f'", [], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
            "pending"
        );
        assert_eq!(
            conn.query_row(
                "SELECT status FROM document_processing_jobs WHERE id=?1",
                [&first.id],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "cancelled"
        );
    }

    #[test]
    fn changed_source_requires_new_job_after_cancellation() {
        let (mut conn, _temp, path) = fixture();
        let first = queue_file(&mut conn, "f").unwrap();
        std::fs::write(path, b"changed source").unwrap();
        assert!(queue_file(&mut conn, "f")
            .unwrap_err()
            .to_string()
            .contains("SOURCE_CHANGED"));
        cancel_job(&mut conn, &first.id).unwrap();
        assert!(retry_job(&mut conn, &first.id)
            .unwrap_err()
            .to_string()
            .contains("SOURCE_CHANGED"));
        let next = queue_file(&mut conn, "f").unwrap();
        assert_ne!(first.source_sha256, next.source_sha256);
    }

    #[test]
    fn missing_completed_artifacts_are_rebuilt() {
        let (mut conn, _temp, _) = fixture();
        let first = queue_file(&mut conn, "f").unwrap();
        conn.execute(
            "UPDATE document_processing_jobs SET status='completed' WHERE id=?1",
            [&first.id],
        )
        .unwrap();
        let next = queue_file(&mut conn, "f").unwrap();
        assert_ne!(first.id, next.id);
        assert_eq!(next.status, "queued");
    }
}
