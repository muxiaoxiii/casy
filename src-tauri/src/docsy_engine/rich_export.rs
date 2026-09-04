use super::export::ExportResult;
use anyhow::{bail, Context, Result};
use base64::Engine;
use docx_rs::{
    BreakType, Docx, Hyperlink, HyperlinkType, Paragraph, Pic, Run, RunFonts, Table,
    TableCell, TableRow,
};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct RichNode {
    #[serde(rename = "type")]
    node_type: String,
    #[serde(default)]
    attrs: Map<String, Value>,
    #[serde(default)]
    content: Vec<RichNode>,
    text: Option<String>,
    #[serde(default)]
    marks: Vec<RichMark>,
}

#[derive(Debug, Clone, Deserialize)]
struct RichMark {
    #[serde(rename = "type")]
    mark_type: String,
    #[serde(default)]
    attrs: Map<String, Value>,
}

#[derive(Debug, Clone)]
struct EvidenceReference {
    label: String,
    target_type: String,
    target_id: String,
    anchor: Option<String>,
}

enum RichBlock {
    Paragraph(Paragraph),
    Table(Table),
}

/// 将 Tiptap/ProseMirror JSON 直接转为 DOCX。
///
/// 这里故意对节点和 mark 使用白名单：导出器遇到尚未实现的结构时返回错误，
/// 而不是生成一个看似成功、实际上丢内容的文件。
pub fn export_rich_docx(
    document: Value,
    title: &str,
    output_path: Option<&str>,
) -> Result<ExportResult> {
    let root: RichNode = serde_json::from_value(document).context("编辑器文档结构无效")?;
    if root.node_type != "doc" {
        bail!("编辑器文档根节点必须是 doc，实际为 {}", root.node_type);
    }

    let mut references = Vec::new();
    let mut blocks = Vec::new();
    for node in &root.content {
        blocks.extend(convert_block(node, &mut references)?);
    }

    if !references.is_empty() {
        blocks.push(RichBlock::Paragraph(heading_paragraph("参考证据列表", 3)));
        for (index, reference) in references.iter().enumerate() {
            let anchor = reference
                .anchor
                .as_deref()
                .filter(|value| !value.is_empty())
                .map(|value| format!("，定位：{value}"))
                .unwrap_or_default();
            let text = format!(
                "{}. {}（{}：{}{}）",
                index + 1,
                reference.label,
                reference.target_type,
                reference.target_id,
                anchor
            );
            blocks.push(RichBlock::Paragraph(text_paragraph(&text)));
        }
    }

    let mut docx = Docx::new();
    if blocks.is_empty() {
        docx = docx.add_paragraph(Paragraph::new());
    } else {
        for block in blocks {
            docx = match block {
                RichBlock::Paragraph(paragraph) => docx.add_paragraph(paragraph),
                RichBlock::Table(table) => docx.add_table(table),
            };
        }
    }

    let output = resolve_output_path(title, output_path)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).context("无法创建 DOCX 导出目录")?;
    }
    let file = fs::File::create(&output).context("无法创建 DOCX 文件")?;
    docx.build().pack(file).context("DOCX 打包失败")?;
    let metadata = fs::metadata(&output).context("无法读取导出文件信息")?;

    Ok(ExportResult {
        output_path: output.to_string_lossy().into_owned(),
        file_size: metadata.len(),
        exported_at: chrono::Local::now()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string(),
    })
}

