use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// 模板字段定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateField {
    /// 字段名（如 "案号"、"客户名称"）
    pub name: String,
    /// 字段类型：text / date / select / party_list / checkbox / radio_group
    pub field_type: String,
    /// 默认值
    pub default_value: Option<String>,
    /// 是否必填
    pub required: bool,
}

/// Docsy 模板信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocsyTemplate {
    /// 模板唯一 ID（基于文件路径的 hash）
    pub id: String,
    /// 模板名称（不含扩展名）
    pub name: String,
    /// 模板文件路径
    pub path: String,
    /// 模板分类（目录名）
    pub category: String,
    /// 字段数量
    pub field_count: usize,
    /// 字段列表
    pub fields: Vec<TemplateField>,
    /// 模板描述（从 docProps 或首段提取）
    pub description: String,
}

/// 获取模板目录路径
/// 优先使用 ~/Documents/Casy/templates/，不存在则创建
fn get_template_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("无法获取用户主目录")?;
    let template_dir = home.join("Documents").join("Casy").join("templates");

    if !template_dir.exists() {
        fs::create_dir_all(&template_dir)?;
        // 创建示例模板目录结构
        let subdirs = ["起诉状", "答辩状", "代理词", "判决书", "裁定书", "其他"];
        for dir in &subdirs {
            fs::create_dir_all(template_dir.join(dir))?;
        }
    }

    Ok(template_dir)
}

/// 扫描模板目录，返回所有可用模板
pub fn list_templates() -> Result<Vec<DocsyTemplate>> {
    let template_dir = get_template_dir()?;
    let mut templates = Vec::new();

    scan_dir(&template_dir, &template_dir, &mut templates)?;

    // 按分类和名称排序
    templates.sort_by(|a, b| a.category.cmp(&b.category).then(a.name.cmp(&b.name)));

    Ok(templates)
}

/// 递归扫描目录
fn scan_dir(base: &Path, dir: &Path, templates: &mut Vec<DocsyTemplate>) -> Result<()> {
    let entries = fs::read_dir(dir)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            scan_dir(base, &path, templates)?;
            continue;
        }

        // 只处理 .docx 和 .docxtpl 文件
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "docx" && ext != "docxtpl" {
            continue;
        }

        // 跳过临时文件（以 ~ 开头）
        let filename = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
        if filename.starts_with('~') || filename.starts_with('.') {
            continue;
        }

        match parse_template(&path, base) {
            Ok(template) => templates.push(template),
            Err(e) => {
                log::warn!("解析模板失败 {:?}: {}", path, e);
            }
        }
    }

    Ok(())
}

/// 解析单个模板文件
fn parse_template(path: &Path, base: &Path) -> Result<DocsyTemplate> {
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("未命名")
        .to_string();

    // 计算分类（相对路径的第一级目录）
    let category = path
        .strip_prefix(base)
        .ok()
        .and_then(|p| p.parent())
        .and_then(|p| p.iter().next())
        .and_then(|s| s.to_str())
        .unwrap_or("其他")
        .to_string();

    // 生成唯一 ID（基于相对路径的 hash）
    let relative = path.strip_prefix(base).unwrap_or(path);
    let id = format!(
        "{:x}",
        md5_hash(&relative.to_string_lossy().as_bytes())
    );

    // 尝试从 docx 文件中提取字段
    let (fields, description) = extract_template_info(path).unwrap_or_default();

    Ok(DocsyTemplate {
        id,
        name,
        path: path.to_string_lossy().to_string(),
        category,
        field_count: fields.len(),
        fields,
        description,
    })
}

/// 从 docx 文件中提取模板字段和描述
fn extract_template_info(path: &Path) -> Result<(Vec<TemplateField>, String)> {
    use zip::ZipArchive;

    let file = fs::File::open(path)?;
    let mut archive = ZipArchive::new(file)?;

    let mut fields = Vec::new();
    let mut description = String::new();

    // 读取 word/document.xml
    if let Ok(mut doc) = archive.by_name("word/document.xml") {
        let mut content = String::new();
        use std::io::Read;
        doc.read_to_string(&mut content)?;

        // 提取字段占位符：{{字段名}} 或 {字段名}
        let re = regex::Regex::new(r"\{\{([^}]+)\}\}|\{([^}]+)\}").unwrap();
        let mut field_names = std::collections::HashSet::new();

        for cap in re.captures_iter(&content) {
            let field_name = cap.get(1).or(cap.get(2)).map(|m| m.as_str().trim());
            if let Some(name) = field_name {
                if !name.is_empty() && !field_names.contains(name) {
                    field_names.insert(name.to_string());
                    fields.push(TemplateField {
                        name: name.to_string(),
                        field_type: infer_field_type(name),
                        default_value: None,
                        required: false,
                    });
                }
            }
        }

        // 提取描述（第一个非空段落的文本）
        let text_re = regex::Regex::new(r"<w:t[^>]*>([^<]+)</w:t>").unwrap();
        let mut first_para = String::new();
        for cap in text_re.captures_iter(&content) {
            if let Some(text) = cap.get(1) {
                first_para.push_str(text.as_str());
                if first_para.len() > 100 {
                    break;
                }
            }
        }
        description = first_para;
    }

    Ok((fields, description))
}

/// 根据字段名推断字段类型
fn infer_field_type(name: &str) -> String {
    if name.contains("日期") || name.contains("时间") {
        "date".to_string()
    } else if name.contains("当事人") || name.contains("原告") || name.contains("被告") {
        "party_list".to_string()
    } else if name.contains("是否") || name.contains("有无") {
        "checkbox".to_string()
    } else if name.contains("类型") || name.contains("结果") {
        "radio_group".to_string()
    } else {
        "text".to_string()
    }
}

/// 简单的 MD5 hash 实现（用于生成 ID）
fn md5_hash(data: &[u8]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish()
}

/// 加载单个模板
pub fn load_template(template_id: &str) -> Result<DocsyTemplate> {
    let templates = list_templates()?;
    templates
        .into_iter()
        .find(|t| t.id == template_id)
        .context(format!("未找到模板: {}", template_id))
}
