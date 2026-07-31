use anyhow::{Context, Result};
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
    let output = match output_path {
        Some(p) => PathBuf::from(p),
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

    fs::write(&output, &output_bytes)?;

    let metadata = fs::metadata(&output)?;

    Ok(ExportResult {
        output_path: output.to_string_lossy().to_string(),
        file_size: metadata.len(),
        exported_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    })
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
            let options = FileOptions::default()
                .compression_method(file.compression());

            zip.start_file(&name, options)?;
            zip.write_all(&content)?;
        }

        zip.finish()?;
    }

    Ok(output_buf)
}

/// 替换 XML 中的占位符
fn replace_placeholders(
    xml: &str,
    values: &HashMap<String, serde_json::Value>,
) -> Result<String> {
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
                        let suffix = obj
                            .get("suffix")
                            .and_then(|s| s.as_str())
                            .unwrap_or("");
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
    let home = dirs::home_dir().context("无法获取用户主目录")?;
    let output_dir = home.join("Documents").join("Casy").join("exports");

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
        assert_eq!(
            escape_xml("Hello & <World>"),
            "Hello &amp; &lt;World&gt;"
        );
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
}
