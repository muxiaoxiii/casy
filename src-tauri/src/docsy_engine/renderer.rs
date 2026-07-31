use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// 模板渲染结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderResult {
    /// 渲染后的 HTML 内容
    pub html: String,
    /// 渲染后的纯文本内容
    pub text: String,
    /// 使用的字段值
    pub used_fields: HashMap<String, String>,
    /// 缺失的必填字段
    pub missing_fields: Vec<String>,
}

/// 渲染模板，用案件数据填充占位符
pub fn render_template(
    template_path: &str,
    values: &HashMap<String, serde_json::Value>,
) -> Result<RenderResult> {
    let path = Path::new(template_path);
    if !path.exists() {
        anyhow::bail!("模板文件不存在: {}", template_path);
    }

    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext {
        "docx" | "docxtpl" => render_docx(path, values),
        _ => anyhow::bail!("不支持的模板格式: {}", ext),
    }
}

/// 渲染 docx 模板
fn render_docx(
    path: &Path,
    values: &HashMap<String, serde_json::Value>,
) -> Result<RenderResult> {
    use zip::ZipArchive;

    let file = fs::File::open(path)?;
    let mut archive = ZipArchive::new(file)?;

    let mut content = String::new();
    let mut used_fields = HashMap::new();
    let mut missing_fields = Vec::new();

    // 读取 word/document.xml
    {
        let mut doc = archive.by_name("word/document.xml")?;
        use std::io::Read;
        doc.read_to_string(&mut content)?;
    }

    // 替换占位符：支持 {{字段名}} 和 {字段名} 两种格式
    let re = regex::Regex::new(r"\{\{([^}]+)\}\}|\{([^}]+)\}")?;

    let result = re.replace_all(&content, |caps: &regex::Captures| {
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
            let display_value = match value {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Array(arr) => {
                    // party_list 格式：[{name, suffix}]
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
            };

            used_fields.insert(field_name.to_string(), display_value.clone());
            display_value
        } else {
            // 字段未提供值，保留原始占位符
            missing_fields.push(field_name.to_string());
            caps[0].to_string()
        }
    });

    let html = convert_docx_xml_to_html(&result)?;
    let text = extract_text_from_xml(&result);

    Ok(RenderResult {
        html,
        text,
        used_fields,
        missing_fields,
    })
}

/// 将 docx XML 转换为简单的 HTML
fn convert_docx_xml_to_html(xml: &str) -> Result<String> {
    let mut html = String::new();
    html.push_str("<div class=\"docx-preview\">");

    // 简单的 XML → HTML 转换
    // 提取段落
    let para_re = regex::Regex::new(r"<w:p[^>]*>(.*?)</w:p>")?;
    let text_re = regex::Regex::new(r"<w:t[^>]*>([^<]+)</w:t>")?;
    let bold_re = regex::Regex::new(r"<w:b\s*/>")?;

    for para_cap in para_re.captures_iter(xml) {
        let para_content = &para_cap[1];
        let mut para_text = String::new();

        // 检查是否粗体
        let is_bold = bold_re.is_match(para_content);

        for text_cap in text_re.captures_iter(para_content) {
            para_text.push_str(&text_cap[1]);
        }

        if !para_text.is_empty() {
            if is_bold {
                html.push_str(&format!("<p><strong>{}</strong></p>", escape_html(&para_text)));
            } else {
                html.push_str(&format!("<p>{}</p>", escape_html(&para_text)));
            }
        }
    }

    html.push_str("</div>");
    Ok(html)
}

/// 从 XML 中提取纯文本
fn extract_text_from_xml(xml: &str) -> String {
    let text_re = regex::Regex::new(r"<w:t[^>]*>([^<]+)</w:t>").unwrap();
    let para_re = regex::Regex::new(r"<w:p[^>]*>").unwrap();

    let mut text = String::new();
    let mut _last_end = 0;

    for cap in para_re.find_iter(xml) {
        // 在段落之间添加换行（第二个及之后的段落）
        if !text.is_empty() {
            text.push('\n');
        }

        // 提取段落内的文本
        let para_end = xml[cap.end()..]
            .find("</w:p>")
            .map(|i| cap.end() + i + 6)
            .unwrap_or(xml.len());

        let para_content = &xml[cap.end()..para_end];
        for text_cap in text_re.captures_iter(para_content) {
            text.push_str(&text_cap[1]);
        }

        _last_end = para_end;
    }

    text
}

/// HTML 转义
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_html() {
        assert_eq!(escape_html("<script>alert('xss')</script>"),
                   "&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;");
    }

    #[test]
    fn test_extract_text() {
        let xml = r#"<w:p><w:t>Hello</w:t></w:p><w:p><w:t>World</w:t></w:p>"#;
        assert_eq!(extract_text_from_xml(xml), "Hello\nWorld");
    }
}
