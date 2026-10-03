//! Versioned, content-addressed OCR image artifacts shared by engine and host.
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, io::Read, path::Path};

pub const MANIFEST: &str = "source.assets.json";
pub const MAX_ASSET_BYTES: u64 = 32 * 1024 * 1024;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub sha256: String,
    pub size: u64,
    pub media_type: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub version: u32,
    pub assets: BTreeMap<String, Asset>,
}
impl Default for Manifest {
    fn default() -> Self {
        Self {
            version: 2,
            assets: BTreeMap::new(),
        }
    }
}
fn valid_id(id: &str) -> bool {
    id.len() == 68
        && id.ends_with(".png")
        && id.as_bytes()[..64]
            .iter()
            .copied()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
pub fn read(root: &Path, manifest: &Manifest, id: &str) -> Result<Vec<u8>> {
    ensure!(
        manifest.version == 2 && valid_id(id),
        "INVALID_ASSET: invalid version or id"
    );
    let entry = manifest
        .assets
        .get(id)
        .context("INVALID_ASSET: image not declared")?;
    ensure!(
        entry.media_type == "image/png" && entry.size <= MAX_ASSET_BYTES,
        "INVALID_ASSET: invalid image size or type"
    );
    let root = root.canonicalize()?;
    let path = root.join("assets").join(id).canonicalize()?;
    ensure!(
        path.starts_with(root.join("assets")) && path.starts_with(&root),
        "INVALID_ASSET: path escape"
    );
    let file = fs::File::open(path)?;
    ensure!(
        file.metadata()?.is_file() && file.metadata()?.len() == entry.size,
        "INVALID_ASSET: size mismatch"
    );
    let mut bytes = Vec::new();
    file.take(entry.size + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 == entry.size,
        "INVALID_ASSET: size changed during read"
    );
    ensure!(
        bytes.starts_with(b"\x89PNG\r\n\x1a\n")
            && hex::encode(Sha256::digest(&bytes)) == entry.sha256
            && id == format!("{}.png", entry.sha256),
        "INVALID_ASSET: checksum mismatch"
    );
    Ok(bytes)
}
pub fn load(root: &Path) -> Result<Manifest> {
    let path = root.join(MANIFEST);
    ensure!(
        fs::metadata(&path)?.len() <= 16 * 1024 * 1024,
        "INVALID_ASSET: manifest too large"
    );
    let manifest: Manifest = serde_json::from_reader(fs::File::open(path)?)?;
    ensure!(manifest.version == 2, "INVALID_ASSET: unsupported version");
    Ok(manifest)
}
pub fn save(root: &Path, manifest: &Manifest) -> Result<()> {
    fs::write(root.join(MANIFEST), serde_json::to_vec_pretty(manifest)?)?;
    Ok(())
}

/// Carry verified artifacts to a new revision without expanding them into its IPC payload.
pub fn copy_to(source: &Path, destination: &Path, manifest: &Manifest) -> Result<()> {
    for id in manifest.assets.keys() {
        let bytes = read(source, manifest, id)?;
        fs::create_dir_all(destination.join("assets"))?;
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination.join("assets").join(id))?;
        file.write_all(&bytes)?;
        file.sync_all()?;
    }
    save(destination, manifest)
}
/// Only replaces generated PNG data URIs; other text remains byte-for-byte intact.
pub fn externalize(markdown: &str, root: &Path, manifest: &mut Manifest) -> Result<String> {
    const PREFIX: &str = "data:image/png;base64,";
    rewrite_images(markdown, |reference| {
        let Some(encoded) = reference.strip_prefix(PREFIX) else {
            return Ok(None);
        };
        ensure!(
            !encoded.is_empty() && encoded.len() as u64 <= MAX_ASSET_BYTES.div_ceil(3) * 4,
            "INVALID_ASSET: encoded image too large"
        );
        let bytes = STANDARD.decode(encoded)?;
        ensure!(
            bytes.starts_with(b"\x89PNG\r\n\x1a\n") && bytes.len() as u64 <= MAX_ASSET_BYTES,
            "INVALID_ASSET: not a bounded PNG"
        );
        let hash = hex::encode(Sha256::digest(&bytes));
        let id = format!("{hash}.png");
        fs::create_dir_all(root.join("assets"))?;
        let entry = Asset {
            sha256: hash,
            size: bytes.len() as u64,
            media_type: "image/png".into(),
        };
        if manifest.assets.contains_key(&id) {
            ensure!(
                read(root, manifest, &id)? == bytes,
                "INVALID_ASSET: existing bytes differ"
            );
        } else {
            use std::io::Write;
            let path = root.join("assets").join(&id);
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            manifest.assets.insert(id.clone(), entry);
        }
        Ok(Some(format!("assets/{id}")))
    })
}

pub fn validate_references(markdown: &str, root: &Path, manifest: &Manifest) -> Result<()> {
    rewrite_images(markdown, |reference| {
        if let Some(id) = reference.strip_prefix("assets/") {
            read(root, manifest, id)?;
        }
        Ok(None)
    })?;
    Ok(())
}

