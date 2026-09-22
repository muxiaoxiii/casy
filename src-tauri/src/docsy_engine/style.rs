use serde::Deserialize;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentStyle {
    pub font_family: String,
    pub body_size: f32,
    pub line_height: f32,
    pub heading_sizes: Vec<f32>,
}
pub fn current() -> &'static DocumentStyle {
    static STYLE: std::sync::OnceLock<DocumentStyle> = std::sync::OnceLock::new();
    STYLE.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../src/shared/editor/document-style.json"
        ))
        .expect("bundled document style")
    })
}
