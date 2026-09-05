use anyhow::{Context, Result, anyhow};
use harumi::{Color, Document, TextRun};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EngineStatus {
    available: bool,
    executable: Option<String>,
    version: Option<String>,
    renderer_available: bool,
    coordinate_model_available: bool,
    ovis_model_available: bool,
    searchable_pdf_available: bool,
    missing: Vec<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProcessRequest {
    job_id: String,
    source_path: String,
    source_sha256: String,
    output_dir: String,
    coordinate_model_dir: Option<String>,
    ovis_model_dir: Option<String>,
    paddle_model_dir: Option<String>,
    cjk_font_path: Option<String>,
    device: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Region {
    text: String,
    bbox: [f32; 4],
    confidence: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Page {
    page_number: u32,
    width: Option<f32>,
    height: Option<f32>,
    plain_text: String,
    markdown: String,
    regions: Vec<Region>,
    confidence: Option<f32>,
    #[serde(default)]
    native_text: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProcessResult {
    source_sha256: String,
    engine: String,
    model_version: Option<String>,
    searchable_pdf_path: String,
    page_ir_path: String,
    markdown_path: String,
    pages: Vec<Page>,
}

fn command_exists(name: &str) -> bool {
    std::process::Command::new(name)
        .arg("-v")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|p| p.exists())
}
fn sha256_file(path: &Path) -> Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut b = [0u8; 1024 * 1024];
    loop {
        let n = f.read(&mut b)?;
        if n == 0 {
            break;
        }
        h.update(&b[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

fn probe() -> EngineStatus {
    let renderer = command_exists("pdftoppm");
    let coord = env_path("CASY_PPOCR_MODEL_DIR").is_some_and(|dir| {
        ["det.onnx", "rec.onnx", "dict.txt"]
            .iter()
            .all(|name| dir.join(name).is_file())
    });
    let ovis = env_path("CASY_OVISOCR2_MODEL_DIR").is_some_and(|dir| {
        [
            "config.json",
            "preprocessor_config.json",
            "tokenizer.json",
            "model.safetensors",
        ]
        .iter()
        .all(|name| dir.join(name).is_file())
    });
    let paddle = env_path("CASY_PADDLEOCR_VL_MODEL_DIR").is_some_and(|dir| {
        [
            "config.json",
            "preprocessor_config.json",
            "tokenizer.json",
            "model.safetensors",
        ]
        .iter()
        .all(|name| dir.join(name).is_file())
    });
    let font = env_path("CASY_OCR_FONT").is_some_and(|path| path.is_file());
    let mut missing = Vec::new();
    if !cfg!(feature = "models") {
        missing.push("models feature".into())
    }
    if !renderer {
        missing.push("pdftoppm".into())
    }
    if !coord {
        missing.push("CASY_PPOCR_MODEL_DIR".into())
    }
    if !ovis && !paddle {
        missing.push("CASY_PADDLEOCR_VL_MODEL_DIR / CASY_OVISOCR2_MODEL_DIR".into())
    }
    if !font {
        missing.push("CASY_OCR_FONT".into())
    }
    EngineStatus {
        available: cfg!(feature = "models") && renderer && coord && (ovis || paddle) && font,
        executable: std::env::current_exe()
            .ok()
            .map(|p| p.display().to_string()),
        version: Some(env!("CARGO_PKG_VERSION").into()),
        renderer_available: renderer,
        coordinate_model_available: coord,
        ovis_model_available: ovis,
        searchable_pdf_available: font,
        missing,
        error: None,
    }
}

fn render_page(source: &Path, temp: &Path, page_number: u32) -> Result<PathBuf> {
    let prefix = temp.join("page");
    let output = std::process::Command::new("pdftoppm")
        .args([
            "-f",
            &page_number.to_string(),
            "-l",
            &page_number.to_string(),
            "-singlefile",
        ])
        .arg("-png")
        .arg("-r")
        .arg("200")
        .args(["-scale-to", "2400"])
        .arg(source)
        .arg(&prefix)
        .output()
        .context("无法启动 pdftoppm")?;
    if !output.status.success() {
        return Err(anyhow!(
            "PDF_RENDER_FAILED: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let page = prefix.with_extension("png");
    if !page.is_file() {
        return Err(anyhow!("PDF_RENDER_FAILED: 没有生成页面图像"));
    }
    Ok(page)
}

fn write_progress(request: &ProcessRequest, current: u32, total: u32) -> Result<()> {
    let root = Path::new(&request.output_dir);
    let pending = root.join("progress.pending");
    std::fs::write(
        &pending,
        serde_json::to_vec(&serde_json::json!({
            "currentPage": current, "totalPages": total
        }))?,
    )?;
    std::fs::rename(pending, root.join("progress.json"))?;
    Ok(())
}

fn raster_pdf(source: &Path, output: &Path) -> Result<()> {
    use image::AnimationDecoder;
    let file = std::io::BufReader::new(std::fs::File::open(source)?);
    let format = image::ImageReader::open(source)?
        .with_guessed_format()?
        .format();
    let multiple = match format {
        Some(image::ImageFormat::Tiff) => tiff::decoder::Decoder::new(file)?.more_images(),
        Some(image::ImageFormat::Gif) => {
            image::codecs::gif::GifDecoder::new(file)?
                .into_frames()
                .take(2)
                .count()
                > 1
        }
        Some(image::ImageFormat::WebP) => {
            image::codecs::webp::WebPDecoder::new(file)?.has_animation()
        }
        Some(image::ImageFormat::Png) => image::codecs::png::PngDecoder::new(file)?.is_apng()?,
        _ => false,
    };
    if multiple {
        return Err(anyhow!(
            "MULTIFRAME_IMAGE: 请先将多页图片转换为 PDF，以保留全部页面"
        ));
    }
    let mut reader = image::ImageReader::open(source)?.with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16000);
    limits.max_image_height = Some(16000);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode()?;
    let size = (
        image.width() as f32 * 72.0 / 200.0,
        image.height() as f32 * 72.0 / 200.0,
    );
    let mut png = std::io::Cursor::new(Vec::new());
    image.write_to(&mut png, image::ImageFormat::Png)?;
    let mut doc = Document::new(size)?;
    doc.page(1)?
        .add_image(png.get_ref(), [0.0, 0.0, size.0, size.1])?;
    doc.save(output)?;
    Ok(())
}

fn native_page(doc: &mut Document, page_number: u32) -> Result<Option<Page>> {
    let text = match doc.extract_text(page_number) {
        Ok(text) => text,
        Err(_) => return Ok(None),
    };
    let meaningful = text.chars().filter(|c| !c.is_whitespace()).count();
    if meaningful < 100 || text.contains('\u{fffd}') || text.contains('\0') {
        return Ok(None);
    }
    let (width, height) = doc.page(page_number)?.size()?;
    let images = match doc.page_image_bboxes(page_number) {
        Ok(images) => images,
        Err(_) => return Ok(None),
    };
    // A substantial embedded image may contain scanned evidence alongside
    // native headers/footers. Keep that page on the OCR path.
    if images
        .iter()
        .any(|bbox| bbox[2].abs() * bbox[3].abs() > width * height * 0.2)
    {
        return Ok(None);
    }
    let fragments = match doc.extract_text_runs(page_number) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };
    let regions = fragments
        .into_iter()
        .filter(|f| !f.text.trim().is_empty())
        .map(|f| Region {
            text: f.text,
            confidence: None,
            bbox: [
                f.x.clamp(0.0, width),
                (height - f.y - f.height).clamp(0.0, height),
                (f.x + f.width.max(0.0)).clamp(0.0, width),
                (height - f.y).clamp(0.0, height),
            ],
        })
        .collect();
    Ok(Some(Page {
        page_number,
        width: Some(width),
        height: Some(height),
        plain_text: text.clone(),
        markdown: text,
        regions,
        confidence: None,
        native_text: true,
    }))
}

#[cfg(feature = "models")]
enum MarkdownRecognizer {
    Ovis(oar_ocr_vl::OvisOcr2),
    Paddle(oar_ocr_vl::PaddleOcrVl),
}

#[cfg(feature = "models")]
impl MarkdownRecognizer {
    fn load(request: &ProcessRequest) -> Result<Self> {
        let device = oar_ocr_vl::utils::parse_device(&request.device)?;
        if let Some(dir) = &request.paddle_model_dir {
            Ok(Self::Paddle(oar_ocr_vl::PaddleOcrVl::from_dir(
                dir, device,
            )?))
        } else {
            let dir = request
                .ovis_model_dir
                .as_deref()
                .ok_or_else(|| anyhow!("MODEL_MISSING: 页面识别模型"))?;
            Ok(Self::Ovis(oar_ocr_vl::OvisOcr2::from_dir(dir, device)?))
        }
    }

    fn parse(&self, image: &image::RgbImage, page_number: u32) -> Result<String> {
        use oar_ocr_vl::PaddleOcrVlTask;
        const MAX_TOKENS: usize = 16384;
        // Coordinates and searchable text keep the 2400px raster; bound the
        // generative model's visual tokens independently of source resolution.
        let scaled = image::DynamicImage::ImageRgb8(image.clone())
            .resize(1280, 1280, image::imageops::FilterType::Lanczos3)
            .to_rgb8();
        let image = &scaled;
        let tokens = match self {
            Self::Ovis(model) => model
                .generate_tokens(std::slice::from_ref(image), MAX_TOKENS)?
                .pop()
                .ok_or_else(|| anyhow!("EMPTY_MODEL_RESULT: 第 {page_number} 页"))??,
            Self::Paddle(model) => model
                .generate_tokens(
                    std::slice::from_ref(image),
                    &[PaddleOcrVlTask::Ocr],
                    MAX_TOKENS,
                )?
                .pop()
                .ok_or_else(|| anyhow!("EMPTY_MODEL_RESULT: 第 {page_number} 页"))?,
        };
        if tokens.len() >= MAX_TOKENS {
            return Err(anyhow!(
                "PAGE_OUTPUT_LIMIT: 第 {page_number} 页达到输出上限，未作为完整结果保存"
            ));
        }
        Ok(match self {
            Self::Ovis(model) => model.decode_tokens(&tokens)?,
            Self::Paddle(model) => model.decode_tokens(&tokens, PaddleOcrVlTask::Ocr)?.1,
        })
    }
}

#[cfg(feature = "models")]
fn recognize(
    request: &ProcessRequest,
    source: &Path,
    temp: &Path,
    total: u32,
) -> Result<Vec<Page>> {
    use oar_ocr::prelude::*;
    let mut coordinate = None;
    let mut recognizer = None;
    let mut document = Document::from_file(source)?;
    let mut pages = Vec::new();
    // Keep only one rendered page and its model inputs alive at a time.
    for page_number in 1..=total {
        if let Some(page) = native_page(&mut document, page_number)? {
            pages.push(page);
            write_progress(request, page_number, total)?;
            continue;
        }
        if coordinate.is_none() {
            let coord_dir = Path::new(
                request
                    .coordinate_model_dir
                    .as_deref()
                    .ok_or_else(|| anyhow!("MODEL_MISSING: coordinate model"))?,
            );
            let det = coord_dir.join("det.onnx");
            let rec = coord_dir.join("rec.onnx");
            let dict = coord_dir.join("dict.txt");
            for path in [&det, &rec, &dict] {
                if !path.is_file() {
                    return Err(anyhow!("MODEL_MISSING: {}", path.display()));
                }
            }
            oar_ocr::core::OrtGlobalThreadPoolOptions::new()
                .with_intra_threads(2)
                .with_inter_threads(1)
                .with_spin_control(false)
                .commit()?;
            coordinate = Some(
                OAROCRBuilder::new(&det, &rec, &dict)
                    .return_word_box(true)
                    .build()?,
            );
            recognizer = Some(MarkdownRecognizer::load(request)?);
        }
        let path = render_page(source, temp, page_number)?;
        let image = image::open(&path)?.to_rgb8();
        let mut coordinate_results = coordinate.as_ref().unwrap().predict(vec![image.clone()])?;
        let ocr = coordinate_results
            .pop()
            .ok_or_else(|| anyhow!("EMPTY_OCR_RESULT: 第 {page_number} 页"))?;
        let markdown = recognizer.as_ref().unwrap().parse(&image, page_number)?;
        let mut regions = Vec::new();
        let mut confidence_sum = 0.0f32;
        for region in ocr.text_regions {
            if let Some((text, confidence)) = region.text_with_confidence() {
                let xs: Vec<_> = region.bounding_box.points.iter().map(|p| p.x).collect();
                let ys: Vec<_> = region.bounding_box.points.iter().map(|p| p.y).collect();
                if xs.is_empty() || ys.is_empty() {
                    continue;
                }
                let mut bbox = [
                    xs.iter().copied().fold(f32::INFINITY, f32::min),
                    ys.iter().copied().fold(f32::INFINITY, f32::min),
                    xs.iter().copied().fold(f32::NEG_INFINITY, f32::max),
                    ys.iter().copied().fold(f32::NEG_INFINITY, f32::max),
                ];
                bbox[0] = bbox[0].clamp(0.0, image.width() as f32);
                bbox[2] = bbox[2].clamp(bbox[0], image.width() as f32);
                bbox[1] = bbox[1].clamp(0.0, image.height() as f32);
                bbox[3] = bbox[3].clamp(bbox[1], image.height() as f32);
                confidence_sum += confidence;
                regions.push(Region {
                    text: text.into(),
                    bbox,
                    confidence: Some(confidence),
                })
            }
        }
        let plain_text = regions
            .iter()
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let confidence = (!regions.is_empty()).then_some(confidence_sum / regions.len() as f32);
        pages.push(Page {
            page_number,
            width: Some(image.width() as f32),
            height: Some(image.height() as f32),
            plain_text,
            markdown,
            regions,
            confidence,
            native_text: false,
        });
        std::fs::remove_file(path)?;
        write_progress(request, page_number, total)?;
    }
    Ok(pages)
}

#[cfg(not(feature = "models"))]
fn recognize(
    request: &ProcessRequest,
    source: &Path,
    _temp: &Path,
    total: u32,
) -> Result<Vec<Page>> {
    let mut document = Document::from_file(source)?;
    let mut pages = Vec::new();
    for number in 1..=total {
        pages.push(native_page(&mut document, number)?.ok_or_else(|| anyhow!(
            "MODEL_RUNTIME_MISSING: 第 {number} 页需要 OCR，请用 --features models（Apple Silicon 可用 metal）构建文档引擎"
        ))?);
        write_progress(request, number, total)?;
    }
    Ok(pages)
}

fn add_search_layer(source: &Path, output: &Path, font_path: &Path, pages: &[Page]) -> Result<()> {
    // The fallback extractor also reads text inside PDF Form XObjects.
    let existing_pages = pdf_extract::extract_text_by_pages(source).unwrap_or_default();
    let mut doc = Document::from_file(source)?;
    let font = doc.embed_font(&std::fs::read(font_path)?)?;
    for page in pages {
        if page.native_text {
            continue;
        }
        let image_w = page.width.unwrap_or(1.0).max(1.0);
        let image_h = page.height.unwrap_or(1.0).max(1.0);
        let (pdf_w, pdf_h) = doc.page(page.page_number)?.size()?;
        let sx = pdf_w / image_w;
        let sy = pdf_h / image_h;
        let existing = existing_pages
            .get(page.page_number as usize - 1)
            .map(|text| {
                text.chars()
                    .filter(|c| !c.is_whitespace())
                    .collect::<String>()
            })
            .unwrap_or_default();
        let runs: Vec<_> = page
            .regions
            .iter()
            .filter(|r| !r.text.trim().is_empty() && r.confidence.unwrap_or(1.0) >= 0.45)
            .filter(|r| {
                let text: String = r.text.chars().filter(|c| !c.is_whitespace()).collect();
                !existing.contains(&text)
            })
            .map(|r| TextRun {
                text: r.text.clone(),
                font,
                x: r.bbox[0] * sx,
                y: pdf_h - r.bbox[3] * sy,
                font_size: ((r.bbox[3] - r.bbox[1]) * sy).max(4.0),
                color: Color::Rgb([0.0; 3]),
                render_mode: 3,
            })
            .collect();
        doc.page(page.page_number)?.add_invisible_text_runs(&runs)?;
    }
    doc.save(output)?;
    Ok(())
}

fn process(request: ProcessRequest) -> Result<ProcessResult> {
    let source = Path::new(&request.source_path);
    let before = sha256_file(source)?;
    if before != request.source_sha256 {
        return Err(anyhow!("SOURCE_CHANGED: 开始处理前哈希不一致"));
    }
    let temp = tempfile::tempdir()?;
    let output_dir = Path::new(&request.output_dir);
    std::fs::create_dir_all(output_dir)?;
    let converted = temp.path().join("image.pdf");
    let pdf_source = if source
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"))
    {
        source
    } else {
        raster_pdf(source, &converted)?;
        converted.as_path()
    };
    let total = Document::from_file(pdf_source)?.page_count();
    if total == 0 {
        return Err(anyhow!("EMPTY_DOCUMENT: 文档没有页面"));
    }
    write_progress(&request, 0, total)?;
    let pages = recognize(&request, pdf_source, temp.path(), total)?;
    let pdf = output_dir.join("source.searchable.pdf");
    let ir = output_dir.join("source.document.json");
    let md = output_dir.join("source.md");
    let font = request
        .cjk_font_path
        .as_deref()
        .ok_or_else(|| anyhow!("FONT_MISSING: CASY_OCR_FONT 未配置"))?;
    add_search_layer(pdf_source, &pdf, Path::new(font), &pages)?;
    std::fs::write(&ir, serde_json::to_vec_pretty(&pages)?)?;
    std::fs::write(
        &md,
        pages
            .iter()
            .map(|p| format!("<!-- page {} -->\n{}", p.page_number, p.markdown))
            .collect::<Vec<_>>()
            .join("\n\n---\n\n"),
    )?;
    let after = sha256_file(source)?;
    if after != before {
        return Err(anyhow!("SOURCE_CHANGED: 处理过程修改了原文件"));
    }
    let native_only = pages.iter().all(|page| page.native_text);
    let model_version = if native_only {
        None
    } else {
        let dir = request
            .paddle_model_dir
            .as_ref()
            .or(request.ovis_model_dir.as_ref())
            .ok_or_else(|| anyhow!("MODEL_MISSING: 页面识别模型"))?;
        Some(format!(
            "weights-sha256:{}",
            sha256_file(&Path::new(dir).join("model.safetensors"))?
        ))
    };
    Ok(ProcessResult {
        source_sha256: after,
        engine: if native_only {
            "harumi-native"
        } else if request.paddle_model_dir.is_some() {
            "oar-ppocrv5+paddleocr-vl"
        } else {
            "oar-ppocrv5+ovisocr2"
        }
        .into(),
        model_version,
        searchable_pdf_path: pdf.display().to_string(),
        page_ir_path: ir.display().to_string(),
        markdown_path: md.display().to_string(),
        pages,
    })
}

fn run() -> Result<()> {
    let command = std::env::args().nth(1).unwrap_or_default();
    match command.as_str() {
        "probe" => println!("{}", serde_json::to_string(&probe())?),
        "process" => {
            let mut input = Vec::new();
            std::io::stdin().read_to_end(&mut input)?;
            let request: ProcessRequest = serde_json::from_slice(&input)?;
            let _ = &request.job_id;
            println!("{}", serde_json::to_string(&process(request)?)?)
        }
        _ => return Err(anyhow!("usage: casy-doc-engine <probe|process>")),
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_text_is_reused_and_large_scanned_regions_are_not_skipped() {
        let font = Path::new("/System/Library/Fonts/Supplemental/Arial.ttf");
        if !font.is_file() {
            return;
        }
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("digital.pdf");
        let mut document = Document::new((595.0, 842.0)).unwrap();
        let font_id = document.embed_font(&std::fs::read(font).unwrap()).unwrap();
        for row in 0..6 {
            document
                .page(1)
                .unwrap()
                .add_text(
                    "Original evidence and contract information",
                    font_id,
                    [30.0, 700.0 - row as f32 * 30.0],
                    12.0,
                    [0.0; 3],
                )
                .unwrap();
        }
        document.save(&source).unwrap();
        let mut reloaded = Document::from_file(&source).unwrap();
        let mut page = native_page(&mut reloaded, 1).unwrap().unwrap();
        assert!(page.native_text);
        let before = reloaded.extract_text(1).unwrap();
        let searchable = temp.path().join("searchable.pdf");
        add_search_layer(&source, &searchable, font, &[page.clone()]).unwrap();
        assert_eq!(
            Document::from_file(&searchable)
                .unwrap()
                .extract_text(1)
                .unwrap(),
            before
        );
        page.native_text = false;
        add_search_layer(&source, &searchable, font, &[page]).unwrap();
        assert_eq!(
            Document::from_file(&searchable)
                .unwrap()
                .extract_text(1)
                .unwrap(),
            before
        );
        let image = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            100,
            100,
            image::Rgb([255; 3]),
        ));
        let mut png = std::io::Cursor::new(Vec::new());
        image.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let mut document = Document::from_file(&source).unwrap();
        document
            .page(1)
            .unwrap()
            .add_image(png.get_ref(), [0.0, 0.0, 595.0, 500.0])
            .unwrap();
        document.save(&source).unwrap();
        assert!(
            native_page(&mut Document::from_file(source).unwrap(), 1)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn page_renderer_handles_double_digit_pages_without_accumulating_images() {
        if !command_exists("pdftoppm") {
            return;
        }
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("pages.pdf");
        let mut document = Document::new((200.0, 200.0)).unwrap();
        for page in 1..12 {
            document
                .insert_blank_page(page, (200.0 + page as f32, 200.0))
                .unwrap();
        }
        document.save(&source).unwrap();
        let render_dir = temp.path().join("render");
        std::fs::create_dir(&render_dir).unwrap();
        for page in [1, 9, 10, 12] {
            let path = render_page(&source, &render_dir, page).unwrap();
            assert!(image::open(&path).is_ok());
            assert_eq!(std::fs::read_dir(&render_dir).unwrap().count(), 1);
        }
    }

    #[test]
    fn image_input_preserves_original_pixels_in_derived_pdf() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("evidence.png");
        let output = temp.path().join("evidence.pdf");
        image::RgbImage::from_pixel(200, 300, image::Rgb([255, 255, 255]))
            .save(&source)
            .unwrap();
        let before = sha256_file(&source).unwrap();
        raster_pdf(&source, &output).unwrap();
        assert_eq!(Document::from_file(output).unwrap().page_count(), 1);
        assert_eq!(before, sha256_file(&source).unwrap());
    }

    #[cfg(feature = "models")]
    #[test]
    #[ignore = "requires local verified model assets, font and CASY_OCR_QA_DIR"]
    fn real_chinese_scanned_pdf() {
        let root = PathBuf::from(std::env::var("CASY_OCR_QA_DIR").expect("CASY_OCR_QA_DIR"));
        std::fs::create_dir_all(&root).unwrap();
        let font_path = std::env::var("CASY_OCR_FONT").unwrap();
        let born_digital = root.join("synthetic-original.pdf");
        let scan = root.join("synthetic-scan.pdf");
        let expected = [
            [
                "民事案件证据目录",
                "原告：张明",
                "被告：华丰公司",
                "第三人：李华",
                "合同金额：128000元",
            ],
            [
                "开庭通知书",
                "案号：（2026）沪0101民初123号",
                "开庭日期：2026年9月15日",
                "地点：第一法庭",
                "请携带身份证件及证据原件",
            ],
        ];
        let mut document = Document::new((595.0, 842.0)).unwrap();
        document.insert_blank_page(1, (595.0, 842.0)).unwrap();
        let font = document
            .embed_font(&std::fs::read(&font_path).unwrap())
            .unwrap();
        for (index, lines) in expected.iter().enumerate() {
            for (row, text) in lines.iter().enumerate() {
                document
                    .page(index as u32 + 1)
                    .unwrap()
                    .add_text(
                        text,
                        font,
                        [50.0, 760.0 - row as f32 * 55.0],
                        20.0,
                        [0.0; 3],
                    )
                    .unwrap();
            }
        }
        document.save(&born_digital).unwrap();
        let render_dir = root.join("fixture-render");
        std::fs::create_dir_all(&render_dir).unwrap();
        let mut scanned = Document::new((595.0, 842.0)).unwrap();
        scanned.insert_blank_page(1, (595.0, 842.0)).unwrap();
        for page in 1..=2 {
            let image = render_page(&born_digital, &render_dir, page).unwrap();
            scanned
                .page(page)
                .unwrap()
                .add_image(&std::fs::read(image).unwrap(), [0.0, 0.0, 595.0, 842.0])
                .unwrap();
        }
        scanned.save(&scan).unwrap();
        assert!(
            Document::from_file(&scan)
                .unwrap()
                .extract_text(1)
                .unwrap()
                .trim()
                .is_empty()
        );
        let hash = sha256_file(&scan).unwrap();
        let started = std::time::Instant::now();
        let result = process(ProcessRequest {
            job_id: "real-smoke".into(),
            source_path: scan.display().to_string(),
            source_sha256: hash.clone(),
            output_dir: root.join("result").display().to_string(),
            coordinate_model_dir: std::env::var("CASY_PPOCR_MODEL_DIR").ok(),
            ovis_model_dir: std::env::var("CASY_OVISOCR2_MODEL_DIR").ok(),
            paddle_model_dir: std::env::var("CASY_PADDLEOCR_VL_MODEL_DIR").ok(),
            cjk_font_path: Some(font_path),
            device: std::env::var("CASY_OCR_DEVICE").unwrap_or("cpu".into()),
        })
        .unwrap();
        std::fs::write(
            root.join("result.json"),
            serde_json::to_vec_pretty(&result).unwrap(),
        )
        .unwrap();
        eprintln!(
            "OCR elapsed: {:.2}s; pages: {}",
            started.elapsed().as_secs_f64(),
            result.pages.len()
        );
        assert_eq!(result.pages.len(), 2);
        assert_eq!(hash, sha256_file(&scan).unwrap());
        let searchable = Document::from_file(&result.searchable_pdf_path).unwrap();
        for (index, page) in result.pages.iter().enumerate() {
            for keyword in if index == 0 {
                vec!["第三人", "李华", "128000"]
            } else {
                vec!["开庭", "2026", "法庭"]
            } {
                assert!(
                    page.plain_text.contains(keyword),
                    "plain text missing {keyword}: {}",
                    page.plain_text
                );
                assert!(
                    page.markdown.contains(keyword),
                    "Markdown missing {keyword}: {}",
                    page.markdown
                );
                assert!(
                    searchable
                        .extract_text(index as u32 + 1)
                        .unwrap()
                        .contains(keyword),
                    "PDF missing {keyword}"
                );
            }
        }
    }

    #[test]
    fn searchable_pdf_is_derived_and_source_is_unchanged() {
        let font = [
            "/System/Library/Fonts/Geneva.ttf",
            "/System/Library/Fonts/Supplemental/Arial.ttf",
        ]
        .into_iter()
        .map(Path::new)
        .find(|path| path.is_file());
        let Some(font) = font else { return };
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source.pdf");
        let output = temp.path().join("source.searchable.pdf");
        Document::new((200.0, 200.0))
            .unwrap()
            .save(&source)
            .unwrap();
        let before = sha256_file(&source).unwrap();
        let pages = vec![Page {
            page_number: 1,
            width: Some(200.0),
            height: Some(200.0),
            plain_text: "Searchable evidence".into(),
            markdown: "Searchable evidence".into(),
            regions: vec![Region {
                text: "Searchable evidence".into(),
                bbox: [10.0, 10.0, 150.0, 30.0],
                confidence: Some(0.99),
            }],
            confidence: Some(0.99),
            native_text: false,
        }];
        add_search_layer(&source, &output, font, &pages).unwrap();
        assert_ne!(source, output);
        assert_eq!(before, sha256_file(&source).unwrap());
        assert!(
            Document::from_file(&output)
                .unwrap()
                .extract_text(1)
                .unwrap()
                .contains("Searchable evidence")
        );
    }
}
