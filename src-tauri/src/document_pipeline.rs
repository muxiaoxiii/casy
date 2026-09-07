use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use DocumentPage as Page;
#[path = "../../tools/casy-doc-engine/src/source_map.rs"]
mod source_map;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DocumentEngineStatus {
    pub available: bool,
    pub executable: Option<String>,
    pub version: Option<String>,
    pub renderer_available: bool,
    pub coordinate_model_available: bool,
    pub ovis_model_available: bool,
    pub searchable_pdf_available: bool,
    pub missing: Vec<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DocumentRegion {
    pub text: String,
    pub bbox: [f32; 4],
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentPage {
    pub page_number: u32,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub plain_text: String,
    pub markdown: String,
    #[serde(default)]
    pub regions: Vec<DocumentRegion>,
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessRequest {
    pub job_id: String,
    pub source_path: String,
    pub source_sha256: String,
    pub output_dir: String,
    pub coordinate_model_dir: Option<String>,
    pub cjk_font_path: Option<String>,
    #[serde(default)]
    pub markdown_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessResult {
    pub source_sha256: String,
    pub engine: String,
    pub model_version: Option<String>,
    pub searchable_pdf_path: Option<String>,
    pub page_ir_path: String,
    pub markdown_path: String,
    #[serde(default)]
    pub source_map_path: Option<String>,
    pub pages: Vec<DocumentPage>,
}

pub fn emit_conversion_progress(job_id: &str, source_path: &str, phase: &str, current: u32, total: u32, elapsed: f64) {
    use tauri::Emitter;
    let remaining = (current > 0 && current < total)
        .then(|| elapsed / current as f64 * (total - current) as f64);
    if let Some(app) = crate::get_app_handle() {
        let _ = app.emit("document-conversion-progress", serde_json::json!({
            "jobId": job_id, "sourcePath": source_path, "phase": phase,
            "currentPage": current, "totalPages": total,
            "elapsedSeconds": elapsed, "remainingSeconds": remaining,
        }));
    }
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file =
        std::fs::File::open(path).with_context(|| format!("无法读取源文件: {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

pub fn engine_executable() -> Option<PathBuf> {
    crate::runtime_paths::runtime_asset("CASY_DOC_ENGINE", if cfg!(windows) { "bin/casy-doc-engine.exe" } else { "bin/casy-doc-engine" })
        .filter(|path| path.is_file())
        .or_else(|| {
            let candidate = crate::db::get_db_path()
                .parent()?
                .join("bin")
                .join(if cfg!(windows) {
                    "casy-doc-engine.exe"
                } else {
                    "casy-doc-engine"
                });
            candidate.is_file().then_some(candidate)
        })
}

pub async fn probe_engine() -> DocumentEngineStatus {
    let executable = match engine_executable() {
        Some(path) => path,
        None => {
            return DocumentEngineStatus {
                available: false,
                executable: None,
                version: None,
                renderer_available: false,
                coordinate_model_available: false,
                ovis_model_available: false,
                searchable_pdf_available: false,
                missing: vec!["casy-doc-engine".into()],
                error: Some("未找到文档引擎；请设置 CASY_DOC_ENGINE 或安装到 Casy/bin".into()),
            }
        }
    };
    let mut command = tokio::process::Command::new(&executable);
    command
        .arg("probe")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let output = tokio::time::timeout(std::time::Duration::from_secs(15), command.output())
        .await
        .unwrap_or_else(|_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "文档引擎探测超时",
            ))
        });
    match output {
        Ok(output) if output.status.success() => serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|error| DocumentEngineStatus {
                available: false,
                executable: Some(executable.display().to_string()),
                version: None,
                renderer_available: false,
                coordinate_model_available: false,
                ovis_model_available: false,
                searchable_pdf_available: false,
                missing: vec![],
                error: Some(format!("文档引擎返回了无效 JSON: {error}")),
            }),
        Ok(output) => DocumentEngineStatus {
            available: false,
            executable: Some(executable.display().to_string()),
            version: None,
            renderer_available: false,
            coordinate_model_available: false,
            ovis_model_available: false,
            searchable_pdf_available: false,
            missing: vec![],
            error: Some(String::from_utf8_lossy(&output.stderr).trim().to_string()),
        },
        Err(error) => DocumentEngineStatus {
            available: false,
            executable: Some(executable.display().to_string()),
            version: None,
            renderer_available: false,
            coordinate_model_available: false,
            ovis_model_available: false,
            searchable_pdf_available: false,
            missing: vec![],
            error: Some(error.to_string()),
        },
    }
}

fn env_path(name: &str) -> Option<String> {
    let relative = match name {
        "CASY_PPOCR_MODEL_DIR" => "models/ppocrv6-medium",
        "CASY_OCR_FONT" => "fonts/NotoSansCJK-Regular.ttf",
        _ => return None,
    };
    crate::runtime_paths::runtime_asset(name, relative).map(|p| p.to_string_lossy().into_owned())
}

pub fn artifact_dir(file_id: &str, sha256: &str) -> Result<PathBuf> {
    if file_id.is_empty()
        || !file_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        || sha256.len() != 64
        || !sha256.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(anyhow!("INVALID_ARTIFACT_ID: 文档任务标识或哈希无效"));
    }
    let base = crate::db::get_db_path()
        .parent()
        .ok_or_else(|| anyhow!("数据库目录不可用"))?
        .join("document-artifacts")
        .join(file_id)
        .join(sha256);
    std::fs::create_dir_all(&base)?;
    Ok(base)
}

pub async fn run_engine(request: ProcessRequest) -> Result<ProcessResult> {
    run_processing(request, true).await
}

/// Standalone conversion has no document_processing_jobs row or database cancellation owner.
pub async fn run_standalone_engine(mut request: ProcessRequest) -> Result<ProcessResult> {
    request.markdown_only = true;
    run_processing(request, false).await
}

async fn run_processing(request: ProcessRequest, tracked: bool) -> Result<ProcessResult> {
    if crate::parse::text_document::supports(Path::new(&request.source_path)) {
        return tokio::task::spawn_blocking(move || {
            let result = crate::parse::text_document::process(&request)?;
            validate_result(&request, &result)?;
            Ok(result)
        })
        .await?;
    }
    let payload = serde_json::to_vec(&request)?;
    run_engine_command(request, "process", payload, tracked).await
}

pub async fn run_revision(request: ProcessRequest, pages: Vec<DocumentPage>) -> Result<ProcessResult> {
    let payload = serde_json::to_vec(&serde_json::json!({"request":request,"pages":pages}))?;
    run_engine_command(request, "revise", payload, true).await
}

async fn run_engine_command(request: ProcessRequest, command: &str, payload: Vec<u8>, tracked: bool) -> Result<ProcessResult> {
    let executable = engine_executable().ok_or_else(|| {
        anyhow!("DOC_ENGINE_NOT_FOUND: 请设置 CASY_DOC_ENGINE 或安装 casy-doc-engine")
    })?;
    execute_engine(request, command, payload, tracked, &executable).await
}

async fn execute_engine(request: ProcessRequest, command: &str, payload: Vec<u8>, tracked: bool, executable: &Path) -> Result<ProcessResult> {
    tracing::info!(job_id = %request.job_id, %command, tracked, executable = %executable.display(), "Starting document engine");
    let mut child = tokio::process::Command::new(executable)
        .arg(command)
        .env("RAYON_NUM_THREADS", "2")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn().context("DOC_ENGINE_START_FAILED: 无法启动文档引擎")?;
    tracing::info!(job_id = %request.job_id, pid = ?child.id(), "Document engine started");
    use tokio::io::AsyncWriteExt;
    child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("无法打开文档引擎输入"))?
        .write_all(&payload)
        .await?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("文档引擎输出不可用"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("文档引擎诊断不可用"))?;
    let wait = async {
        let (status, stdout, stderr) = tokio::try_join!(
            async { child.wait().await.map_err(anyhow::Error::from) },
            read_bounded(stdout, 128 * 1024 * 1024),
            read_bounded(stderr, 1024 * 1024),
        )?;
        Ok::<_, anyhow::Error>((status, stdout, stderr))
    };
    tokio::pin!(wait);
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
    let started = std::time::Instant::now();
    let mut last_progress = std::time::Instant::now();
    let mut current_page = 0;
    let output = loop {
        tokio::select! {
            result = &mut wait => break result?,
            _ = interval.tick() => {
                let conn = if tracked {
                    let conn = crate::db::open_db()?;
                    check_job_running(&conn, &request.job_id)?;
                    Some(conn)
                } else { None };
                if let Ok(bytes) = std::fs::read(Path::new(&request.output_dir).join("progress.json")) {
                    if let Ok(progress) = serde_json::from_slice::<EngineProgress>(&bytes) {
                        if progress.current_page > current_page && progress.current_page <= progress.total_pages {
                            current_page = progress.current_page;
                            last_progress = std::time::Instant::now();
                            tracing::info!(job_id = %request.job_id, current_page, total_pages = progress.total_pages, "Document engine progress");
                        }
                        if progress.total_pages > 0 && progress.current_page <= progress.total_pages {
                            if !tracked {
                                let phase = if progress.current_page == progress.total_pages { "finalizing" } else { "recognizing" };
                                emit_conversion_progress(&request.job_id, &request.source_path, phase, progress.current_page, progress.total_pages, started.elapsed().as_secs_f64());
                            }
                            if let Some(conn) = &conn { conn.execute("UPDATE document_processing_jobs SET current_page=?1,total_pages=?2,progress=?3,updated_at=datetime('now','localtime') WHERE id=?4 AND status='running'",
                                rusqlite::params![progress.current_page,progress.total_pages,0.01 + 0.94 * progress.current_page as f64 / progress.total_pages as f64,request.job_id])?;
                            }
                        }
                    }
                }
                if last_progress.elapsed() > std::time::Duration::from_secs(900) {
                    return Err(anyhow!("DOC_ENGINE_TIMEOUT: 单页处理超过 15 分钟，已终止任务"));
                }
            }
        }
    };
    tracing::info!(job_id = %request.job_id, status = %output.0, "Document engine exited");
    if !output.0.success() {
        return Err(anyhow!(
            "DOC_ENGINE_FAILED: {}",
            String::from_utf8_lossy(&output.2).trim()
        ));
    }
    let result: ProcessResult =
        serde_json::from_slice(&output.1).context("文档引擎结果不是有效 JSON")?;
    validate_result(&request, &result)?;
    Ok(result)
}

fn check_job_running(conn: &rusqlite::Connection, job_id: &str) -> Result<()> {
    use rusqlite::OptionalExtension;
    let status: Option<String> = conn.query_row(
        "SELECT status FROM document_processing_jobs WHERE id=?1", [job_id], |row| row.get(0)
    ).optional().context("DOC_JOB_STATE_FAILED: 无法读取文档任务状态")?;
    match status.as_deref() {
        Some("running") => Ok(()),
        Some("cancelled") => {
            tracing::info!(%job_id, "Document engine cancelled by persisted job state");
            Err(anyhow!("CANCELLED: 文档任务已取消"))
        }
        other => {
            tracing::warn!(%job_id, status = ?other, "Document engine stopped after job state changed");
            Err(anyhow!("DOC_JOB_STATE_CHANGED: 文档任务不存在或已停止运行"))
        }
    }
}

async fn read_bounded(reader: impl tokio::io::AsyncRead + Unpin, limit: usize) -> Result<Vec<u8>> {
    use tokio::io::AsyncReadExt;
    let mut bytes = Vec::new();
    reader
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .await?;
    if bytes.len() > limit {
        return Err(anyhow!("DOC_OUTPUT_LIMIT: 文档引擎输出超过资源上限"));
    }
    Ok(bytes)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EngineProgress {
    current_page: u32,
    total_pages: u32,
}

pub fn supports_path(path: &Path) -> bool {
    if crate::parse::text_document::supports(path) {
        return true;
    }
    matches!(
        path.extension()
            .and_then(|s| s.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("pdf" | "png" | "jpg" | "jpeg" | "tif" | "tiff" | "bmp" | "webp" | "gif")
    )
}

fn validate_result(request: &ProcessRequest, result: &ProcessResult) -> Result<()> {
    if result.source_sha256 != request.source_sha256
        || sha256_file(Path::new(&request.source_path))? != request.source_sha256
    {
        return Err(anyhow!("SOURCE_CHANGED: 处理期间源文件哈希发生变化"));
    }
    let root = std::fs::canonicalize(&request.output_dir)?;
    let source = std::fs::canonicalize(&request.source_path)?;
    let text_document = crate::parse::text_document::supports(Path::new(&request.source_path));
    if !text_document && !request.markdown_only && result.searchable_pdf_path.is_none() {
        return Err(anyhow!("INVALID_PDF: 缺少可搜索 PDF"));
    }
    for path in [&result.page_ir_path, &result.markdown_path]
        .into_iter()
        .chain(result.searchable_pdf_path.iter())
        .chain(result.source_map_path.iter())
    {
        let resolved = std::fs::canonicalize(path).context("ARTIFACT_MISSING: 文档产物缺失")?;
        if !resolved.starts_with(&root) || resolved == source || !resolved.is_file() {
            return Err(anyhow!("INVALID_ARTIFACT: 产物不在任务输出目录"));
        }
    }
    if result.engine.trim().is_empty() || result.pages.is_empty() {
        return Err(anyhow!("INVALID_PAGE_IR: 引擎未返回页面"));
    }
    for (index, page) in result.pages.iter().enumerate() {
        let w = page.width.unwrap_or(0.0);
        let h = page.height.unwrap_or(0.0);
        if page.page_number as usize != index + 1
            || !w.is_finite()
            || !h.is_finite()
            || (!text_document && (w <= 0.0 || h <= 0.0))
            || (text_document
                && (page.width.is_some() || page.height.is_some() || !page.regions.is_empty()))
            || page
                .confidence
                .is_some_and(|v| !v.is_finite() || !(0.0..=1.0).contains(&v))
        {
            return Err(anyhow!("INVALID_PAGE_IR: 页码或页面尺寸无效"));
        }
        for region in &page.regions {
            let [x0, y0, x1, y1] = region.bbox;
            if !region.bbox.iter().all(|v| v.is_finite())
                || x0 < 0.0
                || y0 < 0.0
                || x1 < x0
                || y1 < y0
                || x1 > w + 1.0
                || y1 > h + 1.0
                || region
                    .confidence
                    .is_some_and(|v| !v.is_finite() || !(0.0..=1.0).contains(&v))
            {
                return Err(anyhow!("INVALID_PAGE_IR: 文字区域坐标或置信度无效"));
            }
        }
    }
    let disk_pages: Vec<DocumentPage> =
        serde_json::from_slice(&std::fs::read(&result.page_ir_path)?)?;
    if disk_pages != result.pages {
        return Err(anyhow!("INVALID_PAGE_IR: 落盘页面与返回数据不一致"));
    }
    let expected_md = if text_document {
        result
            .pages
            .iter()
            .map(|p| p.markdown.as_str())
            .collect::<String>()
    } else {
        result
            .pages
            .iter()
            .map(|p| format!("<!-- page {} -->\n{}", p.page_number, p.markdown))
            .collect::<Vec<_>>()
            .join("\n\n---\n\n")
    };
    if std::fs::read_to_string(&result.markdown_path)? != expected_md {
        return Err(anyhow!("INVALID_MARKDOWN: Markdown 备份与页面不一致"));
    }
    if result.engine.starts_with("paddle-onnx-") && result.source_map_path.is_none() {
        return Err(anyhow!("INVALID_SOURCE_MAP: 缺少来源映射"));
    }
    if let Some(path) = &result.source_map_path {
        let map: source_map::SourceMap = serde_json::from_slice(&std::fs::read(path)?)?;
        let (_, expected) = source_map::build(&result.pages, &result.source_sha256);
        if map != expected {
            return Err(anyhow!(
                "INVALID_SOURCE_MAP: 来源映射与页面或 Markdown 不一致"
            ));
        }
    }
    if let Some(pdf) = &result.searchable_pdf_path {
        let mut header = [0u8; 5];
        std::fs::File::open(pdf)?.read_exact(&mut header)?;
        if &header != b"%PDF-" {
            return Err(anyhow!("INVALID_PDF: 可搜索文件不是 PDF"));
        }
    }
    Ok(())
}

pub fn process_request(
    job_id: &str,
    source_path: &str,
    sha256: &str,
    output_dir: &Path,
) -> ProcessRequest {
    ProcessRequest {
        job_id: job_id.into(),
        source_path: source_path.into(),
        source_sha256: sha256.into(),
        output_dir: output_dir.display().to_string(),
        coordinate_model_dir: env_path("CASY_PPOCR_MODEL_DIR"),
        cjk_font_path: env_path("CASY_OCR_FONT"),
        markdown_only: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[cfg(unix)]
    async fn standalone_engine_waits_for_process_instead_of_cancelling_missing_job() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let executable = root.path().join("test-engine");
        std::fs::write(&executable, "#!/bin/sh\ncat >/dev/null\nsleep 0.2\nprintf 'synthetic engine failure' >&2\nexit 7\n").unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        let request = process_request("unregistered-standalone", "/synthetic.pdf", "hash", root.path());
        let error = execute_engine(request, "process", b"{}".to_vec(), false, &executable).await.unwrap_err();
        assert!(error.to_string().contains("DOC_ENGINE_FAILED: synthetic engine failure"), "{error:#}");
    }

    #[test]
    fn job_state_distinguishes_cancellation_from_missing_rows_and_sql_errors() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        assert!(check_job_running(&conn, "job").unwrap_err().to_string().contains("DOC_JOB_STATE_FAILED"));
        conn.execute_batch("CREATE TABLE document_processing_jobs(id TEXT,status TEXT); INSERT INTO document_processing_jobs VALUES('job','running')").unwrap();
        check_job_running(&conn, "job").unwrap();
        assert!(check_job_running(&conn, "missing").unwrap_err().to_string().contains("DOC_JOB_STATE_CHANGED"));
        conn.execute("UPDATE document_processing_jobs SET status='cancelled'", []).unwrap();
        assert!(check_job_running(&conn, "job").unwrap_err().to_string().starts_with("CANCELLED:"));
    }

    #[test]
    fn rejects_path_traversal_and_invalid_hashes() {
        assert!(artifact_dir("../outside", &"a".repeat(64)).is_err());
        assert!(artifact_dir("job", "bad").is_err());
    }

    #[test]
    fn verifies_source_artifact_containment_and_page_backups() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source.pdf");
        std::fs::write(&source, b"%PDF-source").unwrap();
        let root = temp.path().join("artifacts");
        std::fs::create_dir(&root).unwrap();
        let request = process_request(
            "j",
            source.to_str().unwrap(),
            &sha256_file(&source).unwrap(),
            &root,
        );
        let pages = vec![DocumentPage {
            page_number: 1,
            width: Some(100.0),
            height: Some(100.0),
            plain_text: "Evidence".into(),
            markdown: "# Evidence".into(),
            regions: vec![],
            confidence: None,
        }];
        let mut result = ProcessResult {
            source_sha256: request.source_sha256.clone(),
            engine: "test".into(),
            model_version: None,
            searchable_pdf_path: Some(root.join("search.pdf").display().to_string()),
            page_ir_path: root.join("pages.json").display().to_string(),
            markdown_path: root.join("source.md").display().to_string(),
            source_map_path: None,
            pages,
        };
        std::fs::write(
            result.searchable_pdf_path.as_ref().unwrap(),
            b"%PDF-derived",
        )
        .unwrap();
        std::fs::write(
            &result.page_ir_path,
            serde_json::to_vec(&result.pages).unwrap(),
        )
        .unwrap();
        std::fs::write(&result.markdown_path, "<!-- page 1 -->\n# Evidence").unwrap();
        validate_result(&request, &result).unwrap();
        let (_, map) = source_map::build(&result.pages, &result.source_sha256);
        let map_path = root.join("source.map.json");
        std::fs::write(&map_path, serde_json::to_vec(&map).unwrap()).unwrap();
        result.source_map_path = Some(map_path.display().to_string());
        validate_result(&request, &result).unwrap();
        let mut corrupt = serde_json::to_value(&map).unwrap();
        corrupt["markdownSha256"] = serde_json::json!("wrong-hash");
        std::fs::write(&map_path, serde_json::to_vec(&corrupt).unwrap()).unwrap();
        assert!(validate_result(&request, &result)
            .unwrap_err()
            .to_string()
            .contains("INVALID_SOURCE_MAP"));
        std::fs::write(&map_path, serde_json::to_vec(&map).unwrap()).unwrap();
        result.pages[0].page_number = 2;
        assert!(validate_result(&request, &result).is_err());
        result.pages[0].page_number = 1;
        let pdf = result.searchable_pdf_path.clone();
        result.searchable_pdf_path = Some(source.display().to_string());
        assert!(validate_result(&request, &result).is_err());
        result.searchable_pdf_path = pdf;
        let mut markdown_request = request.clone();
        markdown_request.markdown_only = true;
        let mut markdown_result = result.clone();
        markdown_result.searchable_pdf_path = None;
        validate_result(&markdown_request, &markdown_result).unwrap();
        assert!(validate_result(&request, &markdown_result).is_err());
        std::fs::write(&result.markdown_path, "mismatched backup").unwrap();
        assert!(validate_result(&request, &result).is_err());
        std::fs::write(&result.markdown_path, "<!-- page 1 -->\n# Evidence").unwrap();
        std::fs::write(source, b"source changed during processing").unwrap();
        assert!(validate_result(&request, &result)
            .unwrap_err()
            .to_string()
            .contains("SOURCE_CHANGED"));
    }
}
