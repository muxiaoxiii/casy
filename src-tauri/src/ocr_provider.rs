//! OCR Provider 抽象：内置引擎（casy-doc-engine 子进程）与远端 Myna 服务。
//!
// 远端模式把 Myna（MinerU 4.0）的识别结果翻译成与内置引擎完全一致的落盘产物：
// 页 IR（`source.document.json`）、Markdown（`source.md`）、来源映射
// （`source.map.json`）与可搜索 PDF（`source.searchable.pdf`），因此下游
// （流式校验、数据库落库、区域校订、知识索引、来源高亮）零改动。
//!
//! Myna 返回的几何是 0–1 归一化坐标，这里按页面像素尺寸缩放；图片字节随
//! `include_images=true` 返回，按内容哈希外置为 casy 的资产布局。

use anyhow::{Context, Result};
use serde_json::Value;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::background_jobs::ClaimedJob;
use crate::document_pipeline::{self, DocumentPage, DocumentRegion, ProcessResult};

/// 预览渲染尺度（与 `get_document_page` 的 `-scale-to 1600` 一致），
/// 远端几何按此缩放后与界面高亮对齐。
const PREVIEW_SCALE: f32 = 1600.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OcrProvider {
    Builtin,
    Myna,
}

/// 当前配置的 OCR provider：settings `ocr.provider` > env `CASY_OCR_PROVIDER` > builtin。
pub fn configured_provider() -> OcrProvider {
    let raw = std::env::var("CASY_OCR_PROVIDER")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            crate::db::open_db()
                .ok()
                .and_then(|conn| crate::db::get_setting(&conn, "ocr.provider").ok().flatten())
        });
    match raw.as_deref().map(str::trim) {
        Some("myna") => OcrProvider::Myna,
        _ => OcrProvider::Builtin,
    }
}

fn myna_base_url() -> String {
    std::env::var("CASY_MYNA_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            crate::db::open_db()
                .ok()
                .and_then(|conn| crate::db::get_setting(&conn, "ocr.myna_url").ok().flatten())
        })
        .unwrap_or_else(|| "http://127.0.0.1:18600".to_string())
        .trim_end_matches('/')
        .to_string()
}

pub struct MynaClient {
    base_url: String,
    http: reqwest::Client,
}

