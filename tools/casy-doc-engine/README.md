# casy-doc-engine

Casy 的独立本地 Rust 文档引擎。PP-OCRv5 mobile 识别坐标和全文，PaddleOCR-VL 0.9B 生成页面文本，harumi 写入不可见文字层。也可选择 Qwen 0.8B 系 OvisOCR2。原文件始终只读；模型推理没有 Python 服务，也不调用 Tesseract。坐标模型使用 ONNX Runtime，页面模型使用原生 Candle。

模型版构建：

```bash
cargo build --release --features models
# Apple Silicon: cargo build --release --features metal
```

运行前配置：

- `CASY_DOC_ENGINE`：该二进制路径（由主程序读取）
- `CASY_PPOCR_MODEL_DIR`：包含 `det.onnx`、`rec.onnx`、`dict.txt`
- `CASY_PADDLEOCR_VL_MODEL_DIR`：PaddleOCR-VL 本地模型目录（优先）
- `CASY_OVISOCR2_MODEL_DIR`：可选的 OvisOCR2 模型目录；仅在未配置 PaddleOCR-VL 时使用
- `CASY_OCR_FONT`：覆盖目标语言字形的 TTF/OTF，例如 Noto Sans CJK
- `CASY_OCR_DEVICE`：`cpu`、`metal`、`cuda` 或 `cuda:N`

`probe` 只做依赖探测；`process` 从 stdin 接受 JSON 请求、向 stdout 返回 JSON 结果，诊断写 stderr。

下载固定版本模型并逐文件校验哈希，模型保存在仓库外：

```sh
node tools/casy-doc-engine/install-models.mjs /path/to/casy-models paddle
# 第三个参数也可使用 ovis 或 all。
```

下载完成后设置 `CASY_PPOCR_MODEL_DIR=/path/to/casy-models/ppocr` 和
`CASY_PADDLEOCR_VL_MODEL_DIR=/path/to/casy-models/paddleocr-vl`，由相同环境启动 Casy。
Apple Silicon 推荐构建 metal 并设置 `CASY_OCR_DEVICE=metal`。未配置设备时使用 CPU。
模型安装器需要 Node.js；实际推理进程使用 Rust。PDF 渲染仍需 Poppler 的 pdftoppm。

PDF 和单帧常见图片统一进入持久队列。多页 TIFF、动画 GIF/WebP 明确拒绝，需先转为 PDF，
避免只识别第一帧。每次只渲染和识别一页：坐标图像最长 2400px，生成模型输入最长 1280px，
ONNX 共用两个计算线程且禁用空闲自旋。只保留页面文字和坐标，不累计 RGB 图像。
已有可提取文字、且没有大面积扫描图的页面直接使用原生文字，不启动模型，也不重复添加文字层。
这些限制不等于任意卷宗的识别质量保证，细小文字和复杂表格需另行验收。

主程序每秒同步页进度，可取消排队或运行中任务。单页超过 15 分钟未推进即失败；
模型输出达到 token 上限不会作为完整结果保存。重试生成新任务 ID，旧进程不能覆盖新任务。
完成时核对原文件 SHA-256、产物路径、页码、坐标以及 JSON/Markdown 一致性，再回填全文、
应用自动归类规则并建立页级索引。每个任务的产物单独存放。

本地模型验收（仅合成中文扫描件，不访问网络）：

```sh
CASY_OCR_QA_DIR=/path/to/private-qa cargo test --manifest-path tools/casy-doc-engine/Cargo.toml \
  --release --features metal real_chinese_scanned_pdf -- --ignored --nocapture
```

其余 CASY_* 模型及字体环境变量也需设置。测试生成双页扫描 PDF，检查第三人、金额、日期、
全文和可搜索层，并保留独立产物，便于人工核对。
