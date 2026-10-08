use anyhow::{Context, Result, anyhow};
use harumi::{Color, Document, TextFragment, TextRun};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
mod source_map;
mod assets;
#[cfg(feature = "models")]
mod layout;
#[cfg(feature = "models")]
mod table;
#[cfg(feature = "models")]
mod visual;
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
    korean_model_available: bool,
    layout_model_available: bool,
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
    #[serde(default)]
    markdown_only: bool,
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
struct LayoutBlock {
    id: String,
    kind: String,
    bbox: [f32; 4],
    confidence: f32,
    reading_order: u32,
    region_indices: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageLayout {
    model: String,
    blocks: Vec<LayoutBlock>,
    tables: Vec<serde_json::Value>,
    visuals: Vec<serde_json::Value>,
    unresolved_tables: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageTiming {
    render_ms: u64,
    ocr_ms: u64,
    layout_ms: u64,
    total_ms: u64,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    layout: Option<PageLayout>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    timing: Option<PageTiming>,
    #[serde(default)]
    native_text: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProcessResult {
    source_sha256: String,
    engine: String,
    model_version: Option<String>,
    searchable_pdf_path: Option<String>,
    page_ir_path: String,
    markdown_path: String,
    source_map_path: String,
    pages: Vec<Page>,
    elapsed_ms: u64,
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
                "CASY_KOREAN_MODEL_DIR" => "models/korean-ppocrv5-mobile",
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
    let korean = env_path("CASY_KOREAN_MODEL_DIR").is_some_and(|dir| {
        dir.join("rec.onnx").is_file() && dir.join("rec.yml").is_file()
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
    let layout = env_path("CASY_LAYOUT_MODEL").is_some_and(|path| path.is_file());
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
    if !korean {
        missing.push("CASY_KOREAN_MODEL_DIR".into())
    }
    if !font {
        missing.push("CASY_OCR_FONT".into())
    }
    if !layout {
        missing.push("CASY_LAYOUT_MODEL".into())
    }
    EngineStatus {
        available: cfg!(feature = "models") && renderer && coord && korean && layout && font,
        executable: std::env::current_exe()
            .ok()
            .map(|p| p.display().to_string()),
        version: Some(env!("CARGO_PKG_VERSION").into()),
        renderer_available: renderer,
        coordinate_model_available: coord,
        korean_model_available: korean,
        layout_model_available: layout,
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

fn write_progress(
    request: &ProcessRequest,
    phase: &str,
    current: u32,
    total: u32,
    started: &std::time::Instant,
    page_timing: Option<&PageTiming>,
) -> Result<()> {
    let root = Path::new(&request.output_dir);
    let pending = root.join("progress.pending");
    let elapsed_ms = started.elapsed().as_millis() as u64;
    let remaining_ms = (current > 0 && current < total)
        .then(|| elapsed_ms.saturating_mul((total - current) as u64) / current as u64);
    std::fs::write(
        &pending,
        serde_json::to_vec(&serde_json::json!({
            "phase": phase, "currentPage": current, "totalPages": total,
            "elapsedMs": elapsed_ms, "remainingMs": remaining_ms,
            "pageTiming": page_timing
        }))?,
    )?;
    std::fs::rename(pending, root.join("progress.json"))?;
    Ok(())
}

fn read_raster(source: &Path) -> Result<image::DynamicImage> {
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
    Ok(reader.decode()?)
}

fn raster_pdf(source: &Path, output: &Path) -> Result<()> {
    let image = read_raster(source)?;
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

fn raster_ocr_image(source: &Path) -> Result<image::RgbImage> {
    let image = read_raster(source)?;
    let image = if image.width().max(image.height()) > 2400 {
        image.resize(2400, 2400, image::imageops::FilterType::Lanczos3)
    } else { image };
    // Match a PDF's white page when screenshots contain transparent pixels.
    let rgba = image.to_rgba8();
    Ok(image::RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let pixel = rgba.get_pixel(x, y).0;
        let alpha = pixel[3] as u32;
        image::Rgb(std::array::from_fn(|i| ((pixel[i] as u32 * alpha + 255 * (255 - alpha) + 127) / 255) as u8))
    }))
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
    direct_image: bool,
    pipeline_started: &std::time::Instant,
    assets_manifest: &mut assets::Manifest,
) -> Result<Vec<Page>> {
    use oar_ocr::prelude::*;
    let mut pdf_doc = if !direct_image { Document::from_file(source).ok() } else { None };
    let mut coordinate = None;
    let mut korean_recognizer = None;
    let mut layout_predictor = None;
    let mut pages = Vec::new();
    let mut timings = Vec::new();
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
            let threads = std::env::var("CASY_OCR_THREADS").ok()
                .and_then(|value| value.parse::<usize>().ok())
                .filter(|value| (1..=8).contains(value))
                .unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get().min(4)).unwrap_or(2));
            oar_ocr::core::OrtGlobalThreadPoolOptions::new()
                .with_intra_threads(threads)
                .with_inter_threads(1)
                .with_spin_control(false)
                .commit()?;
            coordinate = Some(
                OAROCRBuilder::new(&det, &rec, &dict)
                    .character_dict_content(model_dictionary(coord_dir)?)
                    .return_word_box(false)
                    // Single-line inference avoids expensive padded medium-model CPU batches.
                    .region_batch_size(1)
                    .build()?,
            );
            let korean_dir = env_path("CASY_KOREAN_MODEL_DIR").or_else(|| {
                let candidate = coord_dir.parent()?.join("korean-ppocrv5-mobile");
                candidate.is_dir().then_some(candidate)
            }).ok_or_else(|| anyhow!("MODEL_MISSING: Korean recognition model"))?;
            let korean_model = korean_dir.join("rec.onnx");
            let korean_config = korean_dir.join("rec.yml");
            for path in [&korean_model, &korean_config] {
                if !path.is_file() {
                    return Err(anyhow!("MODEL_MISSING: {}", path.display()));
                }
            }
            let korean_dict = temp.join("korean-dict.txt");
            std::fs::write(&korean_dict, model_dictionary(&korean_dir)?)?;
            korean_recognizer = Some(
                oar_ocr::predictors::TextRecognitionPredictor::builder()
                    .score_threshold(0.0)
                    .dict_path(&korean_dict)
                    .build(&korean_model)?,
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
        write_progress(request, "rendering", page_number - 1, total, pipeline_started, None)?;
        let page_started = std::time::Instant::now();
        let start = std::time::Instant::now();
        let path = if direct_image { None } else { Some(render_page(source, temp, page_number)?) };
        let image = if let Some(path) = &path { image::open(path)?.to_rgb8() } else { raster_ocr_image(source)? };
        let render_ms = start.elapsed().as_millis();
        write_progress(request, "recognizing", page_number - 1, total, pipeline_started, None)?;
        let ocr_start = std::time::Instant::now();
        let mut coordinate_results = coordinate.as_ref().unwrap().predict(vec![image.clone()])?;
        let ocr = coordinate_results
            .pop()
            .ok_or_else(|| anyhow!("EMPTY_OCR_RESULT: 第 {page_number} 页"))?;
        let mut regions = Vec::new();
        let mut recognition_boxes = Vec::new();
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
                regions.push(Region {
                    text: text.into(),
                    bbox,
                    confidence: Some(confidence),
                });
                recognition_boxes.push(region.bounding_box.clone());
            }
        }
        enhance_regions_with_korean(
            &mut regions,
            &recognition_boxes,
            &image,
            korean_recognizer.as_ref().ok_or_else(|| anyhow!("MODEL_MISSING: Korean recognition model"))?,
        )?;
        let ocr_ms = ocr_start.elapsed().as_millis();
        std::fs::write(Path::new(&request.output_dir).join(format!("page-{page_number}.ocr-lines.json")), serde_json::to_vec_pretty(&regions)?)?;
        let layout_start = std::time::Instant::now();
        write_progress(request, "analyzing_layout", page_number - 1, total, pipeline_started, None)?;
        let mut tables = Vec::new();
        let mut visuals = Vec::new();
        let mut unresolved_tables = 0;
        let mut layout_elements = None;
        if let Some(predictor) = &layout_predictor {
            let output = predictor.predict(vec![image.clone()])?;
            if let Some(elements) = output.elements.first() {
                for element in elements.iter().filter(|e| e.element_type == "table" && e.score >= 0.5) {
                    let bbox = [element.bbox.x_min(), element.bbox.y_min(), element.bbox.x_max(), element.bbox.y_max()];
                    if let Some(table) = table::detect(&image, bbox) { tables.push(table); }
                    else {
                        unresolved_tables += 1;
                        if let Some(v) = visual::Visual::capture(&image, bbox, "unresolved_table")? { visuals.push(v); }
                    }
                }
                for element in elements.iter().filter(|e| visual::is_visual(&e.element_type) && e.score >= 0.35) {
                    let bbox = [element.bbox.x_min(), element.bbox.y_min(), element.bbox.x_max(), element.bbox.y_max()];
                    if let Some(v) = visual::Visual::capture(&image, bbox, &element.element_type)? { visuals.push(v); }
                }
                visual::deduplicate(&mut visuals);
                split_table_crossings(&mut regions, &tables, &image, coordinate.as_ref().unwrap())?;
                layout_elements = Some(elements.clone());
            }
        }
        let native_enhanced = enhance_page_regions(&mut regions, &mut pdf_doc, page_number, image.width() as f32, image.height() as f32);
        let mut blocks = Vec::new();
        if let Some(elements) = &layout_elements {
            layout::order_regions(&mut regions, elements, image.width() as f32, image.height() as f32);
            blocks = layout::describe_blocks(&regions, elements);
        }
        let markdown = table::markdown_with_layout(&regions, &mut tables, unresolved_tables, &visuals, &blocks);
        // Do not retain every page's base64 crops while recognizing the rest of a dossier.
        let markdown = assets::externalize(&markdown, Path::new(&request.output_dir), assets_manifest)?;
        let visual_metadata: Vec<_> = visuals.iter().map(|v| serde_json::json!({"kind":v.kind,"bbox":v.bbox,"representation":"source-image","semanticExtraction":false})).collect();
        let table_metadata = tables.iter().map(serde_json::to_value).collect::<serde_json::Result<Vec<_>>>()?;
        let page_layout = layout_elements.as_ref().map(|_| PageLayout {
            model: "pp-doclayout-plus-l".into(),
            blocks,
            tables: table_metadata,
            visuals: visual_metadata,
            unresolved_tables,
        });
        if let Some(layout) = &page_layout {
            std::fs::write(Path::new(&request.output_dir).join(format!("page-{page_number}.layout.json")), serde_json::to_vec_pretty(layout)?)?;
        }
        let plain_text = regions
            .iter()
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let confidence = (!regions.is_empty()).then_some(regions.iter().filter_map(|r| r.confidence).sum::<f32>() / regions.len() as f32);
        let layout_ms = layout_start.elapsed().as_millis() as u64;
        let page_timing = PageTiming {
            render_ms: render_ms as u64,
            ocr_ms: ocr_ms as u64,
            layout_ms,
            total_ms: page_started.elapsed().as_millis() as u64,
        };
        pages.push(Page {
            page_number,
            width: Some(image.width() as f32),
            height: Some(image.height() as f32),
            markdown,
            plain_text,
            regions,
            confidence,
            layout: page_layout,
            timing: Some(page_timing.clone()),
            native_text: native_enhanced,
        });
        if let Some(path) = path { std::fs::remove_file(path)?; }
        timings.push(serde_json::json!({"page":page_number,"renderMs":page_timing.render_ms,"ocrMs":page_timing.ocr_ms,"layoutMs":page_timing.layout_ms,"totalMs":page_timing.total_ms}));
        std::fs::write(Path::new(&request.output_dir).join("timings.json"), serde_json::to_vec(&timings)?)?;
        write_progress(request, "recognizing", page_number, total, pipeline_started, Some(&page_timing))?;
    }
    Ok(pages)
}

#[cfg(feature = "models")]
fn is_hangul(c: char) -> bool {
    ('\u{ac00}'..='\u{d7a3}').contains(&c)
        || ('\u{1100}'..='\u{11ff}').contains(&c)
        || ('\u{3130}'..='\u{318f}').contains(&c)
}

#[cfg(feature = "models")]
fn korean_candidate_should_replace(
    primary_text: &str,
    primary_score: f32,
    korean_text: &str,
    korean_score: f32,
) -> bool {
    let hangul = korean_text.chars().filter(|c| is_hangul(*c)).count();
    let letters = korean_text.chars().filter(|c| c.is_alphabetic()).count();
    if hangul == 0
        || korean_score < 0.78
        || (hangul == 1 && korean_score < 0.92)
        || hangul * 3 < letters.max(1)
    {
        return false;
    }
    if primary_text.chars().any(is_hangul) {
        return korean_score > primary_score;
    }
    let primary_has_cjk = primary_text
        .chars()
        .any(|c| ('\u{3400}'..='\u{4dbf}').contains(&c) || ('\u{4e00}'..='\u{9fff}').contains(&c));
    korean_score >= primary_score + 0.02 || (primary_has_cjk && korean_score >= 0.88)
}

#[cfg(feature = "models")]
fn enhance_regions_with_korean(
    regions: &mut [Region],
    boxes: &[oar_ocr::processors::BoundingBox],
    image: &image::RgbImage,
    predictor: &oar_ocr::predictors::TextRecognitionPredictor,
) -> Result<usize> {
    use oar_ocr::oarocr::{EdgeProcessor, TextCroppingProcessor};
    use std::sync::Arc;

    if regions.is_empty() || regions.len() != boxes.len() {
        return Ok(0);
    }
    let crops = TextCroppingProcessor::new(true)
        .process((Arc::new(image.clone()), boxes.to_vec()))?;
    let candidates: Vec<_> = crops
        .into_iter()
        .enumerate()
        .filter_map(|(index, crop)| crop.map(|crop| (index, (*crop).clone())))
        .collect();
    let mut replaced = 0;
    for batch in candidates.chunks(8) {
        let output = predictor.predict(batch.iter().map(|(_, crop)| crop.clone()).collect())?;
        for (((index, _), korean_text), korean_score) in batch
            .iter()
            .zip(output.texts.iter())
            .zip(output.scores.iter())
        {
            let region = &mut regions[*index];
            if korean_candidate_should_replace(
                &region.text,
                region.confidence.unwrap_or(0.0),
                korean_text,
                *korean_score,
            ) {
                region.text = korean_text.clone();
                region.confidence = Some(*korean_score);
                replaced += 1;
            }
        }
    }
    Ok(replaced)
}

/// A detector line may span two real cells. Recognize each clipped fragment independently;
/// do not divide characters proportionally or guess values from neighboring columns.
#[cfg(feature = "models")]
fn split_table_crossings(regions: &mut Vec<Region>, tables: &[table::Table], image: &image::RgbImage, ocr: &oar_ocr::prelude::OAROCR) -> Result<()> {
    let mut output = Vec::new();
    for region in regions.iter() {
        let mut replacement = None;
        for table in tables {
            let crossing = table.crossing_cells(region);
            if crossing.len() < 2 || crossing.len() > 6 || region.bbox[3]-region.bbox[1] > 120.0 { continue; }
            let mut fragments = Vec::new();
            let mut complete = true;
            for cell_index in crossing {
                let before = fragments.len();
                let c = &table.cells[cell_index];
                let x0 = region.bbox[0].max(c.bbox[0]+4.0).floor().max(0.0) as u32;
                let y0 = region.bbox[1].max(c.bbox[1]+4.0).floor().max(0.0) as u32;
                let x1 = region.bbox[2].min(c.bbox[2]-4.0).ceil().min(image.width() as f32) as u32;
                let y1 = region.bbox[3].min(c.bbox[3]-4.0).ceil().min(image.height() as f32) as u32;
                if x1<=x0+5 || y1<=y0+5 {complete=false;continue}
                let crop=image::imageops::crop_imm(image,x0,y0,x1-x0,y1-y0).to_image();
                let mut padded=image::RgbImage::from_pixel(crop.width()+24,crop.height()+24,image::Rgb([255,255,255]));
                image::imageops::replace(&mut padded,&crop,12,12);
                for result in ocr.predict(vec![padded])? {
                    for r in result.text_regions {
                        if let Some((text,confidence))=r.text_with_confidence() {
                            let b=&r.bounding_box;
                            fragments.push(Region{text:text.into(),confidence:Some(confidence),bbox:[(b.x_min()+x0 as f32-12.0).clamp(0.0,image.width() as f32),(b.y_min()+y0 as f32-12.0).clamp(0.0,image.height() as f32),(b.x_max()+x0 as f32-12.0).clamp(0.0,image.width() as f32),(b.y_max()+y0 as f32-12.0).clamp(0.0,image.height() as f32)]});
                        }
                    }
                }
                if fragments.len() == before {complete=false;}
            }
            // If any fragment cannot be read, keep the complete original line for review.
            if complete && fragments.len()>=2 {replacement=Some(fragments)}
            break;
        }
        if let Some(fragments)=replacement {output.extend(fragments)}else{output.push(region.clone())}
    }
    *regions=output;
    Ok(())
}

#[cfg(feature = "models")]
fn enhance_page_regions(
    regions: &mut Vec<Region>,
    pdf_doc: &mut Option<Document>,
    page_number: u32,
    image_w: f32,
    image_h: f32,
) -> bool {
    let Some(doc) = pdf_doc.as_mut() else {
        return false;
    };
    let page_size = match doc.page(page_number).and_then(|p| p.size()) {
        Ok(sz) => sz,
        Err(_) => return false,
    };
    let raw_text = match doc.extract_text(page_number) {
        Ok(txt) => txt,
        Err(_) => return false,
    };
    let runs = match doc.extract_text_runs(page_number) {
        Ok(r) => r,
        Err(_) => return false,
    };
    enhance_regions_with_native_stream(
        regions,
        image_w,
        image_h,
        page_size,
        &raw_text,
        &runs,
    )
}

#[cfg(feature = "models")]
fn enhance_regions_with_native_stream(
    regions: &mut Vec<Region>,
    image_w: f32,
    image_h: f32,
    page_size: (f32, f32),
    raw_text: &str,
    runs: &[TextFragment],
) -> bool {
    let (pdf_w, pdf_h) = page_size;
    if raw_text.trim().is_empty() || runs.is_empty() || pdf_w <= 0.0 || pdf_h <= 0.0 || image_w <= 0.0 || image_h <= 0.0 {
        return false;
    }

    // 1. Anti-tamper & CMap corruption checks:
    if raw_text.contains('\0') {
        return false;
    }
    let total_chars = raw_text.chars().count().max(1);
    let replacement_count = raw_text.chars().filter(|c| *c == '\u{fffd}').count();
    if replacement_count as f32 / total_chars as f32 > 0.01 {
        return false;
    }
    let pua_count = raw_text.chars().filter(|c| ('\u{e000}'..='\u{f8ff}').contains(c)).count();
    if pua_count as f32 / total_chars as f32 > 0.02 {
        return false;
    }

    // 2. Concordance check: Visual anchors vs Native text
    let mut anchors = Vec::new();
    for r in regions.iter().filter(|r| r.confidence.unwrap_or(1.0) >= 0.60) {
        for word in r.text.split(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == '，' || c == '。' || c == '(' || c == ')' || c == '（' || c == '）') {
            let trimmed = word.trim();
            if trimmed.len() >= 4 && (trimmed.chars().any(|c| c.is_ascii_digit()) || (trimmed.chars().all(|c| c.is_ascii_alphanumeric()) && trimmed.len() >= 5)) {
                anchors.push(trimmed.to_string());
            }
        }
    }
    if !anchors.is_empty() {
        let matched = anchors.iter().filter(|a| raw_text.contains(a.as_str())).count();
        if matched == 0 || (anchors.len() >= 3 && (matched as f32 / anchors.len() as f32) < 0.50) {
            return false;
        }
    }

    // 3. Filter visible fragments only (invisible == false, font_size >= 1.0)
    let visible_runs: Vec<_> = runs
        .iter()
        .filter(|f| !f.invisible && f.font_size >= 1.0 && !f.text.trim().is_empty())
        .collect();
    if visible_runs.is_empty() {
        return false;
    }

    let sx = pdf_w / image_w;
    let sy = pdf_h / image_h;

    // 4. Assign each visible fragment to the best matching visual region
    let mut region_matched: Vec<Vec<&TextFragment>> = vec![Vec::new(); regions.len()];
    for f in visible_runs {
        let f_mid_y = f.y + f.height * 0.4;
        let mut best_idx = None;
        let mut min_dist = f32::INFINITY;

        for (idx, r) in regions.iter().enumerate() {
            let pdf_x0 = r.bbox[0] * sx;
            let pdf_x1 = r.bbox[2] * sx;
            let pdf_y_bottom = pdf_h - r.bbox[3] * sy;
            let pdf_y_top = pdf_h - r.bbox[1] * sy;

            let h_overlap = f.x <= pdf_x1 + 6.0 && (f.x + f.width) >= pdf_x0 - 6.0;
            let v_overlap = f_mid_y >= pdf_y_bottom - f.height * 0.4 && f_mid_y <= pdf_y_top + f.height * 0.4;

            if h_overlap && v_overlap {
                let r_mid_y = (pdf_y_bottom + pdf_y_top) * 0.5;
                let dist = (f_mid_y - r_mid_y).abs();
                if dist < min_dist {
                    min_dist = dist;
                    best_idx = Some(idx);
                }
            }
        }

        if let Some(idx) = best_idx {
            region_matched[idx].push(f);
        }
    }

    // 5. Update regions with authentic native text
    let mut enhanced_any = false;
    for (idx, matched) in region_matched.into_iter().enumerate() {
        if matched.is_empty() {
            continue;
        }
        let mut sorted = matched;
        sorted.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));

        let mut native_line = String::new();
        let mut last_end = 0.0_f32;
        for (i, f) in sorted.iter().enumerate() {
            if i > 0 {
                let gap = f.x - last_end;
                if gap > f.font_size * 0.25 && !native_line.ends_with(' ') && !f.text.starts_with(' ') {
                    native_line.push(' ');
                }
            }
            native_line.push_str(&f.text);
            last_end = f.x + f.width;
        }
        let native_line = native_line.trim();
        if native_line.is_empty() {
            continue;
        }

        let r = &mut regions[idx];
        let has_hangul = native_line.chars().any(|c| {
            ('\u{ac00}'..='\u{d7a3}').contains(&c)
                || ('\u{1100}'..='\u{11ff}').contains(&c)
                || ('\u{3130}'..='\u{318f}').contains(&c)
        });

        let should_adopt = if has_hangul {
            true
        } else if r.confidence.unwrap_or(1.0) < 0.85 || r.text.trim().is_empty() {
            true
        } else {
            let r_chars: std::collections::HashSet<_> = r.text.chars().filter(|c| !c.is_whitespace()).collect();
            let n_chars: std::collections::HashSet<_> = native_line.chars().filter(|c| !c.is_whitespace()).collect();
            let common = r_chars.intersection(&n_chars).count();
            common > 0 || r_chars.is_empty()
        };

        if should_adopt {
            r.text = native_line.to_string();
            r.confidence = Some(1.0);
            enhanced_any = true;
        }
    }

