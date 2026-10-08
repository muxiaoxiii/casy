use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// DOCX 导出结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    /// 输出文件路径
    pub output_path: String,
    /// 文件大小（字节）
    pub file_size: u64,
    /// 导出时间
    pub exported_at: String,
}

/// 导出 DOCX 文件
/// 使用模板替换的方式生成新的 docx 文件
pub fn export_docx(
    template_path: &str,
    values: &HashMap<String, serde_json::Value>,
    output_path: Option<&str>,
) -> Result<ExportResult> {
    let template = Path::new(template_path);
    if !template.exists() {
        anyhow::bail!("模板文件不存在: {}", template_path);
    }

    // 确定输出路径
    // 安全（审查 P0-4）：显式路径必须经统一校验（绝对路径 + .docx + 消除 `..` 穿越）
    let output = match output_path {
        Some(p) => crate::docsy_engine::output_path::resolve_explicit_output_path(p, &["docx"])?,
        None => generate_output_path(template)?,
    };

    // 确保输出目录存在
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    // 读取模板 docx（ZIP 格式）
    let template_bytes = fs::read(template)?;

    // 替换内容并写入新文件
    let output_bytes = replace_docx_content(&template_bytes, values)?;

    // M3/P1-10：裸 fs::write 会在崩溃/磁盘满时留下半份 docx，且秒级时间戳命名
    // 会让同秒内的第二次导出静默覆盖第一次。改为同目录临时文件 + sync_all + 原子 persist，
    // 生成命名下再叠加数字后缀，绝不覆盖既有导出。
    let directory = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let target = match output_path {
        // 用户经保存对话框显式指定的路径仍允许覆盖（rename 本身是原子的）
        Some(_) => output.clone(),
        None => available_export_path(&output),
    };
    {
        use std::io::Write as _;
        let mut temporary = tempfile::Builder::new()
            .prefix(".casy-docx-")
            .tempfile_in(&directory)?;
        temporary.write_all(&output_bytes)?;
        temporary.as_file().sync_all()?;
        match temporary.persist_noclobber(&target) {
            Ok(_) => {}
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                error.file.persist(&target)?;
            }
            Err(error) => return Err(error.error.into()),
        }
    }

    let metadata = fs::metadata(&target)?;

    Ok(ExportResult {
        output_path: target.to_string_lossy().to_string(),
        file_size: metadata.len(),
        exported_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    })
}

/// 生成命名已存在时追加 `_1/_2` 数字后缀（M3/P1-10），绝不覆盖既有导出文件。
fn available_export_path(preferred: &Path) -> PathBuf {
    if !preferred.exists() {
        return preferred.to_path_buf();
    }
    let stem = preferred
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    let extension = preferred
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| format!(".{s}"))
        .unwrap_or_default();
    let parent = preferred
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    for index in 1..1000 {
        let candidate = parent.join(format!("{stem}_{index}{extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    preferred.to_path_buf()
}

/// 替换 docx 文件中的占位符
fn replace_docx_content(
    docx_bytes: &[u8],
    values: &HashMap<String, serde_json::Value>,
) -> Result<Vec<u8>> {
    use std::io::{Read, Write};
    use zip::write::FileOptions;
    use zip::{ZipArchive, ZipWriter};

    let reader = std::io::Cursor::new(docx_bytes);
    let mut archive = ZipArchive::new(reader)?;

    let mut output_buf = Vec::new();
    {
        let writer = std::io::Cursor::new(&mut output_buf);
        let mut zip = ZipWriter::new(writer);

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let name = file.name().to_string();

            let mut content = Vec::new();
            file.read_to_end(&mut content)?;

            // 只处理 word/document.xml
            if name == "word/document.xml" {
                let xml = String::from_utf8_lossy(&content).to_string();
                let replaced = replace_placeholders(&xml, values)?;
                content = replaced.into_bytes();
            }

            // 写入新 ZIP
            let options = FileOptions::default().compression_method(file.compression());

            zip.start_file(&name, options)?;
            zip.write_all(&content)?;
        }

        zip.finish()?;
    }

    Ok(output_buf)
}

/// 替换 XML 中的占位符
fn replace_placeholders(xml: &str, values: &HashMap<String, serde_json::Value>) -> Result<String> {
    let re = regex::Regex::new(r"\{\{([^}]+)\}\}|\{([^}]+)\}")?;

    let result = re.replace_all(xml, |caps: &regex::Captures| {
        let field_name = caps
            .get(1)
            .or(caps.get(2))
            .map(|m| m.as_str().trim())
            .unwrap_or("");

        if field_name.is_empty() {
            return caps[0].to_string();
        }

        // 查找字段值
        if let Some(value) = values.get(field_name) {
            let display_value = format_value(value);
            // XML 转义
            escape_xml(&display_value)
        } else {
            // 字段未提供值，保留原始占位符
            caps[0].to_string()
        }
    });

    Ok(result.into_owned())
}

/// 格式化 JSON 值为字符串
fn format_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => {
            let parts: Vec<String> = arr
                .iter()
                .filter_map(|item| {
                    if let Some(obj) = item.as_object() {
                        let name = obj.get("name")?.as_str()?;
                        let suffix = obj.get("suffix").and_then(|s| s.as_str()).unwrap_or("");
                        if suffix.is_empty() {
                            Some(name.to_string())
                        } else {
                            Some(format!("{}({})", name, suffix))
                        }
                    } else {
                        item.as_str().map(|s| s.to_string())
                    }
                })
                .collect();
            parts.join("、")
        }
        serde_json::Value::Bool(b) => {
            if *b {
                "✓".to_string()
            } else {
                "".to_string()
            }
        }
        serde_json::Value::Null => "".to_string(),
        other => other.to_string(),
    }
}

/// XML 转义
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// 生成输出文件路径
fn generate_output_path(template: &Path) -> Result<PathBuf> {
    let output_dir = crate::runtime_paths::export_root();

    fs::create_dir_all(&output_dir)?;

    let template_name = template
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("{}_{}.docx", template_name, timestamp);

    Ok(output_dir.join(filename))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_xml() {
        assert_eq!(escape_xml("Hello & <World>"), "Hello &amp; &lt;World&gt;");
    }

    #[test]
    fn test_format_value_string() {
        let val = serde_json::Value::String("test".to_string());
        assert_eq!(format_value(&val), "test");
    }

    #[test]
    fn test_format_value_array() {
        let val = serde_json::json!([
            {"name": "张三", "suffix": "原告"},
            {"name": "李四", "suffix": "代理"}
        ]);
        assert_eq!(format_value(&val), "张三(原告)、李四(代理)");
    }

    /// M3/P1-10：秒级时间戳命名下，同秒重复导出不得覆盖上一次的结果。
    #[test]
    fn generated_export_name_never_clobbers_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let preferred = dir.path().join("委托代理合同_20261008_120000.docx");
        std::fs::write(&preferred, b"first").unwrap();
        let first = available_export_path(&preferred);
        assert_ne!(first, preferred);
        assert_eq!(
            first.file_name().unwrap().to_string_lossy(),
            "委托代理合同_20261008_120000_1.docx"
        );
        std::fs::write(&first, b"second").unwrap();
        assert_eq!(
            available_export_path(&preferred)
                .file_name()
                .unwrap()
                .to_string_lossy(),
            "委托代理合同_20261008_120000_2.docx"
        );
        // 目标不存在时保持原命名
        let fresh = dir.path().join("fresh.docx");
        assert_eq!(available_export_path(&fresh), fresh);
    }
}
