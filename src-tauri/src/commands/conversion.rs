use crate::document_pipeline;
use anyhow::{bail, Context, Result};
use std::{io::Write, path::PathBuf};

use super::markdown_export;

#[tauri::command]
pub async fn convert_file_to_markdown(
    source_path: String,
    output_dir: String,
    job_id: Option<String>,
    target_format: Option<String>,
) -> Result<serde_json::Value, String> {
    let job_id = job_id.unwrap_or_else(crate::db::new_id);
    {
        let conn = crate::db::open_db().map_err(|e|e.to_string())?;
        let exists: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM processing_activities WHERE id=?1)",[&job_id],|r|r.get(0)).map_err(|e|e.to_string())?;
        if exists {
            let changed=conn.execute("UPDATE processing_activities SET status='running',stage='preparing',updated_at=datetime('now','localtime') WHERE id=?1 AND kind='conversion' AND status='queued' AND source_path=?2",rusqlite::params![job_id,source_path]).map_err(|e|e.to_string())?;
            if changed!=1 { return Err("转换任务已取消或已经开始，请重新发起".into()); }
        } else {
            let title=std::path::Path::new(&source_path).file_name().and_then(|s|s.to_str()).unwrap_or("文件转换");
            crate::processing::insert(&conn,&job_id,"conversion",title,Some(&source_path),false).map_err(|e|e.to_string())?;
        }
    }
    let _cancellation = crate::processing::ConversionCancellation::register(&job_id);
    // A cancel request can arrive between the persisted start and token registration.
    {
        let conn = crate::db::open_db().map_err(|e| e.to_string())?;
        let cancelling: bool = conn.query_row("SELECT stage='cancelling' FROM processing_activities WHERE id=?1", [&job_id], |r| r.get(0)).map_err(|e| e.to_string())?;
        if cancelling { crate::processing::request_conversion_cancel(&job_id); }
    }
    let started = std::time::Instant::now();
    document_pipeline::emit_conversion_progress(&job_id, &source_path, "preparing", 0, 0, 0.0);
    tracing::info!(%job_id, "Standalone document conversion requested");
    match convert(source_path.clone(), output_dir, &job_id, target_format).await {
        Ok(mut result) => {
            result["elapsedSeconds"] = serde_json::json!(started.elapsed().as_secs_f64());
            let pages = result["pages"].as_u64().unwrap_or(0) as u32;
            let elapsed = result["elapsedSeconds"].as_f64().unwrap_or_else(|| started.elapsed().as_secs_f64());
            document_pipeline::emit_conversion_progress(&job_id, &source_path, "completed", pages, pages, elapsed);
            let conn=crate::db::open_db().map_err(|e|e.to_string())?;
            crate::processing::finish(&conn,&job_id,None,result["outputPath"].as_str()).map_err(|e|e.to_string())?;
            tracing::info!(%job_id, pages = %result["pages"], bytes = %result["bytes"], "Standalone document conversion completed");
            Ok(result)
        }
        Err(error) => {
            if let Ok(conn)=crate::db::open_db() {
                let _=crate::processing::finish(&conn,&job_id,Some(&format!("{error:#}")),None);
            }
            document_pipeline::emit_conversion_progress(&job_id, &source_path, "failed", 0, 0, started.elapsed().as_secs_f64());
            tracing::error!(%job_id, error = %format!("{error:#}"), "Standalone document conversion failed");
            Err(format!("{error:#}"))
        }
    }
}

