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
pub fn build(pages: &[Page], source_sha256: &str) -> (String, SourceMap) {
    let mut markdown = String::new();
    let mut text = String::new();
    let mut text_spans = Vec::new();
    let mut markdown_spans = Vec::new();
    for (i, p) in pages.iter().enumerate() {
        if i > 0 {
            markdown.push_str("\n\n---\n\n");
            text.push_str("\n");
        }
        markdown.push_str(&format!("<!-- page {} -->\n", p.page_number));
        let start = markdown.len();
        markdown.push_str(&p.markdown);
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
            markdown_spans.push(span(p, start, markdown.len(), None, None));
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
        markdown_sha256: hex::encode(Sha256::digest(markdown.as_bytes())),
        text_sha256: hex::encode(Sha256::digest(text.as_bytes())),
        text,
        text_spans,
        markdown_spans,
    };
    (markdown, map)
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
    fn divergent_markdown_never_invents_coordinates() {
        let mut p = page(1, "recognized text");
        p.markdown = "rewritten text".into();
        let (_, map) = build(&[p], "hash");
        assert_eq!(map.markdown_spans[0].bbox, None);
        assert_eq!(map.markdown_spans[0].region_index, None);
        assert!(map.text_spans[0].bbox.is_some());
    }
}
