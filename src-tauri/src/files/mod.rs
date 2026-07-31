use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::db;

/// 案件文件夹根目录
#[allow(dead_code)]
pub fn case_folder_base() -> PathBuf {
    dirs::document_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Casy")
        .join("cases")
}

/// 确保案件文件夹存在，返回路径
#[allow(dead_code)]
pub fn ensure_case_folder(case: &db::cases::Case) -> Result<PathBuf> {
    let base = case_folder_base();
    let case_no = case.case_no.as_deref().unwrap_or("无案号");
    let short_id = &case.id[..8.min(case.id.len())];
    let folder_name = format!("{}_{}", sanitize_filename(case_no), short_id);
    let folder = base.join(&folder_name);

    std::fs::create_dir_all(&folder)?;
    for sub in &["传票", "证据", "交文", "收文", "内部", "通信", "其他"] {
        std::fs::create_dir_all(folder.join(sub))?;
    }

    Ok(folder)
}

/// 清理文件名中的非法字符
#[allow(dead_code)]
pub fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .collect()
}

/// 智能命名（含 SHA-256 防覆盖）
#[allow(dead_code)]
pub fn smart_rename(
    original: &str,
    case: &db::cases::Case,
    category: &str,
    doc_date: Option<&str>,
) -> String {
    let date_str = doc_date
        .map(|d| d.to_string())
        .unwrap_or_else(|| chrono::Local::now().naive_local().date().format("%Y-%m-%d").to_string());

    let category_cn = match category {
        "summons" => "传票",
        "evidence" => "证据",
        "submitted" => "交文",
        "received" => "收文",
        "internal" => "内部",
        "correspondence" => "通信",
        _ => "其他",
    };

    let ext = Path::new(original)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("pdf");

    let case_no = case.case_no.as_deref().unwrap_or("无案号");
    let clean_no: String = case_no
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect();

    // SHA-256 防覆盖
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(original.as_bytes());
    hasher.update(case.id.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    let hash_suffix = &hash[..8];

    format!("{date_str}_{category_cn}_{clean_no}_{hash_suffix}.{ext}")
}

/// 自动分类文件
#[allow(dead_code)]
pub fn auto_classify(file_name: &str) -> &'static str {
    let name_lower = file_name.to_lowercase();
    if name_lower.contains("传票") || name_lower.contains("通知书") {
        return "summons";
    }
    if name_lower.contains("证据") {
        return "evidence";
    }
    if name_lower.contains("起诉状") || name_lower.contains("答辩状") || name_lower.contains("请求书") {
        return "submitted";
    }
    if name_lower.contains("判决") || name_lower.contains("裁定") || name_lower.contains("决定") {
        return "received";
    }
    "other"
}