fn convert_block(node: &RichNode, references: &mut Vec<EvidenceReference>) -> Result<Vec<RichBlock>> {
    match node.node_type.as_str() {
        "paragraph" => Ok(vec![RichBlock::Paragraph(paragraph_from_node(
            node, "", false, None, references,
        )?)]),
        "heading" => {
            let level = attr_u64(node, "level").unwrap_or(1).clamp(1, 6) as u8;
            Ok(vec![RichBlock::Paragraph(paragraph_from_node(
                node,
                "",
                true,
                Some(heading_size(level)),
                references,
            )?)])
        }
        "blockquote" => {
            let mut blocks = Vec::new();
            for child in &node.content {
                match child.node_type.as_str() {
                    "paragraph" | "heading" => blocks.push(RichBlock::Paragraph(
                        paragraph_from_node(child, "│ ", false, None, references)?,
                    )),
                    other => bail!("引用块内暂不支持节点 {other}"),
                }
            }
            Ok(blocks)
        }
        "codeBlock" => {
            let text = collect_plain_text(node)?;
            let mut paragraph = Paragraph::new();
            for (index, line) in text.split('\n').enumerate() {
                if index > 0 {
                    paragraph = paragraph.add_run(Run::new().add_break(BreakType::TextWrapping));
                }
                paragraph = paragraph.add_run(
                    Run::new()
                        .add_text(line)
                        .fonts(RunFonts::new().ascii("Menlo").hi_ansi("Menlo").east_asia("等距更纱黑体 SC")),
                );
            }
            Ok(vec![RichBlock::Paragraph(paragraph)])
        }
        "horizontalRule" => Ok(vec![RichBlock::Paragraph(text_paragraph(
            "────────────────────────────────",
        ))]),
        "bulletList" => convert_list(node, ListKind::Bullet, 0, references),
        "orderedList" => convert_list(node, ListKind::Ordered, 0, references),
        "taskList" => convert_list(node, ListKind::Task, 0, references),
        "table" => Ok(vec![RichBlock::Table(convert_table(node, references)?)]),
        "image" => Ok(vec![RichBlock::Paragraph(image_paragraph(node)?)]),
        other => bail!("DOCX 导出尚不支持节点 {other}；文件未生成，以避免内容丢失"),
    }
}

#[derive(Clone, Copy)]
enum ListKind {
    Bullet,
    Ordered,
    Task,
}

fn convert_list(
    node: &RichNode,
    kind: ListKind,
    depth: usize,
    references: &mut Vec<EvidenceReference>,
) -> Result<Vec<RichBlock>> {
    let mut blocks = Vec::new();
    let start = attr_u64(node, "start").unwrap_or(1) as usize;
    for (index, item) in node.content.iter().enumerate() {
        if item.node_type != "listItem" && item.node_type != "taskItem" {
            bail!("列表包含不支持的节点 {}", item.node_type);
        }
        let marker = match kind {
            ListKind::Bullet => "• ".to_string(),
            ListKind::Ordered => format!("{}. ", start + index),
            ListKind::Task => {
                if attr_bool(item, "checked").unwrap_or(false) {
                    "☑ ".to_string()
                } else {
                    "☐ ".to_string()
                }
            }
        };
        let prefix = format!("{}{}", "  ".repeat(depth), marker);
        let mut first_text_block = true;
        for child in &item.content {
            match child.node_type.as_str() {
                "paragraph" | "heading" => {
                    let actual_prefix = if first_text_block { prefix.as_str() } else { "" };
                    blocks.push(RichBlock::Paragraph(paragraph_from_node(
                        child,
                        actual_prefix,
                        false,
                        None,
                        references,
                    )?));
                    first_text_block = false;
                }
                "bulletList" => blocks.extend(convert_list(child, ListKind::Bullet, depth + 1, references)?),
                "orderedList" => blocks.extend(convert_list(child, ListKind::Ordered, depth + 1, references)?),
                "taskList" => blocks.extend(convert_list(child, ListKind::Task, depth + 1, references)?),
                other => bail!("列表项内暂不支持节点 {other}"),
            }
        }
    }
    Ok(blocks)
}

