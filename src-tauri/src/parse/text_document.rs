use crate::document_pipeline::{DocumentPage, ProcessRequest, ProcessResult};
use anyhow::{bail, Context, Result};
use pulldown_cmark::{Event, Options, Parser};
use std::path::Path;

const CHUNK_CHARS: usize = 3000;

pub fn supports(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|s| s.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("md" | "markdown" | "txt" | "doc" | "docx" | "docm" | "rtf" | "odt")
    )
}

pub fn extract_markdown(path: &Path) -> Result<String> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if matches!(ext.as_str(), "md" | "markdown" | "txt") {
        let mut text = std::fs::read_to_string(path)
            .context("TEXT_ENCODING: 无法读取文件，文本需要 UTF-8 编码")?;
        if text.starts_with('\u{feff}') {
            text.drain(..'\u{feff}'.len_utf8());
        }
        return finish_markdown(text);
    }

    let mut bytes = std::fs::read(path).context("无法读取文档")?;
    // anydoc already implements .doc (MS-DOC). Sidecar doc2x is only a fallback
    // when anydoc rejects a hard legacy binary.
    if ext == "doc" {
        match anydoc::to_markdown_bytes(&bytes, anydoc::Format::Doc) {
            Ok(markdown) if !markdown.trim().is_empty() => return finish_markdown(markdown),
            Ok(_) => {}
            Err(primary) => match crate::parse::doc2docx::convert_doc_to_docx(&bytes) {
                Ok(converted) => bytes = converted,
                Err(sidecar) => {
                    return Err(anyhow::anyhow!(
                        "DOCUMENT_PARSE: 无法解析此旧版 Word 文件，请另存为 DOCX 后重试。anydoc: {primary}；doc2x: {sidecar}"
                    ));
                }
            },
        }
    }
    let format = if ext == "doc" && bytes.starts_with(b"PK") {
        anydoc::Format::Docx
    } else {
        anydoc::Format::from_extension(&ext)
            .or_else(|| anydoc::Format::from_bytes(&bytes))
            .unwrap_or(anydoc::Format::Doc)
    };
    let markdown = anydoc::to_markdown_bytes(&bytes, format).map_err(|e| {
        if ext == "doc" {
            anyhow::anyhow!(
                "DOCUMENT_PARSE: 无法解析此旧版 Word 文件，请另存为 DOCX 后重试。anydoc: {e}"
            )
        } else {
            anyhow::anyhow!("DOCUMENT_PARSE: {e}")
        }
    })?;
    finish_markdown(markdown)
}

fn finish_markdown(markdown: String) -> Result<String> {
    if markdown.trim().is_empty() {
        bail!("NO_TEXT: 文件没有可提取的正文");
    }
    Ok(markdown)
}

fn plain_text(markdown: &str) -> String {
    let mut out = String::new();
    for event in Parser::new_ext(markdown, Options::all()) {
        match event {
            Event::Text(text)
            | Event::Code(text)
            | Event::InlineMath(text)
            | Event::DisplayMath(text) => out.push_str(&text),
            Event::SoftBreak | Event::HardBreak | Event::End(_) => out.push('\n'),
            _ => {}
        }
    }
    out
}

// Preserve every byte in order. Chunks are retrieval segments, never physical Word pages.
pub fn segment_markdown(markdown: &str) -> Vec<DocumentPage> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut chars = 0;
    let mut last_break = None;
    for (offset, ch) in markdown.char_indices() {
        chars += 1;
        if ch == '\n' {
            last_break = Some(offset + 1);
        }
        if chars >= CHUNK_CHARS {
            let end = last_break
                .filter(|end| *end > start + (offset - start) / 2)
                .unwrap_or(offset + ch.len_utf8());
            result.push(segment(result.len(), &markdown[start..end]));
            chars = markdown[end..offset + ch.len_utf8()].chars().count();
            start = end;
            last_break = None;
        }
    }
    if start < markdown.len() {
        result.push(segment(result.len(), &markdown[start..]));
    }
    result
}

fn segment(index: usize, markdown: &str) -> DocumentPage {
    DocumentPage {
        page_number: index as u32 + 1,
        width: None,
        height: None,
        plain_text: plain_text(markdown),
        markdown: markdown.into(),
        regions: vec![],
        confidence: None,
        layout: None,
        timing: None,
        orientation_degrees: None,
    }
}