async fn convert(source_path: String, output_dir: String, job_id: &str, target_format: Option<String>) -> Result<serde_json::Value> {
    crate::processing::check_conversion_cancelled(job_id)?;
    let target_format = target_format.unwrap_or_else(|| "markdown".to_string());
    anyhow::ensure!(matches!(target_format.as_str(), "markdown" | "pdf" | "both"), "不支持的输出格式");
    let is_pdf = matches!(target_format.as_str(), "pdf" | "both");
    let is_md = matches!(target_format.as_str(), "markdown" | "both");

    let (source, destination, hash) = super::run_blocking(move || {
        let source = PathBuf::from(source_path);
        let destination = PathBuf::from(output_dir);
        if !source.is_absolute() || !destination.is_absolute() {
            bail!("请选择源文件和输出目录");
        }
        let source = source.canonicalize()?;
        let destination = destination.canonicalize()?;
        if !source.is_file() || !destination.is_dir() {
            bail!("源文件或输出目录不可用");
        }
        if !document_pipeline::supports_path(&source) {
            bail!("暂不支持此文件格式");
        }
        anyhow::ensure!(!is_pdf || !crate::parse::text_document::supports(&source), "文字文档暂不支持双层 PDF，请选择 Markdown；排版 PDF 请在文书编辑器中导出");
        let hash = document_pipeline::sha256_file(&source)?;
        Ok((source, destination, hash))
    })
    .await
    .map_err(anyhow::Error::msg)?;
    let temporary = tempfile::tempdir()?;
    let mut request = document_pipeline::process_request(
        job_id,
        &source.to_string_lossy(),
        &hash,
        temporary.path(),
    );
    request.markdown_only = !is_pdf;
    let result = document_pipeline::run_standalone_engine(request).await?;
    crate::processing::check_conversion_cancelled(job_id)?;
    let count = result.pages.len();
    drop(result.pages); // Validation is complete; do not retain every page while exporting.

    let elapsed_seconds = result.elapsed_ms as f64 / 1000.0;
    let publish_job = job_id.to_string();
    super::run_blocking(move || {
        crate::processing::check_conversion_cancelled(&publish_job)?;
        if crate::get_app_handle().is_some() {
            let conn = crate::db::open_db()?;
            let changed = conn.execute("UPDATE processing_activities SET stage='publishing' WHERE id=?1 AND status='running' AND stage!='cancelling'", [&publish_job])?;
            anyhow::ensure!(changed == 1, "CONVERSION_CANCELLED: 转换已取消，未发布新文件");
        }
        if document_pipeline::sha256_file(&source)? != hash { bail!("源文件在转换期间发生变化，请重试"); }
        let stem = crate::files::sanitize_filename(source.file_stem().and_then(|s| s.to_str()).unwrap_or("document"));
        let stem: String = stem.chars().take(80).collect();

        let mut pending = Vec::new();
        let mut image_assets = None;
        let mut markdown_bytes_len = 0;
        // Prepare every requested output before publishing any of them.
        if is_md {
            let markdown = std::fs::read_to_string(&result.markdown_path).context("无法读取转换结果 Markdown")?;
            let artifact_root = std::path::Path::new(&result.markdown_path).parent().context("产物目录不可用")?;
            let image_root = if artifact_root.join(document_pipeline::assets::MANIFEST).exists() { artifact_root } else { source.parent().context("源文件目录不可用")? };
            let (markdown, assets) = markdown_export::externalize_images_from(
                &markdown, image_root, &destination,
            )?;
            image_assets = assets;
            markdown_bytes_len = markdown.len();
            let mut output = tempfile::NamedTempFile::new_in(&destination)?;
            output.write_all(markdown.as_bytes())?;
            output.as_file().sync_all()?;
            pending.push(("md", output));
        }
        if is_pdf {
            let engine_pdf = result.searchable_pdf_path.as_ref().context("引擎未生成双层可搜索 PDF")?;
            let mut input = std::fs::File::open(engine_pdf).context("无法读取转换结果 PDF")?;
            let mut output = tempfile::NamedTempFile::new_in(&destination)?;
            std::io::copy(&mut input, &mut output)?;
            output.as_file().sync_all()?;
            pending.push(("searchable.pdf", output));
        }
        let mut published = PublishedOutputs::default();
        let mut md_path_res = None;
        let mut pdf_path_res = None;
        for (extension, output) in pending {
            let path = publish_output(output, &destination, &stem, extension)?;
            published.paths.push(path.clone());
            if extension == "md" { md_path_res = Some(path); } else { pdf_path_res = Some(path); }
        }
        if let Some(assets) = image_assets { let _ = assets.keep(); }
        published.committed = true;

        let primary_output = md_path_res.as_ref().or(pdf_path_res.as_ref()).context("未生成任何目标文件")?;
        Ok(serde_json::json!({
            "outputPath": primary_output.to_string_lossy(),
            "markdownPath": md_path_res.as_ref().map(|p| p.to_string_lossy()),
            "pdfPath": pdf_path_res.as_ref().map(|p| p.to_string_lossy()),
            "pages": count,
            "bytes": markdown_bytes_len,
            "elapsedSeconds": elapsed_seconds
        }))
    }).await.map_err(anyhow::Error::msg)
}