fn convert_table(node: &RichNode, references: &mut Vec<EvidenceReference>) -> Result<Table> {
    let mut rows = Vec::new();
    for row in &node.content {
        if row.node_type != "tableRow" {
            bail!("表格包含不支持的节点 {}", row.node_type);
        }
        let mut cells = Vec::new();
        for cell in &row.content {
            if cell.node_type != "tableCell" && cell.node_type != "tableHeader" {
                bail!("表格行包含不支持的节点 {}", cell.node_type);
            }
            let is_header = cell.node_type == "tableHeader";
            let mut target = TableCell::new();
            if cell.content.is_empty() {
                target = target.add_paragraph(Paragraph::new());
            }
            for child in &cell.content {
                match child.node_type.as_str() {
                    "paragraph" | "heading" => {
                        target = target.add_paragraph(paragraph_from_node(
                            child,
                            "",
                            is_header,
                            None,
                            references,
                        )?);
                    }
                    "bulletList" | "orderedList" | "taskList" => {
                        let kind = match child.node_type.as_str() {
                            "bulletList" => ListKind::Bullet,
                            "orderedList" => ListKind::Ordered,
                            _ => ListKind::Task,
                        };
                        for block in convert_list(child, kind, 0, references)? {
                            match block {
                                RichBlock::Paragraph(paragraph) => target = target.add_paragraph(paragraph),
                                RichBlock::Table(table) => target = target.add_table(table),
                            }
                        }
                    }
                    "table" => target = target.add_table(convert_table(child, references)?),
                    other => bail!("表格单元格内暂不支持节点 {other}"),
                }
            }
            cells.push(target);
        }
        rows.push(TableRow::new(cells).cant_split());
    }
    Ok(Table::new(rows))
}

fn paragraph_from_node(
    node: &RichNode,
    prefix: &str,
    force_bold: bool,
    force_size: Option<usize>,
    references: &mut Vec<EvidenceReference>,
) -> Result<Paragraph> {
    let mut paragraph = Paragraph::new();
    if !prefix.is_empty() {
        paragraph = paragraph.add_run(base_run().add_text(prefix));
    }
    for child in &node.content {
        paragraph = append_inline(paragraph, child, force_bold, force_size, references)?;
    }
    Ok(paragraph)
}

fn append_inline(
    mut paragraph: Paragraph,
    node: &RichNode,
    force_bold: bool,
    force_size: Option<usize>,
    references: &mut Vec<EvidenceReference>,
) -> Result<Paragraph> {
    match node.node_type.as_str() {
        "text" => {
            let text = node.text.as_deref().unwrap_or_default();
            let mut run = base_run().add_text(text);
            if force_bold {
                run = run.bold();
            }
            if let Some(size) = force_size {
                run = run.size(size);
            }
            let mut href = None;
            for mark in &node.marks {
                match mark.mark_type.as_str() {
                    "bold" => run = run.bold(),
                    "italic" => run = run.italic(),
                    "strike" => run = run.strike(),
                    "underline" => run = run.underline("single"),
                    "highlight" => run = run.highlight("yellow"),
                    "code" => {
                        run = run.fonts(
                            RunFonts::new().ascii("Menlo").hi_ansi("Menlo").east_asia("等距更纱黑体 SC"),
                        )
                    }
                    "link" => {
                        href = mark.attrs.get("href").and_then(Value::as_str).map(str::to_owned)
                    }
                    other => bail!("DOCX 导出尚不支持文本格式 {other}"),
                }
            }
            if let Some(url) = href {
                if !is_safe_hyperlink(&url) {
                    bail!("文档包含不安全或不支持的链接协议: {url}");
                }
                paragraph = paragraph.add_hyperlink(Hyperlink::new(url, HyperlinkType::External).add_run(run));
            } else {
                paragraph = paragraph.add_run(run);
            }
        }
        "hardBreak" => {
            paragraph = paragraph.add_run(base_run().add_break(BreakType::TextWrapping));
        }
        "evidenceLink" => {
            let label = attr_str(node, "label")
                .or_else(|| attr_str(node, "targetId"))
                .unwrap_or("证据链接")
                .to_string();
            let target_type = required_attr(node, "targetType")?.to_string();
            let target_id = required_attr(node, "targetId")?.to_string();
            references.push(EvidenceReference {
                label: label.clone(),
                target_type,
                target_id,
                anchor: attr_str(node, "anchor").map(str::to_owned),
            });
            paragraph = paragraph.add_run(base_run().add_text(format!("{}[{}]", label, references.len())));
        }
        "blockReference" => {
            let knowledge_id = required_attr(node, "knowledgeId")?;
            let block_id = attr_str(node, "blockId")
                .map(|value| format!("#{value}"))
                .unwrap_or_default();
            paragraph = paragraph.add_run(base_run().add_text(format!("[知识引用：{knowledge_id}{block_id}]")));
        }
        "wikiLink" => {
            let label = attr_str(node, "label")
                .or_else(|| attr_str(node, "title"))
                .or_else(|| attr_str(node, "target"))
                .unwrap_or("知识链接");
            paragraph = paragraph.add_run(base_run().add_text(format!("[[{label}]]")));
        }
        "image" => paragraph = paragraph.add_run(image_run(node)?),
        other => bail!("DOCX 导出尚不支持行内节点 {other}"),
    }
    Ok(paragraph)
}

