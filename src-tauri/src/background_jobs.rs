//! Durable single-flight document processing worker.
use log::{error, info};
use rusqlite::OptionalExtension;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
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
    // R-02：逐页流式落库，内存中只有一页
    let ir_path = std::path::PathBuf::from(&result.page_ir_path);
    let mut plain_text = String::new();
    crate::document_pipeline::stream_disk_pages(&ir_path, |index, page| {
        tx.execute("INSERT INTO document_pages(job_id,file_id,page_number,width,height,plain_text,markdown,regions_json,confidence,layout_json,timing_json) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",rusqlite::params![job.id,job.file_id,page.page_number,page.width,page.height,page.plain_text,page.markdown,serde_json::to_string(&page.regions)?,page.confidence,page.layout.as_ref().map(serde_json::to_string).transpose()?,page.timing.as_ref().map(serde_json::to_string).transpose()?])?;
        if index > 0 { plain_text.push_str("\n\n"); }
        plain_text.push_str(&page.plain_text);
        Ok(())
    })?;
    tx.execute("UPDATE document_processing_jobs SET status='completed',phase='completed',elapsed_ms=?1,remaining_ms=0,index_status='running',index_error=NULL,engine=?2,model_version=?3,current_page=?4,total_pages=?4,progress=1,searchable_pdf_path=?5,page_ir_path=?6,markdown_path=?7,error_code=NULL,error_message=NULL,completed_at=datetime('now','localtime'),updated_at=datetime('now','localtime') WHERE id=?8",rusqlite::params![result.elapsed_ms,result.engine,result.model_version,result.page_count as i64,result.searchable_pdf_path,result.page_ir_path,result.markdown_path,job.id])?;
    tx.execute("UPDATE case_files SET source_sha256=?1,searchable_pdf_path=?2,document_ir_path=?3,ocr_markdown_path=?4,ocr_engine=?5,ocr_error=NULL,ocr_status='completed',index_status='processing' WHERE id=?6",rusqlite::params![job.source_sha256,result.searchable_pdf_path,result.page_ir_path,result.markdown_path,result.engine,job.file_id])?;
    tx.execute(
        "UPDATE case_files SET ocr_text=?1,updated_at=datetime('now','localtime') WHERE id=?2",
        rusqlite::params![plain_text, job.file_id],
    )?;
    // R-06：完整产物清单与结果摘要同事务落库——窗口断开、重启、重试都以此为准。
    let outputs = serde_json::json!({
        "pageIrPath": result.page_ir_path,
        "markdownPath": result.markdown_path,
        "searchablePdfPath": result.searchable_pdf_path,
        "sourceMapPath": result.source_map_path,
    });
    let markdown_sha256 = crate::document_pipeline::sha256_file(std::path::Path::new(&result.markdown_path)).ok();
    tx.execute(
        "INSERT INTO document_job_results(job_id,file_id,source_sha256,page_count,engine,model_version,outputs_json,markdown_sha256,created_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,datetime('now','localtime'))
         ON CONFLICT(job_id) DO UPDATE SET page_count=excluded.page_count, engine=excluded.engine,
           model_version=excluded.model_version, outputs_json=excluded.outputs_json,
           markdown_sha256=excluded.markdown_sha256",
        rusqlite::params![
            job.id,
            job.file_id,
            job.source_sha256,
            result.page_count as i64,
            result.engine,
            result.model_version,
            serde_json::to_string(&outputs)?,
            markdown_sha256,
        ],
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
    // S4/P0-4：失败即回收自己的产物目录；仍被同一 (file_id, source_sha256) 的
    // 已完成任务引用时保留（校订/重试会复用同一源哈希下的产物）。
    remove_unreferenced_artifact(&job.id, &job.file_id, &job.source_sha256);
}