impl MynaClient {
    pub fn new() -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(600))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .context("无法创建 HTTP 客户端")?;
        Ok(Self { base_url: myna_base_url(), http })
    }

    /// 探活：远端模式必须先确认 Myna 可达，绝不静默回退。
    pub async fn healthz(&self) -> Result<()> {
        let response = self
            .http
            .get(format!("{}/api/healthz", self.base_url))
            .send()
            .await
            .with_context(|| format!("无法连接 Myna  OCR 服务（{}）：请先启动 Myna", self.base_url))?;
        anyhow::ensure!(response.status().is_success(), "Myna 健康检查失败: HTTP {}", response.status());
        Ok(())
    }

    /// 识别（multipart 上传，`include_images=true` 取回图片字节）。
    pub async fn recognize(&self, path: &Path) -> Result<Value> {
        let bytes = std::fs::read(path).with_context(|| format!("无法读取源文件: {}", path.display()))?;
        let file_part = reqwest::multipart::Part::bytes(bytes).file_name(
            path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "upload.bin".into()),
        );
        let form = reqwest::multipart::Form::new()
            .text("mode", "markdown")
            .text("auto_load", "true")
            .text("page", "all")
            .text("include_images", "true")
            .part("file", file_part);
        let response = self
            .http
            .post(format!("{}/api/ocr/recognize", self.base_url))
            .multipart(form)
            .send()
            .await
            .with_context(|| format!("Myna 识别请求失败（{}）", self.base_url))?;
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        anyhow::ensure!(status.is_success(), "Myna 识别失败: HTTP {status} {text}");
        serde_json::from_str(&text).context("Myna 返回了无法解析的结果")
    }

    /// 可搜索 PDF：异步任务 → 轮询进度 → 下载结果。
    pub async fn searchable_pdf(&self, path: &Path, destination: &Path) -> Result<()> {
        let bytes = std::fs::read(path).with_context(|| format!("无法读取源文件: {}", path.display()))?;
        let file_part = reqwest::multipart::Part::bytes(bytes).file_name(
            path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "upload.bin".into()),
        );
        let form = reqwest::multipart::Form::new()
            .text("pages", "all")
            .text("tier", "standard")
            .text("ocr_mode", "auto")
            .part("file", file_part);
        let response = self
            .http
            .post(format!("{}/api/ocr/pdf", self.base_url))
            .multipart(form)
            .send()
            .await
            .context("Myna 可搜索 PDF 任务提交失败")?;
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        anyhow::ensure!(status.is_success(), "Myna 可搜索 PDF 任务提交失败: HTTP {status} {text}");
        let _job_id = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|value| value.get("job_id").and_then(Value::as_str).map(str::to_string))
            .ok_or_else(|| anyhow::anyhow!("Myna 未返回任务 ID"))?;

        let started = Instant::now();
        loop {
            if started.elapsed() > Duration::from_secs(3600) {
                anyhow::bail!("Myna 可搜索 PDF 任务超时（1 小时）");
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
            let progress: Value = self
                .http
                .get(format!("{}/api/ocr/pdf/progress", self.base_url))
                .send()
                .await
                .context("无法查询 Myna PDF 任务进度")?
                .json()
                .await
                .unwrap_or(Value::Null);
            if progress.get("active").and_then(Value::as_bool) == Some(false) {
                if let Some(error) = progress.get("error").and_then(Value::as_str) {
                    anyhow::bail!("Myna 可搜索 PDF 任务失败: {error}");
                }
                if progress.get("output_ready").and_then(Value::as_bool) == Some(true) {
                    break;
                }
            }
        }
        let response = self
            .http
            .get(format!("{}/api/ocr/pdf/result", self.base_url))
            .send()
            .await
            .context("无法下载 Myna 可搜索 PDF")?;
        anyhow::ensure!(response.status().is_success(), "Myna 可搜索 PDF 下载失败: HTTP {}", response.status());
        let bytes = response.bytes().await.context("无法读取 Myna 可搜索 PDF")?;
        anyhow::ensure!(bytes.starts_with(b"%PDF-"), "Myna 返回的可搜索 PDF 不是有效 PDF");
        std::fs::write(destination, &bytes).with_context(|| format!("无法写入 {}", destination.display()))?;
        log::info!("Myna 可搜索 PDF 已下载: {}", destination.display());
        Ok(())
    }
}

fn sha256_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}

/// 页面像素尺寸（预览尺度）：PDF 读 MediaBox，图片读文件头。
fn page_dimensions(path: &Path) -> Result<Vec<(f32, f32)>> {
    let is_pdf = path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"));
    if !is_pdf {
        let (w, h) = image::image_dimensions(path).context("无法读取图片尺寸")?;
        return Ok(vec![scale_dimensions(w as f32, h as f32)]);
    }
    let document = lopdf::Document::load(path).context("无法打开 PDF")?;
    let mut dimensions = Vec::new();
    for (_, page_id) in document.get_pages() {
        let media_box = document
            .get_dictionary(page_id)
            .ok()
            .and_then(|dict| dict.get(b"MediaBox").ok().cloned())
            .and_then(|object| match object {
                lopdf::Object::Array(values) if values.len() >= 4 => Some(values),
                _ => None,
            })
            .map(|values| {
                let number = |index: usize| match &values[index] {
                    lopdf::Object::Real(value) => Some(*value),
                    lopdf::Object::Integer(value) => Some(*value as f32),
                    _ => None,
                };
                (
                    number(0).unwrap_or(0.0),
                    number(1).unwrap_or(0.0),
                    number(2).unwrap_or(612.0),
                    number(3).unwrap_or(792.0),
                )
            })
            .unwrap_or((0.0, 0.0, 612.0, 792.0));
        let width = (media_box.2 - media_box.0).abs().max(1.0);
        let height = (media_box.3 - media_box.1).abs().max(1.0);
        dimensions.push(scale_dimensions(width, height));
    }
    anyhow::ensure!(!dimensions.is_empty(), "PDF 没有页面");
    Ok(dimensions)
}

