use harumi::Document;

fn main() -> anyhow::Result<()> {
    let source = std::env::args().nth(1).ok_or_else(|| anyhow::anyhow!("Expected PDF path"))?;
    let doc = Document::from_file(source)?;
    for page in 1..=doc.page_count() {
        let text = doc.extract_text(page)?;
        println!("{}", serde_json::json!({
            "page": page,
            "characters": text.chars().filter(|c| !c.is_whitespace()).count(),
            "replacementCharacters": text.matches('\u{fffd}').count(),
            "nullCharacters": text.matches('\0').count(),
            "imageBoxes": doc.page_image_bboxes(page)?,
            "regions": doc.extract_text_runs(page)?.len(),
        }));
    }
    Ok(())
}
