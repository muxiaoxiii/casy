use anyhow::{bail, Context, Result};
use base64::Engine;
use sha2::{Digest, Sha256};
use std::path::Path;

/// Keep the directory temporary until the caller has published the Markdown successfully.
/// A separate directory per export avoids collisions with existing files and parallel jobs.
pub(super) fn externalize_images(
    markdown: &str,
    destination: &Path,
) -> Result<(String, Option<tempfile::TempDir>)> {
    let pattern = regex::Regex::new(r#"(?i)data:image/([a-z0-9.+-]+);base64,([^\s\"'<>)]*)"#)?;
    let mut assets = None;
    let mut output = String::with_capacity(markdown.len().min(64 * 1024));
    let mut offset = 0;
    for image in pattern.captures_iter(markdown) {
        let matched = image.get(0).unwrap();
        let extension = match image[1].to_ascii_lowercase().as_str() {
            "png" => "png",
            "jpeg" | "jpg" => "jpg",
            "gif" => "gif",
            "webp" => "webp",
            "bmp" | "x-ms-bmp" => "bmp",
            "tiff" => "tiff",
            "avif" => "avif",
            "svg+xml" => "svg",
            other => bail!("无法导出内嵌图片格式: {other}"),
        };
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&image[2])
            .context("无法导出内嵌图片：base64 数据无效")?;
        if bytes.is_empty() {
            bail!("无法导出内嵌图片：图片数据为空");
        }
        if assets.is_none() {
            assets = Some(tempfile::Builder::new().prefix("casy-images-").tempdir_in(destination)?);
        }
        let directory = assets.as_ref().unwrap().path();
        let name = format!("{}.{}", hex::encode(Sha256::digest(&bytes)), extension);
        let path = directory.join(&name);
        if !path.exists() {
            std::fs::write(&path, &bytes).context("无法保存 Markdown 配图")?;
        }
        output.push_str(&markdown[offset..matched.start()]);
        output.push_str(directory.file_name().unwrap().to_str().unwrap());
        output.push('/');
        output.push_str(&name);
        offset = matched.end();
    }
    output.push_str(&markdown[offset..]);
    Ok((output, assets))
}

