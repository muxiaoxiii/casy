use anyhow::{Context, Result, anyhow};
use harumi::{Color, Document, TextRun};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
mod source_map;
#[cfg(feature = "models")]
mod layout;
#[cfg(feature = "models")]
mod embedding;

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
    cjk_font_path: Option<String>,
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
    source_map_path: String,
    pages: Vec<Page>,
}

fn command_exists(name: &str) -> bool {
    std::process::Command::new(if name == "pdftoppm" { renderer_path() } else { PathBuf::from(name) })
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
        .or_else(|| {
            let exe = std::env::current_exe().ok()?;
            let root = exe.parent()?.parent()?;
            let relative = match name {
                "CASY_PPOCR_MODEL_DIR" => "models/ppocrv6-medium",
                "CASY_OCR_FONT" => "fonts/NotoSansCJK-Regular.ttf",
                "CASY_LAYOUT_MODEL" => "models/layout/pp-doclayout_plus-l.onnx",
                "FONTCONFIG_FILE" => "fonts/fonts.conf",
                "CASY_PDFTOPPM" => if cfg!(windows) { "bin/pdftoppm.exe" } else { "bin/pdftoppm" },
                _ => return None,
            };
            let path = root.join(relative);
            path.exists().then_some(path)
        })
}

fn renderer_path() -> PathBuf { env_path("CASY_PDFTOPPM").unwrap_or_else(|| PathBuf::from("pdftoppm")) }
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
        ["det.onnx", "rec.onnx"]
            .iter()
            .all(|name| dir.join(name).is_file())
            && (dir.join("dict.txt").is_file() || dir.join("rec.yml").is_file())
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
    if !font {
        missing.push("CASY_OCR_FONT".into())
    }
    EngineStatus {
        available: cfg!(feature = "models") && renderer && coord && font,
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
    let mut command = std::process::Command::new(renderer_path());
    if let Some(config) = env_path("FONTCONFIG_FILE") { command.env("FONTCONFIG_FILE",config); }
    let output = command
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

#[cfg(feature = "models")]
fn model_dictionary(dir: &Path) -> Result<String> {
    if dir.join("rec.yml").is_file() {
        let config: serde_yaml_ng::Value =
            serde_yaml_ng::from_slice(&std::fs::read(dir.join("rec.yml"))?)?;
        let characters = config["PostProcess"]["character_dict"]
            .as_sequence()
            .ok_or_else(|| anyhow!("MODEL_CONFIG_INVALID: character_dict"))?;
        let characters = characters
            .iter()
            .map(|v| {
                v.as_str()
                    .ok_or_else(|| anyhow!("MODEL_CONFIG_INVALID: dictionary entry"))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(characters.join("\n") + "\n")
    } else {
        Ok(std::fs::read_to_string(dir.join("dict.txt"))?)
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
    let mut layout_predictor = None;
    let mut pages = Vec::new();
    // Keep only one rendered page and its model inputs alive at a time.
    for page_number in 1..=total {
        // The visible raster is authoritative, including PDFs with deceptive text layers.
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
            for path in [&det, &rec] {
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
                    .character_dict_content(model_dictionary(coord_dir)?)
                    .return_word_box(true)
                    .region_batch_size(6)
                    .build()?,
            );
            let layout_path = env_path("CASY_LAYOUT_MODEL").or_else(|| {
                let candidate = coord_dir.parent()?.join("layout/pp-doclayout_plus-l.onnx");
                candidate.is_file().then_some(candidate)
            });
            if let Some(path) = layout_path {
                layout_predictor = Some(oar_ocr::predictors::LayoutDetectionPredictor::builder()
                    .model_name("pp_doclayout_plus_l")
                    .build(path)?);
            }
        }
        let path = render_page(source, temp, page_number)?;
        let image = image::open(&path)?.to_rgb8();
        let mut coordinate_results = coordinate.as_ref().unwrap().predict(vec![image.clone()])?;
        let ocr = coordinate_results
            .pop()
            .ok_or_else(|| anyhow!("EMPTY_OCR_RESULT: 第 {page_number} 页"))?;
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
        if let Some(predictor) = &layout_predictor {
            let output = predictor.predict(vec![image.clone()])?;
            if let Some(elements) = output.elements.first() {
                layout::order_regions(&mut regions, elements, image.width() as f32, image.height() as f32);
                let blocks: Vec<_> = elements.iter().map(|e| serde_json::json!({"kind":e.element_type,"confidence":e.score,"bbox":[e.bbox.x_min(),e.bbox.y_min(),e.bbox.x_max(),e.bbox.y_max()]})).collect();
                std::fs::write(Path::new(&request.output_dir).join(format!("page-{page_number}.layout.json")), serde_json::to_vec_pretty(&blocks)?)?;
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
            markdown: plain_text.clone(),
            plain_text,
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
    _request: &ProcessRequest,
    _source: &Path,
    _temp: &Path,
    _total: u32,
) -> Result<Vec<Page>> {
    Err(anyhow!(
        "MODEL_RUNTIME_MISSING: 请用 --features models 构建文档引擎；未验证的 PDF 文本层不能代替 OCR"
    ))
}

fn add_search_layer(source: &Path, output: &Path, font_path: &Path, pages: &[Page]) -> Result<()> {
    // Rebuild only the derived PDF from visible pixels. Keeping the old hidden
    // layer would preserve forged text in external viewers even after correct OCR.
    let mut original = Document::from_file(source)?;
    let mut doc = Document::new(original.page(1)?.size()?)?;
    let temp = tempfile::tempdir()?;
    let font = doc.embed_font(&std::fs::read(font_path)?)?;
    for page in pages {
        let (pdf_w, pdf_h) = original.page(page.page_number)?.size()?;
        if page.page_number > 1 {
            doc.insert_blank_page(page.page_number - 1, (pdf_w, pdf_h))?;
        }
        let raster = render_page(source, temp.path(), page.page_number)?;
        doc.page(page.page_number)?
            .add_image(&std::fs::read(raster)?, [0.0, 0.0, pdf_w, pdf_h])?;
        let image_w = page.width.unwrap_or(1.0).max(1.0);
        let image_h = page.height.unwrap_or(1.0).max(1.0);
        let sx = pdf_w / image_w;
        let sy = pdf_h / image_h;
        let runs: Vec<_> = page
            .regions
            .iter()
            .filter(|r| !r.text.trim().is_empty() && r.confidence.unwrap_or(1.0) >= 0.45)
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

fn process(mut request: ProcessRequest) -> Result<ProcessResult> {
    process_pages(&mut request, None)
}

fn process_pages(request: &mut ProcessRequest, corrected: Option<Vec<Page>>) -> Result<ProcessResult> {
    if request.coordinate_model_dir.is_none() { request.coordinate_model_dir = env_path("CASY_PPOCR_MODEL_DIR").map(|p|p.to_string_lossy().into_owned()); }
    if request.cjk_font_path.is_none() { request.cjk_font_path = env_path("CASY_OCR_FONT").map(|p|p.to_string_lossy().into_owned()); }
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
    let is_correction = corrected.is_some();
    let pages = if let Some(pages) = corrected {
        anyhow::ensure!(pages.len() == total as usize && pages.iter().enumerate().all(|(i,p)|p.page_number as usize == i+1), "CORRECTION_PAGES_INVALID");
        pages
    } else { recognize(request, pdf_source, temp.path(), total)? };
    let pdf = output_dir.join("source.searchable.pdf");
    let ir = output_dir.join("source.document.json");
    let md = output_dir.join("source.md");
    let map_path = output_dir.join("source.map.json");
    let font = request
        .cjk_font_path
        .as_deref()
        .ok_or_else(|| anyhow!("FONT_MISSING: CASY_OCR_FONT 未配置"))?;
    add_search_layer(pdf_source, &pdf, Path::new(font), &pages)?;
    std::fs::write(&ir, serde_json::to_vec_pretty(&pages)?)?;
    let (markdown, source_map) = source_map::build(&pages, &before);
    std::fs::write(&md, markdown)?;
    std::fs::write(&map_path, serde_json::to_vec_pretty(&source_map)?)?;
    let after = sha256_file(source)?;
    if after != before {
        return Err(anyhow!("SOURCE_CHANGED: 处理过程修改了原文件"));
    }
    let dir = Path::new(
        request
            .coordinate_model_dir
            .as_deref()
            .ok_or_else(|| anyhow!("MODEL_MISSING"))?,
    );
    let dictionary = if dir.join("rec.yml").is_file() {
        "rec.yml"
    } else {
        "dict.txt"
    };
    let layout_path = env_path("CASY_LAYOUT_MODEL").or_else(|| {
        let candidate=dir.parent()?.join("layout/pp-doclayout_plus-l.onnx");
        candidate.is_file().then_some(candidate)
    });
    let model_version = Some(format!(
        "det:{};rec:{};dictionary:{};layout:{}",
        sha256_file(&dir.join("det.onnx"))?,
        sha256_file(&dir.join("rec.onnx"))?,
        sha256_file(&dir.join(dictionary))?,
        layout_path.as_deref().map(sha256_file).transpose()?.unwrap_or_else(||"none".into())
    ));
    Ok(ProcessResult {
        source_sha256: after,
        engine: if is_correction { "paddle-onnx-corrected" } else { "paddle-onnx-visual" }.into(),
        model_version,
        searchable_pdf_path: pdf.display().to_string(),
        page_ir_path: ir.display().to_string(),
        markdown_path: md.display().to_string(),
        source_map_path: map_path.display().to_string(),
        pages,
    })
}

fn run() -> Result<()> {
    let command = std::env::args().nth(1).unwrap_or_default();
    match command.as_str() {
        #[cfg(feature = "models")]
        "embed" => embedding::serve()?,
        "probe" => println!("{}", serde_json::to_string(&probe())?),
        "process" => {
            let mut input = Vec::new();
            std::io::stdin().read_to_end(&mut input)?;
            let request: ProcessRequest = serde_json::from_slice(&input)?;
            let _ = &request.job_id;
            println!("{}", serde_json::to_string(&process(request)?)?)
        }
        "revise" => {
            #[derive(Deserialize)]
            struct Revision { request: ProcessRequest, pages: Vec<Page> }
            let mut input = Vec::new();
            std::io::stdin().take(128 * 1024 * 1024 + 1).read_to_end(&mut input)?;
            anyhow::ensure!(input.len() <= 128 * 1024 * 1024, "CORRECTION_TOO_LARGE");
            let mut revision: Revision = serde_json::from_slice(&input)?;
            println!("{}", serde_json::to_string(&process_pages(&mut revision.request, Some(revision.pages))?)?);
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
    fn derived_pdf_discards_forged_hidden_text() {
        if !command_exists("pdftoppm") {
            return;
        }
        let font = Path::new("/System/Library/Fonts/Supplemental/Arial.ttf");
        if !font.is_file() {
            return;
        }
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("forged.pdf");
        let output = temp.path().join("derived.pdf");
        let mut doc = Document::new((400.0, 400.0)).unwrap();
        let f = doc.embed_font(&std::fs::read(font).unwrap()).unwrap();
        doc.page(1)
            .unwrap()
            .add_text("Actual amount 128000", f, [30.0, 300.0], 16.0, [0.0; 3])
            .unwrap();
        doc.page(1)
            .unwrap()
            .add_invisible_text_runs(&[TextRun {
                text: "FORGED amount 999999".into(),
                font: f,
                x: 30.0,
                y: 250.0,
                font_size: 16.0,
                color: Color::Rgb([0.0; 3]),
                render_mode: 3,
            }])
            .unwrap();
        doc.save(&source).unwrap();
        let before = sha256_file(&source).unwrap();
        assert!(
            pdf_extract::extract_text(&source)
                .unwrap()
                .contains("999999")
        );
        let page = Page {
            page_number: 1,
            width: Some(400.0),
            height: Some(400.0),
            plain_text: "Actual amount 128000".into(),
            markdown: "Actual amount 128000".into(),
            regions: vec![Region {
                text: "Actual amount 128000".into(),
                bbox: [30.0, 84.0, 230.0, 100.0],
                confidence: Some(0.99),
            }],
            confidence: Some(0.99),
            native_text: false,
        };
        add_search_layer(&source, &output, font, &[page]).unwrap();
        let extracted = pdf_extract::extract_text(&output).unwrap();
        assert!(extracted.contains("128000"));
        assert!(!extracted.contains("999999"));
        assert_eq!(before, sha256_file(&source).unwrap());
        let original_render = render_page(&source, temp.path(), 1).unwrap();
        let original_pixels = image::open(original_render).unwrap().to_rgb8();
        let derived_render = render_page(&output, temp.path(), 1).unwrap();
        let derived_pixels = image::open(derived_render).unwrap().to_rgb8();
        assert_eq!(original_pixels.dimensions(), derived_pixels.dimensions());
        let mean_error: f64 = original_pixels
            .as_raw()
            .iter()
            .zip(derived_pixels.as_raw())
            .map(|(a, b)| (*a as f64 - *b as f64).abs())
            .sum::<f64>()
            / original_pixels.as_raw().len() as f64;
        assert!(mean_error < 2.0, "visible page changed: {mean_error}");
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
            cjk_font_path: Some(font_path),
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

    #[cfg(feature = "models")]
    #[test]
    #[ignore = "requires local medium models and a multilingual font"]
    fn real_multilingual_and_forged_text_pdf() {
        let root = PathBuf::from(std::env::var("CASY_OCR_QA_DIR").unwrap()).join("multilingual");
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("mixed-original.pdf");
        let font_path = std::env::var("CASY_OCR_FONT").unwrap();
        let mut doc = Document::new((595.0, 842.0)).unwrap();
        doc.insert_blank_page(1, (595.0, 842.0)).unwrap();
        let font = doc.embed_font(&std::fs::read(&font_path).unwrap()).unwrap();
        let latin_font = doc
            .embed_font(&std::fs::read("/System/Library/Fonts/Supplemental/Arial.ttf").unwrap())
            .unwrap();
        for (index, text) in [
            "第三人赔偿金额为128000元",
            "Evidence of contractual liability",
            "Prüfung des Schadensersatzes",
            "Réparation du préjudice français",
            "損害賠償請求と契約書",
        ]
        .iter()
        .enumerate()
        {
            let chosen = if (1..=3).contains(&index) {
                latin_font
            } else {
                font
            };
            doc.page(1)
                .unwrap()
                .add_text(
                    text,
                    chosen,
                    [40.0, 760.0 - index as f32 * 65.0],
                    20.0,
                    [0.0; 3],
                )
                .unwrap();
        }
        doc.page(1)
            .unwrap()
            .add_invisible_text_runs(&[TextRun {
                text: "FORGED SECRET 999999".into(),
                font: latin_font,
                x: 40.0,
                y: 400.0,
                font_size: 16.0,
                color: Color::Rgb([0.0; 3]),
                render_mode: 3,
            }])
            .unwrap();
        doc.page(1)
            .unwrap()
            .add_text("Schadens-", latin_font, [40.0, 60.0], 20.0, [0.0; 3])
            .unwrap();
        doc.page(2)
            .unwrap()
            .add_text(
                "ersatz und Beweis",
                latin_font,
                [40.0, 760.0],
                20.0,
                [0.0; 3],
            )
            .unwrap();
        for (row, c) in "損害賠償請求".chars().enumerate() {
            doc.page(2)
                .unwrap()
                .add_text(
                    &c.to_string(),
                    font,
                    [420.0, 650.0 - row as f32 * 30.0],
                    22.0,
                    [0.0; 3],
                )
                .unwrap();
        }
        doc.save(&source).unwrap();
        let before = sha256_file(&source).unwrap();
        let result = process(ProcessRequest {
            job_id: "multilingual".into(),
            source_path: source.display().to_string(),
            source_sha256: before.clone(),
            output_dir: root.join("result").display().to_string(),
            coordinate_model_dir: std::env::var("CASY_PPOCR_MODEL_DIR").ok(),
            cjk_font_path: Some(font_path),
        })
        .unwrap();
        std::fs::write(
            root.join("result.json"),
            serde_json::to_vec_pretty(&result).unwrap(),
        )
        .unwrap();
        for expected in [
            "第三人",
            "128000",
            "Evidence",
            "Prüfung",
            "Schadensersatzes",
            "Réparation",
            "préjudice",
            "français",
            "損害賠償",
        ] {
            assert!(
                result.pages[0].plain_text.contains(expected),
                "missing {expected}: {}",
                result.pages[0].plain_text
            );
        }
        let compact: String = result.pages[1]
            .plain_text
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        assert!(
            compact.contains("損害賠償請求"),
            "vertical Japanese: {compact}"
        );
        assert!(!result.pages.iter().any(|p| p.plain_text.contains("999999")));
        let extracted = pdf_extract::extract_text(&result.searchable_pdf_path).unwrap();
        assert!(!extracted.contains("999999"));
        assert_eq!(before, sha256_file(&source).unwrap());
    }
}
