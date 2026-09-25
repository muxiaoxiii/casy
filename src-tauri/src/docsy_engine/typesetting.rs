//! One restricted, in-memory Typst world for editor preview and PDF export.
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, OnceLock},
    time::Instant,
};
use typst::introspection::Introspector;
use typst::{
    diag::{FileError, FileResult},
    foundations::{Bytes, Datetime, Duration, Label},
    syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot},
    text::{Font, FontBook},
    utils::LazyHash,
    Library, LibraryExt, World,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LayoutOptions {
    pub title: String,
    pub case_no: String,
    pub margin_mm: f64,
    pub binding_mm: f64,
    pub first_line_indent: bool,
    pub header: bool,
    pub skip_first_header: bool,
}
impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            title: String::new(),
            case_no: String::new(),
            margin_mm: 20.,
            binding_mm: 5.,
            first_line_indent: true,
            header: true,
            skip_first_header: true,
        }
    }
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockAnchor {
    pub block: usize,
    pub page: usize,
    pub x: f64,
    pub y: f64,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewPage {
    pub svg: String,
    pub width: f64,
    pub height: f64,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentPreview {
    pub pages: Vec<PreviewPage>,
    pub anchors: Vec<BlockAnchor>,
    pub warnings: Vec<String>,
    pub elapsed_ms: u128,
}
pub struct Compiled {
    pub preview: DocumentPreview,
    pub pdf: Vec<u8>,
}

struct FontStore {
    fonts: Vec<Font>,
    book: LazyHash<FontBook>,
}
fn fonts() -> Result<&'static FontStore> {
    static STORE: OnceLock<std::result::Result<FontStore, String>> = OnceLock::new();
    STORE
        .get_or_init(|| {
            let result = (|| -> Result<FontStore> {
                let path = crate::runtime_paths::bundled_runtime()
                    .context("缺少随包字体")?
                    .join("fonts/NotoSansCJKsc-Regular.otf");
                let data = std::fs::read(path).context("无法读取随包中文字体")?;
                let mut fonts: Vec<_> = Font::iter(Bytes::new(data)).collect();
                ensure!(!fonts.is_empty(), "中文字体无法解析");
                for bytes in typst_assets::fonts() {
                    fonts.extend(Font::iter(Bytes::new(bytes)));
                }
                let book = LazyHash::new(FontBook::from_fonts(&fonts));
                Ok(FontStore { fonts, book })
            })();
            result.map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(|e| anyhow::anyhow!(e.clone()))
}
struct MemoryWorld {
    source: Source,
    files: HashMap<String, Bytes>,
    fonts: &'static FontStore,
}
impl World for MemoryWorld {
    fn library(&self) -> &LazyHash<Library> {
        static LIB: OnceLock<LazyHash<Library>> = OnceLock::new();
        LIB.get_or_init(|| LazyHash::new(Library::default()))
    }
    fn book(&self) -> &LazyHash<FontBook> {
        &self.fonts.book
    }
    fn main(&self) -> FileId {
        self.source.id()
    }
    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main() {
            Ok(self.source.clone())
        } else {
            Err(FileError::AccessDenied)
        }
    }
    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.main() {
            return Ok(Bytes::new(self.source.text().as_bytes().to_vec()));
        }
        self.files
            .get(id.vpath().get_without_slash())
            .cloned()
            .ok_or(FileError::AccessDenied)
    }
    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.fonts.get(index).cloned()
    }
    fn today(&self, _: Option<Duration>) -> Option<Datetime> {
        None
    }
}
fn string(text: &str) -> String {
    serde_json::to_string(text).expect("text string")
}
fn children(node: &Value) -> &[Value] {
    node["content"].as_array().map(Vec::as_slice).unwrap_or(&[])
}
fn attr<'a>(node: &'a Value, key: &str) -> &'a str {
    node["attrs"][key].as_str().unwrap_or("")
}
fn content(text: &str) -> String {
    format!("#text({})", string(text))
}

