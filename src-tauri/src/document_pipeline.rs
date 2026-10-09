use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use DocumentPage as Page;
#[path = "../../tools/casy-doc-engine/src/source_map.rs"]
pub(crate) mod source_map;
#[path = "../../tools/casy-doc-engine/src/assets.rs"]
pub(crate) mod assets;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DocumentEngineStatus {
    pub available: bool,
    pub executable: Option<String>,
    pub version: Option<String>,
    pub renderer_available: bool,
    pub coordinate_model_available: bool,
    #[serde(default)]
    pub korean_model_available: bool,
    pub layout_model_available: bool,
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
    /// 词级框（引擎 PP-OCRv6 词框输出）：来源定位可细化到词；模型不支持时缺省。
    #[serde(default)]
    pub word_boxes: Option<Vec<[f32; 4]>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", default)]
pub struct DocumentPageTiming {
    pub render_ms: u64,
    pub ocr_ms: u64,
    pub layout_ms: u64,
    pub total_ms: u64,
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
    #[serde(default)]
    pub layout: Option<serde_json::Value>,
    #[serde(default)]
    pub timing: Option<DocumentPageTiming>,
    /// 页面级方向校正角（当前仅 180）：bbox/词框均在正置坐标系，
    /// 渲染页面图像时需同步旋转后再叠加高亮。
    #[serde(default)]
    pub orientation_degrees: Option<u16>,
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
    /// R-06 断点续算：页 IR 已有页数，从第 N+1 页继续识别（None/0 = 从头）。
    #[serde(default)]
    pub resume_from: Option<u32>,
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
    /// R-02：页面集合只落盘不回流；父进程经 stream_disk_pages 按需流式读取。
    #[serde(default)]
    pub page_count: u32,
    #[serde(default)]
    pub elapsed_ms: u64,
}

/// 逐页流式读取落盘页 IR（R-02）：任何时刻内存中只有一页。
/// 每轮遍历独立开文件；visit 返回 Err 即中断（错误信息原样透传）。
pub fn stream_disk_pages(
    path: &Path,
    mut visit: impl FnMut(usize, DocumentPage) -> Result<()>,
) -> Result<()> {
    struct PageStream<'a, F> {
        visit: &'a mut F,
        index: usize,
    }
    impl<'de, F: FnMut(usize, DocumentPage) -> Result<()>> serde::de::Visitor<'de> for PageStream<'_, F> {
        type Value = ();
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("page array")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(mut self, mut seq: A) -> std::result::Result<(), A::Error> {
            while let Some(page) = seq.next_element::<DocumentPage>()? {
                (self.visit)(self.index, page).map_err(serde::de::Error::custom)?;
                self.index += 1;
            }
            Ok(())
        }
    }
    let file = std::fs::File::open(path)
        .with_context(|| format!("INVALID_PAGE_IR: 无法读取页 IR {}", path.display()))?;
    use serde::Deserializer as _;
    let mut de = serde_json::Deserializer::from_reader(std::io::BufReader::new(file));
    de.deserialize_seq(PageStream { visit: &mut visit, index: 0 })?;
    de.end()?;
    Ok(())
}

/// R-06：页 IR 的续算起点页数。侧车标记（页数 字节偏移）优先——崩溃至多落后一页；
/// 无侧车时按完整数组计数（识别完成但 finalize 前崩溃的情形）；坏文件返回 0（从头识别）。
pub fn resume_page_count(ir_path: &Path) -> u32 {
    let mut sidecar = ir_path.as_os_str().to_os_string();
    sidecar.push(".resume");
    if let Ok(text) = std::fs::read_to_string(std::path::PathBuf::from(sidecar)) {
        if let Some(count) = text.split_whitespace().next().and_then(|v| v.parse::<u32>().ok()) {
            return count;
        }
    }
    count_disk_pages(ir_path)
}

/// 流式统计落盘页 IR 的页数（R-06 断点续算）；文件不存在返回 0。
pub fn count_disk_pages(path: &Path) -> u32 {
    let mut count = 0u32;
    if stream_disk_pages(path, |_index, _page| {
        count += 1;
        Ok(())
    })
    .is_err()
    {
        return 0;
    }
    count
}

