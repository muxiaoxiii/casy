//! PDF Extractor (Pure Rust Fast Path)
//! Using `pdf-extract` for lightning-fast text extraction from electronic PDFs.
//! Scanned PDFs will be short-circuited and marked for OCR extension.

use log::{info, warn, error};
use std::path::Path;

/// 智能 PDF 极速提取：
/// 尝试抽取 PDF 自带的文本层。如果抽取出的有效字符少于一定阈值，
/// 说明是纯图片扫描件，则返回特定错误供上层标记为需要 OCR。
pub async fn extract_pdf_to_markdown<P: AsRef<Path>>(
    file_path: P,
) -> Result<String, String> {
    let path_str = file_path.as_ref().to_string_lossy().to_string();
    info!("Starting PDF fast text extraction for: {}", path_str);
    
    // 我们将耗时的 CPU 密集型任务放进 spawn_blocking 中
    let path_clone = path_str.clone();
    let text_result = tokio::task::spawn_blocking(move || {
        pdf_extract::extract_text(&path_clone)
    }).await.map_err(|e| format!("Task join error: {}", e))?;

    match text_result {
        Ok(text) => {
            let trimmed = text.trim();
            // 扫描件探针：如果整篇文档提取出的文本过少（少于 50 个非空字符）
            if trimmed.len() < 50 {
                warn!("PDF contains less than 50 characters. Likely a scanned document: {}", path_str);
                return Err("NeedsOCR".to_string());
            }
            
            info!("PDF fast extraction completed successfully for {}. Total characters: {}", path_str, trimmed.len());
            
            // 为了模拟良好的排版结构，我们可以简单给文本加上标题（实际应用中，可以在这里用正则进行基本段落清洗）
            let markdown = format!(
                "# {} 案卷提取结果 (Fast Track)\n\n{}",
                file_path.as_ref().file_name().unwrap_or_default().to_string_lossy(),
                trimmed
            );
            
            Ok(markdown)
        }
        Err(e) => {
            error!("Failed to extract text using pdf-extract: {}", e);
            // 发生解析错误（例如某些被强力加密或极度异常的PDF），同样可以回退给 OCR
            Err("NeedsOCR".to_string())
        }
    }
}