// Only paths created by this export are removed if a later output fails.
#[derive(Default)]
struct PublishedOutputs { paths: Vec<PathBuf>, committed: bool }
impl Drop for PublishedOutputs {
    fn drop(&mut self) {
        if !self.committed {
            for path in &self.paths { let _ = std::fs::remove_file(path); }
        }
    }
}
fn publish_output(mut output: tempfile::NamedTempFile, destination: &std::path::Path, stem: &str, extension: &str) -> Result<PathBuf> {
    for suffix in 0..10000 {
        let name = if suffix == 0 { format!("{stem}.{extension}") } else { format!("{stem} ({suffix}).{extension}") };
        let path = destination.join(name);
        match output.persist_noclobber(&path) {
            Ok(_) => return Ok(path),
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => output = error.file,
            Err(error) => return Err(error.error.into()),
        }
    }
    bail!("同名输出文件过多，请选择其他目录")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn text_pdf_is_rejected_before_creating_any_output() {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("out"); std::fs::create_dir(&output).unwrap();
        let source = root.path().join("正文.md"); std::fs::write(&source, "# 正文").unwrap();
        for format in ["pdf", "both", "invalid"] {
            assert!(convert(source.display().to_string(), output.display().to_string(), "capability-test", Some(format.into())).await.is_err());
            assert_eq!(std::fs::read_dir(&output).unwrap().count(), 0);
        }
    }

    #[tokio::test]
    async fn markdown_attachments_survive_cross_directory_conversion() {
        let root = tempfile::tempdir().unwrap();
        let source_dir = root.path().join("源 文件"); std::fs::create_dir(&source_dir).unwrap();
        let output = root.path().join("输出"); std::fs::create_dir(&output).unwrap();
        let source = source_dir.join("正文.md");
        std::fs::write(source_dir.join("配 图.png"), b"exact image bytes").unwrap();
        std::fs::write(&source, "![图](配%20图.png)\n\n<img src=\"配%20图.png\">\n\n![ref][img]\n\n[img]: 配%20图.png\n").unwrap();
        let result = convert(source.display().to_string(), output.display().to_string(), "attachments-test", None).await.unwrap();
        let md = std::fs::read_to_string(result["markdownPath"].as_str().unwrap()).unwrap();
        let refs = regex::Regex::new(r"casy-images-[^/\s]+/[a-f0-9]+\.png").unwrap();
        let paths: Vec<_> = refs.find_iter(&md).map(|m| m.as_str()).collect();
        assert_eq!(paths.len(), 3);
        assert!(paths.iter().all(|p| *p == paths[0]));
        assert_eq!(std::fs::read(output.join(paths[0])).unwrap(), b"exact image bytes");
        std::fs::remove_file(source_dir.join("配 图.png")).unwrap();
        assert!(convert(source.display().to_string(), output.display().to_string(), "missing-test", None).await.is_err());
        assert_eq!(std::fs::read_dir(&output).unwrap().count(), 2);
    }

    #[test]
    fn failed_multi_output_export_rolls_back_only_its_own_files() {
        let root = tempfile::tempdir().unwrap();
        let existing = root.path().join("keep.md"); std::fs::write(&existing, "original").unwrap();
        let created = root.path().join("new.md"); std::fs::write(&created, "new").unwrap();
        drop(PublishedOutputs { paths: vec![created.clone()], committed: false });
        assert!(!created.exists()); assert_eq!(std::fs::read_to_string(existing).unwrap(), "original");
    }

    #[tokio::test]
    async fn conversion_externalizes_legacy_ocr_images_without_overwriting() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("旧版 OCR.md");
        let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGNQKt8NAAITAVXMpdPnAAAAAElFTkSuQmCC";
        let input = format!("<!-- page 1 -->\n<figure><img src=\"data:image/png;base64,{png}\"><figcaption>公式原图</figcaption></figure>");
        std::fs::write(&source, &input).unwrap();
        let mut paths = Vec::new();
        for _ in 0..2 {
            let result = convert(source.display().to_string(), root.path().display().to_string(), "images-test", None).await.unwrap();
            let path = result["outputPath"].as_str().unwrap().to_owned();
            let md = std::fs::read_to_string(&path).unwrap();
            assert!(!md.contains("base64"));
            assert_eq!(result["bytes"].as_u64().unwrap(), md.len() as u64);
            let relative = md.split("src=\"").nth(1).unwrap().split('"').next().unwrap();
            let image = std::fs::read(root.path().join(relative)).unwrap();
            assert_eq!(image::load_from_memory(&image).unwrap().width(), 1);
            paths.push(path);
        }
        assert_ne!(paths[0], paths[1]);
        assert!(paths[0].ends_with("旧版 OCR (1).md"));
        assert!(paths[1].ends_with("旧版 OCR (2).md"));
        assert_eq!(std::fs::read_to_string(source).unwrap(), input);
    }

    #[tokio::test]
    #[ignore = "requires bundled document engine and CASY_CONVERSION_TEST_PDF"]
    async fn standalone_pdf_conversion_with_real_engine() {
        let source = std::env::var("CASY_CONVERSION_TEST_PDF").unwrap();
        let before = document_pipeline::sha256_file(std::path::Path::new(&source)).unwrap();
        let output = match std::env::var("CASY_CONVERSION_QA_DIR") {
            Ok(root) => tempfile::tempdir_in(root).unwrap(),
            Err(_) => tempfile::tempdir().unwrap(),
        };
        let result = convert(source.clone(), output.path().display().to_string(), "ocr-export-qa", None).await.unwrap();
        assert!(result["pages"].as_u64().unwrap() > 0);
        assert!(result["bytes"].as_u64().unwrap() > 0);
        let markdown = std::fs::read_to_string(result["outputPath"].as_str().unwrap()).unwrap();
        assert!(markdown.contains("<!-- page 1 -->"));
        assert!(!markdown.contains("data:image/"));
        if std::env::var_os("CASY_CONVERSION_EXPECT_IMAGES").is_some() {
            let references = regex::Regex::new(r#"src="(casy-images-[^"]+)""#).unwrap();
            let mut count = 0;
            for image in references.captures_iter(&markdown) {
                image::open(output.path().join(&image[1])).unwrap();
                count += 1;
            }
            assert!(count > 0, "fixture must exercise OCR visual crops");
        }
        assert_eq!(before, document_pipeline::sha256_file(std::path::Path::new(&source)).unwrap());
        if std::env::var_os("CASY_CONVERSION_QA_DIR").is_some() {
            eprintln!("OCR export QA saved to {}", output.keep().display());
        }
    }
    #[tokio::test]
    async fn conversion_preserves_source_and_existing_output() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("证据.md");
        let text = "# Prüfung français 日本語\n\n| 项目 | 金额 |\n|---|---|\n| 赔偿 | 100 |";
        std::fs::write(&source, text).unwrap();
        let result = convert(
            source.display().to_string(),
            root.path().display().to_string(),
            "test-conversion",
            None,
        )
        .await
        .unwrap();
        assert_eq!(std::fs::read_to_string(&source).unwrap(), text);
        assert!(result["outputPath"]
            .as_str()
            .unwrap()
            .ends_with("证据 (1).md"));
        assert_eq!(
            std::fs::read_to_string(result["outputPath"].as_str().unwrap()).unwrap(),
            text
        );
        assert!(convert(source.display().to_string(), "relative".into(), "test-invalid", None)
            .await
            .is_err());
    }
}
