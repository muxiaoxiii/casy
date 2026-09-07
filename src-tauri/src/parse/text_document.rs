use crate::document_pipeline::{DocumentPage, ProcessRequest, ProcessResult};
use anyhow::{bail, Context, Result};
use pulldown_cmark::{Event, Options, Parser};
use std::{io::Read, path::Path};

const MAX_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_MARKDOWN_BYTES: usize = 64 * 1024 * 1024;
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
    let file = std::fs::File::open(path)?;
    if file.metadata()?.len() > MAX_SOURCE_BYTES {
        bail!("DOCUMENT_TOO_LARGE: 文本文档超过 64 MiB 处理上限");
    }
    let mut bytes = Vec::new();
    file.take(MAX_SOURCE_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_SOURCE_BYTES {
        bail!("DOCUMENT_TOO_LARGE: 文本文档超过 64 MiB 处理上限");
    }
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let markdown = if matches!(ext.as_str(), "md" | "markdown" | "txt") {
        String::from_utf8(bytes)
            .context("TEXT_ENCODING: 文本需要 UTF-8 编码")?
            .trim_start_matches('\u{feff}')
            .to_owned()
    } else {
        anydoc::to_markdown_bytes(&bytes, anydoc::Format::from_extension(&ext)).map_err(|e| {
            if ext == "doc" {
                anyhow::anyhow!(
                    "DOCUMENT_PARSE: 无法解析此旧版 Word 文件，请另存为 DOCX 后重试。{e}"
                )
            } else {
                anyhow::anyhow!("DOCUMENT_PARSE: {e}")
            }
        })?
    };
    if markdown.trim().is_empty() {
        bail!("NO_TEXT: 文件没有可提取的正文");
    }
    if markdown.len() > MAX_MARKDOWN_BYTES {
        bail!("DOCUMENT_TOO_LARGE: 提取正文超过 64 MiB 处理上限");
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
    serde_json::to_writer(std::fs::File::create(&page_ir_path)?, &pages)?;
    Ok(ProcessResult {
        source_map_path: None,
        source_sha256: request.source_sha256.clone(),
        engine: "text-document".into(),
        model_version: Some("anydoc-0.1.8".into()),
        searchable_pdf_path: None,
        page_ir_path: page_ir_path.display().to_string(),
        markdown_path: markdown_path.display().to_string(),
        pages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
