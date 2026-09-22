# casy-doc-engine

Casy 的独立 Rust 文档引擎。核对日期：2026-09-22；对应 0.1.3 生产验证版。应用版本、引擎包版本和模型标识分别管理，不能只读某一处版本推断安装包内容。

## 当前管线

主识别器为 PP-OCRv6 medium，使用独立 Korean PP-OCRv5 mobile 及配套字典增强韩文候选。PP-DocLayout Plus-L **已接入**；[layout.rs](src/layout.rs)、[table.rs](src/table.rs)、[visual.rs](src/visual.rs) 负责阅读顺序、表格恢复和图表/公式等裁图。复杂表格不能保证完整结构化，保留原图与辅助文字。

PDF 逐页渲染、OCR，并对原生文字作辅助校核；不会仅因为存在文字层就跳过可见页面识别。派生可搜索 PDF 使用栅格页面和新文字层，不直接继承原文的隐藏文字、表单、附件、签名或完整矢量精度。当前渲染边长上限 2400px；原文件不改写。图片入口按单帧处理，多帧材料先转 PDF。

推理由 Rust/oar-ocr 执行，不需要 Python 服务。PaddleOCR-VL/OvisOCR2 不是默认路线；兼容探测字段和历史计划不代表已启用这些模型。E5 embedding 的 `embed` 命令为检索服务，与 OCR 模型分开。

## 构建与资源

```sh
cargo build --manifest-path tools/casy-doc-engine/Cargo.toml --release --features models
node tools/casy-doc-engine/install-models.mjs /absolute/model-directory
# 仅查看下载清单，不访问网络：
node tools/casy-doc-engine/install-models.mjs /absolute/model-directory --dry-run
```

模型安装脚本固定来源版本、文件长度及哈希；字典与识别权重必须配套。格式相同不意味着可直接替换模型，替换前需核对预处理、输出类别、语言、质量和许可。

| 环境变量 | 用途 |
| --- | --- |
| `CASY_DOC_ENGINE` | 主程序选择引擎二进制 |
| `CASY_PPOCR_MODEL_DIR` | `ppocrv6-medium` 主模型目录 |
| `CASY_KOREAN_MODEL_DIR` | `korean-ppocrv5-mobile` |
| `CASY_LAYOUT_MODEL` | `layout/pp-doclayout_plus-l.onnx` |
| `CASY_PDFTOPPM` | Poppler 渲染器 |
| `CASY_OCR_FONT` | 生成 PDF 所用字体文件 |
| `CASY_EMBEDDING_MODEL_DIR` | E5 模型与 tokenizer |

完整包按相邻 runtime 目录查找资源，不应依赖开发机环境覆盖。完整应用资源准备由仓库脚本管理，还包括动态库、字体、embedding、Zvec 和许可；仅执行 install-models 不会生成完整安装包。见[运行时分发](../../docs/runtime-distribution-plan.md)。

## 协议与产物

[main.rs](src/main.rs) 的入口包括 `probe`、`process`、`revise`、`embed`；请求结构由引擎及主程序的 [document_pipeline.rs](../../src-tauri/src/document_pipeline.rs) 定义，处理请求通过 stdin JSON 传入。stdout 是机器可读结果，进度另写文件。

每个 OCR 任务生成：

- `source.document.json`：页序、尺寸、文本、区域、置信度、版面和耗时。
- `source.md`：同一页结果的 Markdown，内部裁图仍可为 base64。
- `source.map.json`：源文件、正文与 Markdown 哈希，UTF-8 字节范围、页码和区域坐标。
- `source.searchable.pdf`：请求需要时生成的派生 PDF。
- `progress.json`：阶段、已处理页数、耗时等进度信息。

主程序校验源哈希、页 IR、Markdown 和映射后才入库或导出。用户 Markdown 导出通过 [markdown_export.rs](../../src-tauri/src/commands/markdown_export.rs) 外置图片；引擎内部自包含产物并未因此全部改为附件引用。文字文档的 anydoc/分段路径位于主程序，不生成本引擎的可搜索 PDF。

## 大文件与验证

当前工作区使用缓冲 JSON writer，主程序将 stdout 分块写临时文件。但识别仍返回全部页集合，来源映射仍构建完整 Markdown，PDF 生成及校验也存在整份数据驻留。不能称为全链路有界内存处理。主程序保留 15 分钟无页数进展保护；这不是每份文档的总耗时上限。

```sh
cargo test --manifest-path tools/casy-doc-engine/Cargo.toml --locked --features models
# 配好已校验的模型、字体和独立产物目录后，单独运行真实 OCR 用例：
CASY_OCR_QA_DIR=/absolute/private-qa cargo test \
  --manifest-path tools/casy-doc-engine/Cargo.toml --features models \
  real_multilingual_and_forged_text_pdf -- --ignored --nocapture
```

先前模型特性测试 19 通过、5 忽略；本轮文档整理未重跑引擎测试。1 页图表导出验证不等于真实长 PDF、跨页表格或所有语言已验收。验收日期及源码覆盖限制见[项目状态](../../Casy-STATUS.md)，大文件与来源规则见[文档处理](../../docs/DOCUMENT_PIPELINE.md)，待修问题见[代码审阅](../../docs/CODE_REVIEW_2026-09-22.md)。
