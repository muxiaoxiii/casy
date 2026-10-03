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
    pub phase: String,
    pub elapsed_seconds: f64,
    pub remaining_seconds: Option<f64>,
    pub page_timing: Option<document_pipeline::DocumentPageTiming>,
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
        phase: row.get(9)?,
        elapsed_seconds: row.get(10)?,
        remaining_seconds: row.get(11)?,
        page_timing: row.get::<_,Option<String>>(12)?.map(|value|serde_json::from_str(&value)).transpose().map_err(|error|rusqlite::Error::FromSqlConversionFailure(12,rusqlite::types::Type::Text,Box::new(error)))?,
        searchable_pdf_path: row.get(13)?,
        page_ir_path: row.get(14)?,
        markdown_path: row.get(15)?,
        error_code: row.get(16)?,
        error_message: row.get(17)?,
        created_at: row.get(18)?,
        updated_at: row.get(19)?,
    })
}
const JOB_COLUMNS:&str="id,file_id,source_sha256,status,engine,model_version,current_page,total_pages,progress,phase,elapsed_ms/1000.0,remaining_ms/1000.0,timing_json,searchable_pdf_path,page_ir_path,markdown_path,error_code,error_message,created_at,updated_at";

#[tauri::command]
pub async fn get_document_engine_status() -> Result<document_pipeline::DocumentEngineStatus, String>
{
    Ok(document_pipeline::probe_engine().await)
}

#[tauri::command]
pub async fn queue_document_processing(file_id: String) -> Result<DocumentJobDto, String> {
    run_blocking(move || queue_file(&mut *db::open_db()?, &file_id)).await
}