pub fn emit_conversion_progress(job_id: &str, source_path: &str, phase: &str, current: u32, total: u32, elapsed: f64) {
    emit_progress_snapshot(job_id, source_path, phase, current, total, elapsed, None, None, true)
}

#[allow(clippy::too_many_arguments)]
fn emit_progress_snapshot(
    job_id: &str,
    source_path: &str,
    phase: &str,
    current: u32,
    total: u32,
    elapsed: f64,
    remaining: Option<f64>,
    page_timing: Option<&serde_json::Value>,
    persist_activity: bool,
) {
    use tauri::Emitter;
    let remaining = remaining.or_else(|| (current > 0 && current < total)
        .then(|| elapsed / current as f64 * (total - current) as f64));
    if let Some(app) = crate::get_app_handle() {
        if persist_activity {
            crate::processing::progress(
                job_id, phase, current, total, (elapsed * 1000.0) as u64,
                remaining.map(|value| (value * 1000.0) as u64), page_timing,
            );
        }
        let _ = app.emit("document-conversion-progress", serde_json::json!({
            "jobId": job_id, "sourcePath": source_path, "phase": phase,
            "currentPage": current, "totalPages": total,
            "elapsedSeconds": elapsed, "remainingSeconds": remaining,
            "pageTiming": page_timing,
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
                korean_model_available: false,
                layout_model_available: false,
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
                korean_model_available: false,
                layout_model_available: false,
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
            korean_model_available: false,
            layout_model_available: false,
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
            korean_model_available: false,
            layout_model_available: false,
            ovis_model_available: false,
            searchable_pdf_available: false,
            missing: vec![],
            error: Some(error.to_string()),
        },
    }
}

fn env_path(name: &str) -> Option<String> {
    let relative = match name {
        "CASY_PPOCR_MODEL_DIR" => return ocr_model_dir().map(|p| p.to_string_lossy().into_owned()),
        "CASY_OCR_FONT" => "fonts/NotoSansCJK-Regular.ttf",
        _ => return None,
    };
    crate::runtime_paths::runtime_asset(name, relative).map(|p| p.to_string_lossy().into_owned())
}

/// OCR 识别模型档位：`models/ocr/<tier>/{det.onnx,rec.onnx,dict.txt}`。
///
/// 解析顺序：`CASY_PPOCR_MODEL_DIR`（显式目录）→ `CASY_OCR_TIER`（默认 small，
/// small 约 30MB，tiny 约 6MB，medium 约 132MB 可选下载）→ 旧布局
/// `models/ppocrv6-medium` 兜底（老安装不停摆）。目录缺 det.onnx 即视为不可用。
pub fn ocr_model_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("CASY_PPOCR_MODEL_DIR")
        .map(PathBuf::from)
        .filter(|p| p.join("det.onnx").is_file())
    {
        return Some(dir);
    }
    let root = crate::runtime_paths::bundled_runtime()?;
    let tier = configured_ocr_tier();
    let tier_dir = root.join("models/ocr").join(&tier);
    if tier_dir.join("det.onnx").is_file() {
        return Some(tier_dir);
    }
    // 旧安装布局兜底
    let legacy = root.join("models/ppocrv6-medium");
    legacy.join("det.onnx").is_file().then_some(legacy)
}

/// 当前配置的 OCR 档位：env `CASY_OCR_TIER` > settings `ocr.tier` > `small`。
/// 只接受已安装的档位；配置了但未下载时回退 small 并记日志（调用方按需提示）。
pub fn configured_ocr_tier() -> String {
    if let Some(tier) = std::env::var("CASY_OCR_TIER").ok().filter(|value| !value.trim().is_empty()) {
        return normalize_tier(&tier);
    }
    let from_settings = crate::db::open_db()
        .ok()
        .and_then(|conn| crate::db::get_setting(&conn, "ocr.tier").ok().flatten())
        .filter(|value| !value.trim().is_empty());
    match from_settings {
        Some(tier) => normalize_tier(&tier),
        None => "small".to_string(),
    }
}

fn normalize_tier(tier: &str) -> String {
    let tier = tier.trim().to_ascii_lowercase();
    if matches!(tier.as_str(), "tiny" | "small" | "medium") {
        tier
    } else {
        log::warn!("未知 OCR 档位 {tier}，回退 small");
        "small".to_string()
    }
}

/// 当前可用的 OCR 档位列表（供探活与设置界面展示）。
pub fn available_ocr_tiers() -> Vec<String> {
    let Some(root) = crate::runtime_paths::bundled_runtime() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(root.join("models/ocr")) else {
        return Vec::new();
    };
    let mut tiers: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.path().join("det.onnx").is_file())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    tiers.sort();
    tiers
}