/// 产物目录是否仍被同一 (file_id, source_sha256) 的**其他**已完成任务引用。
/// 只要还有一个已完成任务在读这套产物（source.md / assets / 可搜索 PDF），就不能删。
fn artifact_referenced_by_completed(
    conn: &rusqlite::Connection,
    job_id: &str,
    file_id: &str,
    source_sha256: &str,
) -> anyhow::Result<bool> {
    let referenced: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM document_processing_jobs WHERE file_id=?1 AND source_sha256=?2 AND status='completed' AND id<>?3)",
        rusqlite::params![file_id, source_sha256, job_id],
        |row| row.get(0),
    )?;
    Ok(referenced)
}

/// 同一任务的产物可能落在两种布局下（都以任务 id 或文件 id 开头）：
/// `document-artifacts/<job_id>/<sha>/`（后台 worker）与
/// `document-artifacts/<file_id>/<sha>/<job_id>/`（区域校订、存储升级）。
fn artifact_candidates(job_id: &str, file_id: &str, source_sha256: &str) -> Vec<PathBuf> {
    [
        crate::document_pipeline::artifact_dir_path(job_id, source_sha256),
        crate::document_pipeline::artifact_dir_path(file_id, source_sha256)
            .map(|base| base.join(job_id)),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// 回收未被已完成任务引用的产物目录（S4/P0-4）。全程 best-effort：
/// 无法判定引用关系时保守保留，永不删除已完成任务的产物。
pub(crate) fn remove_unreferenced_artifact(job_id: &str, file_id: &str, source_sha256: &str) {
    let referenced = match crate::db::open_db() {
        Ok(conn) => artifact_referenced_by_completed(&conn, job_id, file_id, source_sha256)
            .unwrap_or(true),
        Err(error) => {
            log::warn!("跳过产物回收（无法读取任务状态）: {error}");
            true
        }
    };
    if referenced {
        return;
    }
    for path in artifact_candidates(job_id, file_id, source_sha256) {
        remove_artifact_dir(&path);
    }
}

/// 删除单个产物目录；NotFound 视为正常（从未产出或已回收）。
fn remove_artifact_dir(path: &Path) {
    match std::fs::remove_dir_all(path) {
        Ok(()) => info!("已回收文档产物目录: {}", path.display()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => log::warn!("文档产物目录回收失败 {}: {error}", path.display()),
    }
}

/// 启动扫描用的任务索引：id → (file_id, source_sha256, status)，外加所有已完成引用。
struct ArtifactJobIndex {
    jobs: HashMap<String, (String, String, String)>,
    file_ids: HashSet<String>,
    completed: HashSet<(String, String)>,
}

impl ArtifactJobIndex {
    fn load(conn: &rusqlite::Connection) -> anyhow::Result<Self> {
        let mut stmt = conn
            .prepare("SELECT id,file_id,source_sha256,status FROM document_processing_jobs")?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut jobs = HashMap::new();
        let mut file_ids = HashSet::new();
        let mut completed = HashSet::new();
        for (id, file_id, source_sha256, status) in rows {
            if status == "completed" {
                completed.insert((file_id.clone(), source_sha256.clone()));
            }
            file_ids.insert(file_id.clone());
            jobs.insert(id, (file_id, source_sha256, status));
        }
        Ok(Self {
            jobs,
            file_ids,
            completed,
        })
    }

    fn is_file_id(&self, file_id: &str) -> bool {
        self.file_ids.contains(file_id)
    }

    /// 只有"已失败/已取消"且"没有其他已完成任务引用磁盘上这套产物"时才可回收。
    /// queued/running/completed 一律保留。
    fn recyclable(&self, job_id: &str, sha_on_disk: &str) -> bool {
        let Some((file_id, _, status)) = self.jobs.get(job_id) else {
            return false;
        };
        if status != "failed" && status != "cancelled" {
            return false;
        }
        !self
            .completed
            .contains(&(file_id.clone(), sha_on_disk.to_string()))
    }
}

/// 一级目录下的子目录列表。
fn subdirs(path: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(path) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect()
}

/// 启动扫描（S4/P0-4）：删除任务行已不存在、或任务已失败/已取消且无已完成任务引用的产物目录。
/// 已完成与 queued/running 任务的产物一律保留；无法归属到任务 id 的目录（如存储升级的
/// `asset-upgrade-*` 暂存目录）保守保留，避免误删被 completed job 引用的产物。
fn scan_orphan_artifacts() {
    let conn = match crate::db::open_db() {
        Ok(conn) => conn,
        Err(error) => {
            log::warn!("跳过文档产物回收扫描: {error}");
            return;
        }
    };
    let index = match ArtifactJobIndex::load(&conn) {
        Ok(index) => index,
        Err(error) => {
            log::warn!("跳过文档产物回收扫描（任务索引读取失败）: {error}");
            return;
        }
    };
    let Some(base) = crate::db::get_db_path()
        .parent()
        .map(|parent| parent.join("document-artifacts"))
    else {
        return;
    };
    let jobs = match std::fs::read_dir(&base) {
        Ok(jobs) => jobs,
        // 从未产生过产物属正常，不是错误
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => {
            log::warn!("文档产物目录扫描失败 {}: {error}", base.display());
            return;
        }
    };
    for entry in jobs.flatten() {
        let level1 = entry.path();
        if !level1.is_dir() {
            continue;
        }
        let name1 = entry.file_name().to_string_lossy().into_owned();
        // 布局 A：document-artifacts/<job_id>/<sha256>/
        if index.jobs.contains_key(&name1) {
            for sha in subdirs(&level1) {
                let sha_name = sha
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if index.recyclable(&name1, &sha_name) {
                    remove_artifact_dir(&sha);
                }
            }
            continue;
        }
        // 布局 B：document-artifacts/<file_id>/<sha256>/<job_id>/
        if index.is_file_id(&name1) {
            for sha in subdirs(&level1) {
                let sha_name = sha
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                for job in subdirs(&sha) {
                    let job_name = job
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    if index.recyclable(&job_name, &sha_name) {
                        remove_artifact_dir(&job);
                    }
                }
            }
            continue;
        }
        // 既不是任务 id 也不是任何任务的文件 id：没有引用者的孤儿目录
        remove_artifact_dir(&level1);
    }
}

/// R-06 崩溃恢复：引擎已把产物写全（页 IR + Markdown）但任务被标记中断时，
/// 用落盘产物直接完成任务，不重跑 OCR。源文件哈希仍是权威校验。
fn try_recover_interrupted_job(conn: &rusqlite::Connection, job_id: &str, file_id: &str, sha: &str, source_path: &str) -> bool {
    let dir = match crate::document_pipeline::artifact_dir_path(job_id, sha) {
        Ok(dir) => dir,
        Err(_) => return false,
    };
    let markdown_path = dir.join("source.md");
    let ir_path = dir.join("source.document.json");
    if !markdown_path.is_file() || !ir_path.is_file() {
        return false;
    }
    // 源文件必须仍是同一份
    if crate::document_pipeline::sha256_file(std::path::Path::new(source_path))
        .map(|hash| hash != sha)
        .unwrap_or(true)
    {
        return false;
    }
    // 页 IR 必须可完整流式读取（顺带得到页数）
    let mut page_count = 0u32;
    if crate::document_pipeline::stream_disk_pages(&ir_path, |_i, _page| {
        page_count += 1;
        Ok(())
    })
    .is_err()
        || page_count == 0
    {
        return false;
    }
    let searchable = dir.join("source.searchable.pdf");
    let source_map = dir.join("source.map.json");
    let result = crate::document_pipeline::ProcessResult {
        source_sha256: sha.to_string(),
        engine: "recovered-after-interrupt".into(),
        model_version: None,
        searchable_pdf_path: searchable.is_file().then(|| searchable.display().to_string()),
        page_ir_path: ir_path.display().to_string(),
        markdown_path: markdown_path.display().to_string(),
        source_map_path: source_map.is_file().then(|| source_map.display().to_string()),
        page_count,
        elapsed_ms: 0,
    };
    // persist_success 要求任务处于 running：先归还运行态再落库（失败即保持中断态）
    if conn
        .execute(
            "UPDATE document_processing_jobs SET status='running',error_code=NULL,error_message=NULL WHERE id=?1 AND status='failed' AND error_code='INTERRUPTED'",
            [job_id],
        )
        .unwrap_or(0)
        != 1
    {
        return false;
    }
    let job = ClaimedJob {
        id: job_id.to_string(),
        file_id: file_id.to_string(),
        source_path: source_path.to_string(),
        source_sha256: sha.to_string(),
    };
    match persist_success(&job, &result) {
        Ok(()) => {
            log::info!("R-06 崩溃恢复：任务 {job_id} 产物已落盘，直接完成（未重跑 OCR）");
            true
        }
        Err(error) => {
            log::warn!("R-06 崩溃恢复失败 {job_id}: {error}");
            false
        }
    }
}

/// 启动时对中断任务尝试崩溃恢复（有界：每次启动最多 20 个）。
fn recover_interrupted_jobs() {
    let Ok(conn) = crate::db::open_db() else { return };
    let Ok(mut stmt) = conn.prepare(
        "SELECT j.id,j.file_id,j.source_sha256,f.file_path FROM document_processing_jobs j
         JOIN case_files f ON f.id=j.file_id
         WHERE j.status='failed' AND j.error_code='INTERRUPTED' AND f.deleted_at IS NULL
         ORDER BY j.created_at LIMIT 20",
    ) else { return };
    let Ok(rows) = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .and_then(|rows| rows.collect::<rusqlite::Result<Vec<_>>>())
    else {
        return;
    };
    for (job_id, file_id, sha, source_path) in rows {
        try_recover_interrupted_job(&conn, &job_id, &file_id, &sha, &source_path);
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
    let mut request = crate::document_pipeline::process_request(
        &job.id,
        &job.source_path,
        &job.source_sha256,
        &output_dir,
    );
    // R-06 断点续算：同一任务的产物目录里已有部分页 IR 时，从断点继续识别。
    let resumed = crate::document_pipeline::resume_page_count(&output_dir.join("source.document.json"));
    if resumed > 0 {
        log::info!("任务 {} 从第 {} 页续算（已落盘 {resumed} 页）", job.id, resumed + 1);
        request.resume_from = Some(resumed);
    }
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
                        let _ = conn.execute("UPDATE document_processing_jobs SET index_status='completed',updated_at=datetime('now','localtime') WHERE id=?1", [&job.id]);
                        let _ = conn.execute(
                            "UPDATE case_files SET index_status='completed' WHERE id=?1",
                            [&job.file_id],
                        );
                    }
                }
                Err(message) => {
                    error!("PageIndex build failed {}: {}", job.id, message);
                    if let Ok(conn) = crate::db::open_db() {
                        let _ = conn.execute("UPDATE document_processing_jobs SET index_status='failed',index_error=?2,updated_at=datetime('now','localtime') WHERE id=?1", rusqlite::params![job.id,message]);
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
    if let Ok(conn)=crate::db::open_db() {
        if let Err(error)=crate::processing::recover(&conn) { log::error!("处理中心恢复失败: {error}"); }
    }
    crate::processing::service("email","邮件监听","disabled","等待手动启用",None);
    crate::workspace_sync::start();
    tauri::async_runtime::spawn(async move {
        if let Ok(conn) = crate::db::open_db() {
            if let Err(error) = crate::db::knowledge_index::recover_interrupted(&conn) {
                error!("knowledge index recovery failed: {}", error);
                return;
            }
        }
        // Idle vector cache checks must back off; a fixed 3s loop caused perpetual
        // SQLite write transactions even when the queue and fingerprint were unchanged.
        let mut idle_secs: u64 = 3;
        loop {
            match crate::db::knowledge_index::process_next().await {
                Ok(true) => {
                    idle_secs = 3;
                }
                Ok(false) => {
                    let result = tokio::task::spawn_blocking(|| {
                        crate::processing::service("vectors","向量检索缓存","running","检查索引变化",None);
                        let result=crate::db::vector_index::prepare();
                        crate::processing::service("vectors","向量检索缓存",if result.is_ok(){"waiting"}else{"failed"},"空闲时退避检查；有变化立即重建",result.as_ref().err().map(ToString::to_string).as_deref());
                        result
                    }).await;
                    if let Ok(Err(error)) = result {
                        log::warn!("Vector cache synchronization failed: {error}");
                        tokio::time::sleep(Duration::from_secs(10)).await;
                        idle_secs = 3;
                        continue;
                    }
                    tokio::time::sleep(Duration::from_secs(idle_secs)).await;
                    idle_secs = (idle_secs * 2).min(60);
                }
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
        // R-06：先尝试用落盘产物恢复中断任务（此时 running 已被标记为 failed），
        // 再做 S4/P0-4 的产物回收——顺序反了会把可恢复的产物删掉。
        recover_interrupted_jobs();
        // S4/P0-4：此时上次异常退出遗留的 running 已被标记为 failed，可安全回收其产物
        scan_orphan_artifacts();
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

#[cfg(test)]
mod tests {
    use super::*;

    fn jobs_table() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE document_processing_jobs(id TEXT,file_id TEXT,source_sha256 TEXT,status TEXT)",
        )
        .unwrap();
        conn
    }

    #[test]
    fn completed_job_of_the_same_hash_keeps_artifacts() {
        let conn = jobs_table();
        conn.execute(
            "INSERT INTO document_processing_jobs VALUES('older','f','sha','completed')",
            [],
        )
        .unwrap();
        // 同一 (file_id, sha) 已有完成任务在读这套产物 → 失败任务不得回收
        assert!(artifact_referenced_by_completed(&conn, "newer", "f", "sha").unwrap());
    }

    #[test]
    fn failed_or_other_hash_jobs_do_not_keep_artifacts() {
        let conn = jobs_table();
        conn.execute(
            "INSERT INTO document_processing_jobs VALUES('other','f','sha','failed')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO document_processing_jobs VALUES('other-file','f2','sha','completed')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO document_processing_jobs VALUES('other-hash','f','other','completed')",
            [],
        )
        .unwrap();
        assert!(!artifact_referenced_by_completed(&conn, "newer", "f", "sha").unwrap());
    }

    #[test]
    fn the_job_itself_is_never_counted_as_a_reference() {
        let conn = jobs_table();
        conn.execute(
            "INSERT INTO document_processing_jobs VALUES('self','f','sha','completed')",
            [],
        )
        .unwrap();
        assert!(!artifact_referenced_by_completed(&conn, "self", "f", "sha").unwrap());
    }

    fn index_with(rows: &[(&str, &str, &str, &str)]) -> ArtifactJobIndex {
        let conn = jobs_table();
        for &(id, file_id, sha, status) in rows.iter() {
            conn.execute(
                "INSERT INTO document_processing_jobs VALUES(?1,?2,?3,?4)",
                rusqlite::params![id, file_id, sha, status],
            )
            .unwrap();
        }
        ArtifactJobIndex::load(&conn).unwrap()
    }

    /// 启动扫描的回收判定：只有"已失败/已取消"且"无其他已完成任务引用同一 (file_id, sha)"才删。
    #[test]
    fn startup_scan_only_recycles_unreferenced_failed_or_cancelled_artifacts() {
        let index = index_with(&[
            ("done", "f", "sha", "completed"),
            ("failed", "f", "sha", "failed"),
            ("failed-other", "f", "other", "failed"),
            ("cancelled", "f", "sha3", "cancelled"),
            ("running", "f", "sha4", "running"),
            ("queued", "f2", "sha5", "queued"),
        ]);
        // 同一 (file_id, sha) 已有完成任务在读 → 保留
        assert!(!index.recyclable("failed", "sha"));
        // 无 completed 引用的失败/取消任务 → 回收
        assert!(index.recyclable("failed-other", "other"));
        assert!(index.recyclable("cancelled", "sha3"));
        // completed 永不回收；排队/运行中可能正在写，也保留
        assert!(!index.recyclable("done", "sha"));
        assert!(!index.recyclable("running", "sha4"));
        assert!(!index.recyclable("queued", "sha5"));
        // 归属不到任务 id 的目录不走这条判定（由孤儿分支处理）
        assert!(!index.recyclable("missing", "sha"));
        assert!(index.is_file_id("f") && index.is_file_id("f2") && !index.is_file_id("nope"));
    }
}