pub(crate) fn queue_file(
    conn: &mut rusqlite::Connection,
    file_id: &str,
) -> anyhow::Result<DocumentJobDto> {
    let source_path: String = conn.query_row(
        "SELECT file_path FROM case_files WHERE id=?1 AND deleted_at IS NULL",
        [file_id],
        |row| row.get(0),
    )?;
    let path = std::path::Path::new(&source_path);
    if !document_pipeline::supports_path(path) {
        anyhow::bail!("支持 PDF、图片、Markdown、TXT、Word、RTF 和 ODT");
    }
    let source_sha256 = document_pipeline::sha256_file(path)?;
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let unchanged: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM case_files WHERE id=?1 AND file_path=?2 AND deleted_at IS NULL)", rusqlite::params![file_id,source_path], |r|r.get(0))?;
    if !unchanged {
        anyhow::bail!("文件已移动或移除，请刷新后重试");
    }
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
            && (crate::parse::text_document::supports(path) || job.engine.starts_with("paddle-onnx-"))
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
pub async fn list_case_document_jobs(case_id: String) -> Result<Vec<DocumentJobDto>, String> {
    run_blocking(move || {
        let conn = db::open_db()?;
        let columns = JOB_COLUMNS.split(',').map(|column|format!("j.{column}")).collect::<Vec<_>>().join(",");
        let mut stmt = conn.prepare(&format!("SELECT {columns} FROM document_processing_jobs j
            JOIN case_files f ON f.id=j.file_id WHERE f.case_id=?1 AND f.deleted_at IS NULL
            AND j.rowid=(SELECT max(newer.rowid) FROM document_processing_jobs newer WHERE newer.file_id=j.file_id)"))?;
        let rows = stmt.query_map([case_id],map_job)?.collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    }).await
}

#[tauri::command]
pub async fn retry_document_job(job_id: String) -> Result<(), String> {
    run_blocking(move || retry_job(&mut *db::open_db()?, &job_id)).await
}

pub(crate) fn retry_job(conn: &mut rusqlite::Connection, job_id: &str) -> anyhow::Result<()> {
    let (path, expected): (String, String) = conn.query_row(
        "SELECT f.file_path,j.source_sha256 FROM document_processing_jobs j JOIN case_files f ON f.id=j.file_id WHERE j.id=?1 AND f.deleted_at IS NULL",
        [job_id], |row| Ok((row.get(0)?,row.get(1)?)),
    )?;
    if document_pipeline::sha256_file(std::path::Path::new(&path))? != expected {
        anyhow::bail!("SOURCE_CHANGED: 文件已变化，请创建新处理任务");
    }
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    // A retry gets its own identity so a cancelled process cannot publish into it.
    let live: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM document_processing_jobs j JOIN case_files f ON f.id=j.file_id WHERE j.id=?1 AND f.file_path=?2 AND f.deleted_at IS NULL)",rusqlite::params![job_id,path],|r|r.get(0))?;
    if !live {
        anyhow::bail!("文件已移动或移除，请刷新后重试");
    }
    let changed = tx.execute(
        "INSERT INTO document_processing_jobs(id,file_id,source_sha256)
         SELECT ?2,file_id,source_sha256 FROM document_processing_jobs
         WHERE id=?1 AND status IN ('failed','cancelled','completed') AND NOT EXISTS
         (SELECT 1 FROM document_processing_jobs newer WHERE newer.file_id=document_processing_jobs.file_id
          AND (newer.rowid>document_processing_jobs.rowid OR newer.status IN ('queued','running')))",
        rusqlite::params![job_id,db::new_id()],
    )?;
    if changed != 1 {
        anyhow::bail!("仅可重新处理最新的已完成、失败或已取消任务；不能重复启动正在处理的文件");
    }
    tx.execute("UPDATE case_files SET ocr_status='pending',index_status='pending',ocr_error=NULL WHERE id=(SELECT file_id FROM document_processing_jobs WHERE id=?1)", [job_id])?;
    tx.commit()?;
    Ok(())
}

#[tauri::command]
pub async fn cancel_document_job(job_id: String) -> Result<(), String> {
    run_blocking(move || cancel_job(&mut *db::open_db()?, &job_id)).await
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

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DocumentPageView {
    pub file_id: String,
    pub job_id: String,
    pub file_name: String,
    pub page_number: u32,
    pub total_pages: u32,
    pub markdown: String,
    pub image_data: Option<String>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub regions: Vec<document_pipeline::DocumentRegion>,
    pub layout: Option<serde_json::Value>,
    pub timing: Option<document_pipeline::DocumentPageTiming>,
}

#[tauri::command]
pub async fn correct_document_region(file_id: String, job_id: String, page_number: u32, region_index: usize, expected_text: String, text: String) -> Result<String, String> {
    let (job, request, pages) = run_blocking(move || {
        anyhow::ensure!(text.chars().count() <= 10000, "单一区域文字不能超过 10000 字");
        let mut conn = db::open_db()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let (source, hash, engine): (String,String,String) = tx.query_row("SELECT f.file_path,j.source_sha256,j.engine FROM case_files f JOIN document_processing_jobs j ON j.file_id=f.id WHERE f.id=?1 AND j.id=?2 AND j.status='completed' AND f.deleted_at IS NULL", rusqlite::params![file_id,job_id], |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
        anyhow::ensure!(engine.starts_with("paddle-onnx-"), "此文档没有可校订的 OCR 区域");
        let newest:String = tx.query_row("SELECT id FROM document_processing_jobs WHERE file_id=?1 AND status='completed' ORDER BY rowid DESC LIMIT 1",[&file_id],|r|r.get(0))?;
        anyhow::ensure!(newest == job_id, "OCR_VERSION_CHANGED: 已有更新版本，请重新打开后校订");
        let running:bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM document_processing_jobs WHERE file_id=?1 AND status IN ('queued','running'))",[&file_id],|r|r.get(0))?;
        anyhow::ensure!(!running, "此文档正在处理，请完成后校订");
        anyhow::ensure!(document_pipeline::sha256_file(std::path::Path::new(&source))? == hash,"SOURCE_CHANGED: 原文件已变化，请重新处理");
        let mut stmt = tx.prepare("SELECT page_number,width,height,plain_text,markdown,regions_json,confidence,layout_json,timing_json FROM document_pages WHERE job_id=?1 ORDER BY page_number")?;
        let raw = stmt.query_map([&job_id], |r|Ok((r.get::<_,u32>(0)?,r.get::<_,Option<f32>>(1)?,r.get::<_,Option<f32>>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?,r.get::<_,Option<f32>>(6)?,r.get::<_,Option<String>>(7)?,r.get::<_,Option<String>>(8)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        drop(stmt);
        let mut pages = raw.into_iter().map(|(page_number,width,height,plain_text,markdown,regions,confidence,layout,timing)| Ok(document_pipeline::DocumentPage {page_number,width,height,plain_text,markdown,regions:serde_json::from_str(&regions)?,confidence,layout:layout.map(|value|serde_json::from_str(&value)).transpose()?,timing:timing.map(|value|serde_json::from_str(&value)).transpose()?})).collect::<anyhow::Result<Vec<_>>>()?;
        let old_markdown: String = tx.query_row("SELECT markdown_path FROM document_processing_jobs WHERE id=?1",[&job_id],|r|r.get(0))?;
        let old_root = std::path::Path::new(&old_markdown).parent().ok_or_else(||anyhow::anyhow!("文档目录无效"))?;
        let page = pages.iter_mut().find(|p|p.page_number==page_number).ok_or_else(||anyhow::anyhow!("页码不存在"))?;
        let plain_markdown = page.markdown == page.plain_text
            || page.markdown == page.regions.iter().map(|region| region.text.as_str()).collect::<Vec<_>>().join("\n");
        let region = page.regions.get_mut(region_index).ok_or_else(||anyhow::anyhow!("区域不存在"))?;
        anyhow::ensure!(region.text == expected_text,"OCR_VERSION_CHANGED: 区域文字已变化");
        let structured_markdown = document_pipeline::source_map::corrected_markdown(&page.markdown, region_index, &region.text, &text);
        anyhow::ensure!(structured_markdown.is_some() || plain_markdown,
            "OCR_REGION_UNMAPPED: 无法安全定位此区域，未修改正文或图片");
        region.text = text;
        region.confidence = None;
        page.plain_text = page.regions.iter().map(|r|r.text.as_str()).collect::<Vec<_>>().join("\n");
        page.markdown = structured_markdown.unwrap_or_else(|| page.plain_text.clone());
        let id = db::new_id();
        let output = document_pipeline::artifact_dir(&file_id,&hash)?.join(&id);
        std::fs::create_dir_all(&output)?;
        if old_root.join(document_pipeline::assets::MANIFEST).exists() {
            let manifest = document_pipeline::assets::load(old_root)?;
            for page in &pages {
                document_pipeline::assets::validate_references(&page.markdown, old_root, &manifest)?;
            }
            document_pipeline::assets::copy_to(old_root, &output, &manifest)?;
        }
        let request = document_pipeline::process_request(&id,&source,&hash,&output);
        tx.execute("INSERT INTO document_processing_jobs(id,file_id,source_sha256,status,engine,started_at) VALUES(?1,?2,?3,'running','paddle-onnx-corrected',datetime('now','localtime'))",rusqlite::params![id,file_id,hash])?;
        tx.commit()?;
        Ok((crate::background_jobs::ClaimedJob {id,file_id,source_path:source,source_sha256:hash},request,pages))
    }).await?;
    let result = document_pipeline::run_revision(request,pages).await;
    let job_id = job.id.clone();
    let index_file = job.file_id.clone();
    let index_source = job.source_path.clone();
    let saved = run_blocking(move || {
        match result {
            Ok(result) => {
                if let Err(error) = crate::background_jobs::persist_success(&job,&result) {
                    crate::background_jobs::persist_failure(&job,&error);
                    return Err(error);
                }
                Ok(job_id)
            }
            Err(error) => { crate::background_jobs::persist_failure(&job,&error); Err(error) }
        }
    }).await?;
    let indexing = crate::ai::page_index::build_page_index_tree(&index_file,&index_source).await;
    run_blocking(move || {
        let conn = db::open_db()?;
        conn.execute("UPDATE case_files SET index_status=?1 WHERE id=?2",rusqlite::params![if indexing.is_ok(){"completed"}else{"failed"},index_file])?;
        Ok(())
    }).await?;
    Ok(saved)
}

#[tauri::command]
pub async fn get_document_page(
    file_id: String,
    job_id: String,
    page_number: u32,
) -> Result<DocumentPageView, String> {
    let (mut view, source, hash, render_source) = run_blocking(move || {
        let conn = db::open_db()?;
        load_document_page(&conn, &file_id, &job_id, page_number)
    })
    .await?;
    if let Some(pdf) = render_source {
        let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
        let prefix = temp.path().join("page");
        let mut renderer = tokio::process::Command::new(crate::runtime_paths::pdf_renderer());
        if let Some(config) = crate::runtime_paths::runtime_asset("FONTCONFIG_FILE","fonts/fonts.conf") { renderer.env("FONTCONFIG_FILE",config); }
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(60),
            renderer
                .args([
                    "-f",
                    &page_number.to_string(),
                    "-l",
                    &page_number.to_string(),
                    "-singlefile",
                    "-png",
                    "-scale-to",
                    "1600",
                ])
                .arg(pdf)
                .arg(&prefix)
                .kill_on_drop(true)
                .output(),
        )
        .await
        .map_err(|_| "页面预览超时".to_owned())?
        .map_err(|e| format!("无法渲染 PDF 页面: {e}"))?;
        if !output.status.success() {
            return Err("无法渲染原 PDF 页面".into());
        }
        let path = prefix.with_extension("png");
        if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 16 * 1024 * 1024 {
            return Err("页面预览超过大小限制".into());
        }
        use base64::Engine;
        view.image_data = Some(format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD
                .encode(std::fs::read(path).map_err(|e| e.to_string())?)
        ));
    }
    run_blocking(move || {
        if document_pipeline::sha256_file(std::path::Path::new(&source))? != hash {
            anyhow::bail!("SOURCE_CHANGED: 原文件已变化，请重新处理后定位");
        }
        Ok(())
    })
    .await?;
    Ok(view)
}

fn load_document_page(
    conn: &rusqlite::Connection,
    file: &str,
    job: &str,
    number: u32,
) -> anyhow::Result<(DocumentPageView, String, String, Option<String>)> {
    let (view,source,hash,engine,searchable) = conn.query_row(r#"
        SELECT f.file_name,f.file_path,j.source_sha256,j.engine,j.searchable_pdf_path,j.total_pages,
               p.markdown,p.width,p.height,p.regions_json,p.layout_json,p.timing_json
        FROM case_files f JOIN document_processing_jobs j ON j.file_id=f.id
        JOIN document_pages p ON p.job_id=j.id AND p.file_id=f.id
        WHERE f.id=?1 AND j.id=?2 AND p.page_number=?3 AND j.status='completed' AND f.deleted_at IS NULL
    "#,rusqlite::params![file,job,number],|r|Ok((DocumentPageView {
        file_id:file.into(),job_id:job.into(),file_name:r.get(0)?,page_number:number,total_pages:r.get(5)?,
        markdown:r.get(6)?,width:r.get(7)?,height:r.get(8)?,regions:vec![],image_data:None,
        layout:r.get::<_,Option<String>>(10)?.map(|value|serde_json::from_str(&value)).transpose().map_err(|error|rusqlite::Error::FromSqlConversionFailure(10,rusqlite::types::Type::Text,Box::new(error)))?,
        timing:r.get::<_,Option<String>>(11)?.map(|value|serde_json::from_str(&value)).transpose().map_err(|error|rusqlite::Error::FromSqlConversionFailure(11,rusqlite::types::Type::Text,Box::new(error)))?
    },r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,
      (r.get::<_,Option<String>>(4)?,r.get::<_,String>(9)?))))?;
    if document_pipeline::sha256_file(std::path::Path::new(&source))? != hash {
        anyhow::bail!("SOURCE_CHANGED: 原文件已变化，请重新处理后定位");
    }
    let mut view = view;
    view.regions = serde_json::from_str(&searchable.1)?;
    let render = if engine == "text-document" {
        None
    } else if std::path::Path::new(&source)
        .extension()
        .is_some_and(|s| s.eq_ignore_ascii_case("pdf"))
    {
        Some(source.clone())
    } else {
        Some(
            searchable
                .0
                .ok_or_else(|| anyhow::anyhow!("缺少图片文档的 PDF 预览"))?,
        )
    };
    Ok((view, source, hash, render))
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
    fn completed_job_can_be_explicitly_reprocessed_without_erasing_previous_result() {
        let (mut conn, _temp, _path) = fixture();
        let original = queue_file(&mut conn, "f").unwrap();
        conn.execute("UPDATE document_processing_jobs SET status='completed',markdown_path='old-result.md' WHERE id=?1", [&original.id]).unwrap();
        retry_job(&mut conn, &original.id).unwrap();
        let next = queue_file(&mut conn, "f").unwrap();
        assert_ne!(original.id, next.id);
        assert_eq!(next.status, "queued");
        assert!(retry_job(&mut conn, &original.id).is_err());
        assert!(retry_job(&mut conn, &next.id).is_err());
        let old:(String,String)=conn.query_row("SELECT status,markdown_path FROM document_processing_jobs WHERE id=?1",[&original.id],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
        assert_eq!(old,("completed".into(),"old-result.md".into()));
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

    #[test]
    fn source_view_rejects_wrong_binding_deleted_files_and_changed_originals() {
        let (mut conn, _temp, path) = fixture();
        let job = queue_file(&mut conn, "f").unwrap();
        conn.execute("UPDATE document_processing_jobs SET status='completed',engine='text-document',total_pages=1 WHERE id=?1",[&job.id]).unwrap();
        conn.execute("INSERT INTO document_pages(job_id,file_id,page_number,markdown) VALUES(?1,'f',1,'evidence')",[&job.id]).unwrap();
        let (view, _, _, _) = load_document_page(&conn, "f", &job.id, 1).unwrap();
        assert_eq!(view.markdown, "evidence");
        assert!(load_document_page(&conn, "other", &job.id, 1).is_err());
        assert!(load_document_page(&conn, "f", &job.id, 2).is_err());
        conn.execute(
            "UPDATE case_files SET deleted_at='2026-09-07' WHERE id='f'",
            [],
        )
        .unwrap();
        assert!(load_document_page(&conn, "f", &job.id, 1).is_err());
        conn.execute("UPDATE case_files SET deleted_at=NULL WHERE id='f'", [])
            .unwrap();
        std::fs::write(path, b"changed original").unwrap();
        assert!(load_document_page(&conn, "f", &job.id, 1)
            .unwrap_err()
            .to_string()
            .contains("SOURCE_CHANGED"));
    }
}