fn scale_dimensions(width: f32, height: f32) -> (f32, f32) {
    let (width, height) = (width.max(1.0), height.max(1.0));
    if width >= height {
        (PREVIEW_SCALE, (height * PREVIEW_SCALE / width).max(1.0))
    } else {
        ((width * PREVIEW_SCALE / height).max(1.0), PREVIEW_SCALE)
    }
}

/// 归一化四边形 → 像素轴对齐 bbox。
fn quad_to_pixel_bbox(quad: &Value, width: f32, height: f32) -> Option<[f32; 4]> {
    let points = quad.as_array()?;
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for point in points {
        let pair = point.as_array()?;
        if pair.len() < 2 {
            return None;
        }
        xs.push(pair[0].as_f64()? as f32 * width);
        ys.push(pair[1].as_f64()? as f32 * height);
    }
    if xs.is_empty() || ys.is_empty() {
        return None;
    }
    Some([
        xs.iter().copied().fold(f32::INFINITY, f32::min).clamp(0.0, width),
        ys.iter().copied().fold(f32::INFINITY, f32::min).clamp(0.0, height),
        xs.iter().copied().fold(f32::NEG_INFINITY, f32::max).clamp(0.0, width),
        ys.iter().copied().fold(f32::NEG_INFINITY, f32::max).clamp(0.0, height),
    ])
}

/// 把 Myna 的 `images[]` 落成内容哈希资产，返回 `image_id → 相对路径`。
fn materialize_images(output_dir: &Path, payload: &Value) -> Result<std::collections::HashMap<String, String>> {
    let mut map = std::collections::HashMap::new();
    let Some(images) = payload.get("images").and_then(Value::as_array) else {
        return Ok(map);
    };
    let asset_root = output_dir.join("casy-images");
    for image in images {
        let Some(id) = image.get("id").and_then(Value::as_str) else { continue };
        let Some(data) = image.get("data_base64").and_then(Value::as_str).filter(|value| !value.is_empty()) else {
            continue;
        };
        let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data)
            .context("Myna 图片 base64 解码失败")?;
        let sha = sha256_bytes(&bytes);
        let extension = image
            .get("media_type")
            .and_then(Value::as_str)
            .and_then(|value| value.split('/').nth(1))
            .filter(|value| !value.is_empty())
            .unwrap_or("png")
            .to_string();
        let name = format!("{sha}.{extension}");
        let target = asset_root.join(&name);
        if !target.exists() {
            std::fs::create_dir_all(&asset_root).with_context(|| format!("无法创建 {}", asset_root.display()))?;
            std::fs::write(&target, &bytes).with_context(|| format!("无法写入 {}", target.display()))?;
        }
        map.insert(id.to_string(), format!("casy-images/{name}"));
    }
    Ok(map)
}

/// 把 Myna markdown 里的图片引用改写成本地资产相对路径：
/// ① base64 data URI（MinerU 内联）→ 落地后替换；② `images/...` 路径 → 按 id 映射替换。
fn rewrite_markdown_images(markdown: &str, image_map: &std::collections::HashMap<String, String>, output_dir: &Path) -> Result<String> {
    let mut result = markdown.to_string();
    // base64 data URI：按内容哈希落地（与独立转换的图片外置同一命名规则）
    let pattern = regex::Regex::new(r"!\[[^\]]*\]\(\s*data:image/([a-zA-Z0-9.+-]+);base64,([A-Za-z0-9+/=]+)\s*\)")
        .context("图片正则编译失败")?;
    let mut replacements: Vec<(String, String)> = Vec::new();
    for capture in pattern.captures_iter(markdown) {
        let whole = capture.get(0).map(|m| m.as_str().to_string()).unwrap_or_default();
        let extension = capture.get(1).map(|m| m.as_str().to_string()).unwrap_or_else(|| "png".into());
        let data = capture.get(2).map(|m| m.as_str().to_string()).unwrap_or_default();
        let Ok(bytes) = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data) else {
            continue;
        };
        let sha = sha256_bytes(&bytes);
        let name = format!("{sha}.{extension}");
        let target = output_dir.join("casy-images").join(&name);
        if !target.exists() {
            std::fs::create_dir_all(output_dir.join("casy-images")).ok();
            std::fs::write(&target, &bytes).ok();
        }
        replacements.push((whole, format!("casy-images/{name}")));
    }
    for (from, to) in replacements {
        result = result.replace(&from, &format!("![]({to})"));
    }
    // MinerU 相对路径引用（images/...）→ 本地资产
    for (id, local) in image_map {
        result = result.replace(&format!("({id})"), &format!("({local})"));
    }
    Ok(result)
}