/// 仅计算产物目录，不创建（回收扫描需要"先判定、再落盘"的纯路径版本）。
pub fn artifact_dir_path(file_id: &str, sha256: &str) -> Result<PathBuf> {
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
    Ok(base)
}

pub fn artifact_dir(file_id: &str, sha256: &str) -> Result<PathBuf> {
    let base = artifact_dir_path(file_id, sha256)?;
    std::fs::create_dir_all(&base)?;
    Ok(base)
}

pub async fn run_engine(request: ProcessRequest) -> Result<ProcessResult> {
    run_processing(request, true).await
}

/// Standalone conversion has no document_processing_jobs row or database cancellation owner.
pub async fn run_standalone_engine(request: ProcessRequest) -> Result<ProcessResult> {
    run_processing(request, false).await
}

async fn run_processing(request: ProcessRequest, tracked: bool) -> Result<ProcessResult> {
    if crate::parse::text_document::supports(Path::new(&request.source_path)) {
        // S5/P1-9：文字路径此前既无取消检查也无超时保护，取消只能等阻塞任务自然结束。
        crate::processing::check_conversion_cancelled(&request.job_id)?;
        let job_id = request.job_id.clone();
        let blocking = tokio::task::spawn_blocking(move || -> anyhow::Result<ProcessResult> {
            let started = std::time::Instant::now();
            let mut result = crate::parse::text_document::process(&request)?;
            result.elapsed_ms = started.elapsed().as_millis() as u64;
            validate_result(&request, &result)?;
            Ok(result)
        });
        // 超时只放弃等待：阻塞任务仍会在后台跑完，日志记录其去向以便排查。
        return match tokio::time::timeout(std::time::Duration::from_secs(600), blocking).await {
            Ok(joined) => Ok(joined??),
            Err(_) => {
                tracing::warn!(%job_id, "文字文档处理超过 600 秒，已放弃等待（阻塞任务可能仍在后台完成）");
                Err(anyhow::anyhow!("DOC_ENGINE_TIMEOUT: 文字文档处理超过 10 分钟无响应，已终止任务"))
            }
        };
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
            spool_engine_output(stdout),
            read_bounded(stderr, 1024 * 1024),
        )?;
        Ok::<_, anyhow::Error>((status, stdout, stderr))
    };
    tokio::pin!(wait);
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
    let started = std::time::Instant::now();
    let mut last_progress = std::time::Instant::now();
    let mut current_page = 0;
    let mut current_phase = String::new();
    let output = loop {
        tokio::select! {
            result = &mut wait => break result?,
            _ = interval.tick() => {
                if !tracked { crate::processing::check_conversion_cancelled(&request.job_id)?; }
                let conn = if tracked {
                    let conn = crate::db::open_db()?;
                    check_job_running(&conn, &request.job_id)?;
                    Some(conn)
                } else { None };
                // S5/P1-9：进度文件读取/解析失败此前被 `if let Ok` 静默吞掉，
                // 停滞保护超时时没有任何线索。此处按 NotFound（引擎尚未落盘）静默、
                // 其余失败告警处理，900s 停滞常数保持不变。
                let progress_path = Path::new(&request.output_dir).join("progress.json");
                match std::fs::read(&progress_path) {
                    Ok(bytes) => match serde_json::from_slice::<EngineProgress>(&bytes) {
                        Ok(progress) => {
                            if progress.current_page > current_page && progress.current_page <= progress.total_pages {
                                current_page = progress.current_page;
                                last_progress = std::time::Instant::now();
                                tracing::info!(job_id = %request.job_id, current_page, total_pages = progress.total_pages, "Document engine progress");
                            }
                            if progress.total_pages > 0 && progress.current_page <= progress.total_pages {
                                let phase = progress.phase.as_deref().unwrap_or(if progress.current_page == progress.total_pages { "finalizing" } else { "recognizing" });
                                if current_phase != phase { current_phase = phase.to_string(); last_progress = std::time::Instant::now(); }
                                let elapsed = if progress.elapsed_ms > 0 { progress.elapsed_ms as f64 / 1000.0 } else { started.elapsed().as_secs_f64() };
                                let remaining = progress.remaining_ms.map(|value| value as f64 / 1000.0);
                                emit_progress_snapshot(&request.job_id, &request.source_path, phase, progress.current_page, progress.total_pages, elapsed, remaining, progress.page_timing.as_ref(), !tracked);
                                if let Some(conn) = &conn { conn.execute("UPDATE document_processing_jobs SET phase=?1,current_page=?2,total_pages=?3,progress=?4,elapsed_ms=?5,remaining_ms=?6,timing_json=COALESCE(?7,timing_json),updated_at=datetime('now','localtime') WHERE id=?8 AND status='running'",
                                    rusqlite::params![phase,progress.current_page,progress.total_pages,0.01 + 0.94 * progress.current_page as f64 / progress.total_pages as f64,progress.elapsed_ms,progress.remaining_ms,progress.page_timing.as_ref().map(serde_json::to_string).transpose()?,request.job_id])?;
                                }
                            }
                        }
                        Err(error) => tracing::warn!(job_id = %request.job_id, path = %progress_path.display(), %error, "文档引擎进度文件解析失败，本次刷新跳过"),
                    },
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => tracing::warn!(job_id = %request.job_id, path = %progress_path.display(), %error, "文档引擎进度文件读取失败，本次刷新跳过"),
                }
                if last_progress.elapsed() > std::time::Duration::from_secs(900) {
                    return Err(anyhow!("DOC_ENGINE_TIMEOUT: 当前阶段超过 15 分钟没有页数或阶段进展，已终止任务"));
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
    tokio::task::spawn_blocking(move || {
        let result: ProcessResult = serde_json::from_reader(std::io::BufReader::new(output.1.reopen()?))
            .context("文档引擎结果不是有效 JSON")?;
        validate_result(&request, &result)?;
        Ok(result)
    }).await?
}

/// Drain stdout incrementally so large documents do not require a second, contiguous
/// JSON buffer or an arbitrary transport byte limit. Cancellation drops the temp file.
async fn spool_engine_output(reader: impl tokio::io::AsyncRead + Unpin) -> Result<tempfile::NamedTempFile> {
    use tokio::io::AsyncWriteExt;
    let file = tempfile::NamedTempFile::new().context("无法创建 OCR 结果临时文件")?;
    let mut reader = tokio::io::BufReader::with_capacity(256 * 1024, reader);
    let mut writer = tokio::fs::File::from_std(file.reopen()?);
    tokio::io::copy_buf(&mut reader, &mut writer).await.context("无法保存 OCR 结果，请检查磁盘空间")?;
    writer.flush().await?;
    Ok(file)
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
    #[serde(default)]
    phase: Option<String>,
    current_page: u32,
    total_pages: u32,
    #[serde(default)]
    elapsed_ms: u64,
    #[serde(default)]
    remaining_ms: Option<u64>,
    #[serde(default)]
    page_timing: Option<serde_json::Value>,
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
    if result.engine.trim().is_empty() || result.page_count == 0 {
        return Err(anyhow!("INVALID_PAGE_IR: 引擎未返回页面"));
    }
    // R-02：逐页流式校验落盘 IR，不整份物化
    let ir_path = Path::new(&result.page_ir_path).to_path_buf();
    let mut seen_pages = 0u32;
    stream_disk_pages(&ir_path, |index, page| {
        seen_pages += 1;
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
        if let Some(timing) = &page.timing {
            if timing.total_ms < timing.render_ms
                || timing.total_ms < timing.ocr_ms
                || timing.total_ms < timing.layout_ms
            {
                return Err(anyhow!("INVALID_PAGE_IR: 页面耗时明细无效"));
            }
        }
        if let Some(layout) = &page.layout {
            let blocks = layout.get("blocks").and_then(serde_json::Value::as_array)
                .ok_or_else(|| anyhow!("INVALID_PAGE_IR: Paddle 版面块缺失"))?;
            for (order, block) in blocks.iter().enumerate() {
                let bbox = block.get("bbox").and_then(serde_json::Value::as_array)
                    .filter(|bbox| bbox.len() == 4)
                    .ok_or_else(|| anyhow!("INVALID_PAGE_IR: 版面块坐标无效"))?;
                let coords = bbox.iter().map(|value| value.as_f64()).collect::<Option<Vec<_>>>()
                    .ok_or_else(|| anyhow!("INVALID_PAGE_IR: 版面块坐标无效"))?;
                if !coords.iter().all(|value| value.is_finite())
                    || coords[0] < 0.0 || coords[1] < 0.0
                    || coords[2] < coords[0] || coords[3] < coords[1]
                    || coords[2] > w as f64 + 1.0 || coords[3] > h as f64 + 1.0
                    || block.get("readingOrder").and_then(serde_json::Value::as_u64) != Some(order as u64)
                {
                    return Err(anyhow!("INVALID_PAGE_IR: 版面块顺序或边界无效"));
                }
            }
        }
        let _ = index;
        Ok(())
    })?;
    anyhow::ensure!(seen_pages == result.page_count, "INVALID_PAGE_IR: 落盘页数与引擎摘要不一致");
    let manifest = if root.join(assets::MANIFEST).exists() { Some(assets::load(&root)?) } else { None };
    stream_disk_pages(&ir_path, |_index, page| {
        if let Some(manifest) = &manifest {
            assets::validate_references(&page.markdown, &root, manifest)?;
        } else {
            anyhow::ensure!(!page.markdown.contains("src=\"assets/"), "INVALID_ASSET: missing manifest");
        }
        Ok(())
    })?;
    let mut expected_md = Sha256::new();
    stream_disk_pages(&ir_path, |index, page| {
        if !text_document {
            if index > 0 { expected_md.update(b"\n\n---\n\n"); }
            expected_md.update(format!("<!-- page {} -->\n", page.page_number));
        }
        expected_md.update(page.markdown.as_bytes());
        Ok(())
    })?;
    if sha256_file(Path::new(&result.markdown_path))? != hex::encode(expected_md.finalize()) {
        return Err(anyhow!("INVALID_MARKDOWN: Markdown 备份与页面不一致"));
    }
    if result.engine.starts_with("paddle-onnx-") && result.source_map_path.is_none() {
        return Err(anyhow!("INVALID_SOURCE_MAP: 缺少来源映射"));
    }
    if let Some(path) = &result.source_map_path {
        let map: source_map::SourceMap = serde_json::from_reader(std::io::BufReader::new(std::fs::File::open(path)?))?;
        // R-02：流式重建期望来源映射（累计文本与 span，不保留逐页 regions/markdown 全量）
        let mut builder = source_map::SourceMapBuilder::new(&result.source_sha256, std::io::sink());
        stream_disk_pages(&ir_path, |_index, page| {
            builder.append_page(&page)?;
            Ok(())
        })?;
        if map != builder.finish()? {
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
        resume_from: None,
    }
}

#[cfg(test)]
mod stream_count_tests {
    use super::*;

    /// R-06：count_disk_pages 与 stream_disk_pages 对同一文件给出一致页数；坏文件记 0。
    #[test]
    fn count_matches_stream_for_page_ir() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("segments.json");
        let pages = vec![
            DocumentPage {
                page_number: 1,
                width: None,
                height: None,
                plain_text: "一".into(),
                markdown: "一".into(),
                regions: vec![],
                confidence: None,
                layout: None,
                timing: None,
                orientation_degrees: None,
            },
            DocumentPage {
                page_number: 2,
                width: None,
                height: None,
                plain_text: "二".into(),
                markdown: "二".into(),
                regions: vec![],
                confidence: None,
                layout: None,
                timing: None,
                orientation_degrees: None,
            },
        ];
        std::fs::write(&path, serde_json::to_vec(&pages).unwrap()).unwrap();
        assert_eq!(count_disk_pages(&path), 2);
        let mut visited = 0;
        stream_disk_pages(&path, |index, page| {
            assert_eq!(page.page_number as usize, index + 1);
            visited += 1;
            Ok(())
        })
        .unwrap();
        assert_eq!(visited, 2);
        assert_eq!(count_disk_pages(&root.path().join("missing.json")), 0);
        let broken = root.path().join("broken.json");
        std::fs::write(&broken, b"[{\"pageNumber\":").unwrap();
        assert_eq!(count_disk_pages(&broken), 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn engine_output_streams_past_128_mib_and_removes_temporary_files() {
        use tokio::io::AsyncReadExt;
        let length = 129 * 1024 * 1024;
        let output = spool_engine_output(tokio::io::repeat(b'X').take(length)).await.unwrap();
        let path = output.path().to_path_buf();
        assert_eq!(output.as_file().metadata().unwrap().len(), length);
        let mut reader = std::io::BufReader::new(output.reopen().unwrap());
        let mut buffer = [0; 64 * 1024];
        let mut count = 0;
        loop {
            let n = reader.read(&mut buffer).unwrap();
            if n == 0 { break; }
            assert!(buffer[..n].iter().all(|&b| b == b'X'));
            count += n as u64;
        }
        assert_eq!(count, length);
        drop(reader);
        drop(output);
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn interrupted_engine_stream_propagates_the_read_error() {
        struct Broken;
        impl tokio::io::AsyncRead for Broken {
            fn poll_read(self: std::pin::Pin<&mut Self>, _: &mut std::task::Context<'_>, _: &mut tokio::io::ReadBuf<'_>) -> std::task::Poll<std::io::Result<()>> {
                std::task::Poll::Ready(Err(std::io::Error::other("engine stream interrupted")))
            }
        }
        let error = spool_engine_output(Broken).await.unwrap_err();
        assert!(format!("{error:#}").contains("engine stream interrupted"));
    }

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
            layout: None,
            timing: None,
            orientation_degrees: None,
        }];
        let mut result = ProcessResult {
            source_sha256: request.source_sha256.clone(),
            engine: "test".into(),
            model_version: None,
            searchable_pdf_path: Some(root.join("search.pdf").display().to_string()),
            page_ir_path: root.join("pages.json").display().to_string(),
            markdown_path: root.join("source.md").display().to_string(),
            source_map_path: None,
            page_count: 1,
            elapsed_ms: 0,
        };
        std::fs::write(
            result.searchable_pdf_path.as_ref().unwrap(),
            b"%PDF-derived",
        )
        .unwrap();
        // R-02：页 IR 是唯一事实源，结果只带路径与页数
        std::fs::write(&result.page_ir_path, serde_json::to_vec(&pages).unwrap()).unwrap();
        std::fs::write(&result.markdown_path, "<!-- page 1 -->\n# Evidence").unwrap();
        validate_result(&request, &result).unwrap();
        let (_, map) = source_map::build(&pages, &result.source_sha256);
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
        // 落盘页 IR 被篡改（页码不再是 1）→ 流式校验必须失败
        let mut tampered = pages.clone();
        tampered[0].page_number = 2;
        std::fs::write(&result.page_ir_path, serde_json::to_vec(&tampered).unwrap()).unwrap();
        assert!(validate_result(&request, &result).is_err());
        std::fs::write(&result.page_ir_path, serde_json::to_vec(&pages).unwrap()).unwrap();
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
