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
    let coord = env_path("CASY_PPOCR_MODEL_DIR").is_some();
    let ovis = env_path("CASY_OVISOCR2_MODEL_DIR").is_some();
    let font = env_path("CASY_OCR_FONT").is_some();
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
    if !ovis {
        missing.push("CASY_OVISOCR2_MODEL_DIR".into())
    }
    if !font {
        missing.push("CASY_OCR_FONT".into())
    }
    EngineStatus {
        available: cfg!(feature = "models") && renderer && coord && ovis && font,
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

fn render_pages(source: &Path, temp: &Path) -> Result<Vec<PathBuf>> {
    let prefix = temp.join("page");
    let output = std::process::Command::new("pdftoppm")
        .arg("-png")
        .arg("-r")
        .arg("200")
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
    let mut pages: Vec<_> = std::fs::read_dir(temp)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|v| v.to_str()) == Some("png"))
        .collect();
    pages.sort();
    if pages.is_empty() {
        return Err(anyhow!("PDF_RENDER_FAILED: 没有生成页面图像"));
    }
    Ok(pages)
}

#[cfg(feature = "models")]
fn recognize(request: &ProcessRequest, images: &[PathBuf]) -> Result<Vec<Page>> {
    use oar_ocr::prelude::*;
    use oar_ocr_vl::ovisocr2::DEFAULT_MAX_NEW_TOKENS;
    use oar_ocr_vl::{OvisOcr2, utils::parse_device};
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
    let coordinate = OAROCRBuilder::new(&det, &rec, &dict)
        .return_word_box(true)
        .build()?;
    let ovis_dir = request
        .ovis_model_dir
        .as_deref()
        .ok_or_else(|| anyhow!("MODEL_MISSING: OvisOCR2"))?;
    let ovis = OvisOcr2::from_dir(ovis_dir, parse_device(&request.device)?)?;
    let rgb: Vec<_> = images
        .iter()
        .map(|path| image::open(path).map(|v| v.to_rgb8()))
        .collect::<Result<_, _>>()?;
    let coordinate_results = coordinate.predict(rgb.clone())?;
    let markdown_results = ovis.parse_with_image_tags(&rgb, DEFAULT_MAX_NEW_TOKENS, true)?;
    let mut pages = Vec::new();
    for (index, (ocr, markdown)) in coordinate_results
        .into_iter()
        .zip(markdown_results)
        .enumerate()
    {
        let image = &rgb[index];
        let mut regions = Vec::new();
        let mut confidence_sum = 0.0f32;
        for region in ocr.text_regions {
            if let Some((text, confidence)) = region.text_with_confidence() {
                let xs: Vec<_> = region.bounding_box.points.iter().map(|p| p.x).collect();
                let ys: Vec<_> = region.bounding_box.points.iter().map(|p| p.y).collect();
                if xs.is_empty() || ys.is_empty() {
                    continue;
                }
                let bbox = [
                    xs.iter().copied().fold(f32::INFINITY, f32::min),
                    ys.iter().copied().fold(f32::INFINITY, f32::min),
                    xs.iter().copied().fold(f32::NEG_INFINITY, f32::max),
                    ys.iter().copied().fold(f32::NEG_INFINITY, f32::max),
                ];
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
            page_number: (index + 1) as u32,
            width: Some(image.width() as f32),
            height: Some(image.height() as f32),
            plain_text,
            markdown: markdown?,
            regions,
            confidence,
        });
    }
    Ok(pages)
}

#[cfg(not(feature = "models"))]
fn recognize(_request: &ProcessRequest, _images: &[PathBuf]) -> Result<Vec<Page>> {
    Err(anyhow!(
        "MODEL_RUNTIME_MISSING: 请用 --features models（Apple Silicon 可用 metal）构建文档引擎"
    ))
}

fn add_search_layer(source: &Path, output: &Path, font_path: &Path, pages: &[Page]) -> Result<()> {
    let mut doc = Document::from_file(source)?;
    let font = doc.embed_font(&std::fs::read(font_path)?)?;
    for page in pages {
        let image_w = page.width.unwrap_or(1.0).max(1.0);
        let image_h = page.height.unwrap_or(1.0).max(1.0);
        let (pdf_w, pdf_h) = doc.page(page.page_number)?.size()?;
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

fn process(request: ProcessRequest) -> Result<ProcessResult> {
    let source = Path::new(&request.source_path);
    let before = sha256_file(source)?;
    if before != request.source_sha256 {
        return Err(anyhow!("SOURCE_CHANGED: 开始处理前哈希不一致"));
    }
    let temp = tempfile::tempdir()?;
    let images = render_pages(source, temp.path())?;
    let pages = recognize(&request, &images)?;
    let output_dir = Path::new(&request.output_dir);
    std::fs::create_dir_all(output_dir)?;
    let pdf = output_dir.join("source.searchable.pdf");
    let ir = output_dir.join("source.document.json");
    let md = output_dir.join("source.md");
    let font = request
        .cjk_font_path
        .as_deref()
        .ok_or_else(|| anyhow!("FONT_MISSING: CASY_OCR_FONT 未配置"))?;
    add_search_layer(source, &pdf, Path::new(font), &pages)?;
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
    Ok(ProcessResult {
        source_sha256: after,
        engine: "oar-ppocrv5+ovisocr2".into(),
        model_version: Some("OvisOCR2/oar-ocr-0.9.2".into()),
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
