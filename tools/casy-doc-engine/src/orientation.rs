//! 文本行方向分类（0/180）。
//!
//! 按 PaddleOCR 官方 `TextClassifier` 规格用 ort 直接实现：
//! 行图 resize 到高 48、宽按宽高比自适应（上限 192），**BGR** 通道序
//! （cv2 管线顺序——这是与 oar_ocr 自带适配器的关键差异，后者喂 RGB，
//! 实测预测接近随机），归一化 `/255 → (x-0.5)/0.5`，CHW，右侧补零到 192。
//! oar_ocr 的 `with_text_line_orientation_classification` 接线保留但默认关，
//! 本模块是当前可用的实现路径。

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use image::RgbImage;
use oar_ocr::oarocr::{EdgeProcessor, TextCroppingProcessor};

const IMG_H: u32 = 48;
const IMG_W: u32 = 192;
/// PaddleOCR cls_thresh 默认值：单行 180 分类得分超过它才计一票。
const LINE_THRESHOLD: f32 = 0.9;

pub struct LineOrientationClassifier {
    session: ort::session::Session,
}

impl LineOrientationClassifier {
    pub fn new(model_path: &Path) -> Result<Self> {
        let session = ort::session::Session::builder()
            .map_err(|error| anyhow::anyhow!("ORIENTATION_MODEL: {error}"))?
            .with_optimization_level(ort::session::builder::GraphOptimizationLevel::Level3)
            .map_err(|error| anyhow::anyhow!("ORIENTATION_MODEL: {error}"))?
            .commit_from_file(model_path)
            .map_err(|error| anyhow::anyhow!("ORIENTATION_MODEL: 无法加载 {}: {error}", model_path.display()))?;
        Ok(Self { session })
    }

    /// 单行预处理：返回 3×48×192 的 CHW 数据（BGR 通道序）。
    fn preprocess(line: &RgbImage) -> Vec<f32> {
        let h = line.height().max(1);
        let w = line.width().max(1);
        let wh_ratio = w as f32 / h as f32;
        let max_wh_ratio = (IMG_W as f32 / IMG_H as f32).max(wh_ratio);
        let img_w = (IMG_H as f32 * max_wh_ratio) as u32;
        let resized_w = if (IMG_H as f32 * wh_ratio).ceil() > img_w as f32 {
            img_w
        } else {
            (IMG_H as f32 * wh_ratio).ceil() as u32
        }
        .min(IMG_W)
        .max(1);
        let resized = image::imageops::resize(line, resized_w, IMG_H, image::imageops::FilterType::Triangle);
        let mut tensor = vec![0.0f32; (3 * IMG_H * IMG_W) as usize];
        let plane = (IMG_H * IMG_W) as usize;
        for y in 0..IMG_H {
            for x in 0..resized_w {
                let px = resized.get_pixel(x, y).0;
                // RGB → BGR：cv2 训练的模型要 BGR 顺序
                let bgr = [px[2], px[1], px[0]];
                for c in 0..3 {
                    let value = (bgr[c] as f32 / 255.0 - 0.5) / 0.5;
                    tensor[c * plane + (y * IMG_W + x) as usize] = value;
                }
            }
        }
        tensor
    }

    /// 对一批行图分类，返回每行的“180 类别”得分。
    pub fn score_lines(&mut self, lines: &[RgbImage]) -> Result<Vec<f32>> {
        if lines.is_empty() {
            return Ok(Vec::new());
        }
        let batch = lines.len();
        let mut data = Vec::with_capacity(batch * 3 * IMG_H as usize * IMG_W as usize);
        for line in lines {
            data.extend(Self::preprocess(line));
        }
        let input = ndarray::Array4::<f32>::from_shape_vec((batch, 3, IMG_H as usize, IMG_W as usize), data)
            .map_err(|error| anyhow::anyhow!("ORIENTATION_TENSOR: {error}"))?;
        let tensor = ort::value::Tensor::from_array(input)?;
        let outputs = self.session.run(ort::inputs! { "x" => tensor })?;
        let scores = outputs[0].try_extract_array::<f32>()?;
        // 输出 [N,2]：index 1 为 “180” 类别（PaddleOCR label_list = ['0','180']）
        Ok((0..batch)
            .map(|n| scores[ndarray::IxDyn(&[n, 1])])
            .collect())
    }

    /// 页面级判定：裁剪所有行 → 分类 → 多数票。行数不足或票数过半才判倒置。
    pub fn page_is_flipped(&mut self, image: &RgbImage, boxes: &[oar_ocr::processors::BoundingBox]) -> Result<bool> {
        if boxes.is_empty() {
            return Ok(false);
        }
        let crops = TextCroppingProcessor::new(true).process((Arc::new(image.clone()), boxes.to_vec()))?;
        let lines: Vec<RgbImage> = crops.into_iter().flatten().map(|crop| (*crop).clone()).collect();
        if lines.is_empty() {
            return Ok(false);
        }
        let scores = self.score_lines(&lines)?;
        let votes = scores.iter().filter(|score| **score > LINE_THRESHOLD).count();
        Ok(votes * 2 > scores.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 纯色行图（避免 resize 插值影响通道断言）。
    fn solid(w: u32, h: u32, rgb: [u8; 3]) -> RgbImage {
        RgbImage::from_fn(w, h, |_x, _y| image::Rgb(rgb))
    }

    /// 预处理契约（PaddleOCR resize_norm_img）：高 48、宽 ≤192、BGR 通道序、值域 [-1,1]。
    #[test]
    fn preprocess_matches_paddle_contract() {
        // 宽高比 8:1 → 自适应宽度 192（上限）
        let wide = LineOrientationClassifier::preprocess(&solid(400, 50, [0, 128, 255]));
        assert_eq!(wide.len(), 3 * 48 * 192);
        // 通道 0 应为 B：对左上角像素，B = 原 R 通道……构造行 x=0 处 R=0,G=128,B=255
        // BGR 后通道0 = B = 255 → (1-0.5)/0.5 = 1.0
        assert!((wide[0] - 1.0).abs() < 1e-6, "channel 0 must be B (=1.0)");
        // 通道 2 应为 R = 0 → (0-0.5)/0.5 = -1.0
        let plane = 48 * 192;
        assert!((wide[2 * plane] - (-1.0)).abs() < 1e-6, "channel 2 must be R (=-1.0)");
        // 右侧补零：x 超过 resized_w 的位置必须是 0
        // 8:1 的图 resized_w = min(48*8,192)=192 → 无补零；改用 4:1 以内验证补零
        let narrow = LineOrientationClassifier::preprocess(&solid(96, 48, [0, 128, 255])); // 2:1 → resized_w=96，其余补零
        assert!((narrow[96] - 0.0).abs() < 1e-6, "padding right of the line must be zero");
    }

}