fn image_paragraph(node: &RichNode) -> Result<Paragraph> {
    Ok(Paragraph::new().add_run(image_run(node)?))
}

fn image_run(node: &RichNode) -> Result<Run> {
    let source = required_attr(node, "src")?;
    let bytes = load_image_bytes(source)?;
    let pic = std::panic::catch_unwind(|| Pic::new(&bytes))
        .map_err(|_| anyhow::anyhow!("图片无法解析，DOCX 未生成"))?;
    Ok(base_run().add_image(pic))
}

fn load_image_bytes(source: &str) -> Result<Vec<u8>> {
    if source.starts_with("data:image/") {
        let (metadata, encoded) = source
            .split_once(',')
            .context("图片 data URL 不完整")?;
        if !metadata.ends_with(";base64") {
            bail!("仅支持 base64 编码的图片 data URL");
        }
        return base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .context("图片 base64 数据无效");
    }

    let path = if let Some(path) = source.strip_prefix("file://") {
        PathBuf::from(path)
    } else {
        PathBuf::from(source)
    };
    if !path.is_absolute() {
        bail!("图片必须是 base64 数据或绝对本地路径，暂不支持: {source}");
    }
    fs::read(&path).with_context(|| format!("无法读取图片: {}", path.display()))
}

fn collect_plain_text(node: &RichNode) -> Result<String> {
    let mut output = String::new();
    for child in &node.content {
        match child.node_type.as_str() {
            "text" => output.push_str(child.text.as_deref().unwrap_or_default()),
            "hardBreak" => output.push('\n'),
            other => bail!("代码块内暂不支持节点 {other}"),
        }
    }
    Ok(output)
}

fn base_run() -> Run {
    Run::new().fonts(
        RunFonts::new()
            .ascii("Aptos")
            .hi_ansi("Aptos")
            .east_asia("宋体"),
    )
}

fn text_paragraph(text: &str) -> Paragraph {
    Paragraph::new().add_run(base_run().add_text(text))
}

fn heading_paragraph(text: &str, level: u8) -> Paragraph {
    Paragraph::new().add_run(base_run().add_text(text).bold().size(heading_size(level)))
}

fn heading_size(level: u8) -> usize {
    match level {
        1 => 32,
        2 => 28,
        3 => 24,
        _ => 22,
    }
}

fn attr_str<'a>(node: &'a RichNode, key: &str) -> Option<&'a str> {
    node.attrs.get(key).and_then(Value::as_str)
}

fn required_attr<'a>(node: &'a RichNode, key: &str) -> Result<&'a str> {
    attr_str(node, key)
        .filter(|value| !value.is_empty())
        .with_context(|| format!("节点 {} 缺少属性 {key}", node.node_type))
}

