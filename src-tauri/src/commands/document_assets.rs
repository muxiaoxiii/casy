use crate::{db, document_pipeline::assets};
use base64::{engine::general_purpose::STANDARD, Engine};
use rusqlite::OptionalExtension;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageState {
    job_id: String,
    external_images: bool,
    can_optimize: bool,
    can_rollback: bool,
}

#[tauri::command]
pub async fn get_document_storage_state(file_id: String) -> Result<StorageState, String> {
    super::run_blocking(move || {
        let conn = db::open_db()?;
        let (job, markdown, engine): (String, String, String) = conn.query_row(
            "SELECT j.id,j.markdown_path,j.engine FROM document_processing_jobs j
             JOIN case_files f ON f.id=j.file_id
             WHERE f.id=?1 AND f.deleted_at IS NULL AND j.status='completed'
             ORDER BY j.rowid DESC LIMIT 1",
            [&file_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        let root = std::path::Path::new(&markdown)
            .parent()
            .ok_or_else(|| anyhow::anyhow!("文档产物目录无效"))?;
        let external_images = if root.join(assets::MANIFEST).exists() {
            assets::load(root)?;
            true
        } else {
            false
        };
        let can_rollback = previous_job(&conn, &job)?.is_some();
        Ok(StorageState {
            job_id: job,
            external_images,
            can_optimize: !external_images && engine.starts_with("paddle-onnx"),
            can_rollback,
        })
    })
    .await
}

fn previous_job(conn: &rusqlite::Connection, job_id: &str) -> anyhow::Result<Option<String>> {
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key=?1",
            [format!("document_asset_upgrade:{job_id}")],
            |r| r.get(0),
        )
        .optional()?;
    value
        .map(|value| {
            let receipt: serde_json::Value = serde_json::from_str(&value)?;
            anyhow::ensure!(receipt["version"] == 2, "升级回执版本无效");
            receipt["previousJobId"]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| anyhow::anyhow!("升级回执无效"))
        })
        .transpose()
}

fn validate_pages(
    conn: &rusqlite::Connection,
    job_id: &str,
    file_id: &str,
    ir_path: &str,
    markdown_path: &str,
    hash: &str,
) -> anyhow::Result<Vec<crate::document_pipeline::DocumentPage>> {
    use crate::document_pipeline::{self, DocumentPage};
    let pages: Vec<DocumentPage> = serde_json::from_reader(std::fs::File::open(ir_path)?)?;
    anyhow::ensure!(
        !pages.is_empty()
            && pages
                .iter()
                .enumerate()
                .all(|(index, page)| page.page_number as usize == index + 1),
        "页面序列无效"
    );
    let expected = document_pipeline::source_map::write_to(&pages, hash, std::io::sink())?;
    anyhow::ensure!(
        document_pipeline::sha256_file(std::path::Path::new(markdown_path))?
            == expected.markdown_sha256,
        "正文与页面不一致"
    );
    let root = std::path::Path::new(markdown_path)
        .parent()
        .ok_or_else(|| anyhow::anyhow!("文档目录无效"))?;
    if root.join("source.map.json").exists() {
        let map: document_pipeline::source_map::SourceMap =
            serde_json::from_reader(std::fs::File::open(root.join("source.map.json"))?)?;
        anyhow::ensure!(map == expected, "来源映射与页面不一致");
    }
    let manifest = if root.join(assets::MANIFEST).exists() {
        assets::load(root)?
    } else {
        assets::Manifest::default()
    };
    let count: i64 = conn.query_row(
        "SELECT count(*) FROM document_pages WHERE job_id=?1",
        [job_id],
        |r| r.get(0),
    )?;
    anyhow::ensure!(count as usize == pages.len(), "数据库页数与产物不一致");
    for page in &pages {
        assets::validate_references(&page.markdown, root, &manifest)?;
        let stored = conn.query_row(
            "SELECT width,height,plain_text,markdown,regions_json,confidence,layout_json,timing_json,orientation_degrees
             FROM document_pages WHERE job_id=?1 AND file_id=?2 AND page_number=?3",
            rusqlite::params![job_id, file_id, page.page_number],
            |r| Ok((r.get::<_, Option<f32>>(0)?, r.get::<_, Option<f32>>(1)?,
                r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?,
                r.get::<_, Option<f32>>(5)?, r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?, r.get::<_, Option<u16>>(8)?)),
        )?;
        let stored = DocumentPage {
            page_number: page.page_number,
            width: stored.0,
            height: stored.1,
            plain_text: stored.2,
            markdown: stored.3,
            regions: serde_json::from_str(&stored.4)?,
            confidence: stored.5,
            layout: stored
                .6
                .map(|value| serde_json::from_str(&value))
                .transpose()?,
            timing: stored
                .7
                .map(|value| serde_json::from_str(&value))
                .transpose()?,
            orientation_degrees: stored.8,
        };
        anyhow::ensure!(*page == stored, "数据库页面与产物不一致");
    }
    Ok(pages)
}