/// Transform image destinations only; prose, fenced examples and captions are immutable.
pub fn rewrite_images(
    markdown: &str,
    mut rewrite: impl FnMut(&str) -> Result<Option<String>>,
) -> Result<String> {
    use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
    let html_src =
        regex::Regex::new(r#"(?is)<img\b[^>]*?\s+src\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))"#)?;
    let mut edits = Vec::new();
    let mut image: Option<(std::ops::Range<usize>, String, String, String)> = None;
    for (event, range) in Parser::new_ext(markdown, Options::all()).into_offset_iter() {
        match event {
            Event::Start(Tag::Image {
                dest_url, title, ..
            }) => {
                if let Some(destination) = rewrite(&dest_url)? {
                    image = Some((range, destination, title.into_string(), String::new()));
                }
            }
            Event::Text(text) | Event::Code(text) if image.is_some() => {
                image.as_mut().unwrap().3.push_str(&text);
            }
            Event::End(TagEnd::Image) => {
                if let Some((range, destination, title, alt)) = image.take() {
                    let alt = alt
                        .replace('\\', "\\\\")
                        .replace('[', "\\[")
                        .replace(']', "\\]");
                    let title = if title.is_empty() {
                        String::new()
                    } else {
                        format!(" \"{}\"", title.replace('\\', "\\\\").replace('"', "\\\""))
                    };
                    edits.push((range, format!("![{alt}]({destination}{title})")));
                }
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                for captures in html_src.captures_iter(&html) {
                    let src = captures
                        .get(1)
                        .or(captures.get(2))
                        .or(captures.get(3))
                        .unwrap();
                    if let Some(destination) = rewrite(src.as_str())? {
                        edits.push((
                            range.start + src.start()..range.start + src.end(),
                            destination,
                        ));
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
        ensure!(
            range.start >= offset,
            "INVALID_ASSET: overlapping image references"
        );
        output.push_str(&markdown[offset..range.start]);
        output.push_str(&replacement);
        offset = range.end;
    }
    output.push_str(&markdown[offset..]);
    Ok(output)
}
/// Compatibility boundary for editors/exporters which do not yet resolve asset IDs.
pub fn inline(markdown: &str, root: &Path, manifest: &Manifest) -> Result<String> {
    rewrite_images(markdown, |reference| {
        if let Some(id) = reference.strip_prefix("assets/") {
            let bytes = read(root, manifest, id)?;
            return Ok(Some(format!(
                "data:image/png;base64,{}",
                STANDARD.encode(bytes)
            )));
        }
        Ok(None)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn references_leave_prose_code_and_captions_unchanged_and_copy_independently() {
        let original = tempfile::tempdir().unwrap();
        let revision = tempfile::tempdir().unwrap();
        let png = b"\x89PNG\r\n\x1a\nfixture";
        let uri = format!("data:image/png;base64,{}", STANDARD.encode(png));
        let source =
            format!("Prose {uri}\n\n`<img src=\"{uri}\">`\n\n<img src='{uri}'>\n\n![crop]({uri})");
        let mut manifest = Manifest::default();
        let result = externalize(&source, original.path(), &mut manifest).unwrap();
        assert!(result.starts_with(&format!("Prose {uri}\n\n`<img src=\"{uri}\">`")));
        assert_eq!(manifest.assets.len(), 1);
        let id = manifest.assets.keys().next().unwrap();
        let with_prose = format!(
            "{result}\n\nProse assets/missing.png\n\n`<img src=\"assets/missing.png\">`\n\n<figcaption>assets/{id}</figcaption>"
        );
        validate_references(&with_prose, original.path(), &manifest).unwrap();
        let inlined = inline(&with_prose, original.path(), &manifest).unwrap();
        assert!(inlined.contains(&format!("<figcaption>assets/{id}</figcaption>")));
        assert!(inlined.contains(&format!("![crop]({uri})")));
        copy_to(original.path(), revision.path(), &manifest).unwrap();
        fs::remove_dir_all(original.path().join("assets")).unwrap();
        validate_references(
            &with_prose,
            revision.path(),
            &load(revision.path()).unwrap(),
        )
        .unwrap();
        assert!(
            validate_references(
                "<img src=\"assets/missing.png\">",
                revision.path(),
                &manifest
            )
            .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_asset_cannot_escape_its_document() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let png = b"\x89PNG\r\n\x1a\nfixture";
        let uri = format!(
            "<img src=\"data:image/png;base64,{}\">",
            STANDARD.encode(png)
        );
        let mut manifest = Manifest::default();
        externalize(&uri, root.path(), &mut manifest).unwrap();
        let id = manifest.assets.keys().next().unwrap();
        let path = root.path().join("assets").join(id);
        fs::write(outside.path().join(id), png).unwrap();
        fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(outside.path().join(id), &path).unwrap();
        assert!(read(root.path(), &manifest, id).is_err());
    }
    #[test]
    fn externalization_preserves_bytes_deduplicates_and_rejects_tampering() {
        let root = tempfile::tempdir().unwrap();
        let png = b"\x89PNG\r\n\x1a\nfixture";
        let uri = format!("data:image/png;base64,{}", STANDARD.encode(png));
        let source = format!("Text <img src=\"{uri}\">\n<img src=\"{uri}\"> end");
        let mut manifest = Manifest::default();
        let result = externalize(&source, root.path(), &mut manifest).unwrap();
        assert!(!result.contains("base64"));
        assert_eq!(manifest.assets.len(), 1);
        let id = manifest.assets.keys().next().unwrap();
        assert_eq!(read(root.path(), &manifest, id).unwrap(), png);
        save(root.path(), &manifest).unwrap();
        assert_eq!(load(root.path()).unwrap(), manifest);
        validate_references(&result, root.path(), &manifest).unwrap();
        assert!(read(root.path(), &manifest, "../secret").is_err());
        fs::write(root.path().join("assets").join(id), b"corrupt").unwrap();
        assert!(validate_references(&result, root.path(), &manifest).is_err());
    }
}