fn attr_u64(node: &RichNode, key: &str) -> Option<u64> {
    node.attrs.get(key).and_then(Value::as_u64)
}

fn attr_bool(node: &RichNode, key: &str) -> Option<bool> {
    node.attrs.get(key).and_then(Value::as_bool)
}

fn is_safe_hyperlink(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    lower.starts_with("https://")
        || lower.starts_with("http://")
        || lower.starts_with("mailto:")
        || lower.starts_with("tel:")
}

fn resolve_output_path(title: &str, output_path: Option<&str>) -> Result<PathBuf> {
    if let Some(path) = output_path {
        // 安全（审查 P0-4）：显式路径必须经统一校验，原本此处零校验直接返回
        return crate::docsy_engine::output_path::resolve_explicit_output_path(path, &["docx"]);
    }
    let output_dir = crate::runtime_paths::export_root();
    let safe_title: String = title
        .chars()
        .map(|character| match character {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => character,
        })
        .collect();
    let safe_title = safe_title.trim().trim_matches('.');
    let safe_title = if safe_title.is_empty() { "未命名文书" } else { safe_title };
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    Ok(output_dir.join(format!("{safe_title}_{timestamp}.docx")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use zip::ZipArchive;

    #[test]
    fn exports_chinese_table_and_evidence_reference() {
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("edited.docx");
        let document = serde_json::json!({
            "type": "doc",
            "content": [
                {"type": "heading", "attrs": {"level": 1}, "content": [{"type": "text", "text": "案件意见", "marks": [{"type": "highlight"}]}]},
                {"type": "paragraph", "content": [
                    {"type": "text", "text": "正文"},
                    {"type": "evidenceLink", "attrs": {"targetType": "file", "targetId": "file-1", "label": "合同扫描件", "anchor": "page:12"}}
                ]},
                {"type": "taskList", "content": [
                    {"type": "taskItem", "attrs": {"checked": true}, "content": [
                        {"type": "paragraph", "content": [{"type": "text", "text": "核对原件", "marks": [{"type": "bold"}]}]}
                    ]}
                ]},
                {"type": "image", "attrs": {
                    "src": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Y9Z4OQAAAAASUVORK5CYII=",
                    "alt": "证据截图"
                }},
                {"type": "table", "content": [
                    {"type": "tableRow", "content": [
                        {"type": "tableHeader", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "项目"}]}]},
                        {"type": "tableHeader", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "内容"}]}]}
                    ]},
                    {"type": "tableRow", "content": [
                        {"type": "tableCell", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "案号"}]}]},
                        {"type": "tableCell", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "（2026）示例"}]}]}
                    ]}
                ]}
            ]
        });

        let result = export_rich_docx(document, "测试", Some(output.to_str().unwrap())).unwrap();
        assert!(result.file_size > 0);

        let file = fs::File::open(output).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();
        let mut xml = String::new();
        archive.by_name("word/document.xml").unwrap().read_to_string(&mut xml).unwrap();
        assert!(xml.contains("案件意见"));
        assert!(xml.contains("合同扫描件"));
        assert!(xml.contains("参考证据列表"));
        assert!(xml.contains("page:12"));
        assert!(xml.contains("（2026）示例"));
        assert!(xml.contains("核对原件"));
        assert!(xml.contains("宋体"));
        assert!(xml.contains("w:highlight"));
        assert!(archive.file_names().any(|name| name.starts_with("word/media/")));
    }

    #[test]
    fn rejects_unknown_nodes_instead_of_dropping_them() {
        let document = serde_json::json!({
            "type": "doc",
            "content": [{"type": "unsupportedWidget", "attrs": {"payload": "must-not-disappear"}}]
        });
        let error = export_rich_docx(document, "测试", Some("/tmp/should-not-exist.docx"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("unsupportedWidget"));
    }
}