struct Translator {
    files: HashMap<String, Bytes>,
    notes: HashMap<String, String>,
    note_refs: HashSet<String>,
    used_notes: HashMap<String, usize>,
    fonts: &'static FontStore,
    count: usize,
}
impl Translator {
    fn check_text(&self, text: &str) -> Result<()> {
        for ch in text
            .chars()
            .filter(|c| !c.is_whitespace() && !matches!(*c, '\u{200d}' | '\u{fe0e}' | '\u{fe0f}'))
        {
            ensure!(
                self.fonts
                    .fonts
                    .iter()
                    .any(|f| f.info().coverage.contains(ch as u32)),
                "随包字体无法显示字符 {ch}（U+{:04X}）",
                ch as u32
            );
        }
        Ok(())
    }
    fn collect_notes(&mut self, node: &Value, depth: usize) -> Result<()> {
        ensure!(depth < 80, "文档嵌套过深");
        self.count += 1;
        ensure!(self.count <= 30_000, "文档节点过多，请分段排版");
        if node["type"] == "footnoteDefinition" {
            let label = attr(node, "label");
            ensure!(!label.is_empty(), "脚注标识不能为空");
            ensure!(
                self.notes
                    .insert(label.into(), attr(node, "source").into())
                    .is_none(),
                "脚注定义重复：{label}"
            );
        }
        if node["type"] == "footnoteReference" {
            self.note_refs.insert(attr(node, "label").into());
        }
        for child in children(node) {
            self.collect_notes(child, depth + 1)?;
        }
        Ok(())
    }
    fn asset(&mut self, bytes: Vec<u8>, extension: &str) -> String {
        let path = format!("asset-{}.{}", self.files.len(), extension);
        self.files.insert(path.clone(), Bytes::new(bytes));
        path
    }
    fn inline(&mut self, node: &Value, depth: usize) -> Result<String> {
        ensure!(depth < 80, "文档嵌套过深");
        let kind = node["type"].as_str().unwrap_or("");
        match kind {
            "text" => {
                let text = node["text"].as_str().unwrap_or("");
                self.check_text(text)?;
                let mut out = content(text);
                for mark in node["marks"].as_array().into_iter().flatten() {
                    out = match mark["type"].as_str().unwrap_or("") {
                        "bold" => format!("#strong[{out}]"),
                        "italic" => format!("#emph[{out}]"),
                        "underline" => format!("#underline[{out}]"),
                        "strike" => format!("#strike[{out}]"),
                        "highlight" => format!("#highlight[{out}]"),
                        "code" => format!("#raw({})", string(text)),
                        "link" => {
                            let href = mark["attrs"]["href"].as_str().unwrap_or("");
                            if href.starts_with("https://")
                                || href.starts_with("http://")
                                || href.starts_with("mailto:")
                            {
                                format!("#link({})[{out}]", string(href))
                            } else {
                                bail!("排版不支持链接协议：{href}，未生成不完整文件")
                            }
                        }
                        other => bail!("排版不支持 {other} 格式，未生成不完整文件"),
                    };
                }
                Ok(out)
            }
            "hardBreak" => Ok("#linebreak()".into()),
            "mathInline" | "mathBlock" => {
                let source = attr(node, "source");
                ensure!(source.len() <= 100_000, "公式过长");
                let math = mitex::convert_math(source, None)
                    .map_err(|e| anyhow::anyhow!("公式无法排版：{e}"))?;
                Ok(if kind == "mathBlock" {
                    format!("$ {math} $")
                } else {
                    format!("${math}$")
                })
            }
            "footnoteReference" => {
                let label = attr(node, "label");
                let note = self
                    .notes
                    .get(label)
                    .with_context(|| format!("脚注未定义：{label}"))?;
                self.check_text(note)?;
                if let Some(index) = self.used_notes.get(label) {
                    return Ok(format!("#footnote(<casy-note-{index}>)"));
                }
                let index = self.used_notes.len();
                self.used_notes.insert(label.into(), index);
                Ok(format!("#footnote[{}] <casy-note-{index}>", content(note)))
            }
            "wikiLink" => {
                self.check_text(attr(node, "title"))?;
                Ok(content(attr(node, "title")))
            }
            "evidenceLink" => {
                let label = attr(node, "label");
                self.check_text(label)?;
                Ok(content(label))
            }
            other => bail!("排版段落暂不支持 {other}，原始内容已保留"),
        }
    }
    fn inlines(&mut self, node: &Value, depth: usize) -> Result<String> {
        children(node)
            .iter()
            .map(|n| self.inline(n, depth + 1))
            .collect()
    }
    fn block(&mut self, node: &Value, depth: usize) -> Result<String> {
        ensure!(depth < 80, "文档嵌套过深");
        Ok(match node["type"].as_str().unwrap_or("") {
            "paragraph" => {
                let body = self.inlines(node, depth)?;
                match attr(node, "textAlign") {
                    "center" => format!("#align(center)[#set par(justify: false)\n{body}]"),
                    "right" => format!("#align(right)[#set par(justify: false)\n{body}]"),
                    "left" => format!("#align(left)[#set par(justify: false)\n{body}]"),
                    _ => body,
                }
            }
            "heading" => {
                let level = node["attrs"]["level"].as_u64().unwrap_or(1).clamp(1, 6);
                format!("#heading(level: {level})[{}]", self.inlines(node, depth)?)
            }
            "blockquote" => format!(
                "#block(inset: (left: 12pt), stroke: (left: 1pt + gray))[{}]",
                self.blocks(node, depth)?
            ),
            "bulletList" | "orderedList" | "taskList" => {
                let kind = node["type"].as_str().unwrap();
                let mut items = Vec::new();
                for item in children(node) {
                    ensure!(
                        matches!(item["type"].as_str(), Some("listItem" | "taskItem")),
                        "列表结构无效"
                    );
                    let body = self.blocks(item, depth + 1)?;
                    let prefix = if kind == "taskList" {
                        if item["attrs"]["checked"] == true {
                            "☑ "
                        } else {
                            "☐ "
                        }
                    } else {
                        ""
                    };
                    items.push(format!("[{}{body}]", content(prefix)));
                }
                if kind == "orderedList" {
                    format!(
                        "#enum(start: {}, {})",
                        node["attrs"]["start"].as_u64().unwrap_or(1),
                        items.join(",")
                    )
                } else {
                    format!(
                        "#list({}, {})",
                        if kind == "taskList" {
                            "marker: []"
                        } else {
                            "marker: [•]"
                        },
                        items.join(",")
                    )
                }
            }
            "codeBlock" => {
                let source = children(node)
                    .iter()
                    .map(|v| v["text"].as_str().unwrap_or(""))
                    .collect::<String>();
                if attr(node, "language") == "mermaid" {
                    ensure!(source.len() <= 100_000, "图表过长");
                    let mut diagram_options=mermaid_rs_renderer::RenderOptions::default();
                    diagram_options.theme.font_family="Noto Sans CJK SC".into();
                    let svg = mermaid_rs_renderer::render_strict(&source,diagram_options)
                        .context("Mermaid 无法排版，源码已保留")?;
                    let path = self.asset(svg.into_bytes(), "svg");
                    format!("#image({}, width: 100%)", string(&path))
                } else {
                    self.check_text(&source)?;
                    format!("#raw({}, block: true)", string(&source))
                }
            }
            "mathBlock" => self.inline(node, depth)?,
            "footnoteDefinition" => {
                if self.note_refs.contains(attr(node, "label")) {
                    String::new()
                } else {
                    let body = format!("[{}] {}", attr(node, "label"), attr(node, "source"));
                    self.check_text(&body)?;
                    content(&body)
                }
            }
            "horizontalRule" => "#line(length: 100%, stroke: 0.5pt)".into(),
            "image" => {
                let bytes = super::rich_export::load_image_bytes(attr(node, "src"))?;
                ensure!(bytes.len() <= 30 * 1024 * 1024, "图片超过 30 MiB");
                let format = image::guess_format(&bytes).context("图片格式无法识别")?;
                let extension = format
                    .extensions_str()
                    .first()
                    .copied()
                    .context("图片格式无效")?;
                let decoded = image::load_from_memory(&bytes).context("图片无法解码")?;
                let ratio = decoded.height() as f64 / f64::from(decoded.width().max(1));
                let path = self.asset(bytes, extension);
                let width = (node["attrs"]["width"]
                    .as_f64()
                    .unwrap_or(500.)
                    .clamp(1., 10000.)
                    * 0.75)
                    .min(550. / ratio);
                format!(
                    "#layout(size => image({}, width: calc.min({width}pt, size.width)))",
                    string(&path)
                )
            }
            "table" => self.table(node, depth)?,
            other => bail!("排版暂不支持 {other} 节点，未生成不完整文件"),
        })
    }
    fn blocks(&mut self, node: &Value, depth: usize) -> Result<String> {
        children(node)
            .iter()
            .map(|n| self.block(n, depth + 1))
            .collect::<Result<Vec<_>>>()
            .map(|v| v.join("\n\n"))
    }
    fn table(&mut self, node: &Value, depth: usize) -> Result<String> {
        let rows = children(node);
        ensure!(rows.len() <= 5000, "表格行数过多");
        let mut occupied = vec![vec![false; 100]; rows.len()];
        let mut cells = vec![];
        let mut columns = 0;
        let mut widths = [None; 100];
        let header = rows.first().is_some_and(|r| {
            !children(r).is_empty()
                && children(r).iter().all(|c| {
                    c["type"] == "tableHeader" && c["attrs"]["rowspan"].as_u64().unwrap_or(1) == 1
                })
        });
        for (r, row) in rows.iter().enumerate() {
            ensure!(row["type"] == "tableRow", "表格行结构无效");
            let mut c = 0;
            for cell in children(row) {
                ensure!(
                    matches!(cell["type"].as_str(), Some("tableCell" | "tableHeader")),
                    "表格单元格结构无效"
                );
                while c < 100 && occupied[r][c] {
                    c += 1;
                }
                let cs = cell["attrs"]["colspan"].as_u64().unwrap_or(1) as usize;
                let rs = cell["attrs"]["rowspan"].as_u64().unwrap_or(1) as usize;
                ensure!(
                    cs > 0
                        && rs > 0
                        && cs <= 100
                        && rs <= rows.len()
                        && c + cs <= 100
                        && r + rs <= rows.len(),
                    "表格合并范围无效"
                );
                for rr in occupied.iter_mut().skip(r).take(rs) {
                    for v in rr.iter_mut().skip(c).take(cs) {
                        ensure!(!*v, "表格合并重叠");
                        *v = true;
                    }
                }
                if let Some(values) = cell["attrs"]["colwidth"].as_array() {
                    for (i, value) in values.iter().take(cs).enumerate() {
                        if let Some(width) = value
                            .as_f64()
                            .filter(|w| w.is_finite() && *w > 0. && *w <= 10000.)
                        {
                            widths[c + i].get_or_insert(width);
                        }
                    }
                }
                let body = self.blocks(cell, depth + 1)?;
                let align = match attr(cell, "align") {
                    "right" => "right",
                    "center" => "center",
                    _ => "left",
                };
                let body = if cell["type"] == "tableHeader" {
                    format!("#strong[{body}]")
                } else {
                    body
                };
                cells.push((r,format!("table.cell(x: {c}, y: {r}, colspan: {cs}, rowspan: {rs}, align: {align})[{body}]")));
                c += cs;
                columns = columns.max(c);
            }
        }
        ensure!(columns > 0, "表格没有单元格");
        let mut args = vec![];
        if header {
            args.push(format!(
                "table.header({})",
                cells
                    .iter()
                    .filter(|(r, _)| *r == 0)
                    .map(|(_, s)| s.clone())
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        args.extend(
            cells
                .into_iter()
                .filter(|(r, _)| !header || *r > 0)
                .map(|(_, s)| s),
        );
        let columns = format!(
            "({})",
            widths[..columns]
                .iter()
                .map(|w| format!("{}fr", w.unwrap_or(100.)))
                .collect::<Vec<_>>()
                .join(",")
                + ","
        );
        Ok(format!(
            "#table(columns: {columns}, inset: 6pt, stroke: 0.5pt, {})",
            args.join(",")
        ))
    }
}

pub fn compile(root: &Value, options: &LayoutOptions) -> Result<Arc<Compiled>> {
    let started = Instant::now();
    ensure!(
        root["type"] == "doc" && root.to_string().len() <= 4 * 1024 * 1024,
        "排版文档无效或超过 4 MiB"
    );
    ensure!(
        options.margin_mm.is_finite()
            && (10.0..=40.0).contains(&options.margin_mm)
            && options.binding_mm.is_finite()
            && (0.0..=15.0).contains(&options.binding_mm),
        "页边距范围无效"
    );
    ensure!(
        options.title.len() <= 1000 && options.case_no.len() <= 300,
        "页眉内容过长"
    );
    let fonts = fonts()?;
    let mut t = Translator {
        files: HashMap::new(),
        notes: HashMap::new(),
        note_refs: HashSet::new(),
        used_notes: HashMap::new(),
        fonts,
        count: 0,
    };
    t.collect_notes(root, 0)?;
    t.check_text(&options.title)?;
    t.check_text(&options.case_no)?;
    let header = if options.header {
        format!("context {{ if {} or counter(page).get().first() > 1 {{ text(size: 9pt)[{} #h(1fr) {}] }} }}",!options.skip_first_header,content(&options.title),content(&options.case_no))
    } else {
        "none".into()
    };
    let mut source=format!("#set page(paper: \"a4\", margin: (top: {}mm, bottom: {}mm, left: {}mm, right: {}mm), header: {header}, footer: context align(center)[第 #counter(page).display() 页 / 共 #counter(page).final().first() 页])\n#set text(font: (\"Noto Sans CJK SC\", \"Libertinus Serif\"), size: 12pt, lang: \"zh\")\n#set par(justify: true, leading: 7.8pt, first-line-indent: {}em)\n",options.margin_mm,options.margin_mm,options.margin_mm+options.binding_mm,options.margin_mm,if options.first_line_indent{2}else{0});
    for (index, node) in children(root).iter().enumerate() {
        if node["type"] == "footnoteDefinition" && t.note_refs.contains(attr(node, "label")) {
            continue;
        }
        source.push_str(&format!(
            "\n#metadata({index}) <casy-block-{index}>\n{}\n\n",
            t.block(node, 0)?
        ));
    }
    let mut hash = Sha256::new();
    hash.update(source.as_bytes());
    let mut files: Vec<_> = t.files.iter().collect();
    files.sort_by_key(|(name, _)| *name);
    for (name, bytes) in files {
        hash.update(name.as_bytes());
        hash.update(bytes.as_slice());
    }
    let key = format!("{:x}", hash.finalize());
    type CompiledCache = (String, Arc<Compiled>);
    static CACHE: OnceLock<Mutex<Option<CompiledCache>>> = OnceLock::new();
    let mut cache = CACHE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|_| anyhow::anyhow!("排版缓存不可用"))?;
    if let Some((old, result)) = &*cache {
        if old == &key {
            return Ok(result.clone());
        }
    }
    let id = FileId::new(RootedPath::new(
        VirtualRoot::Project,
        VirtualPath::new("main.typ").unwrap(),
    ));
    let world = MemoryWorld {
        source: Source::new(id, source),
        files: t.files,
        fonts,
    };
    let warned = typst::compile::<typst_layout::PagedDocument>(&world);
    let warnings = warned
        .warnings
        .iter()
        .map(|e| e.message.to_string())
        .collect::<Vec<_>>();
    let document = warned.output.map_err(|errs| {
        anyhow::anyhow!(
            "排版失败：{}",
            errs.iter()
                .map(|e| e.message.to_string())
                .collect::<Vec<_>>()
                .join("；")
        )
    })?;
    ensure!(document.pages().len() <= 300, "文档超过 300 页，请分段预览");
    let pdf = typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default())
        .map_err(|e| anyhow::anyhow!("PDF 生成失败：{e:?}"))?;
    let pages = document
        .pages()
        .iter()
        .map(|page| PreviewPage {
            svg: typst_svg::svg(page, &typst_svg::SvgOptions::default()),
            width: page.frame.size().x.to_pt(),
            height: page.frame.size().y.to_pt(),
        })
        .collect();
    let mut anchors = vec![];
    for index in 0..children(root).len() {
        if let Ok(content) = document.introspector().query_label(
            Label::new(typst::utils::PicoStr::intern(&format!(
                "casy-block-{index}"
            )))
            .expect("nonempty label"),
        ) {
            if let Some(pos) = content
                .location()
                .and_then(|loc| document.introspector().position(loc))
            {
                anchors.push(BlockAnchor {
                    block: index,
                    page: pos.page.get(),
                    x: pos.point.x.to_pt(),
                    y: pos.point.y.to_pt(),
                });
            }
        }
    }
    let result = Arc::new(Compiled {
        pdf,
        preview: DocumentPreview {
            pages,
            anchors,
            warnings,
            elapsed_ms: started.elapsed().as_millis(),
        },
    });
    *cache = Some((key, result.clone()));
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn p(text: &str) -> Value {
        json!({"type":"paragraph","content":[{"type":"text","text":text}]})
    }
    #[test]
    fn chinese_pages_and_anchors() {
        let root = json!({"type":"doc","content":(0..90).map(|n|p(&format!("第{n}项事实。核对证据、期限与当事人信息。"))).collect::<Vec<_>>()});
        let out = compile(&root, &LayoutOptions::default()).unwrap();
        assert!(out.preview.pages.len() > 1);
        assert_eq!(out.preview.anchors.len(), 90);
        let text: String = pdf_extract::extract_text_from_mem(&out.pdf)
            .unwrap()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        assert!(text.contains("第89项事实"), "{text:?}");
        let pages = pdf_extract::extract_text_from_mem_by_pages(&out.pdf).unwrap();
        for anchor in &out.preview.anchors {
            let text: String = pages[anchor.page - 1]
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            assert!(
                text.contains(&format!("第{}项事实", anchor.block)),
                "block {} points to wrong page {}",
                anchor.block,
                anchor.page
            );
        }
    }
    #[test]
    fn repeated_and_unreferenced_notes_are_not_lost() {
        let root = json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"依据"},{"type":"footnoteReference","attrs":{"label":"law"}},{"type":"text","text":"再次引用"},{"type":"footnoteReference","attrs":{"label":"law"}}]},{"type":"footnoteDefinition","attrs":{"label":"law","source":"唯一的法条依据"}},{"type":"footnoteDefinition","attrs":{"label":"unused","source":"尚未引用的说明"}}]});
        let out = compile(&root, &LayoutOptions::default()).unwrap();
        let text = pdf_extract::extract_text_from_mem(&out.pdf).unwrap();
        assert_eq!(text.matches("唯一的法条依据").count(), 1);
        assert!(text.contains("尚未引用的说明"));
        assert!(out.preview.anchors.iter().any(|a| a.block == 2));
    }
    #[test]
    fn diagrams_lists_images_and_cache() {
        let temp = tempfile::tempdir().unwrap();
        let image_path = temp.path().join("image.png");
        image::DynamicImage::new_rgb8(20, 40)
            .save(&image_path)
            .unwrap();
        let root = json!({"type":"doc","content":[{"type":"codeBlock","attrs":{"language":"mermaid"},"content":[{"type":"text","text":"flowchart LR\nA[事实] --> B[证据]"}]},{"type":"taskList","content":[{"type":"taskItem","attrs":{"checked":true},"content":[p("核对")]}]},{"type":"image","attrs":{"src":image_path.to_str().unwrap(),"width":200}}]});
        let opts = LayoutOptions::default();
        let first = compile(&root, &opts).unwrap();
        let next = compile(&root, &opts).unwrap();
        assert_eq!(first.pdf, next.pdf);
        assert_eq!(next.preview.anchors.len(), 3);
        assert!(
            next.preview.warnings.is_empty(),
            "{:?}",
            next.preview.warnings
        );
        let other = compile(
            &root,
            &LayoutOptions {
                margin_mm: 30.,
                ..opts
            },
        )
        .unwrap();
        assert_ne!(first.pdf, other.pdf);
    }
    #[test]
    fn rejects_missing_notes_glyphs_and_malformed_tables() {
        for node in [
            json!({"type":"paragraph","content":[{"type":"footnoteReference","attrs":{"label":"missing"}}]}),
            p("\u{10ffff}"),
            json!({"type":"table","content":[{"type":"bogus","content":[]} ]}),
        ] {
            assert!(compile(
                &json!({"type":"doc","content":[node]}),
                &LayoutOptions::default()
            )
            .is_err());
        }
    }
    #[test]
    fn math_notes_and_rejection() {
        let root = json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"mathInline","attrs":{"source":"x_i + y_i"}},{"type":"footnoteReference","attrs":{"label":"law"}}]},{"type":"footnoteDefinition","attrs":{"label":"law","source":"第六十五条"}}]});
        let out = compile(&root, &LayoutOptions::default()).unwrap();
        assert!(pdf_extract::extract_text_from_mem(&out.pdf)
            .unwrap()
            .contains("第六十五条"));
        assert!(compile(
            &json!({"type":"doc","content":[{"type":"rawHtmlBlock"}]}),
            &LayoutOptions::default()
        )
        .is_err());
        assert!(compile(
            &json!({"type":"doc","content":[{"type":"image","attrs":{"src":"/missing/casy.png"}}]}),
            &LayoutOptions::default()
        )
        .is_err());
    }
}