/// Resolve image attachments against the original document, never the process cwd.
/// Only files within that document's directory may be copied by an untrusted Markdown file.
pub(super) fn externalize_images_from(
    markdown: &str,
    source_directory: &Path,
    destination: &Path,
) -> Result<(String, Option<tempfile::TempDir>)> {
    use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
    let (markdown, mut assets) = externalize_images(markdown, destination)?;
    let base = source_directory.canonicalize()?;
    let base_url = reqwest::Url::from_directory_path(&base)
        .map_err(|_| anyhow::anyhow!("图片源目录不可用"))?;
    let mut copy_image = |reference: &str| -> Result<Option<String>> {
        // Network images and anchors retain their meaning without making network requests.
        if reference.is_empty() || reference.starts_with('#') || reference.starts_with("//") {
            return Ok(None);
        }
        if assets.as_ref().is_some_and(|dir| reference.starts_with(&format!("{}/", dir.path().file_name().unwrap().to_string_lossy()))) {
            return Ok(None);
        }
        let url = base_url.join(reference).context("图片引用无效")?;
        if url.scheme() != "file" { return Ok(None); }
        let source = url.to_file_path().map_err(|_| anyhow::anyhow!("图片路径无效"))?
            .canonicalize().with_context(|| format!("配图不存在：{reference}"))?;
        anyhow::ensure!(source.starts_with(&base) && source.is_file(), "配图必须位于原文档目录内：{reference}");
        let extension = source.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
        anyhow::ensure!(matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tif" | "tiff" | "avif" | "svg"), "不支持的配图格式：{reference}");
        if assets.is_none() { assets = Some(tempfile::Builder::new().prefix("casy-images-").tempdir_in(destination)?); }
        let directory = assets.as_ref().unwrap().path();
        // Copy and hash the same bytes, so concurrent source edits cannot misname the attachment.
        let mut input = std::io::BufReader::new(std::fs::File::open(&source)?);
        let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        use std::io::{Read, Write};
        loop {
            let n = input.read(&mut buffer)?;
            if n == 0 { break; }
            hash.update(&buffer[..n]); temporary.write_all(&buffer[..n])?;
        }
        temporary.as_file().sync_all()?;
        let name = format!("{}.{}", hex::encode(hash.finalize()), extension);
        if !directory.join(&name).exists() { temporary.persist_noclobber(directory.join(&name))?; }
        Ok(Some(format!("{}/{name}", directory.file_name().unwrap().to_string_lossy())))
    };
    let html_src = regex::Regex::new(r#"(?is)<img\b[^>]*?\bsrc\s*=\s*(?:"([^"]*)"|'([^']*)')"#)?;
    let mut edits = Vec::new();
    let mut image: Option<(std::ops::Range<usize>, String, String, String)> = None;
    for (event, range) in Parser::new_ext(&markdown, Options::all()).into_offset_iter() {
        match event {
            Event::Start(Tag::Image { dest_url, title, .. }) => {
                if let Some(path) = copy_image(&dest_url)? { image = Some((range, path, title.to_string(), String::new())); }
            }
            Event::Text(text) | Event::Code(text) if image.is_some() => image.as_mut().unwrap().3.push_str(&text),
            Event::End(TagEnd::Image) => {
                if let Some((range, path, title, alt)) = image.take() {
                    let alt = alt.replace('\\', "\\\\").replace('[', "\\[").replace(']', "\\]");
                    let title = if title.is_empty() { String::new() } else { format!(" \"{}\"", title.replace('\\', "\\\\").replace('"', "\\\"")) };
                    edits.push((range, format!("![{alt}]({path}{title})")));
                }
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                for captures in html_src.captures_iter(&html) {
                    let url = captures.get(1).or_else(|| captures.get(2)).unwrap();
                    if let Some(path) = copy_image(url.as_str())? {
                        edits.push((range.start + url.start()..range.start + url.end(), path));
                    }
                }
            }
            _ => {}
        }
    }
    edits.sort_by_key(|(range, _)| range.start);
    let mut output = String::with_capacity(markdown.len());
    let mut offset = 0;
    for (range, replacement) in edits {
        anyhow::ensure!(range.start >= offset, "配图引用范围重叠");
        output.push_str(&markdown[offset..range.start]); output.push_str(&replacement); offset = range.end;
    }
    output.push_str(&markdown[offset..]);
    Ok((output, assets))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGNQKt8NAAITAVXMpdPnAAAAAElFTkSuQmCC";

    #[tokio::test]
    async fn editor_export_preserves_images_without_inline_base64() {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("editor.md");
        let input = format!("# 公式\n![公式](data:image/png;base64,{PNG})");
        super::super::docs::export_editor_document(
            serde_json::json!({"type":"doc","content":[]}), input, "公式".into(),
            "md".into(), output.display().to_string(), None,
        ).await.unwrap();
        let md = std::fs::read_to_string(output).unwrap();
        assert!(!md.contains("base64"));
        let relative = md.split("](").nth(1).unwrap().trim_end_matches(')');
        assert_eq!(std::fs::read(root.path().join(relative)).unwrap(), base64::engine::general_purpose::STANDARD.decode(PNG).unwrap());
    }

    #[test]
    fn html_and_markdown_images_keep_pixels_and_relative_links_when_moved() {
        let root = tempfile::tempdir().unwrap();
        let export = root.path().join("中文 空格 # &");
        std::fs::create_dir(&export).unwrap();
        let input = format!("<!-- page 1 -->\n<figure><img src=\"data:image/png;base64,{PNG}\"><figcaption>公式原图</figcaption></figure>\n![same](data:image/png;base64,{PNG})\n正文");
        let (md, assets) = externalize_images(&input, &export).unwrap();
        let assets = assets.unwrap();
        assert!(!md.contains("base64"));
        assert!(md.contains("<figcaption>公式原图</figcaption>"));
        assert!(md.ends_with("正文"));
        assert_eq!(std::fs::read_dir(assets.path()).unwrap().count(), 1);
        let relative = md.split("src=\"").nth(1).unwrap().split('"').next().unwrap();
        assert!(md.contains(&format!("![same]({relative})")));
        let _ = assets.keep();
        std::fs::write(export.join("证据.md"), &md).unwrap();
        let moved = root.path().join("moved");
        std::fs::rename(&export, &moved).unwrap();
        let bytes = std::fs::read(moved.join(relative)).unwrap();
        assert_eq!(bytes, base64::engine::general_purpose::STANDARD.decode(PNG).unwrap());
        assert_eq!(image::load_from_memory(&bytes).unwrap().width(), 1);
    }

    #[test]
    fn failed_exports_clean_up_and_plain_text_creates_no_assets() {
        let root = tempfile::tempdir().unwrap();
        let text = "# 正文\n\n![external](https://example.com/image.png)";
        let (md, assets) = externalize_images(text, root.path()).unwrap();
        assert_eq!(md, text);
        assert!(assets.is_none());
        let invalid = format!("![valid](data:image/png;base64,{PNG})\n![invalid](data:image/png;base64,!!)");
        assert!(externalize_images(&invalid, root.path()).is_err());
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
        let (_, assets) = externalize_images(&format!("![image](data:image/png;base64,{PNG})"), root.path()).unwrap();
        drop(assets);
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }
}