    enhanced_any
}

#[cfg(not(feature = "models"))]
fn recognize(
    _request: &ProcessRequest,
    _source: &Path,
    _temp: &Path,
    _total: u32,
    _direct_image: bool,
    _pipeline_started: &std::time::Instant,
    _assets_manifest: &mut assets::Manifest,
) -> Result<Vec<Page>> {
    Err(anyhow!(
        "MODEL_RUNTIME_MISSING: 请用 --features models 构建文档引擎；未验证的 PDF 文本层不能代替 OCR"
    ))
}

fn add_search_layer(source: &Path, output: &Path, font_path: &Path, pages: &[Page], mut progress: impl FnMut(u32) -> Result<()>) -> Result<()> {
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
        progress(page.page_number)?;
    }
    doc.save(output)?;
    Ok(())
}

fn process(mut request: ProcessRequest) -> Result<ProcessResult> {
    process_pages(&mut request, None)
}

fn process_pages(request: &mut ProcessRequest, corrected: Option<Vec<Page>>) -> Result<ProcessResult> {
    let pipeline_started = std::time::Instant::now();
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
    let is_pdf = source
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"));
    let direct_image = !is_pdf && request.markdown_only && corrected.is_none();
    let pdf_source = if is_pdf || direct_image {
        source
    } else {
        raster_pdf(source, &converted)?;
        converted.as_path()
    };
    let total = if direct_image { 1 } else { Document::from_file(pdf_source)?.page_count() };
    if total == 0 {
        return Err(anyhow!("EMPTY_DOCUMENT: 文档没有页面"));
    }
    write_progress(request, "preparing", 0, total, &pipeline_started, None)?;
    let is_correction = corrected.is_some();
    let mut assets_manifest = if is_correction && output_dir.join(assets::MANIFEST).exists() {
        assets::load(output_dir)?
    } else {
        assets::Manifest::default()
    };
    let mut pages = if let Some(pages) = corrected {
        anyhow::ensure!(pages.len() == total as usize && pages.iter().enumerate().all(|(i,p)|p.page_number as usize == i+1), "CORRECTION_PAGES_INVALID");
        pages
    } else { recognize(request, pdf_source, temp.path(), total, direct_image, &pipeline_started, &mut assets_manifest)? };
    write_progress(request, "finalizing", total, total, &pipeline_started, pages.last().and_then(|page| page.timing.as_ref()))?;
    let pdf = output_dir.join("source.searchable.pdf");
    let ir = output_dir.join("source.document.json");
    let md = output_dir.join("source.md");
    let map_path = output_dir.join("source.map.json");
    if !request.markdown_only {
        let font = request
            .cjk_font_path
            .as_deref()
            .ok_or_else(|| anyhow!("FONT_MISSING: CASY_OCR_FONT 未配置"))?;
        add_search_layer(pdf_source, &pdf, Path::new(font), &pages, |page| {
            write_progress(request, &format!("finalizing:{page}"), total, total, &pipeline_started, None)
        })?;
    }
    for (index, page) in pages.iter_mut().enumerate() {
        // S1 残留：外置 + 校验整段此前零进度，父进程 900s 停滞保护会误杀长卷宗。
        // 每页一个独立相位，父进程相位变化即刷新停滞计时。
        write_progress(request, &format!("finalizing:assets:{}", index + 1), total, total, &pipeline_started, None)?;
        page.markdown = assets::externalize(&page.markdown, output_dir, &mut assets_manifest)?;
        assets::validate_references(&page.markdown, output_dir, &assets_manifest)?;
    }
    assets::save(output_dir, &assets_manifest)?;
    write_progress(request, "finalizing:page-ir", total, total, &pipeline_started, None)?;
    write_json_file(&ir, &pages)?;
    write_progress(request, "finalizing:source-map", total, total, &pipeline_started, None)?;
    let source_map = source_map::write_to(&pages, &before, std::io::BufWriter::new(std::fs::File::create(&md)?))?;
    write_json_file(&map_path, &source_map)?;
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
    let korean_dir = env_path("CASY_KOREAN_MODEL_DIR").or_else(|| {
        let candidate = dir.parent()?.join("korean-ppocrv5-mobile");
        candidate.is_dir().then_some(candidate)
    }).ok_or_else(|| anyhow!("MODEL_MISSING: Korean recognition model"))?;
    let model_version = Some(format!(
        "det:{};rec:{};dictionary:{};korean-rec:{};korean-dictionary:{};layout:{};structure:ruled-grid-v2;visuals:source-crops-v1",
        sha256_file(&dir.join("det.onnx"))?,
        sha256_file(&dir.join("rec.onnx"))?,
        sha256_file(&dir.join(dictionary))?,
        sha256_file(&korean_dir.join("rec.onnx"))?,
        sha256_file(&korean_dir.join("rec.yml"))?,
        layout_path.as_deref().map(sha256_file).transpose()?.unwrap_or_else(||"none".into())
    ));
    let elapsed_ms = pipeline_started.elapsed().as_millis() as u64;
    write_progress(request, "completed", total, total, &pipeline_started, pages.last().and_then(|page| page.timing.as_ref()))?;
    Ok(ProcessResult {
        source_sha256: after,
        engine: if is_correction {
            "paddle-onnx-corrected"
        } else if pages.iter().any(|p| p.native_text) {
            "paddle-onnx-dual-stream"
        } else {
            "paddle-onnx-visual"
        }.into(),
        model_version,
        searchable_pdf_path: (!request.markdown_only).then(|| pdf.display().to_string()),
        page_ir_path: ir.display().to_string(),
        markdown_path: md.display().to_string(),
        source_map_path: map_path.display().to_string(),
        pages,
        elapsed_ms,
    })
}

