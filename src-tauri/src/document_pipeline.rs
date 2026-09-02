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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentRegion {
    pub text: String,
    pub bbox: [f32; 4],
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    match tokio::process::Command::new(&executable)
        .arg("probe")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
    {
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
    let base = crate::db::get_db_path()
        .parent()
        .ok_or_else(|| anyhow!("数据库目录不可用"))?
        .join("document-artifacts")
        .join(file_id)
        .join(&sha256[..sha256.len().min(16)]);
    std::fs::create_dir_all(&base)?;
    Ok(base)
}

pub async fn run_engine(request: ProcessRequest) -> Result<ProcessResult> {
    let executable = engine_executable().ok_or_else(|| {
        anyhow!("DOC_ENGINE_NOT_FOUND: 请设置 CASY_DOC_ENGINE 或安装 casy-doc-engine")
    })?;
    let mut child = tokio::process::Command::new(executable)
        .arg("process")
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
    let output = child.wait_with_output().await?;
    if !output.status.success() {
        return Err(anyhow!(
            "DOC_ENGINE_FAILED: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let result: ProcessResult =
        serde_json::from_slice(&output.stdout).context("文档引擎结果不是有效 JSON")?;
    if result.source_sha256 != request.source_sha256 {
        return Err(anyhow!("SOURCE_CHANGED: 处理期间源文件哈希发生变化"));
    }
    Ok(result)
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
        cjk_font_path: env_path("CASY_OCR_FONT"),
        device: std::env::var("CASY_OCR_DEVICE").unwrap_or_else(|_| "cpu".into()),
    }
}
