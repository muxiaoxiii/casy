//! Searchable PDF from the same Typst layout used by the editor preview.
use anyhow::Result;
use serde_json::Value;

/// Thin wrapper over `typesetting::compile` for unit tests; production export
/// goes through `commands::docs` → `typesetting` directly.
#[allow(dead_code)]
pub fn export_pdf(root: &Value) -> Result<Vec<u8>> {
    Ok(
        super::typesetting::compile(root, &super::typesetting::LayoutOptions::default())?
            .pdf
            .clone(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::Document;
    #[test]
    fn searchable_chinese_pdf_paginates_and_repeats_table_headers() {
        let p = |text: &str| serde_json::json!({"type":"paragraph","content":[{"type":"text","text":text}]});
        let mut rows = vec![
            serde_json::json!({"type":"tableRow","content":[{"type":"tableHeader","content":[p("证据名称")]},{"type":"tableHeader","content":[p("核对内容")]}]}),
        ];
        for n in 0..75 {
            rows.push(serde_json::json!({"type":"tableRow","content":[{"type":"tableCell","content":[p(&format!("材料{n}"))]},{"type":"tableCell","content":[p("核对原件、转文日期与送达记录")]}]}))
        }
        let root = serde_json::json!({"type":"doc","content":[{"type":"heading","attrs":{"level":1},"content":[{"type":"text","text":"中文案情梳理"}]},{"type":"table","content":rows}]});
        let bytes = export_pdf(&root).unwrap();
        let doc = Document::load_mem(&bytes).unwrap();
        assert!(doc.get_pages().len() >= 3);
        let text: String = pdf_extract::extract_text_from_mem(&bytes)
            .unwrap()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        assert!(text.contains("中文案情梳理"));
        assert!(text.contains("材料74"), "{text:?}");
        assert!(text.matches("证据名称").count() >= 3);
    }
    #[test]
    fn pdf_refuses_unknown_nodes_instead_of_dropping_content() {
        assert!(export_pdf(&serde_json::json!({"type":"doc","content":[{"type":"rawHtmlBlock","attrs":{"raw":"preserved"}}]})).unwrap_err().to_string().contains("rawHtmlBlock"));
    }
}