#[tauri::command]
pub async fn read_document_asset(
    file_id: String,
    job_id: String,
    asset_id: String,
) -> Result<String, String> {
    super::run_blocking(move || {
        let conn = db::open_db()?;
        let markdown: String = conn.query_row(
            "SELECT j.markdown_path FROM document_processing_jobs j JOIN case_files f ON f.id=j.file_id
             WHERE j.id=?1 AND f.id=?2 AND f.deleted_at IS NULL AND j.status='completed'",
            rusqlite::params![job_id, file_id], |row| row.get(0),
        )?;
        let root = std::path::Path::new(&markdown).parent().ok_or_else(|| anyhow::anyhow!("文档产物目录无效"))?;
        let manifest = assets::load(root)?;
        let bytes = assets::read(root, &manifest, &asset_id)?;
        Ok(STANDARD.encode(bytes))
    }).await
}

/// Repackage an existing successful OCR result. No model inference or source edit.
#[tauri::command]
pub async fn optimize_document_storage(file_id: String, job_id: String) -> Result<String, String> {
    super::run_blocking(move || {
        use crate::document_pipeline;
        use std::path::Path;
        let mut conn = db::open_db()?;
        let (source, hash, ir_path, markdown_path, engine): (String,String,String,String,String) = conn.query_row(
            "SELECT f.file_path,j.source_sha256,j.page_ir_path,j.markdown_path,j.engine
             FROM document_processing_jobs j JOIN case_files f ON f.id=j.file_id
             WHERE j.id=?1 AND f.id=?2 AND j.status='completed' AND f.deleted_at IS NULL",
            rusqlite::params![job_id,file_id], |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)),
        )?;
        anyhow::ensure!(engine.starts_with("paddle-onnx"), "此操作仅适用于已完成的 OCR 产物");
        anyhow::ensure!(document_pipeline::sha256_file(Path::new(&source))? == hash, "原文件已变化，未升级产物");
        let old_root = Path::new(&markdown_path).parent().ok_or_else(||anyhow::anyhow!("文档目录无效"))?;
        let pages_before = std::fs::read(&ir_path)?;
        let mut page_records: Vec<serde_json::Value> = serde_json::from_slice(&pages_before)?;
        let mut pages = validate_pages(&conn, &job_id, &file_id, &ir_path, &markdown_path, &hash)?;
        let base = document_pipeline::artifact_dir(&file_id, &hash)?;
        let staging = tempfile::Builder::new().prefix("asset-upgrade-").tempdir_in(&base)?;
        let root = staging.path();
        let mut manifest = assets::Manifest::default();
        if old_root.join(assets::MANIFEST).exists() {
            let old_manifest = assets::load(old_root)?;
            assets::copy_to(old_root, root, &old_manifest)?;
            manifest = old_manifest;
        }
        // Check the old page/file contract before transforming either representation.
        let old_map = document_pipeline::source_map::write_to(&pages,&hash,std::io::sink())?;
        anyhow::ensure!(document_pipeline::sha256_file(Path::new(&markdown_path))? == old_map.markdown_sha256, "旧正文与页面不一致，未升级");
        for page in &mut pages {
            page.markdown = assets::externalize(&page.markdown,root,&mut manifest)?;
            assets::validate_references(&page.markdown,root,&manifest)?;
        }
        assets::save(root,&manifest)?;
        let new_ir = root.join("source.document.json");
        let new_md = root.join("source.md");
        for (record, page) in page_records.iter_mut().zip(&pages) {
            record["markdown"] = serde_json::Value::String(page.markdown.clone());
        }
        std::fs::write(&new_ir,serde_json::to_vec(&page_records)?)?;
        let new_map = document_pipeline::source_map::write_to(&pages,&hash,std::io::BufWriter::new(std::fs::File::create(&new_md)?))?;
        anyhow::ensure!(new_map.text_sha256 == old_map.text_sha256, "升级改变了识别文字");
        std::fs::write(root.join("source.map.json"),serde_json::to_vec(&new_map)?)?;
        anyhow::ensure!(document_pipeline::sha256_file(&new_md)? == new_map.markdown_sha256, "新正文校验失败");
        for entry in walk_files(root)? {
            std::fs::OpenOptions::new().write(true).open(entry)?.sync_all()?;
        }
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let present: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM case_files WHERE id=?1 AND file_path=?2 AND deleted_at IS NULL)",
            rusqlite::params![file_id, source], |r| r.get(0),
        )?;
        anyhow::ensure!(present, "原文件已移动或移除，未升级产物");
        let latest: String = tx.query_row("SELECT id FROM document_processing_jobs WHERE file_id=?1 AND status='completed' ORDER BY rowid DESC LIMIT 1",[&file_id],|r|r.get(0))?;
        anyhow::ensure!(latest == job_id,"已有新的 OCR 版本，请刷新后重试");
        let busy: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM document_processing_jobs WHERE file_id=?1 AND status IN ('queued','running'))",[&file_id],|r|r.get(0))?;
        anyhow::ensure!(!busy,"文档正在处理中，请稍后重试");
        anyhow::ensure!(std::fs::read(&ir_path)? == pages_before && document_pipeline::sha256_file(Path::new(&source))? == hash,"升级期间源数据变化");
        validate_pages(&tx, &job_id, &file_id, &ir_path, &markdown_path, &hash)?;
        if old_root.join(assets::MANIFEST).exists()
            && !pages_before.windows(b"data:image/png;base64,".len())
                .any(|window| window == b"data:image/png;base64,") {
            return Ok(job_id);
        }
        let new_id = db::new_id();
        tx.execute("INSERT INTO document_processing_jobs(id,file_id,source_sha256,status,engine,model_version,current_page,total_pages,progress,searchable_pdf_path,page_ir_path,markdown_path,phase,index_status,completed_at)
            SELECT ?1,file_id,source_sha256,'completed',engine,model_version,current_page,total_pages,1,searchable_pdf_path,?2,?3,'completed',index_status,datetime('now','localtime') FROM document_processing_jobs WHERE id=?4",
            rusqlite::params![new_id,new_ir.to_string_lossy(),new_md.to_string_lossy(),job_id])?;
        for page in &pages {
            tx.execute("INSERT INTO document_pages(job_id,file_id,page_number,width,height,plain_text,markdown,regions_json,confidence,layout_json,timing_json) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",rusqlite::params![new_id,file_id,page.page_number,page.width,page.height,page.plain_text,page.markdown,serde_json::to_string(&page.regions)?,page.confidence,page.layout.as_ref().map(serde_json::to_string).transpose()?,page.timing.as_ref().map(serde_json::to_string).transpose()?])?;
        }
        tx.execute("UPDATE case_files SET document_ir_path=?1,ocr_markdown_path=?2 WHERE id=?3",rusqlite::params![new_ir.to_string_lossy(),new_md.to_string_lossy(),file_id])?;
        tx.execute("INSERT INTO settings(key,value) VALUES(?1,?2)",rusqlite::params![format!("document_asset_upgrade:{new_id}"),serde_json::json!({"previousJobId":job_id,"version":2}).to_string()])?;
        // Keep disk artifacts before committing references. A failed commit leaves only an orphan.
        let _ = staging.keep();
        tx.commit()?;
        Ok(new_id)
    }).await
}
fn walk_files(root: &std::path::Path) -> anyhow::Result<Vec<std::path::PathBuf>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            files.extend(walk_files(&entry.path())?);
        } else {
            files.push(entry.path());
        }
    }
    Ok(files)
}