pub fn process(request: &ProcessRequest) -> Result<ProcessResult> {
    let source = Path::new(&request.source_path);
    let markdown = extract_markdown(source)?;
    let pages = segment_markdown(&markdown);
    let root = Path::new(&request.output_dir);
    std::fs::create_dir_all(root)?;
    let markdown_path = root.join("document.md");
    let page_ir_path = root.join("segments.json");
    std::fs::write(&markdown_path, markdown)?;
    use std::io::Write;
    let mut writer = std::io::BufWriter::new(std::fs::File::create(&page_ir_path)?);
    serde_json::to_writer(&mut writer, &pages)?;
    writer.flush()?;
    Ok(ProcessResult {
        source_map_path: None,
        elapsed_ms: 0,
        source_sha256: request.source_sha256.clone(),
        engine: "text-document".into(),
        model_version: Some("anydoc-0.2.4".into()),
        searchable_pdf_path: None,
        page_ir_path: page_ir_path.display().to_string(),
        markdown_path: markdown_path.display().to_string(),
        page_count: pages.len() as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_text_source_and_extracted_markdown_larger_than_64_mib() {
        use std::io::Write;
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("large.md");
        let mut file = std::fs::File::create(&path).unwrap();
        let block = vec![b'a'; 1024 * 1024];
        for _ in 0..65 { file.write_all(&block).unwrap(); }
        file.write_all("\n最后一行完整保留".as_bytes()).unwrap();
        drop(file);
        let markdown = extract_markdown(&path).unwrap();
        assert!(markdown.len() > 64 * 1024 * 1024);
        assert_eq!(markdown.len() as u64, std::fs::metadata(path).unwrap().len());
        assert!(markdown.ends_with("\n最后一行完整保留"));
    }

    #[test]
    fn segments_keep_all_content_and_bound_cjk_chunks() {
        let source = format!(
            "# 标题\n\n{}\n\n{}",
            "中文正文".repeat(5000),
            "a".repeat(10000)
        );
        let pages = segment_markdown(&source);
        assert!(pages.len() > 5);
        assert_eq!(
            pages
                .iter()
                .map(|p| p.markdown.as_str())
                .collect::<String>(),
            source
        );
        assert!(pages
            .iter()
            .all(|p| p.markdown.chars().count() <= CHUNK_CHARS && p.width.is_none()));
    }

    #[test]
    fn docx_keeps_headings_tables_and_chinese_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.docx");
        use docx_rs::*;
        Docx::new()
            .add_paragraph(
                Paragraph::new()
                    .style("Heading1")
                    .add_run(Run::new().add_text("证据清单")),
            )
            .add_table(Table::new(vec![
                TableRow::new(vec![
                    TableCell::new()
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("第三人"))),
                    TableCell::new()
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("标的额"))),
                ]),
                TableRow::new(vec![
                    TableCell::new()
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("乙公司"))),
                    TableCell::new()
                        .add_paragraph(Paragraph::new().add_run(Run::new().add_text("125000.25"))),
                ]),
            ]))
            .build()
            .pack(std::fs::File::create(&path).unwrap())
            .unwrap();
        let md = extract_markdown(&path).unwrap();
        for term in ["证据清单", "第三人", "乙公司", "125000.25", "|"] {
            assert!(md.contains(term), "{md}");
        }
    }

    #[test]
    fn extracts_legacy_doc_when_sidecar_ready() {
        if crate::parse::doc2docx::converter_path().is_none() {
            eprintln!("skip: doc2x not prepared");
            return;
        }
        let sample = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/doc-import/sample.doc");
        if !sample.is_file() {
            eprintln!("skip: sample.doc missing");
            return;
        }
        let md = extract_markdown(&sample).expect("extract sample.doc");
        assert!(!md.trim().is_empty());
        assert!(
            md.contains("Test Document") || md.contains("sample") || md.contains("document"),
            "unexpected markdown: {md}"
        );
    }

    #[test]
    fn anydoc_parses_legacy_doc_without_sidecar() {
        let sample = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/doc-import/sample.doc");
        if !sample.is_file() {
            eprintln!("skip: sample.doc missing");
            return;
        }
        let bytes = std::fs::read(&sample).unwrap();
        let md = anydoc::to_markdown_bytes(&bytes, anydoc::Format::Doc)
            .expect("anydoc must parse .doc without doc2x");
        assert!(!md.trim().is_empty(), "anydoc produced empty markdown");
        assert!(
            md.to_ascii_lowercase().contains("document") || md.contains("sample"),
            "unexpected anydoc .doc output: {md}"
        );
    }
}
