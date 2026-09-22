use super::Page;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub page_number: u32,
    pub region_index: Option<usize>,
    pub bbox: Option<[f32; 4]>,
    pub width: Option<f32>,
    pub height: Option<f32>,
}
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceMap {
    pub version: u32,
    pub offset_unit: String,
    pub source_sha256: String,
    pub markdown_sha256: String,
    pub text_sha256: String,
    pub text: String,
    pub text_spans: Vec<Span>,
    pub markdown_spans: Vec<Span>,
}
fn span(
    p: &Page,
    start: usize,
    end: usize,
    region_index: Option<usize>,
    bbox: Option<[f32; 4]>,
) -> Span {
    Span {
        start,
        end,
        page_number: p.page_number,
        region_index,
        bbox,
        width: p.width,
        height: p.height,
    }
}
pub fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\"', "&quot;")
        .replace('\n', "<br>")
}
pub fn tagged_region(index: usize, text: &str) -> String {
    format!(
        "<span data-ocr-region=\"{index}\">{}</span>",
        escape_html(text)
    )
}
/// Update only a uniquely identified source fragment; preserve table topology and other cells.
pub fn corrected_markdown(markdown: &str, index: usize, old: &str, new: &str) -> Option<String> {
    let tag = tagged_region(index, old);
    (markdown.match_indices(&tag).count() == 1)
        .then(|| markdown.replacen(&tag, &tagged_region(index, new), 1))
}
struct MarkdownWriter<W: std::io::Write> { writer: W, length: usize, hash: Sha256 }
impl<W: std::io::Write> MarkdownWriter<W> {
    fn push_str(&mut self, value: &str) -> std::io::Result<()> {
        self.writer.write_all(value.as_bytes())?;
        self.hash.update(value.as_bytes()); self.length += value.len(); Ok(())
    }
    fn len(&self) -> usize { self.length }
}

pub fn write_to(pages: &[Page], source_sha256: &str, writer: impl std::io::Write) -> std::io::Result<SourceMap> {
    let mut markdown = MarkdownWriter { writer, length: 0, hash: Sha256::new() };
    let mut text = String::new();
    let mut text_spans = Vec::new();
    let mut markdown_spans = Vec::new();
    for (i, p) in pages.iter().enumerate() {
        if i > 0 {
            markdown.push_str("\n\n---\n\n")?;
            text.push_str("\n");
        }
        markdown.push_str(&format!("<!-- page {} -->\n", p.page_number))?;
        let start = markdown.len();
        markdown.push_str(&p.markdown)?;
        let region_text = p
            .regions
            .iter()
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if !p.regions.is_empty() && p.markdown == region_text {
            let mut offset = start;
            for (index, r) in p.regions.iter().enumerate() {
                markdown_spans.push(span(
                    p,
                    offset,
                    offset + r.text.len(),
                    Some(index),
                    Some(r.bbox),
                ));
                offset += r.text.len() + 1;
            }
        } else {
            let mut tagged_spans = Vec::new();
            for (index, region) in p.regions.iter().enumerate() {
                let tag = tagged_region(index, &region.text);
                let matches: Vec<_> = p.markdown.match_indices(&tag).collect();
                if matches.len() == 1 {
                    let prefix = format!("<span data-ocr-region=\"{index}\">");
                    let offset = start + matches[0].0 + prefix.len();
                    tagged_spans.push(span(
                        p,
                        offset,
                        offset + escape_html(&region.text).len(),
                        Some(index),
                        Some(region.bbox),
                    ));
                }
            }
            if tagged_spans.len() == p.regions.len() && !tagged_spans.is_empty() {
                tagged_spans.sort_by_key(|s| s.start);
                markdown_spans.extend(tagged_spans);
            } else {
                markdown_spans.push(span(p, start, markdown.len(), None, None));
            }
        }
        if p.regions.is_empty() {
            let start = text.len();
            text.push_str(&p.plain_text);
            text_spans.push(span(p, start, text.len(), None, None));
        } else {
            for (index, r) in p.regions.iter().enumerate() {
                if index > 0 {
                    text.push_str("\n");
                }
                let start = text.len();
                text.push_str(&r.text);
                text_spans.push(span(p, start, text.len(), Some(index), Some(r.bbox)));
            }
        }
    }
    let map = SourceMap {
        version: 1,
        offset_unit: "utf8-bytes".into(),
        source_sha256: source_sha256.into(),
        markdown_sha256: hex::encode(markdown.hash.finalize()),
        text_sha256: hex::encode(Sha256::digest(text.as_bytes())),
        text,
        text_spans,
        markdown_spans,
    };
    markdown.writer.flush()?;
    Ok(map)
}

