//! Preserve model-detected non-text content without pretending OCR extracts its semantics.
use crate::{Region, source_map};
use anyhow::Result;
use base64::{Engine, engine::general_purpose::STANDARD};
use image::{ImageFormat, RgbImage};

pub struct Visual {
    pub kind: String,
    pub bbox: [f32; 4],
    png: Vec<u8>,
}

pub fn is_visual(kind: &str) -> bool {
    matches!(
        kind,
        "image" | "chart" | "figure" | "formula" | "display_formula" | "inline_formula" | "seal"
    )
}

impl Visual {
    pub fn capture(image: &RgbImage, bbox: [f32; 4], kind: &str) -> Result<Option<Self>> {
        if bbox.iter().any(|n| !n.is_finite()) || bbox[2] <= bbox[0] || bbox[3] <= bbox[1] {
            return Ok(None);
        }
        let x = (bbox[0] - 8.0).floor().clamp(0.0, image.width() as f32) as u32;
        let y = (bbox[1] - 8.0).floor().clamp(0.0, image.height() as f32) as u32;
        let right = (bbox[2] + 8.0).ceil().clamp(x as f32, image.width() as f32) as u32;
        let bottom = (bbox[3] + 8.0)
            .ceil()
            .clamp(y as f32, image.height() as f32) as u32;
        if right - x < 8 || bottom - y < 8 {
            return Ok(None);
        }
        let crop = image::imageops::crop_imm(image, x, y, right - x, bottom - y).to_image();
        let mut png = std::io::Cursor::new(Vec::new());
        crop.write_to(&mut png, ImageFormat::Png)?;
        Ok(Some(Self {
            kind: kind.into(),
            bbox: [x as f32, y as f32, right as f32, bottom as f32],
            png: png.into_inner(),
        }))
    }
    pub fn contains(&self, b: [f32; 4]) -> bool {
        let area = ((b[2] - b[0]) * (b[3] - b[1])).max(1.0);
        intersection(self.bbox, b) / area > 0.6
    }
    pub fn html(&self, regions: &[Region], indices: &[usize]) -> String {
        let label = match self.kind.as_str() {
            "formula" | "display_formula" | "inline_formula" => {
                "公式原图（未转换为 LaTeX，请以原图为准）"
            }
            "chart" => "图表原图（保留曲线、坐标轴与图例；未提取曲线数据）",
            "seal" => "印章原图",
            "unresolved_table" => "表格原图（结构待核对）",
            _ => "图片原图",
        };
        let mut html = format!(
            "<figure><img alt=\"{label}\" width=\"{}\" height=\"{}\" src=\"data:image/png;base64,{}\"><figcaption>{label}</figcaption></figure>",
            (self.bbox[2] - self.bbox[0]) as u32,
            (self.bbox[3] - self.bbox[1]) as u32,
            STANDARD.encode(&self.png),
        );
        if !indices.is_empty() {
            html.push_str(
                "\n<details><summary>辅助识别文字（可能有误，不代表图形结构）</summary>\n<p>",
            );
            html.push_str(
                &indices
                    .iter()
                    .map(|&i| source_map::tagged_region(i, &regions[i].text))
                    .collect::<Vec<_>>()
                    .join("<br>"),
            );
            html.push_str("</p></details>");
        }
        html
    }
}
fn intersection(a: [f32; 4], b: [f32; 4]) -> f32 {
    (a[2].min(b[2]) - a[0].max(b[0])).max(0.0) * (a[3].min(b[3]) - a[1].max(b[1])).max(0.0)
}
pub fn deduplicate(visuals: &mut Vec<Visual>) {
    // Retain the larger source crop if the detector labels the same figure twice.
    visuals.sort_by(|a, b| {
        ((b.bbox[2] - b.bbox[0]) * (b.bbox[3] - b.bbox[1]))
            .total_cmp(&((a.bbox[2] - a.bbox[0]) * (a.bbox[3] - a.bbox[1])))
    });
    let mut kept: Vec<Visual> = Vec::new();
    for v in std::mem::take(visuals) {
        if !kept.iter().any(|a| {
            intersection(a.bbox, v.bbox) / ((v.bbox[2] - v.bbox[0]) * (v.bbox[3] - v.bbox[1])) > 0.9
        }) {
            kept.push(v);
        }
    }
    kept.sort_by(|a, b| {
        a.bbox[1]
            .total_cmp(&b.bbox[1])
            .then(a.bbox[0].total_cmp(&b.bbox[0]))
    });
    *visuals = kept;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_roundtrip_preserves_pixels_and_formula_text_is_only_auxiliary() {
        let mut image = RgbImage::from_pixel(100, 100, image::Rgb([255, 255, 255]));
        image.put_pixel(45, 45, image::Rgb([1, 2, 3]));
        let visual = Visual::capture(&image, [20.0, 20.0, 80.0, 80.0], "formula")
            .unwrap()
            .unwrap();
        let restored = image::load_from_memory(&visual.png).unwrap().to_rgb8();
        assert_eq!(restored.get_pixel(33, 33).0, [1, 2, 3]);
        let regions = vec![Region {
            text: "x < y & z".into(),
            bbox: [30.0, 30.0, 60.0, 50.0],
            confidence: Some(0.8), word_boxes: None,
        }];
        let md = crate::table::markdown_with_visuals(&regions, &mut [], 0, &[visual]);
        assert!(md.contains("未转换为 LaTeX"));
        assert!(md.contains("<details>"));
        assert_eq!(md.matches("x &lt; y &amp; z").count(), 1);
        assert!(!md.contains("file://"));
        assert!(md.contains("data:image/png;base64,"));
        assert!(
            source_map::corrected_markdown(&md, 0, "x < y & z", "x < y")
                .unwrap()
                .contains("<figure>")
        );
    }
    #[test]
    fn crops_are_clamped_and_duplicates_do_not_repeat_ocr() {
        let im = RgbImage::new(100, 100);
        let mut vs = vec![
            Visual::capture(&im, [-20.0, -20.0, 120.0, 120.0], "chart")
                .unwrap()
                .unwrap(),
            Visual::capture(&im, [5.0, 5.0, 90.0, 90.0], "image")
                .unwrap()
                .unwrap(),
        ];
        deduplicate(&mut vs);
        assert_eq!(vs.len(), 1);
        assert_eq!(vs[0].bbox, [0.0, 0.0, 100.0, 100.0]);
        assert!(
            Visual::capture(&im, [f32::NAN, 0.0, 1.0, 1.0], "chart")
                .unwrap()
                .is_none()
        );
    }
}