#[tauri::command]
pub async fn rollback_document_storage(file_id: String, job_id: String) -> Result<String, String> {
    super::run_blocking(move || {
        let mut conn=db::open_db()?;
        let tx=conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let previous = previous_job(&tx, &job_id)?.ok_or_else(||anyhow::anyhow!("没有可回退的存储升级"))?;
        let latest:String=tx.query_row("SELECT id FROM document_processing_jobs WHERE file_id=?1 AND status='completed' ORDER BY rowid DESC LIMIT 1",[&file_id],|r|r.get(0))?;
        anyhow::ensure!(latest==job_id,"已有新版本，不能回退此存储升级");
        let busy:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM document_processing_jobs WHERE file_id=?1 AND status IN ('queued','running'))",[&file_id],|r|r.get(0))?;
        anyhow::ensure!(!busy,"文档正在处理");
        let (ir,md,hash,source,pdf):(String,String,String,String,Option<String>)=tx.query_row(
            "SELECT j.page_ir_path,j.markdown_path,j.source_sha256,f.file_path,j.searchable_pdf_path
             FROM document_processing_jobs j JOIN case_files f ON f.id=j.file_id
             WHERE j.id=?1 AND f.id=?2 AND f.deleted_at IS NULL AND j.status='completed'",
            rusqlite::params![previous,file_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))?;
        anyhow::ensure!(crate::document_pipeline::sha256_file(std::path::Path::new(&source))? == hash, "原文件已变化，未回退");
        if let Some(pdf) = pdf {
            anyhow::ensure!(std::path::Path::new(&pdf).is_file(), "旧 PDF 产物缺失，未回退");
        }
        validate_pages(&tx, &previous, &file_id, &ir, &md, &hash)?;
        tx.execute("UPDATE document_processing_jobs SET status='cancelled',phase='storage_upgrade_reverted' WHERE id=?1",[&job_id])?;
        tx.execute("UPDATE case_files SET document_ir_path=?1,ocr_markdown_path=?2 WHERE id=?3",rusqlite::params![ir,md,file_id])?;
        let previous=previous.to_owned();tx.commit()?;Ok(previous)
    }).await
}