/// 远端识别并落盘（与内置引擎相同的产物布局），随后走统一的 persist_success。
pub(crate) async fn recognize_via_myna(job: &ClaimedJob) -> Result<()> {
    let client = MynaClient::new()?;
    client.healthz().await?;
    let source = std::path::Path::new(&job.source_path);
    let dimensions = page_dimensions(source)?;

    document_pipeline::emit_conversion_progress(&job.id, &job.source_path, "uploading", 0, 1, 0.0);
    let payload = client.recognize(source).await?;
    let pages_payload = payload.get("pages").and_then(Value::as_array).context("Myna 结果缺少 pages")?;
    anyhow::ensure!(!pages_payload.is_empty(), "Myna 未识别出任何页面");

    let output_dir = document_pipeline::artifact_dir(&job.id, &job.source_sha256)?;
    let image_map = materialize_images(&output_dir, &payload)?;

    let mut pages: Vec<DocumentPage> = Vec::with_capacity(pages_payload.len());
    for (index, page) in pages_payload.iter().enumerate() {
        let page_number = page.get("page_number").and_then(Value::as_u64).unwrap_or(index as u64 + 1) as u32;
        let (width, height) = dimensions.get(index).copied().unwrap_or((PREVIEW_SCALE, PREVIEW_SCALE));
        let markdown = page.get("markdown").and_then(Value::as_str).unwrap_or("");
        let markdown = rewrite_markdown_images(markdown, &image_map, &output_dir)?;
        let mut regions = Vec::new();
        if let Some(items) = page.get("regions").and_then(Value::as_array) {
            for region in items {
                let Some(bbox) = region.get("quad").and_then(|quad| quad_to_pixel_bbox(quad, width, height)) else {
                    continue;
                };
                regions.push(DocumentRegion {
                    text: region.get("text").and_then(Value::as_str).unwrap_or("").to_string(),
                    bbox,
                    confidence: region.get("score").and_then(Value::as_f64).map(|value| value as f32),
                    word_boxes: None,
                });
            }
        }
        let plain_text = regions.iter().map(|region| region.text.as_str()).collect::<Vec<_>>().join("\n");
        pages.push(DocumentPage {
            page_number,
            width: Some(width),
            height: Some(height),
            plain_text,
            markdown,
            regions,
            confidence: None,
            layout: None,
            timing: None,
            orientation_degrees: None,
        });
    }

    // 页 IR + Markdown + 来源映射一次性写成（与引擎产物同构，validate_result 可流式校验）
    let ir_path = output_dir.join("source.document.json");
    std::fs::write(&ir_path, serde_json::to_vec(&pages)?).with_context(|| format!("无法写入 {}", ir_path.display()))?;
    let md_path = output_dir.join("source.md");
    let mut builder = document_pipeline::source_map::SourceMapBuilder::new(
        &job.source_sha256,
        std::io::BufWriter::new(std::fs::File::create(&md_path)?),
    );
    for page in &pages {
        builder.append_page(page)?;
    }
    let source_map = builder.finish()?;
    let map_path = output_dir.join("source.map.json");
    std::fs::write(&map_path, serde_json::to_vec(&source_map)?)?;

    // 可搜索 PDF（Myna 异步任务）
    let pdf_path = output_dir.join("source.searchable.pdf");
    client.searchable_pdf(source, &pdf_path).await?;

    let result = ProcessResult {
        source_sha256: job.source_sha256.clone(),
        engine: "myna-mineru-4.0".into(),
        model_version: Some("mineru-4.0".into()),
        searchable_pdf_path: Some(pdf_path.display().to_string()),
        page_ir_path: ir_path.display().to_string(),
        markdown_path: md_path.display().to_string(),
        source_map_path: Some(map_path.display().to_string()),
        page_count: pages.len() as u32,
        elapsed_ms: 0,
    };
    crate::background_jobs::persist_success(job, &result)
}
