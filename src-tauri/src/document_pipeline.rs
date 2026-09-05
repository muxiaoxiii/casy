use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    pub ovis_model_dir: Option<String>,
    pub paddle_model_dir: Option<String>,
    pub cjk_font_path: Option<String>,
    pub device: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessResult {
    pub source_sha256: String,
    pub engine: String,
    pub model_version: Option<String>,
    pub searchable_pdf_path: String,
    pub page_ir_path: String,
    pub markdown_path: String,
    pub pages: Vec<DocumentPage>,
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
    std::env::var_os("CASY_DOC_ENGINE")
        .map(PathBuf::from)
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
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
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
    let executable = engine_executable().ok_or_else(|| {
        anyhow!("DOC_ENGINE_NOT_FOUND: 请设置 CASY_DOC_ENGINE 或安装 casy-doc-engine")
    })?;
    let mut child = tokio::process::Command::new(executable)
        .arg("process")
        .env("RAYON_NUM_THREADS", "2")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    use tokio::io::AsyncWriteExt;
    child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("无法打开文档引擎输入"))?
        .write_all(&serde_json::to_vec(&request)?)
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
    let mut last_progress = std::time::Instant::now();
    let mut current_page = 0;
    let output = loop {
        tokio::select! {
            result = &mut wait => break result?,
            _ = interval.tick() => {
                let conn = crate::db::open_db()?;
                let running: bool = conn.query_row(
                    "SELECT status='running' FROM document_processing_jobs WHERE id=?1",
                    [&request.job_id], |row| row.get(0),
                ).unwrap_or(false);
                if !running { return Err(anyhow!("CANCELLED: 文档任务已取消")); }
                if let Ok(bytes) = std::fs::read(Path::new(&request.output_dir).join("progress.json")) {
                    if let Ok(progress) = serde_json::from_slice::<EngineProgress>(&bytes) {
                        if progress.current_page > current_page && progress.current_page <= progress.total_pages {
                            current_page = progress.current_page;
                            last_progress = std::time::Instant::now();
                        }
                        if progress.total_pages > 0 && progress.current_page <= progress.total_pages {
                            conn.execute("UPDATE document_processing_jobs SET current_page=?1,total_pages=?2,progress=?3,updated_at=datetime('now','localtime') WHERE id=?4 AND status='running'",
                                rusqlite::params![progress.current_page,progress.total_pages,0.01 + 0.94 * progress.current_page as f64 / progress.total_pages as f64,request.job_id])?;
                        }
                    }
                }
                if last_progress.elapsed() > std::time::Duration::from_secs(900) {
                    return Err(anyhow!("DOC_ENGINE_TIMEOUT: 单页处理超过 15 分钟，已终止任务"));
                }
            }
        }
    };
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
    for path in [
        &result.searchable_pdf_path,
        &result.page_ir_path,
        &result.markdown_path,
    ] {
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
            || w <= 0.0
            || h <= 0.0
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
    let expected_md = result
        .pages
        .iter()
        .map(|p| format!("<!-- page {} -->\n{}", p.page_number, p.markdown))
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");
    if std::fs::read_to_string(&result.markdown_path)? != expected_md {
        return Err(anyhow!("INVALID_MARKDOWN: Markdown 备份与页面不一致"));
    }
    let mut header = [0u8; 5];
    std::fs::File::open(&result.searchable_pdf_path)?.read_exact(&mut header)?;
    if &header != b"%PDF-" {
        return Err(anyhow!("INVALID_PDF: 可搜索文件不是 PDF"));
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
        ovis_model_dir: env_path("CASY_OVISOCR2_MODEL_DIR"),
        paddle_model_dir: env_path("CASY_PADDLEOCR_VL_MODEL_DIR"),
        cjk_font_path: env_path("CASY_OCR_FONT"),
        device: std::env::var("CASY_OCR_DEVICE").unwrap_or_else(|_| "cpu".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            searchable_pdf_path: root.join("search.pdf").display().to_string(),
            page_ir_path: root.join("pages.json").display().to_string(),
            markdown_path: root.join("source.md").display().to_string(),
            pages,
        };
        std::fs::write(&result.searchable_pdf_path, b"%PDF-derived").unwrap();
        std::fs::write(
            &result.page_ir_path,
            serde_json::to_vec(&result.pages).unwrap(),
        )
        .unwrap();
        std::fs::write(&result.markdown_path, "<!-- page 1 -->\n# Evidence").unwrap();
        validate_result(&request, &result).unwrap();
        result.pages[0].page_number = 2;
        assert!(validate_result(&request, &result).is_err());
        result.pages[0].page_number = 1;
        let pdf = result.searchable_pdf_path.clone();
        result.searchable_pdf_path = source.display().to_string();
        assert!(validate_result(&request, &result).is_err());
        result.searchable_pdf_path = pdf;
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