fn write_json_file(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut writer = std::io::BufWriter::new(std::fs::File::create(path)?);
    serde_json::to_writer_pretty(&mut writer, value)?;
    writer.flush()?;
    Ok(())
}

fn write_result(value: &impl Serialize) -> Result<()> {
    let mut writer = std::io::BufWriter::new(std::io::stdout().lock());
    serde_json::to_writer(&mut writer, value)?;
    writeln!(writer)?;
    writer.flush()?;
    Ok(())
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
            write_result(&process(request)?)?;
        }
        "revise" => {
            #[derive(Deserialize)]
            struct Revision { request: ProcessRequest, pages: Vec<Page> }
            let mut input = Vec::new();
            std::io::stdin().take(128 * 1024 * 1024 + 1).read_to_end(&mut input)?;
            anyhow::ensure!(input.len() <= 128 * 1024 * 1024, "CORRECTION_TOO_LARGE");
            let mut revision: Revision = serde_json::from_slice(&input)?;
            write_result(&process_pages(&mut revision.request, Some(revision.pages))?)?;
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
    fn direct_image_keeps_pixels_and_composites_transparency_on_white() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("transparent.png");
        let mut input = image::RgbaImage::from_pixel(20, 10, image::Rgba([0, 0, 0, 0]));
        input.put_pixel(3, 4, image::Rgba([15, 30, 45, 255]));
        input.save(&path).unwrap();
        let output = raster_ocr_image(&path).unwrap();
        assert_eq!(output.dimensions(), (20, 10));
        assert_eq!(output.get_pixel(0, 0).0, [255, 255, 255]);
        assert_eq!(output.get_pixel(3, 4).0, [15, 30, 45]);
    }

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
            layout: None,
            timing: None,
            native_text: false,
        };
        add_search_layer(&source, &output, font, &[page], |_| Ok(())).unwrap();
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
    #[cfg(feature = "models")]
    fn korean_candidate_requires_script_evidence_and_confidence() {
        assert!(korean_candidate_should_replace(
            "圣斗引 外人 10-2019-0078013",
            0.65,
            "출원인 주식회사 포스코 10-2019-0078013",
            0.94,
        ));
        assert!(!korean_candidate_should_replace(
            "中华人民共和国民法典",
            0.96,
            "대한민국 민법전",
            0.87,
        ));
        assert!(!korean_candidate_should_replace(
            "contract evidence",
            0.91,
            "contract evidence",
            0.99,
        ));
        assert!(!korean_candidate_should_replace("甲", 0.91, "한", 0.91));
    }

    #[test]
    #[cfg(feature = "models")]
    fn dual_stream_enhances_korean_and_rejects_forged_and_corrupt() {
        let font_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("src-tauri/runtime/fonts/NotoSansCJK-Regular.ttf");
        if !font_path.is_file() {
            return;
        }
        let font_bytes = std::fs::read(&font_path).unwrap();
        let mut doc = Document::new((400.0, 400.0)).unwrap();
        let f = doc.embed_font(&font_bytes).unwrap();
        doc.page(1)
            .unwrap()
            .add_text("출원인 주식회사 포스코 10-2019-0078013", f, [30.0, 300.0], 16.0, [0.0; 3])
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

        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("test.pdf");
        doc.save(&source).unwrap();

        let doc = Document::from_file(&source).unwrap();
        let raw_text = doc.extract_text(1).unwrap();
        let runs = doc.extract_text_runs(1).unwrap();

        let mut regions = vec![Region {
            text: "圣斗引 外人 10-2019-0078013".into(),
            bbox: [30.0, 80.0, 350.0, 105.0],
            confidence: Some(0.65),
        }];

        let enhanced = enhance_regions_with_native_stream(
            &mut regions,
            400.0,
            400.0,
            (400.0, 400.0),
            &raw_text,
            &runs,
        );
        assert!(enhanced, "native stream should enhance valid Korean region");
        assert!(regions[0].text.contains("포스코"), "text: {}", regions[0].text);
        assert!(regions[0].text.contains("10-2019-0078013"));
        assert_eq!(regions[0].confidence, Some(1.0));
        assert!(!regions.iter().any(|r| r.text.contains("999999")), "invisible forged text must be rejected");

        // Anti-OCR tampered checks: null byte
        let corrupt_null = format!("{}\0hack", raw_text);
        let mut test_regions = regions.clone();
        assert!(!enhance_regions_with_native_stream(
            &mut test_regions,
            400.0,
            400.0,
            (400.0, 400.0),
            &corrupt_null,
            &runs,
        ));

        // CMap corruption (> 1% replacement chars)
        let corrupt_cmap = format!("{}\u{fffd}\u{fffd}\u{fffd}\u{fffd}\u{fffd}", raw_text);
        let mut test_regions = regions.clone();
        assert!(!enhance_regions_with_native_stream(
            &mut test_regions,
            400.0,
            400.0,
            (400.0, 400.0),
            &corrupt_cmap,
            &runs,
        ));

        // Discordant text (anchors disagree)
        let discordant_text = "Completely unrelated content with numbers 777-888-999 and other text";
        let mut test_regions = regions.clone();
        assert!(!enhance_regions_with_native_stream(
            &mut test_regions,
            400.0,
            400.0,
            (400.0, 400.0),
            discordant_text,
            &runs,
        ));
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
            markdown_only: false,
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
        let searchable = Document::from_file(result.searchable_pdf_path.as_deref().unwrap()).unwrap();
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

    #[cfg(feature = "models")]
    #[test]
    #[ignore = "requires local verified OCR, Korean and layout models"]
    fn real_korean_scan_uses_script_matched_recognizer() {
        let root = PathBuf::from(std::env::var("CASY_OCR_QA_DIR").expect("CASY_OCR_QA_DIR"))
            .join("korean-scan");
        std::fs::create_dir_all(&root).unwrap();
        let font_path = std::env::var("CASY_OCR_FONT").unwrap();
        let source_pdf = root.join("source.pdf");
        let mut document = Document::new((700.0, 500.0)).unwrap();
        let font = document.embed_font(&std::fs::read(&font_path).unwrap()).unwrap();
        for (row, text) in [
            "출원인 주식회사 포스코",
            "특허 침해 손해배상 청구",
            "中华人民共和国民法典",
            "Patent evidence 10-2019-0078013",
        ].iter().enumerate() {
            document.page(1).unwrap().add_text(
                text,
                font,
                [40.0, 420.0 - row as f32 * 90.0],
                24.0,
                [0.0; 3],
            ).unwrap();
        }
        document.save(&source_pdf).unwrap();
        let render_dir = root.join("render");
        std::fs::create_dir_all(&render_dir).unwrap();
        let scan = render_page(&source_pdf, &render_dir, 1).unwrap();
        let result = process(ProcessRequest {
            job_id: "korean-scan".into(),
            source_path: scan.display().to_string(),
            source_sha256: sha256_file(&scan).unwrap(),
            output_dir: root.join("result").display().to_string(),
            coordinate_model_dir: std::env::var("CASY_PPOCR_MODEL_DIR").ok(),
            cjk_font_path: Some(font_path),
            markdown_only: true,
        }).unwrap();
        std::fs::write(
            root.join("result.json"),
            serde_json::to_vec_pretty(&result).unwrap(),
        ).unwrap();
        let text = &result.pages[0].plain_text;
        assert!(text.contains("출원인"), "Korean applicant missing: {text}");
        assert!(text.contains("포스코"), "Korean company missing: {text}");
        assert!(text.contains("특허"), "Korean patent text missing: {text}");
        assert!(text.contains("손해배상"), "Korean damages text missing: {text}");
        assert!(text.contains("中华人民共和国"), "Chinese regression: {text}");
        assert!(text.contains("10-2019-0078013"), "Latin/digit regression: {text}");
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
            layout: None,
            timing: None,
            native_text: false,
        }];
        add_search_layer(&source, &output, font, &pages, |_| Ok(())).unwrap();
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
            markdown_only: false,
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
        let extracted = pdf_extract::extract_text(result.searchable_pdf_path.as_deref().unwrap()).unwrap();
        assert!(!extracted.contains("999999"));
        assert_eq!(before, sha256_file(&source).unwrap());
    }
}