#[cfg(test)]
pub fn build(pages: &[Page], source_sha256: &str) -> (String, SourceMap) {
    let mut bytes = Vec::new();
    let map = write_to(pages, source_sha256, &mut bytes).unwrap();
    (String::from_utf8(bytes).unwrap(), map)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn page(number: u32, text: &str) -> Page {
        serde_json::from_value(serde_json::json!({ "pageNumber":number,"width":400.0,"height":600.0,
   "plainText":text,"markdown":text,"regions":[{"text":text,"bbox":[20.0,30.0,300.0,50.0],"confidence":0.9}],
   "confidence":0.9,"nativeText":false })).unwrap()
    }
    #[test]
    fn multilingual_offsets_bind_exact_bytes_and_both_pages() {
        let pages = vec![page(1, "Prüfung français 日本語"), page(2, "跨页证据")];
        let (md, map) = build(&pages, "source-hash");
        assert_eq!(
            md,
            "<!-- page 1 -->\nPrüfung français 日本語\n\n---\n\n<!-- page 2 -->\n跨页证据"
        );
        assert_eq!(map.text, "Prüfung français 日本語\n跨页证据");
        for (index, span) in map.text_spans.iter().enumerate() {
            assert_eq!(&map.text[span.start..span.end], pages[index].plain_text);
            assert_eq!(span.page_number, index as u32 + 1);
            assert_eq!(span.bbox, Some(pages[index].regions[0].bbox));
        }
        for (index, span) in map.markdown_spans.iter().enumerate() {
            assert_eq!(&md[span.start..span.end], pages[index].plain_text);
            assert!(span.bbox.is_some());
        }
        assert_eq!(
            map.markdown_sha256,
            hex::encode(Sha256::digest(md.as_bytes()))
        );
    }
    #[test]
    fn table_markup_maps_escaped_values_and_corrections_preserve_grid() {
        let mut p = page(1, "<0.001 & value");
        p.markdown = format!(
            "<table><tr><td rowspan=\"2\">{}</td></tr><tr></tr></table>",
            tagged_region(0, &p.regions[0].text)
        );
        let (md, map) = build(&[p.clone()], "hash");
        let mapped = &map.markdown_spans[0];
        assert_eq!(&md[mapped.start..mapped.end], "&lt;0.001 &amp; value");
        assert_eq!(mapped.region_index, Some(0));
        let revised = corrected_markdown(&p.markdown, 0, "<0.001 & value", "<0.002").unwrap();
        assert!(revised.contains("rowspan=\"2\""));
        assert!(revised.contains("&lt;0.002"));
        assert!(corrected_markdown(&revised, 0, "old text", "bad").is_none());
        p.markdown.push_str(&tagged_region(0, &p.regions[0].text));
        assert!(
            build(&[p], "hash").1.markdown_spans[0]
                .region_index
                .is_none()
        );
    }
    #[test]
    fn divergent_markdown_never_invents_coordinates() {
        let mut p = page(1, "recognized text");
        p.markdown = "rewritten text".into();
        let (_, map) = build(&[p], "hash");
        assert_eq!(map.markdown_spans[0].bbox, None);
        assert_eq!(map.markdown_spans[0].region_index, None);
        assert!(map.text_spans[0].bbox.is_some());
    }
}
